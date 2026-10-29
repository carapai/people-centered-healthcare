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
