//! The shared Axum router for central, edge and desktop (ADR 0036; dependency-rules.md:
//! routers live in `health-http` and `health-fhir-server`).
//!
//! - [`app`] adds the system routes (`/health`, `/ready`) and request tracing to a runtime's
//!   API routes.
//! - Handlers are plain Axum services, so they are transport-agnostic: HTTP serves them over a
//!   socket, and the desktop's IPC dispatch calls the same `Router` with `tower::ServiceExt::oneshot`
//!   and no socket.
//! - Request spans carry the method, the matched route **template**, a request id, the status and
//!   the duration. The concrete path, the query string and headers are never recorded, because
//!   they can hold identifiers and search terms (ADR 0047).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::{MatchedPath, Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use serde_json::json;
use tracing::Instrument;

/// The response header carrying the request id, for support without reading any record.
pub const REQUEST_ID: &str = "x-request-id";

/// How long one readiness check may take before it counts as failing.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(2);

/// One dependency the node needs before it serves traffic, such as the migrated database.
pub trait ReadinessCheck: Send + Sync + 'static {
    /// A short, static name shown on `/ready`, e.g. `database`.
    fn name(&self) -> &'static str;

    /// Whether the dependency is usable now.
    fn check(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;
}

/// The readiness checks behind `/ready`.
#[derive(Clone, Default)]
pub struct Readiness {
    checks: Arc<Vec<Arc<dyn ReadinessCheck>>>,
}

impl Readiness {
    /// Readiness over `checks`; with none, the node is always ready.
    pub fn new(checks: Vec<Arc<dyn ReadinessCheck>>) -> Self {
        Self {
            checks: Arc::new(checks),
        }
    }

    /// Runs every check, each bounded by [`CHECK_TIMEOUT`].
    pub async fn evaluate(&self) -> Vec<(&'static str, bool)> {
        let mut results = Vec::with_capacity(self.checks.len());
        for check in self.checks.iter() {
            let passed = tokio::time::timeout(CHECK_TIMEOUT, check.check())
                .await
                .unwrap_or(false);
            results.push((check.name(), passed));
        }
        results
    }
}

async fn health() -> Response {
    Json(json!({ "status": "pass" })).into_response()
}

async fn ready(State(readiness): State<Readiness>) -> Response {
    let results = readiness.evaluate().await;
    let all = results.iter().all(|(_, passed)| *passed);
    let checks: serde_json::Map<String, serde_json::Value> = results
        .into_iter()
        .map(|(name, passed)| (name.to_owned(), json!(if passed { "pass" } else { "fail" })))
        .collect();
    let status = if all {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let body = json!({ "status": if all { "ready" } else { "not-ready" }, "checks": checks });
    (status, Json(body)).into_response()
}

/// `/health` (liveness: the process answers) and `/ready` (every [`ReadinessCheck`] passes).
pub fn system_routes(readiness: Readiness) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .with_state(readiness)
}

/// Traces one request: an `http.request` span with the method, route template and request id,
/// and a closing event with the status and duration.
pub async fn trace_request(request: Request, next: Next) -> Response {
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "unmatched".to_owned(), |p| p.as_str().to_owned());
    let request_id = uuid::Uuid::now_v7().to_string();
    let span = tracing::info_span!(
        "http.request",
        http.method = %request.method(),
        http.route = %route,
        request_id = %request_id,
        http.status_code = tracing::field::Empty,
        duration_ms = tracing::field::Empty,
    );
    let started = Instant::now();
    let mut response = next.run(request).instrument(span.clone()).await;
    let status = response.status().as_u16();
    let duration_ms = started.elapsed().as_secs_f64() * 1000.0;
    span.record("http.status_code", status);
    span.record("duration_ms", duration_ms);
    span.in_scope(|| tracing::info!(http.status_code = status, duration_ms, "request completed"));
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(REQUEST_ID, value);
    }
    response
}

/// A runtime's application: its `api` routes plus the system routes, all traced.
pub fn app(api: Router, readiness: Readiness) -> Router {
    api.merge(system_routes(readiness))
        .layer(middleware::from_fn(trace_request))
}
