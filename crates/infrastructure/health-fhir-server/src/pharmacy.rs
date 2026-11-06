//! Prescribing and the pharmacy worklist over HTTP (#108; inventory-and-procurement.md §
//! Dispensing), behind `subject_layer`, for the session's facility. Prescriptions are the
//! medications module's (`health-medications-app`): a prescription reserves and moves no stock.
//!
//! - Prescribing is the consultation's `POST /encounters/{id}/orders` `{kind: "medication", …}`
//!   ([`prescribe`]): the patient's open prescriptions and current allergies go to the clinical
//!   safety checks first. Unacknowledged warnings answer 409 with `warnings`, and nothing is
//!   written; the prescriber sends the order again with `acknowledge: true`.
//! - `GET /worklist` needs [`VIEW_PERMISSION`]: the facility's open prescriptions, oldest first.
//! - `POST /prescriptions/{id}/status` `{status}` needs `medications.prescribe`: hold, resume,
//!   stop, cancel or complete a prescription of this facility.
//! - Dispensing (#128): preparing, completing and cancelling a dispense (see `dispensing`).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::{FACILITY_TAG_SYSTEM, Subject};
use health_clinical_documentation_app::consultation::{Concept, Context};
use health_medications::{BasicChecks, Medicine, Prescribe, PrescriptionStatus};
use health_medications_app::{MedicationError, Medications, fhir, patient, worklist};
use health_time::Timestamp;
use serde::Deserialize;
use serde_json::json;

use crate::FhirServer;
use crate::consultation::PRESCRIBE_PERMISSION;
use crate::programs::found;

/// The permission to see the pharmacy worklist.
pub const VIEW_PERMISSION: &str = "pharmacy.view";

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn failed(error: &MedicationError) -> Response {
    match error {
        MedicationError::Unacknowledged(warnings) => (
            StatusCode::CONFLICT,
            Json(json!({ "error": error.to_string(), "warnings": warnings })),
        )
            .into_response(),
        MedicationError::Refused(_) => {
            refused(StatusCode::UNPROCESSABLE_ENTITY, &error.to_string())
        }
        MedicationError::AlreadyWritten(_) => refused(StatusCode::CONFLICT, &error.to_string()),
        MedicationError::NotFound(_) => refused(StatusCode::NOT_FOUND, &error.to_string()),
        MedicationError::Store(_) => refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
    }
}

/// A prescription in the consultation's Encounter.
pub(crate) async fn prescribe(
    server: &FhirServer,
    context: &Context,
    id: &str,
    concept: &Concept,
    dosage: &str,
    days: Option<u32>,
    acknowledged: bool,
) -> Response {
    let Ok(at) = context.at.parse::<Timestamp>() else {
        return refused(StatusCode::SERVICE_UNAVAILABLE, "try again");
    };
    let request = Prescribe {
        id: id.to_owned(),
        patient: context.patient.clone(),
        encounter: Some(context.encounter.clone()),
        facility: context.facility.clone(),
        medicine: Medicine {
            text: concept.display.trim().to_owned(),
            system: concept.system.clone(),
            code: concept.code.clone(),
        },
        dosage: dosage.to_owned(),
        days,
        prescriber: context.author.clone(),
        at,
        based_on: None,
    };
    // What the safety checks know: the patient's prescriptions and allergies.
    let mut known = found(
        server,
        "MedicationRequest",
        &[("patient", &context.patient)],
    )
    .await;
    known.extend(
        found(
            server,
            "AllergyIntolerance",
            &[("patient", &context.patient)],
        )
        .await,
    );
    match Medications::new(server.store.clone())
        .prescribe(&request, &patient(&known), &BasicChecks, acknowledged)
        .await
    {
        Ok((written, warnings)) => {
            let mut body = fhir(&written);
            body["warnings"] = json!(warnings);
            (StatusCode::CREATED, Json(body)).into_response()
        }
        Err(error) => failed(&error),
    }
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

async fn worklist_route(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
) -> Response {
    let subject = match staff(subject, VIEW_PERMISSION) {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let tag = format!("{FACILITY_TAG_SYSTEM}|{}", subject.facility);
    let requests = found(&server, "MedicationRequest", &[("_tag", &tag)]).await;
    Json(worklist(&requests, &subject.facility)).into_response()
}

#[derive(Deserialize)]
struct StatusBody {
    status: String,
}

async fn status(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<StatusBody>,
) -> Response {
    let subject = match staff(subject, PRESCRIBE_PERMISSION) {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let Some(to) = PrescriptionStatus::from_code(&body.status) else {
        return refused(StatusCode::BAD_REQUEST, "not a MedicationRequest status");
    };
    // Only this facility's prescriptions.
    let tag = format!("{FACILITY_TAG_SYSTEM}|{}", subject.facility);
    let ours = found(&server, "MedicationRequest", &[("_tag", &tag)])
        .await
        .iter()
        .any(|r| r["id"] == id.as_str());
    if !ours {
        return refused(StatusCode::NOT_FOUND, "no such prescription here");
    }
    match Medications::new(server.store.clone())
        .transition(&id, to, Timestamp::now())
        .await
    {
        Ok(next) => Json(next).into_response(),
        Err(error) => failed(&error),
    }
}

impl FhirServer {
    /// The pharmacy routes, to mount behind `subject_layer`.
    pub fn pharmacy_router(&self) -> Router {
        Router::new()
            .route("/worklist", get(worklist_route))
            .route("/prescriptions/{id}/status", post(status))
            .merge(crate::dispensing::routes())
            .with_state(self.clone())
    }
}
