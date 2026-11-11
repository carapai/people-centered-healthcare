//! The shared router over `oneshot`, with no socket: the same path the desktop IPC dispatch
//! uses (ADR 0036).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use health_http::{REQUEST_ID, Readiness, ReadinessCheck, app};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

struct Flag(Arc<AtomicBool>);

impl ReadinessCheck for Flag {
    fn name(&self) -> &'static str {
        "database"
    }

    fn check(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async { self.0.load(Ordering::SeqCst) })
    }
}

struct Hangs;

impl ReadinessCheck for Hangs {
    fn name(&self) -> &'static str {
        "collector"
    }

    fn check(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::pending())
    }
}

async fn call(router: &Router, uri: &str) -> (StatusCode, Value, Option<String>) {
    let response = router
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).expect("request"))
        .await
        .expect("infallible");
    let status = response.status();
    let request_id = response
        .headers()
        .get(REQUEST_ID)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body, request_id)
}

#[tokio::test]
async fn health_is_always_up_and_ready_follows_the_checks() {
    let up = Arc::new(AtomicBool::new(false));
    let router = app(
        Router::new(),
        Readiness::new(vec![Arc::new(Flag(up.clone()))]),
    );

    let (status, body, request_id) = call(&router, "/health").await;
    assert_eq!(
        (status, body["status"].as_str()),
        (StatusCode::OK, Some("pass"))
    );
    assert_eq!(request_id.map(|id| id.len()), Some(36), "a UUID request id");

    let (status, body, _) = call(&router, "/ready").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["checks"]["database"], "fail");

    up.store(true, Ordering::SeqCst);
    let (status, body, _) = call(&router, "/ready").await;
    assert_eq!(
        (status, body["status"].as_str()),
        (StatusCode::OK, Some("ready"))
    );

    let (status, ..) = call(&router, "/nowhere").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test(start_paused = true)]
async fn a_hanging_check_times_out_as_not_ready() {
    let router = app(Router::new(), Readiness::new(vec![Arc::new(Hangs)]));
    let (status, body, _) = call(&router, "/ready").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["checks"]["collector"], "fail");
}

#[test]
fn the_document_describes_the_system_routes_and_their_bodies() {
    let document = serde_json::to_value(health_http::document(
        "test",
        utoipa::openapi::OpenApi::default(),
    ))
    .unwrap_or(Value::Null);
    assert_eq!(document["info"]["title"], "test");
    assert_eq!(
        document["paths"]["/health"]["get"]["operationId"],
        "getHealth"
    );
    let ready = &document["paths"]["/ready"]["get"]["responses"];
    for status in ["200", "503"] {
        assert_eq!(
            ready[status]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ReadinessReport"
        );
    }
    assert_eq!(
        document["components"]["schemas"]["ReadyStatus"]["enum"],
        serde_json::json!(["ready", "not-ready"])
    );
}

#[tokio::test]
async fn responses_carry_the_security_headers_unless_their_handler_set_its_own() {
    use axum::http::header;
    use axum::routing::get;

    let api = Router::new()
        .route("/api/record", get(|| async { "{}" }))
        .route(
            "/assets/app.js",
            get(|| async {
                (
                    [(header::CACHE_CONTROL, "public, max-age=31536000, immutable")],
                    "",
                )
            }),
        );
    let router = app(api, Readiness::new(Vec::new()));
    let get = |path: &'static str| {
        router
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).expect("request"))
    };

    let record = get("/api/record").await.expect("response");
    let headers = record.headers();
    assert_eq!(
        headers["content-security-policy"],
        health_http::CONTENT_SECURITY_POLICY
    );
    assert!(
        health_http::CONTENT_SECURITY_POLICY.contains("frame-ancestors 'none'")
            && !health_http::CONTENT_SECURITY_POLICY.contains("unsafe")
    );
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(headers["x-frame-options"], "DENY");
    assert_eq!(headers["referrer-policy"], "no-referrer");
    assert_eq!(headers["cross-origin-resource-policy"], "same-origin");
    assert_eq!(headers["cross-origin-opener-policy"], "same-origin");
    assert_eq!(headers["cross-origin-embedder-policy"], "require-corp");
    assert!(
        headers["permissions-policy"]
            .to_str()
            .expect("ascii")
            .contains("camera=()")
    );
    // A patient's record is not stored by the browser or a proxy.
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");

    // A handler's own caching stays; the system routes carry the headers too.
    let asset = get("/assets/app.js").await.expect("response");
    assert_eq!(
        asset.headers()[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    let health = get("/health").await.expect("response");
    assert_eq!(health.headers()["x-frame-options"], "DENY");
}
