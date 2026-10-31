//! Each request's subject (ADR 0056): the verified access token, the current permission snapshot
//! for the session's facility, the country's role → permission mappings and the declared purpose
//! of use, as the `health_access_policy::Subject` and `health_audit::Agent` request extensions
//! the FHIR server decides and audits with.
//!
//! **Break-glass** (offline-auth.md § Break-glass):
//! - `POST /break-glass` (with [`Subjects::routes`]) grants one: one patient, one hour, the
//!   `BTG` reason and a justification. It is stored for the session with its AuditEvent, the
//!   in-charge's alert and a privacy-officer review due in 72 hours.
//! - While a grant is active, the session's subject carries it and its agent is flagged, so
//!   every access is audited with `BTG`.
//! - The in-charge lists the facility's alerts (`GET /break-glass/alerts`) and acknowledges
//!   them (`POST /break-glass/alerts/{id}/acknowledge`); their roles map to
//!   [`ALERTS_PERMISSION`].

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{OriginalUri, Path, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use health_access_policy::{
    BreakGlass, BreakGlassPolicy, BreakGlassRefused, Subject, UnknownPurpose, purpose_of_use,
};
use health_audit::{Agent, Auditor};
use health_identity::snapshot::RolePermissions;
use health_identity_app::subject_of;
use health_time::SignedDuration;
use serde_json::json;

use crate::server::{AccessError, AccessVerifier};
use crate::store::{AuthStore, BreakGlassRecord};

/// The use-case permission a facility in-charge's roles map to, to see break-glass alerts.
pub const ALERTS_PERMISSION: &str = "break-glass.alerts";

/// How long after a grant the privacy officer's review is due (offline-auth.md: 72 h of the
/// event reaching central; an edge's grants are due from when they arrive, with sync).
pub const REVIEW_WITHIN: SignedDuration = SignedDuration::from_hours(72);

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
    auditor: Option<Arc<Auditor>>,
    policy: BreakGlassPolicy,
}

impl<S> Clone for Subjects<S> {
    fn clone(&self) -> Self {
        Self {
            verifier: self.verifier.clone(),
            roles: Arc::clone(&self.roles),
            auditor: self.auditor.clone(),
            policy: self.policy,
        }
    }
}

impl<S: AuthStore> Subjects<S> {
    /// Subjects verified by `verifier`, with permissions from `roles`.
    pub fn new(verifier: AccessVerifier<S>, roles: RolePermissions) -> Self {
        Self {
            verifier,
            roles: Arc::new(roles),
            auditor: None,
            policy: BreakGlassPolicy::default(),
        }
    }

    /// These subjects, auditing break-glass grants with `auditor`.
    #[must_use]
    pub fn with_auditor(mut self, auditor: Auditor) -> Self {
        self.auditor = Some(Arc::new(auditor));
        self
    }

    /// The subject and audit agent of a `method` request to the absolute `uri`. Permissions come
    /// from the snapshot current now, so a changed or revoked snapshot applies at once.
    pub async fn resolve(
        &self,
        headers: &HeaderMap,
        method: &str,
        uri: &str,
    ) -> Result<(Subject, Agent, uuid::Uuid), Refused> {
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
        let mut subject = subject_of(&snapshot, &self.roles, purpose.clone());
        let grant = self
            .verifier
            .store()
            .active_break_glass(principal.session, self.verifier.now())
            .await
            .map_err(|_| AccessError::InvalidToken)?;
        subject.break_glass = grant.map(|r| BreakGlass {
            patient: r.event.patient,
            granted_at: r.event.at,
            expires_at: r.event.expires_at,
        });
        let agent = Agent {
            who: None,
            display: format!("account {}", principal.account),
            client: Some(principal.client_id),
            network: None,
            purpose: vec![purpose],
            // Every access under an active grant is audited with BTG.
            break_glass: subject.break_glass.is_some(),
        };
        Ok((subject, agent, principal.session))
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
        Ok((subject, agent, _)) => {
            request.extensions_mut().insert(subject);
            request.extensions_mut().insert(agent);
            next.run(request).await
        }
        Err(Refused::Access(AccessError::Missing)) => next.run(request).await,
        Err(refused) => refused.into_response(),
    }
}

/// A break-glass request.
#[derive(Debug, serde::Deserialize)]
struct GlassRequest {
    /// `Patient/{id}`.
    patient: String,
    /// `BTG`.
    reason: String,
    /// Why.
    justification: String,
}

fn problem(status: StatusCode, code: &str, diagnostics: &str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/fhir+json")],
        json!({
            "resourceType": "OperationOutcome",
            "issue": [{ "severity": "error", "code": code, "diagnostics": diagnostics }],
        })
        .to_string(),
    )
        .into_response()
}

impl<S: AuthStore + 'static> Subjects<S> {
    /// The break-glass routes (`/break-glass`, `/break-glass/alerts`, …); nest them where the
    /// request paths stay on the issuer's origin (e.g. under `/api`).
    pub fn routes(&self) -> Router {
        Router::new()
            .route("/break-glass", post(grant::<S>))
            .route("/break-glass/alerts", get(alerts::<S>))
            .route(
                "/break-glass/alerts/{id}/acknowledge",
                post(acknowledge::<S>),
            )
            .with_state(self.clone())
    }

    async fn caller(
        &self,
        request: &RequestParts,
    ) -> Result<(Subject, Agent, uuid::Uuid), Refused> {
        self.resolve(
            &request.headers,
            request.method.as_str(),
            &self.absolute(&request.path),
        )
        .await
    }
}

/// What the break-glass handlers need of the request.
struct RequestParts {
    headers: HeaderMap,
    method: axum::http::Method,
    path: String,
}

impl RequestParts {
    fn of(headers: HeaderMap, method: axum::http::Method, uri: &OriginalUri) -> Self {
        Self {
            headers,
            method,
            path: uri.path().to_owned(),
        }
    }
}

fn record_json(record: &BreakGlassRecord) -> serde_json::Value {
    json!({
        "id": record.id,
        "account": record.event.account,
        "facility": record.event.facility,
        "patient": record.event.patient,
        "reason": record.event.reason,
        "justification": record.event.justification,
        "at": record.event.at.to_string(),
        "expires_at": record.event.expires_at.to_string(),
        "review_due": record.review_due.to_string(),
        "acknowledged": record.acknowledged,
    })
}

/// `POST /break-glass`.
async fn grant<S: AuthStore + 'static>(
    State(subjects): State<Subjects<S>>,
    method: axum::http::Method,
    uri: OriginalUri,
    headers: HeaderMap,
    Json(body): Json<GlassRequest>,
) -> Response {
    let parts = RequestParts::of(headers, method, &uri);
    let (subject, agent, session) = match subjects.caller(&parts).await {
        Ok(caller) => caller,
        Err(refused) => return refused.into_response(),
    };
    if !body.patient.starts_with("Patient/") || body.patient.len() <= "Patient/".len() {
        return problem(
            StatusCode::BAD_REQUEST,
            "value",
            "patient must be a Patient reference",
        );
    }
    let now = subjects.verifier.now();
    let (_, event) = match subjects.policy.grant(
        &subject,
        &body.patient,
        &body.reason,
        &body.justification,
        now,
    ) {
        Ok(granted) => granted,
        Err(refused @ BreakGlassRefused::NotPermitted) => {
            return problem(StatusCode::FORBIDDEN, "forbidden", &refused.to_string());
        }
        Err(refused @ BreakGlassRefused::MissingReason) => {
            return problem(StatusCode::BAD_REQUEST, "required", &refused.to_string());
        }
        Err(refused @ BreakGlassRefused::AlreadyActive) => {
            return problem(StatusCode::CONFLICT, "conflict", &refused.to_string());
        }
    };
    let record = BreakGlassRecord {
        id: uuid::Uuid::now_v7().to_string(),
        session,
        review_due: event.at.checked_add(REVIEW_WITHIN).unwrap_or(event.at),
        event,
        acknowledged: false,
    };
    let audit = subjects.auditor.as_ref().map(|auditor| {
        auditor.break_glass(
            &agent,
            &record.event.patient,
            &record.event.justification,
            now,
        )
    });
    match subjects
        .verifier
        .store()
        .grant_break_glass(&record, audit.as_ref())
        .await
    {
        Ok(()) => (StatusCode::CREATED, Json(record_json(&record))).into_response(),
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "transient",
            "break-glass is unavailable",
        ),
    }
}

/// The caller, when they may see their facility's alerts.
async fn in_charge<S: AuthStore + 'static>(
    subjects: &Subjects<S>,
    parts: &RequestParts,
) -> Result<Subject, Box<Response>> {
    let (subject, _, _) = subjects
        .caller(parts)
        .await
        .map_err(|refused| Box::new(refused.into_response()))?;
    if subject.permissions.has(ALERTS_PERMISSION) {
        Ok(subject)
    } else {
        Err(Box::new(problem(
            StatusCode::FORBIDDEN,
            "forbidden",
            "break-glass alerts are for the facility in-charge",
        )))
    }
}

/// `GET /break-glass/alerts`: the facility's unacknowledged alerts.
async fn alerts<S: AuthStore + 'static>(
    State(subjects): State<Subjects<S>>,
    method: axum::http::Method,
    uri: OriginalUri,
    headers: HeaderMap,
) -> Response {
    let parts = RequestParts::of(headers, method, &uri);
    let subject = match in_charge(&subjects, &parts).await {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    match subjects
        .verifier
        .store()
        .break_glass_alerts(&subject.facility)
        .await
    {
        Ok(records) => Json(records.iter().map(record_json).collect::<Vec<_>>()).into_response(),
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "transient",
            "break-glass is unavailable",
        ),
    }
}

/// `POST /break-glass/alerts/{id}/acknowledge`.
async fn acknowledge<S: AuthStore + 'static>(
    State(subjects): State<Subjects<S>>,
    Path(id): Path<String>,
    method: axum::http::Method,
    uri: OriginalUri,
    headers: HeaderMap,
) -> Response {
    let parts = RequestParts::of(headers, method, &uri);
    let subject = match in_charge(&subjects, &parts).await {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    match subjects
        .verifier
        .store()
        .acknowledge_break_glass(&id, &subject.facility)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => problem(
            StatusCode::NOT_FOUND,
            "not-found",
            "no such alert at this facility",
        ),
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "transient",
            "break-glass is unavailable",
        ),
    }
}
