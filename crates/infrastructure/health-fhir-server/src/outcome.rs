//! Errors as OperationOutcome responses (fhir-r5-model.md: errors are 4xx/5xx with an
//! OperationOutcome).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use health_fhir_search::SearchError;
use health_persistence::{StoreError, WriteError};
use health_surrealdb::SearchFailure;
use serde_json::json;

use crate::http::fhir_json;

/// A request the server refuses, with its status and OperationOutcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FhirError {
    /// The HTTP status.
    pub status: StatusCode,
    /// The OperationOutcome `issue.code`.
    pub code: &'static str,
    /// The diagnostics shown to the client.
    pub diagnostics: String,
}

impl FhirError {
    /// Builds an error.
    pub fn new(status: StatusCode, code: &'static str, diagnostics: impl Into<String>) -> Self {
        Self {
            status,
            code,
            diagnostics: diagnostics.into(),
        }
    }

    pub(crate) fn bad_request(diagnostics: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid", diagnostics)
    }

    pub(crate) fn not_supported(diagnostics: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "not-supported", diagnostics)
    }

    pub(crate) fn not_found(diagnostics: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not-found", diagnostics)
    }

    pub(crate) fn gone() -> Self {
        Self::new(StatusCode::GONE, "deleted", "the resource has been deleted")
    }
}

/// An OperationOutcome with one issue.
pub fn operation_outcome(severity: &str, code: &str, diagnostics: &str) -> serde_json::Value {
    json!({
        "resourceType": "OperationOutcome",
        "issue": [{ "severity": severity, "code": code, "diagnostics": diagnostics }]
    })
}

impl IntoResponse for FhirError {
    fn into_response(self) -> Response {
        let severity = if self.status.is_server_error() {
            "fatal"
        } else {
            "error"
        };
        let body = operation_outcome(severity, self.code, &self.diagnostics).to_string();
        fhir_json(self.status, body)
    }
}

impl From<StoreError> for FhirError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::UnknownResourceType(t) => {
                Self::not_found(format!("resource type {t} is not supported"))
            }
            // The adapter's message is operational detail, not for clients.
            StoreError::Unavailable(_) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "transient",
                "the store is unavailable; retry later",
            ),
        }
    }
}

impl From<WriteError> for FhirError {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::PreconditionFailed {
                current: Some(current),
            } => Self::new(
                StatusCode::PRECONDITION_FAILED,
                "conflict",
                format!("the current version is {current}"),
            ),
            WriteError::PreconditionFailed { current: None } => Self::new(
                StatusCode::PRECONDITION_FAILED,
                "conflict",
                "the resource does not exist",
            ),
            WriteError::Gone => Self::gone(),
            WriteError::Conflict => Self::new(
                StatusCode::CONFLICT,
                "conflict",
                "a concurrent write kept conflicting; retry",
            ),
            WriteError::GuardHeld(_) => Self::new(
                StatusCode::CONFLICT,
                "duplicate",
                "a unique value is already used by another record",
            ),
            WriteError::Counter(_) => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "exception",
                "a number sequence is exhausted",
            ),
            WriteError::Json(e) => Self::bad_request(e.to_string()),
            WriteError::Store(e) => e.into(),
        }
    }
}

impl From<SearchError> for FhirError {
    fn from(error: SearchError) -> Self {
        let code = match &error {
            SearchError::UnknownParameter(_)
            | SearchError::UnsupportedModifier { .. }
            | SearchError::UnsupportedPrefix { .. }
            | SearchError::UnsupportedFeature(_)
            | SearchError::NotReady(_) => "not-supported",
            SearchError::InvalidValue { .. } => "invalid",
        };
        Self::new(StatusCode::BAD_REQUEST, code, error.to_string())
    }
}

impl From<SearchFailure> for FhirError {
    fn from(error: SearchFailure) -> Self {
        match error {
            SearchFailure::Unsupported(e) => Self::not_supported(e.to_string()),
            SearchFailure::Store(e) => e.into(),
        }
    }
}
