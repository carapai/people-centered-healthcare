//! The consultation's commands over HTTP (#104; clinical-workspaces.md § Orders), behind
//! `subject_layer`, for the session's facility. The client sends commands; the node builds the
//! FHIR resources (`health_clinical_documentation_app::consultation`) and writes them through the
//! write path's validation (tiers 1–2) and reference checks.
//!
//! - `POST /encounters/{id}/notes` `{id, text}` needs [`DOCUMENT_PERMISSION`]: a Composition;
//! - `POST /encounters/{id}/diagnoses` `{id, concept, provisional?}` needs
//!   [`DOCUMENT_PERMISSION`]: a Condition;
//! - `POST /encounters/{id}/orders` `{id, kind: "service", concept, note?}` needs
//!   [`ORDER_PERMISSION`]: a ServiceRequest;
//! - `POST /encounters/{id}/orders` `{id, kind: "medication", concept, dosage, days?,
//!   acknowledge?}` needs [`PRESCRIBE_PERMISSION`]: a prescription, through the medications
//!   module and its clinical safety checks (`crate::pharmacy`).
//!
//! The Encounter must be in progress at the session's facility (404 for another facility's,
//! 409 for one not in progress). Ids are the client's UUIDv7s: a retried command answers the
//! resource already written (200) instead of writing it again (201).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_clinical_documentation_app::consultation::{
    CommandRefused, Concept, Context, diagnosis, note, service_order,
};
use health_encounters::EncounterStatus;
use health_encounters_app::{Encounters, EncountersError};
use health_identifiers::ResourceId;
use health_persistence::{Precondition, RawJson, ResourceType, Store, Transaction};
use health_time::Timestamp;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::FhirServer;
use crate::ops::{self, Attempt};

/// The permission to write notes and diagnoses.
pub const DOCUMENT_PERMISSION: &str = "consultation.document";
/// The permission to order tests and procedures.
pub const ORDER_PERMISSION: &str = "orders.request";
/// The permission to prescribe.
pub const PRESCRIBE_PERMISSION: &str = "medications.prescribe";

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn staff(subject: Option<Extension<Subject>>, permission: &str) -> Result<Subject, Box<Response>> {
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(refused(StatusCode::UNAUTHORIZED, "sign in")));
    };
    if !subject.permissions.has(permission) {
        return Err(Box::new(refused(
            StatusCode::FORBIDDEN,
            &format!("this needs the {permission} permission"),
        )));
    }
    Ok(subject)
}

/// The command's context: the Encounter `id`, in progress at the session's facility.
async fn context(
    server: &FhirServer,
    subject: &Subject,
    id: &str,
) -> Result<Context, Box<Response>> {
    let encounter = match Encounters::new(server.store.clone()).encounter(id).await {
        Ok(found) if found.facility == subject.facility => found,
        Ok(_) | Err(EncountersError::NotFound(_)) => {
            return Err(Box::new(refused(
                StatusCode::NOT_FOUND,
                "no such visit here",
            )));
        }
        Err(_) => {
            return Err(Box::new(refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "try again",
            )));
        }
    };
    if encounter.status != EncounterStatus::InProgress {
        return Err(Box::new(refused(
            StatusCode::CONFLICT,
            "the visit is not in progress",
        )));
    }
    Ok(Context {
        encounter: format!("Encounter/{}", encounter.id),
        patient: encounter.patient,
        facility: encounter.facility,
        author: subject.account.clone(),
        at: Timestamp::now().to_string(),
    })
}

/// Writes `resource` as a new resource, or answers the one a retried command already wrote.
async fn write(server: &FhirServer, resource: Value) -> Response {
    let (Some(kind), Some(id)) = (
        resource["resourceType"]
            .as_str()
            .and_then(|t| t.parse::<ResourceType>().ok()),
        resource["id"]
            .as_str()
            .and_then(|i| i.parse::<ResourceId>().ok()),
    ) else {
        return refused(StatusCode::BAD_REQUEST, "not a resource");
    };
    let found = health_fhir_validation::validate_value(&resource);
    if !found.is_empty() {
        return crate::outcome::FhirError::invalid_resource(&found).into_response();
    }
    if let Err(error) = server.check_invariants(&resource).await {
        return error.into_response();
    }
    let attempt = async {
        let mut tx = server.store.begin().await?;
        if let Some(existing) = tx.read(&kind, id).await? {
            let value: Value = serde_json::from_str(existing.json.as_str()).unwrap_or(Value::Null);
            return Ok::<_, Attempt>((StatusCode::OK, value));
        }
        let json = RawJson::from_validated(resource.to_string());
        let (_, stored) = ops::write(&mut tx, &kind, id, &json, Precondition::MustNotExist).await?;
        crate::references::check_written(server, &mut tx, &stored).await?;
        tx.commit().await?;
        let value: Value = serde_json::from_str(stored.as_str()).unwrap_or(Value::Null);
        Ok((StatusCode::CREATED, value))
    };
    match attempt.await {
        Ok((status, value)) => (status, Json(value)).into_response(),
        Err(Attempt::Fail(error)) => error.into_response(),
        Err(Attempt::Conflict) => refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
    }
}

fn command_refused(error: &CommandRefused) -> Response {
    refused(StatusCode::UNPROCESSABLE_ENTITY, &error.to_string())
}

fn client_id(id: &str) -> Result<(), Box<Response>> {
    id.parse::<ResourceId>().map(|_| ()).map_err(|_| {
        Box::new(refused(
            StatusCode::BAD_REQUEST,
            "the id is a client-generated UUIDv7 (ADR 0013)",
        ))
    })
}

macro_rules! or_return {
    ($result:expr) => {
        match $result {
            Ok(found) => found,
            Err(response) => return *response,
        }
    };
}

#[derive(Deserialize)]
struct NoteBody {
    id: String,
    text: String,
}

async fn add_note(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(encounter): Path<String>,
    Json(body): Json<NoteBody>,
) -> Response {
    let subject = or_return!(staff(subject, DOCUMENT_PERMISSION));
    or_return!(client_id(&body.id));
    let context = or_return!(context(&server, &subject, &encounter).await);
    match note(&body.id, &context, &body.text) {
        Ok(resource) => write(&server, resource).await,
        Err(error) => command_refused(&error),
    }
}

#[derive(Deserialize)]
struct DiagnosisBody {
    id: String,
    concept: Concept,
    #[serde(default)]
    provisional: bool,
}

async fn add_diagnosis(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(encounter): Path<String>,
    Json(body): Json<DiagnosisBody>,
) -> Response {
    let subject = or_return!(staff(subject, DOCUMENT_PERMISSION));
    or_return!(client_id(&body.id));
    let context = or_return!(context(&server, &subject, &encounter).await);
    match diagnosis(&body.id, &context, &body.concept, body.provisional) {
        Ok(resource) => write(&server, resource).await,
        Err(error) => command_refused(&error),
    }
}

fn laboratory() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum OrderBody {
    Service {
        id: String,
        concept: Concept,
        #[serde(default)]
        note: Option<String>,
        /// A laboratory test, which laboratory follows (#109); the default.
        #[serde(default = "laboratory")]
        laboratory: bool,
    },
    Medication {
        id: String,
        concept: Concept,
        dosage: String,
        #[serde(default)]
        days: Option<u32>,
        #[serde(default)]
        acknowledge: bool,
    },
}

async fn add_order(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(encounter): Path<String>,
    Json(body): Json<OrderBody>,
) -> Response {
    let (permission, id) = match &body {
        OrderBody::Service { id, .. } => (ORDER_PERMISSION, id),
        OrderBody::Medication { id, .. } => (PRESCRIBE_PERMISSION, id),
    };
    let subject = or_return!(staff(subject, permission));
    or_return!(client_id(id));
    let context = or_return!(context(&server, &subject, &encounter).await);
    match &body {
        OrderBody::Service {
            id,
            concept,
            note,
            laboratory,
        } => match service_order(id, &context, concept, note.as_deref(), *laboratory) {
            Ok(resource) => {
                let response = write(&server, resource).await;
                if *laboratory && response.status() == StatusCode::CREATED {
                    crate::lab::follow(&server, &context, id, concept).await;
                }
                response
            }
            Err(error) => command_refused(&error),
        },
        // A prescription is the medications module's (#108).
        OrderBody::Medication {
            id,
            concept,
            dosage,
            days,
            acknowledge,
        } => {
            crate::pharmacy::prescribe(&server, &context, id, concept, dosage, *days, *acknowledge)
                .await
        }
    }
}

impl FhirServer {
    /// The consultation's command routes, to mount behind `subject_layer`.
    pub fn consultation_router(&self) -> Router {
        Router::new()
            .route("/encounters/{id}/notes", post(add_note))
            .route("/encounters/{id}/diagnoses", post(add_diagnosis))
            .route("/encounters/{id}/orders", post(add_order))
            .with_state(self.clone())
    }
}
