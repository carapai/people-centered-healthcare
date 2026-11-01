//! The patient portal's self-registration API (#111; patient-identity.md § Self-registration):
//! `GET /registration` is the signed-in patient's Registration Request, and
//! `POST /registration` submits their QuestionnaireResponse. Both need a patient's
//! [`PatientSubject`] (`health_auth::patient_layer`).
//!
//! The client registry is reached here through the patient-engagement application's
//! [`PatientRegistry`] port: `$match` for the answers, and a self-registered source Patient that
//! is never auto-linked to a golden record.

use axum::Extension;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use health_access_policy::PatientSubject;
use health_audit::Agent;
use health_client_registry::person::PortalAccess;
use health_identifiers::ResourceId;
use health_patient_engagement::registration::Status;
use health_patient_engagement_app::{
    AuditTrail as EngagementAudit, Candidate, EngagementError, PatientRegistry, RegistrationView,
    SelfRegistration,
};
use health_persistence::{RawJson, WriteError};
use health_registries_app::matching::Matcher;
use health_registries_app::{AuditTrail, ClientRegistry};
use health_surrealdb::{SurrealStore, SurrealTx};
use serde_json::{Value, json};

use crate::FhirServer;
use crate::http::require_json_body;
use crate::outcome::FhirError;
use crate::registry::{RegistryAudit, RegistrySettings};

/// The client registry for self-registration.
pub(crate) struct PortalRegistry {
    store: SurrealStore,
    settings: std::sync::Arc<RegistrySettings>,
    audit: RegistryAudit,
}

impl PatientRegistry for PortalRegistry {
    async fn find(&self, patient: &Value) -> Result<Option<Vec<Candidate>>, EngagementError> {
        let matcher = Matcher::new(
            self.store.clone(),
            self.settings.rules.clone(),
            self.settings.config.clone(),
        );
        let matched = matcher
            .find(patient, 10, false)
            .await
            .map_err(|e| EngagementError::Registry(e.to_string()))?;
        if matched.insufficient {
            return Ok(None);
        }
        Ok(Some(
            matched
                .candidates
                .into_iter()
                .map(|c| Candidate {
                    patient: c.id.to_string(),
                    certain: c.evaluation.grade.auto_links(),
                })
                .collect(),
        ))
    }

    async fn create_self_registered(
        &self,
        id: ResourceId,
        patient: &Value,
        actor: &str,
    ) -> Result<(), EngagementError> {
        ClientRegistry::new(
            self.store.clone(),
            self.audit.clone(),
            self.settings.config.clone(),
        )
        .create_self_registered(id, patient, actor)
        .await
        .map_err(|e| EngagementError::Registry(e.to_string()))
    }
}

impl EngagementAudit<SurrealTx> for RegistryAudit {
    async fn written(
        &self,
        tx: &mut SurrealTx,
        actor: &str,
        reason: &str,
        json: &RawJson,
    ) -> Result<(), WriteError> {
        AuditTrail::written(self, tx, actor, reason, json).await
    }
}

type Registration = SelfRegistration<SurrealStore, PortalRegistry, RegistryAudit>;

fn use_case(
    server: &FhirServer,
    patient: Option<Extension<PatientSubject>>,
) -> Result<(PatientSubject, Registration), FhirError> {
    let Some(settings) = server.registry.clone() else {
        return Err(FhirError::not_found("the client registry is not enabled"));
    };
    let Some(Extension(patient)) = patient else {
        return Err(FhirError::new(
            StatusCode::UNAUTHORIZED,
            "login",
            "self-registration needs a patient signed in with their phone",
        ));
    };
    let audit = RegistryAudit {
        auditor: server.audit.clone(),
        agent: Agent {
            display: format!("patient account {}", patient.account),
            ..Agent::unauthenticated()
        },
    };
    let registry = PortalRegistry {
        store: server.store.clone(),
        settings,
        audit: audit.clone(),
    };
    Ok((
        patient,
        SelfRegistration::new(server.store.clone(), registry, audit),
    ))
}

fn status_code(status: Status) -> &'static str {
    match status {
        Status::Draft => "draft",
        Status::Submitted => "submitted",
        Status::Verified => "verified",
        Status::Review => "review",
        Status::Matched => "matched",
        Status::Completed => "completed",
    }
}

fn body(view: &RegistrationView) -> Value {
    json!({
        "status": status_code(view.status),
        "patient": view.patient,
        "access": match view.access {
            PortalAccess::None => "none",
            PortalAccess::SelfSubmitted => "self-submitted",
            PortalAccess::Records => "records",
        },
    })
}

fn failed(error: EngagementError) -> FhirError {
    match error {
        EngagementError::Invalid(message) => {
            FhirError::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid", message)
        }
        EngagementError::AlreadySubmitted => FhirError::new(
            StatusCode::CONFLICT,
            "duplicate",
            "this account has already submitted its registration",
        ),
        EngagementError::Registry(_) | EngagementError::Store(_) => FhirError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "transient",
            "registration is unavailable; try again",
        ),
    }
}

/// `GET /registration`.
async fn registration_status(
    State(server): State<FhirServer>,
    patient: Option<Extension<PatientSubject>>,
) -> Result<Response, FhirError> {
    let (patient, registration) = use_case(&server, patient)?;
    match registration
        .status(&patient.account)
        .await
        .map_err(failed)?
    {
        Some(view) => Ok(axum::Json(body(&view)).into_response()),
        None => Err(FhirError::not_found("no registration has been submitted")),
    }
}

/// `POST /registration`: the body is the self-registration QuestionnaireResponse.
async fn submit_registration(
    State(server): State<FhirServer>,
    patient: Option<Extension<PatientSubject>>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<Response, FhirError> {
    let (patient, registration) = use_case(&server, patient)?;
    require_json_body(&headers)?;
    let response: Value = serde_json::from_slice(&bytes).map_err(|_| {
        FhirError::new(StatusCode::BAD_REQUEST, "structure", "the body is not JSON")
    })?;
    let view = registration
        .submit(&patient.account, &patient.phone, &response)
        .await
        .map_err(failed)?;
    Ok((StatusCode::CREATED, axum::Json(body(&view))).into_response())
}

impl FhirServer {
    /// The portal's self-registration routes (see the module docs); mount them behind
    /// `health_auth::patient_layer`. Without the client registry they answer 404.
    pub fn portal_router(&self) -> axum::Router {
        axum::Router::new()
            .route(
                "/registration",
                axum::routing::get(registration_status).post(submit_registration),
            )
            .with_state(self.clone())
    }
}
