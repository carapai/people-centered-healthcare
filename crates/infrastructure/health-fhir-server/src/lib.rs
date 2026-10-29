//! FHIR R5 REST over the SurrealDB store (persistence.md § REST semantics implemented here;
//! fhir-r5-model.md § Versioning and REST obligations).
//!
//! [`FhirServer::router`] serves, relative to the FHIR base:
//!
//! | Interaction | Route |
//! |---|---|
//! | capabilities | `GET /metadata` |
//! | read, update, delete | `GET`, `PUT`, `DELETE /{type}/{id}` |
//! | vread | `GET /{type}/{id}/_history/{vid}` |
//! | create | `POST /{type}` |
//! | search-type | `GET /{type}`, `POST /{type}/_search` |
//! | history | `GET /{type}/{id}/_history`, `/{type}/_history`, `/_history` |
//!
//! | conditional create, update, delete | `POST /{type}` with `If-None-Exist`; `PUT`, `DELETE /{type}?…` |
//!
//! Transaction and batch Bundles and reference integrity are separate tasks; until then the
//! CapabilityStatement does not declare them and the requests are refused.
//! Request bodies are checked only for shape (a JSON object of the path's type) until validation
//! tiers 1–2 (#59) are on the write path.

mod bundle;
mod capability;
mod conditional;
mod history;
mod http;
mod interactions;
mod outcome;
mod search;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::routing::{get, post};
use health_fhir_search::SearchPolicy;
use health_identifiers::ResourceId;
use health_persistence::{ResourceType, RetryPolicy};
use health_surrealdb::{SurrealStore, ownership};
use health_time::Timestamp;

pub use capability::{TYPE_INTERACTIONS, capability_statement};
pub use http::{FHIR_JSON, etag, http_date};
pub use outcome::{FhirError, operation_outcome};
pub use search::CURSOR;

/// The FHIR REST server for one node.
#[derive(Clone)]
pub struct FhirServer {
    store: SurrealStore,
    base: Arc<str>,
    policy: Arc<dyn SearchPolicy>,
    retry: RetryPolicy,
    started: Timestamp,
}

impl FhirServer {
    /// A server over `store` whose public base URL is `base` (e.g. `https://central.ug/fhir`),
    /// restricting every search with `policy`.
    pub fn new(
        store: SurrealStore,
        base: impl Into<String>,
        policy: Arc<dyn SearchPolicy>,
    ) -> Self {
        Self {
            store,
            base: base.into().trim_end_matches('/').into(),
            policy,
            retry: RetryPolicy::default(),
            started: Timestamp::now(),
        }
    }

    /// The routes, relative to the FHIR base; nest them under it (e.g. `/fhir`).
    pub fn router(self) -> Router {
        Router::new()
            .route("/metadata", get(interactions::metadata))
            .route("/_history", get(history::system))
            .route(
                "/{type}",
                get(search::get)
                    .post(interactions::create)
                    .put(conditional::update)
                    .delete(conditional::delete),
            )
            .route("/{type}/_search", post(search::post))
            .route("/{type}/_history", get(history::of_type))
            .route(
                "/{type}/{id}",
                get(interactions::read)
                    .put(interactions::update)
                    .delete(interactions::delete),
            )
            .route("/{type}/{id}/_history", get(history::instance))
            .route("/{type}/{id}/_history/{vid}", get(interactions::vread))
            .with_state(self)
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{path}", self.base)
    }

    /// Waits before retrying a use case that lost a write-write conflict.
    async fn back_off(attempt: u32) {
        tokio::time::sleep(Duration::from_millis(5 * u64::from(attempt))).await;
    }
}

/// A resource type this server stores, or 404.
fn resource_type(name: &str) -> Result<ResourceType, FhirError> {
    let unsupported = || FhirError::not_found(format!("resource type {name} is not supported"));
    ownership::resource(name).ok_or_else(unsupported)?;
    name.parse().map_err(|_| unsupported())
}

/// A resource id from a path, or `None` when it cannot name a stored resource (ids are
/// client-generated UUIDs, ADR 0013).
fn resource_id(text: &str) -> Option<ResourceId> {
    text.parse().ok()
}

/// The decoded `name=value` pairs of a query string or form body.
fn pairs(text: Option<&str>) -> Vec<(String, String)> {
    form_urlencoded::parse(text.unwrap_or_default().as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}
