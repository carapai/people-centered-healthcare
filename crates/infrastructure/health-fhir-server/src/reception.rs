//! Reception and queues over HTTP (#102; clinical-workspaces.md), behind `subject_layer`, for the
//! session's facility. Checking in needs [`CHECK_IN_PERMISSION`]; moving and ending Encounters
//! [`MANAGE_PERMISSION`]; seeing queues [`QUEUE_VIEW_PERMISSION`]; calling and serving patients
//! [`QUEUE_WORK_PERMISSION`].
//!
//! - `POST /check-in` `{id, patient, servicePoint, class?}`: a new Encounter (its id a client
//!   UUIDv7) and the patient in that service point's queue;
//! - `POST /encounters/{id}/move` `{servicePoint}`, `POST /encounters/{id}/status` `{status}`;
//! - `GET /queues/{servicePoint}`: the queue, emergencies first, then by arrival;
//! - `POST /queue/{entry}/{step}`: `call`, `serve`, `recall`, `finish` or `leave`.
//!
//! Service points are named per facility: the session's facility qualifies them
//! (`KAM013/triage`), so queues of different facilities never mix. Each encounters step is
//! followed into patient-flow at once, until in-process event dispatch delivers the outbox
//! events; a retry of the follow-up changes nothing, since queue entries are kept once.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_encounters::{CheckIn, Class, EncounterEvent, EncounterStatus};
use health_encounters_app::{Encounters, EncountersError};
use health_identifiers::ResourceId;
use health_patient_flow::Step;
use health_patient_flow_app::{Flow, FlowError};
use health_persistence::{ResourceType, Store, Transaction};
use health_time::Timestamp;
use serde::Deserialize;
use serde_json::json;

use crate::FhirServer;

/// The permission to check patients in.
pub const CHECK_IN_PERMISSION: &str = "encounters.checkin";
/// The permission to move and end Encounters.
pub const MANAGE_PERMISSION: &str = "encounters.manage";
/// The permission to see queues.
pub const QUEUE_VIEW_PERMISSION: &str = "queues.view";
/// The permission to call and serve patients.
pub const QUEUE_WORK_PERMISSION: &str = "queues.work";

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn encounters_failed(error: &EncountersError) -> Response {
    let status = match error {
        EncountersError::NotFound(_) => StatusCode::NOT_FOUND,
        EncountersError::AlreadyOpen(_) => StatusCode::CONFLICT,
        EncountersError::Store(_) => return refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
        EncountersError::Refused(_) => StatusCode::UNPROCESSABLE_ENTITY,
    };
    refused(status, &error.to_string())
}

fn flow_failed(error: &FlowError) -> Response {
    let status = match error {
        FlowError::NotFound(_) => StatusCode::NOT_FOUND,
        FlowError::Store(_) => return refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
        FlowError::Refused(_) => StatusCode::UNPROCESSABLE_ENTITY,
    };
    refused(status, &error.to_string())
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

macro_rules! staff_or_return {
    ($subject:expr, $permission:expr) => {
        match staff($subject, $permission) {
            Ok(found) => found,
            Err(response) => return *response,
        }
    };
}

/// The service point `name` at the session's facility.
fn qualified(subject: &Subject, name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty() && !name.contains('/')).then(|| format!("{}/{name}", subject.facility))
}

/// Follows `event` into patient-flow, reporting a failure as such: the encounter step stands,
/// and the follow-up can be retried.
async fn follow(server: &FhirServer, event: &EncounterEvent) -> Result<(), Box<Response>> {
    Flow::new(server.store.clone())
        .follow(event)
        .await
        .map_err(|error| Box::new(flow_failed(&error)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckInBody {
    id: String,
    patient: String,
    service_point: String,
    #[serde(default)]
    class: Option<Class>,
}

async fn check_in(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<CheckInBody>,
) -> Response {
    let subject = staff_or_return!(subject, CHECK_IN_PERMISSION);
    let Some(service_point) = qualified(&subject, &body.service_point) else {
        return refused(StatusCode::BAD_REQUEST, "name a service point");
    };
    if body.id.parse::<ResourceId>().is_err() {
        return refused(
            StatusCode::BAD_REQUEST,
            "the id is a client-generated UUIDv7 (ADR 0013)",
        );
    }
    // The patient must exist here.
    let known = async {
        let id = body
            .patient
            .strip_prefix("Patient/")?
            .parse::<ResourceId>()
            .ok()?;
        let kind: ResourceType = "Patient".parse().ok()?;
        let mut tx = server.store.begin().await.ok()?;
        tx.read(&kind, id).await.ok().flatten()
    };
    if known.await.is_none_or(|head| head.version.deleted) {
        return refused(StatusCode::UNPROCESSABLE_ENTITY, "no such patient");
    }
    let request = CheckIn {
        id: body.id,
        patient: body.patient,
        facility: subject.facility.clone(),
        service_point,
        class: body.class.unwrap_or(Class::Ambulatory),
        by: subject.account.clone(),
        at: Timestamp::now(),
    };
    match Encounters::new(server.store.clone())
        .check_in(&request)
        .await
    {
        Ok((encounter, event)) => {
            if let Err(response) = follow(&server, &event).await {
                return *response;
            }
            (StatusCode::CREATED, Json(json!(encounter))).into_response()
        }
        Err(error) => encounters_failed(&error),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveBody {
    service_point: String,
}

async fn move_encounter(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<MoveBody>,
) -> Response {
    let subject = staff_or_return!(subject, MANAGE_PERMISSION);
    let Some(service_point) = qualified(&subject, &body.service_point) else {
        return refused(StatusCode::BAD_REQUEST, "name a service point");
    };
    match Encounters::new(server.store.clone())
        .move_to(&id, &service_point, Timestamp::now())
        .await
    {
        Ok((encounter, event)) => match follow(&server, &event).await {
            Ok(()) => Json(json!(encounter)).into_response(),
            Err(response) => *response,
        },
        Err(error) => encounters_failed(&error),
    }
}

#[derive(Deserialize)]
struct StatusBody {
    status: EncounterStatus,
}

async fn encounter_status(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<StatusBody>,
) -> Response {
    let _ = staff_or_return!(subject, MANAGE_PERMISSION);
    match Encounters::new(server.store.clone())
        .transition(&id, body.status, Timestamp::now())
        .await
    {
        Ok((encounter, event)) => match follow(&server, &event).await {
            Ok(()) => Json(json!(encounter)).into_response(),
            Err(response) => *response,
        },
        Err(error) => encounters_failed(&error),
    }
}

async fn queue(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(name): Path<String>,
) -> Response {
    let subject = staff_or_return!(subject, QUEUE_VIEW_PERMISSION);
    let Some(service_point) = qualified(&subject, &name) else {
        return refused(StatusCode::BAD_REQUEST, "name a service point");
    };
    match Flow::new(server.store.clone()).queue(&service_point).await {
        Ok(entries) => Json(json!(entries)).into_response(),
        Err(error) => flow_failed(&error),
    }
}

async fn queue_step(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path((entry, step)): Path<(String, String)>,
) -> Response {
    let subject = staff_or_return!(subject, QUEUE_WORK_PERMISSION);
    let step = match step.as_str() {
        "call" => Step::Call {
            by: subject.account.clone(),
        },
        "serve" => Step::Serve,
        "recall" => Step::Recall,
        "finish" => Step::Finish,
        "leave" => Step::Leave,
        _ => return refused(StatusCode::NOT_FOUND, "no such step"),
    };
    let flow = Flow::new(server.store.clone());
    // Only an entry at the session's facility, checked before the step is taken.
    let prefix = format!("{}/", subject.facility);
    match flow.entry(&entry).await {
        Ok(found) if found.service_point.starts_with(&prefix) => {}
        Ok(_) => return refused(StatusCode::NOT_FOUND, "no such queue entry here"),
        Err(error) => return flow_failed(&error),
    }
    match flow.step(&entry, &step, Timestamp::now()).await {
        Ok(next) => Json(json!(next)).into_response(),
        Err(error) => flow_failed(&error),
    }
}

impl FhirServer {
    /// The reception and queue routes, to mount behind `subject_layer`.
    pub fn reception_router(&self) -> Router {
        Router::new()
            .route("/check-in", post(check_in))
            .route("/encounters/{id}/move", post(move_encounter))
            .route("/encounters/{id}/status", post(encounter_status))
            .route("/queues/{service_point}", get(queue))
            .route("/queue/{entry}/{step}", post(queue_step))
            .with_state(self.clone())
    }
}
