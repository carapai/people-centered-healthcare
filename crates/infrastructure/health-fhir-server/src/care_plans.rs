//! Care plans over HTTP (#110), behind `subject_layer`, for the session's facility. Plans are
//! care-plans' (`health-care-plans-app`).
//!
//! - `POST /plans` `{id, patient, title, description?}` with [`MANAGE_PERMISSION`]: a draft;
//! - `POST /plans/{id}/status` `{status}`, `/addresses` `{condition}` and `/activities`
//!   `{request}` with [`MANAGE_PERMISSION`]: only the patient's own records here are linked;
//! - `GET /patients/{id}/plans` with [`VIEW_PERMISSION`] or [`MANAGE_PERMISSION`]: the patient's
//!   plans here.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::{FACILITY_TAG_SYSTEM, Subject};
use health_care_plans::{CarePlan, Draft, PlanStatus};
use health_care_plans_app::{CarePlanError, CarePlans, care_plan};
use health_time::Timestamp;
use serde::Deserialize;
use serde_json::json;

use crate::FhirServer;
use crate::programs::found;

/// The permission to write care plans.
pub const MANAGE_PERMISSION: &str = "careplans.manage";
/// The permission to read them.
pub const VIEW_PERMISSION: &str = "careplans.view";

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn failed(error: &CarePlanError) -> Response {
    let status = match error {
        CarePlanError::Refused(_) | CarePlanError::NotThePatients(_) => {
            StatusCode::UNPROCESSABLE_ENTITY
        }
        CarePlanError::Exists(_) => StatusCode::CONFLICT,
        CarePlanError::NotFound(_) => StatusCode::NOT_FOUND,
        CarePlanError::Store(_) => return refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
    };
    refused(status, &error.to_string())
}

fn staff(subject: Option<Extension<Subject>>, any_of: &[&str]) -> Result<Subject, Box<Response>> {
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(refused(StatusCode::UNAUTHORIZED, "sign in")));
    };
    if !any_of.iter().any(|p| subject.permissions.has(p)) {
        return Err(Box::new(refused(
            StatusCode::FORBIDDEN,
            &format!("this needs the {} permission", any_of[0]),
        )));
    }
    Ok(subject)
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
struct DraftBody {
    id: String,
    patient: String,
    title: String,
    #[serde(default)]
    description: Option<String>,
}

async fn draft(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<DraftBody>,
) -> Response {
    let subject = or_return!(staff(subject, &[MANAGE_PERMISSION]));
    if body.id.parse::<health_identifiers::ResourceId>().is_err() {
        return refused(
            StatusCode::BAD_REQUEST,
            "the id is a client-generated UUIDv7 (ADR 0013)",
        );
    }
    let request = Draft {
        id: body.id,
        patient: body.patient,
        facility: subject.facility.clone(),
        title: body.title,
        description: body.description,
        author: subject.account.clone(),
        at: Timestamp::now(),
    };
    match CarePlans::new(server.store.clone()).draft(request).await {
        Ok(plan) => (StatusCode::CREATED, Json(plan)).into_response(),
        Err(error) => failed(&error),
    }
}

/// Plan `id`, if it is this facility's.
async fn ours(server: &FhirServer, subject: &Subject, id: &str) -> Result<CarePlan, Box<Response>> {
    match CarePlans::new(server.store.clone()).care_plan(id).await {
        Ok(plan) if plan.facility == subject.facility => Ok(plan),
        Ok(_) => Err(Box::new(refused(
            StatusCode::NOT_FOUND,
            "no such care plan here",
        ))),
        Err(error) => Err(Box::new(failed(&error))),
    }
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
    let subject = or_return!(staff(subject, &[MANAGE_PERMISSION]));
    or_return!(ours(&server, &subject, &id).await);
    let Some(to) = PlanStatus::from_code(&body.status) else {
        return refused(StatusCode::BAD_REQUEST, "not a CarePlan status");
    };
    match CarePlans::new(server.store.clone())
        .transition(&id, to, Timestamp::now())
        .await
    {
        Ok(plan) => Json(plan).into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Deserialize)]
struct ConditionBody {
    condition: String,
}

async fn address(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<ConditionBody>,
) -> Response {
    let subject = or_return!(staff(subject, &[MANAGE_PERMISSION]));
    or_return!(ours(&server, &subject, &id).await);
    match CarePlans::new(server.store.clone())
        .address(&id, &body.condition)
        .await
    {
        Ok(plan) => Json(plan).into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Deserialize)]
struct RequestBody {
    request: String,
}

async fn activity(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<RequestBody>,
) -> Response {
    let subject = or_return!(staff(subject, &[MANAGE_PERMISSION]));
    or_return!(ours(&server, &subject, &id).await);
    match CarePlans::new(server.store.clone())
        .plan(&id, &body.request)
        .await
    {
        Ok(plan) => Json(plan).into_response(),
        Err(error) => failed(&error),
    }
}

async fn patient_plans(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(patient): Path<String>,
) -> Response {
    let subject = or_return!(staff(subject, &[VIEW_PERMISSION, MANAGE_PERMISSION]));
    let tag = format!("{FACILITY_TAG_SYSTEM}|{}", subject.facility);
    let reference = format!("Patient/{patient}");
    let mut plans: Vec<CarePlan> = found(&server, "CarePlan", &[("_tag", &tag)])
        .await
        .iter()
        .filter_map(care_plan)
        .filter(|p| p.patient == reference)
        .collect();
    plans.sort_by_key(|p| std::cmp::Reverse(p.created));
    Json(plans).into_response()
}

impl FhirServer {
    /// The care-plan routes, to mount behind `subject_layer`.
    pub fn care_plans_router(&self) -> Router {
        Router::new()
            .route("/plans", post(draft))
            .route("/plans/{id}/status", post(status))
            .route("/plans/{id}/addresses", post(address))
            .route("/plans/{id}/activities", post(activity))
            .route("/patients/{id}/plans", get(patient_plans))
            .with_state(self.clone())
    }
}
