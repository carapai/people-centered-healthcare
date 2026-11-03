//! `POST /provisioning/nodes`: an edge redeems its one-time code. The code is the only
//! credential: the edge has no account at central yet. Nest the router where the API is.
//!
//! The body is JSON: `code`, `request` (the facility intermediate's PKCS#10 request, PEM) and
//! `encrypted` (whether the edge passed the full-disk encryption gate). The answer is 201 with
//! `node_id`, `short_code`, `facility` and `chain` (the intermediate, then the issuing CA, PEM);
//! 403 `code_refused` for an unknown, expired or used code; 422 `not_encrypted`; 400
//! `invalid_request` for a request the issuing CA refuses; 503 if the store failed.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use health_identity::CodeRefused;
use serde::Deserialize;
use serde_json::json;

use crate::{Error, Issuer, NodeRegistry};

#[derive(Clone)]
struct Routes {
    registry: NodeRegistry,
    issuer: Arc<dyn Issuer>,
}

/// The provisioning route over `registry`, issuing with `issuer`.
pub fn router(registry: NodeRegistry, issuer: Arc<dyn Issuer>) -> Router {
    Router::new()
        .route("/provisioning/nodes", post(provision))
        .with_state(Routes { registry, issuer })
}

#[derive(Deserialize)]
struct ProvisionRequest {
    code: String,
    request: String,
    encrypted: bool,
}

fn answer(status: StatusCode, body: serde_json::Value) -> Response {
    let no_store = [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))];
    (status, no_store, Json(body)).into_response()
}

async fn provision(State(routes): State<Routes>, Json(body): Json<ProvisionRequest>) -> Response {
    match routes
        .registry
        .redeem(
            &body.code,
            &body.request,
            body.encrypted,
            routes.issuer.as_ref(),
        )
        .await
    {
        Ok(provisioned) => answer(
            StatusCode::CREATED,
            json!({
                "node_id": provisioned.node.id.to_string(),
                "short_code": provisioned.node.short_code,
                "facility": provisioned.node.facility,
                "chain": provisioned.chain_pem,
            }),
        ),
        Err(Error::Refused(CodeRefused::NotEncrypted)) => answer(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": "not_encrypted", "detail": CodeRefused::NotEncrypted.to_string() }),
        ),
        Err(Error::Refused(refused)) => answer(
            StatusCode::FORBIDDEN,
            json!({ "error": "code_refused", "detail": refused.to_string() }),
        ),
        Err(Error::Request(detail)) => answer(
            StatusCode::BAD_REQUEST,
            json!({ "error": "invalid_request", "detail": detail }),
        ),
        Err(Error::Store(detail)) => {
            tracing::error!(%detail, "node provisioning failed");
            answer(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({ "error": "unavailable" }),
            )
        }
    }
}
