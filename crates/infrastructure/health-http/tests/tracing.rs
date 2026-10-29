//! Request tracing through the export layer. In its own test binary: `tracing` caches callsite
//! interest per process, so a test that installs a thread-local subscriber must not share a
//! binary with tests that run without one.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use health_http::{REQUEST_ID, Readiness, app};
use health_observability::{ExportLayer, LogSink};
use http_body_util::BodyExt;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use serde_json::Value;
use tower::ServiceExt;
use tracing_subscriber::layer::SubscriberExt;

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

#[derive(Default)]
struct Lines(Mutex<Vec<String>>);

impl LogSink for Lines {
    fn write(&self, line: &str) {
        self.0.lock().expect("lines").push(line.to_owned());
    }
}

#[tokio::test]
async fn request_spans_record_the_route_template_never_the_path_or_query() {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let lines = Arc::new(Lines::default());
    let subscriber = tracing_subscriber::registry().with(ExportLayer::new(
        Some(provider.tracer("router-test")),
        lines.clone(),
    ));
    let _guard = tracing::subscriber::set_default(subscriber);

    let api = Router::new().route("/fhir/{resource_type}/{id}", get(|| async { "ok" }));
    let router = app(api, Readiness::default());
    let (status, ..) = call(
        &router,
        "/fhir/Patient/CM90012345678X?name=Akello%20Grace&phone=0772123456",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    provider.force_flush().expect("flush");

    let spans = exporter.get_finished_spans().expect("spans");
    let span = spans
        .iter()
        .find(|s| s.name == "http.request")
        .expect("request span");
    let attribute = |key: &str| {
        span.attributes
            .iter()
            .find(|kv| kv.key.as_str() == key)
            .map(|kv| kv.value.to_string())
    };
    assert_eq!(
        attribute("http.route").as_deref(),
        Some("/fhir/{resource_type}/{id}")
    );
    assert_eq!(attribute("http.method").as_deref(), Some("GET"));
    assert_eq!(attribute("http.status_code").as_deref(), Some("200"));
    assert!(attribute("duration_ms").is_some());
    let exported = format!("{spans:?}\n{}", lines.0.lock().expect("lines").join("\n"));
    for secret in ["CM90012345678X", "Akello", "0772123456", "name=", "phone="] {
        assert!(!exported.contains(secret), "`{secret}` was exported");
    }
}
