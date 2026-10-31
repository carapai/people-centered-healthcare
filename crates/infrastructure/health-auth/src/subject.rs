//! Each request's subject (ADR 0056): the verified access token, the current permission snapshot
//! for the session's facility, the country's role → permission mappings and the declared purpose
//! of use, as the `health_access_policy::Subject` and `health_audit::Agent` request extensions
//! the FHIR server decides and audits with.

use std::sync::Arc;

use axum::extract::{OriginalUri, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use health_access_policy::{Subject, UnknownPurpose, purpose_of_use};
use health_audit::Agent;
use health_identity::snapshot::RolePermissions;
use health_identity_app::subject_of;
use serde_json::json;

use crate::server::{AccessError, AccessVerifier};
use crate::store::AuthStore;

/// The header a request declares its purpose of use in (ADR 0056).
pub const PURPOSE_HEADER: &str = "x-purpose-of-use";

/// Why a request has no subject.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// No valid access token, or no current snapshot for the session's facility: 401.
    #[error(transparent)]
    Access(#[from] AccessError),
    /// The declared purpose of use is not a v3 PurposeOfUse code: 400.
    #[error(transparent)]
    Purpose(#[from] UnknownPurpose),
}

impl IntoResponse for Refused {
    fn into_response(self) -> Response {
        match self {
            Self::Access(error) => error.into_response(),
            Self::Purpose(error) => (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/fhir+json")],
                json!({
                    "resourceType": "OperationOutcome",
                    "issue": [{ "severity": "error", "code": "value", "diagnostics": error.to_string() }],
                })
                .to_string(),
            )
                .into_response(),
        }
    }
}

/// Builds each request's subject; [`subject_layer`] runs it in front of resource routes.
pub struct Subjects<S> {
    verifier: AccessVerifier<S>,
    roles: Arc<RolePermissions>,
}

impl<S> Clone for Subjects<S> {
    fn clone(&self) -> Self {
        Self {
            verifier: self.verifier.clone(),
            roles: Arc::clone(&self.roles),
        }
    }
}

impl<S: AuthStore> Subjects<S> {
    /// Subjects verified by `verifier`, with permissions from `roles`.
    pub fn new(verifier: AccessVerifier<S>, roles: RolePermissions) -> Self {
        Self {
            verifier,
            roles: Arc::new(roles),
        }
    }

    /// The subject and audit agent of a `method` request to the absolute `uri`. Permissions come
    /// from the snapshot current now, so a changed or revoked snapshot applies at once.
    pub async fn resolve(
        &self,
        headers: &HeaderMap,
        method: &str,
        uri: &str,
    ) -> Result<(Subject, Agent), Refused> {
        let principal = self.verifier.verify(headers, method, uri).await?;
        let declared = match headers.get(PURPOSE_HEADER) {
            Some(value) => Some(
                value
                    .to_str()
                    .map_err(|_| UnknownPurpose("(not text)".into()))?,
            ),
            None => None,
        };
        let purpose = purpose_of_use(declared)?;
        let facility = principal
            .facility
            .as_deref()
            .ok_or(AccessError::InvalidToken)?;
        let snapshot = self
            .verifier
            .current_snapshot(principal.account, facility)
            .await?;
        let subject = subject_of(&snapshot, &self.roles, purpose.clone());
        let agent = Agent {
            who: None,
            display: format!("account {}", principal.account),
            client: Some(principal.client_id),
            network: None,
            purpose: vec![purpose],
            break_glass: false,
        };
        Ok((subject, agent))
    }

    /// The absolute URL of a request path, on the issuer's origin (for the DPoP `htu`).
    fn absolute(&self, path: &str) -> String {
        format!("{}{path}", self.verifier.origin())
    }
}

/// Middleware: inserts the request's `Subject` and `Agent`, or refuses it (401, or 400 for an
/// unknown purpose of use). A request with no credentials at all passes without a subject, so
/// the routes decide: the FHIR server answers 401 except for `/metadata`. Use with
/// `axum::middleware::from_fn_with_state`.
pub async fn subject_layer<S: AuthStore + 'static>(
    State(subjects): State<Subjects<S>>,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request
        .extensions()
        .get::<OriginalUri>()
        .map_or_else(|| request.uri().path().to_owned(), |o| o.path().to_owned());
    let uri = subjects.absolute(&path);
    match subjects
        .resolve(request.headers(), request.method().as_str(), &uri)
        .await
    {
        Ok((subject, agent)) => {
            request.extensions_mut().insert(subject);
            request.extensions_mut().insert(agent);
            next.run(request).await
        }
        Err(Refused::Access(AccessError::Missing)) => next.run(request).await,
        Err(refused) => refused.into_response(),
    }
}
