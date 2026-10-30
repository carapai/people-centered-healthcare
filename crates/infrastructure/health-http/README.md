# health-http

Layer: **infrastructure**. The shared Axum router for central, edge and desktop (ADR 0036; dependency-rules.md).

- **`app(api, readiness)`** merges a runtime's API routes with the system routes and wraps them all in `trace_request`.
- **System routes:**
  - **`/health`** is liveness: the process answers.
  - **`/ready`** returns 200 only when every `ReadinessCheck` passes, each within 2 s, and 503 with the failing check names otherwise.
- **OpenAPI (`utoipa`):** the system routes' bodies are typed (`Health`, `ReadinessReport`) and documented. `system_api()` is an `OpenApiRouter`, and `document(title, api)` merges a runtime's paths with the system routes' into its OpenAPI document. `@health/api-client` is generated from that document.
- **`trace_request`** opens an `http.request` span with the method, the matched route **template**, a UUIDv7 request id (also sent as `x-request-id`), the status and the duration. The concrete path, the query string and headers are never recorded.
- **Transport-agnostic:** handlers are plain services. The desktop IPC dispatch calls the same `Router` through `tower::ServiceExt::oneshot`, which is how the tests call it too.

`tests/tracing.rs` is a separate test binary: `tracing` caches callsite interest per process, so a test that installs a thread-local subscriber cannot share a binary with tests that run without one.
