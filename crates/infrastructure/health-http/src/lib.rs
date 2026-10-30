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
//! - Responses are typed and documented with `utoipa` (client-application.md § Types): [`system_api`]
//!   carries the system routes' OpenAPI paths, and [`document`] builds a runtime's OpenAPI document,
//!   from which `@health/api-client` generates TypeScript types and Zod schemas.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::{MatchedPath, Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;
use tracing::Instrument;
use utoipa::ToSchema;
use utoipa::openapi::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

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

/// Liveness: the process answers.
#[derive(Debug, Serialize, ToSchema)]
pub struct Health {
    /// Always `pass`.
    pub status: HealthStatus,
}

/// Always `pass` when the process answers at all.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HealthStatus {
    /// The process answers.
    Pass,
}

/// Readiness: whether every dependency the node needs is usable.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReadinessReport {
    /// `ready` when every check passes.
    pub status: ReadyStatus,
    /// Each check's result, by its static name (e.g. `database`).
    pub checks: BTreeMap<String, CheckStatus>,
}

/// `ready` when every check passes.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ReadyStatus {
    /// Every check passes.
    Ready,
    /// At least one check fails.
    NotReady,
}

/// One readiness check's result.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CheckStatus {
    /// The dependency is usable.
    Pass,
    /// The dependency is unusable, or did not answer within [`CHECK_TIMEOUT`].
    Fail,
}

/// Liveness: the process answers.
#[utoipa::path(get, path = "/health", tag = "system", operation_id = "getHealth",
    responses((status = 200, description = "The process answers", body = Health)))]
async fn health() -> Json<Health> {
    Json(Health {
        status: HealthStatus::Pass,
    })
}

/// Readiness: every readiness check passes.
#[utoipa::path(get, path = "/ready", tag = "system", operation_id = "getReady",
    responses(
        (status = 200, description = "Every check passes", body = ReadinessReport),
        (status = 503, description = "A check fails", body = ReadinessReport),
    ))]
async fn ready(State(readiness): State<Readiness>) -> Response {
    let results = readiness.evaluate().await;
    let all = results.iter().all(|(_, passed)| *passed);
    let checks = results
        .into_iter()
        .map(|(name, passed)| {
            let status = if passed {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            };
            (name.to_owned(), status)
        })
        .collect();
    let (code, status) = if all {
        (StatusCode::OK, ReadyStatus::Ready)
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, ReadyStatus::NotReady)
    };
    (code, Json(ReadinessReport { status, checks })).into_response()
}

/// The system routes with their OpenAPI paths, for a runtime to merge into its own API.
pub fn system_api() -> OpenApiRouter<Readiness> {
    OpenApiRouter::new()
        .routes(routes!(health))
        .routes(routes!(ready))
}

/// `/health` (liveness: the process answers) and `/ready` (every [`ReadinessCheck`] passes).
pub fn system_routes(readiness: Readiness) -> Router {
    system_api().with_state(readiness).into()
}

/// A runtime's OpenAPI document: its `api` paths and schemas plus the system routes', under
/// `title`, with the workspace version.
pub fn document(title: &str, api: OpenApi) -> OpenApi {
    let (_, system) = system_api().split_for_parts();
    let mut document = api.merge_from(system);
    document.info.title = title.to_owned();
    document.info.version = env!("CARGO_PKG_VERSION").to_owned();
    document.info.description = None;
    document.info.license = None;
    document.info.contact = None;
    document
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
