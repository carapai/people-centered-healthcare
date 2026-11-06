//! Patient requests over HTTP (#114; patient-identity.md § Patient-initiated workflows, §60):
//! **a request never becomes an order without staff action.**
//!
//! - **Patients** (`patient_requests_router`, behind `health_auth::patient_layer`):
//!   - `GET /requests`: their requests, for every Patient their account is linked to;
//!   - `GET /prescriptions`: their prescriptions to ask a refill of (with `level3` access only);
//!   - `POST /requests` `{id, patient, kind, prior? | facility, what? | reason, preferred?, note?}`:
//!     a refill, care or appointment proposal;
//!   - `POST /requests/{type}/{id}/withdraw` while pending.
//! - **Staff** (`requests_router`, behind `subject_layer`, at their facility):
//!   - `POST /{type}/{id}/accept`: a refill is prescribed through medications and its safety
//!     checks (`medications.prescribe`; `{acknowledge?}`), a care request becomes an order
//!     (`orders.request`), an appointment is booked `{start, end}` (`appointments.book`). The
//!     order names the proposal (`basedOn`), and the proposal is closed as accepted;
//!   - `POST /{type}/{id}/decline` `{reason}` with any of those permissions.
//!
//! The order of an accepted request has an id derived from its proposal, so a retried
//! acceptance writes it once.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::{FACILITY_TAG_SYSTEM, PatientSubject, Subject};
use health_client_registry::person::PortalAccess;
use health_identifiers::{DerivationName, OpId, ResourceId, derive_id};
use health_medications::{BasicChecks, Medicine, Prescribe};
use health_medications_app::{
    MedicationError, Medications, patient as safety_context, prescription,
};
use health_patient_engagement::requests::{RequestKind, RequestState};
use health_patient_engagement_app::requests::{
    Asked, PatientRequests, RequestError, RequestView, Submission, view,
};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store, Transaction, WriteError, WriteRequest,
};
use health_time::Timestamp;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::FhirServer;
use crate::programs::found;

/// The permission to book appointments.
pub const BOOK_PERMISSION: &str = "appointments.book";

const ORDER_RULE: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/rules/order-of-request";

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn failed(error: &RequestError) -> Response {
    let status = match error {
        RequestError::Refused(_) | RequestError::NotAPrescription(_) => {
            StatusCode::UNPROCESSABLE_ENTITY
        }
        RequestError::NotLinked => StatusCode::FORBIDDEN,
        RequestError::Exists(_) => StatusCode::CONFLICT,
        RequestError::NotFound(_) => StatusCode::NOT_FOUND,
        RequestError::Store(_) => return refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
    };
    refused(status, &error.to_string())
}

fn view_json(request: &RequestView, proposal: &Value) -> Value {
    json!({
        "reference": request.reference,
        "kind": match request.kind { RequestKind::Refill => "refill", RequestKind::Care => "care", RequestKind::Appointment => "appointment" },
        "state": match request.state { RequestState::Pending => "pending", RequestState::Accepted => "accepted", RequestState::Closed => "closed" },
        "title": request.title,
        "patient": request.patient,
        "facility": request.facility,
        "reason": proposal["statusReason"]["text"].as_str().or_else(|| proposal["cancellationReason"]["text"].as_str()),
        "start": proposal["start"].as_str(),
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

fn patient(subject: Option<Extension<PatientSubject>>) -> Result<PatientSubject, Box<Response>> {
    subject
        .map(|Extension(p)| p)
        .ok_or_else(|| Box::new(refused(StatusCode::UNAUTHORIZED, "sign in")))
}

async fn my_requests(
    State(server): State<FhirServer>,
    subject: Option<Extension<PatientSubject>>,
) -> Response {
    let me = or_return!(patient(subject));
    let requests = PatientRequests::new(server.store.clone());
    let Ok(linked) = requests.linked(&me.account).await else {
        return refused(StatusCode::SERVICE_UNAVAILABLE, "try again");
    };
    let mut out = Vec::new();
    for reference in &linked {
        for kind in ["MedicationRequest", "ServiceRequest", "Appointment"] {
            for proposal in found(&server, kind, &[("patient", reference)]).await {
                if let Some(request) = view(&proposal) {
                    out.push(view_json(&request, &proposal));
                }
            }
        }
    }
    Json(out).into_response()
}

async fn my_prescriptions(
    State(server): State<FhirServer>,
    subject: Option<Extension<PatientSubject>>,
) -> Response {
    let me = or_return!(patient(subject));
    let requests = PatientRequests::new(server.store.clone());
    let Ok(linked) = requests.linked(&me.account).await else {
        return refused(StatusCode::SERVICE_UNAVAILABLE, "try again");
    };
    let mut out = Vec::new();
    for reference in &linked {
        // A patient's own clinical records need level3 (patient-portal.md).
        if requests.access(&me.account, reference).await.ok() != Some(PortalAccess::Records) {
            continue;
        }
        for resource in found(&server, "MedicationRequest", &[("patient", reference)]).await {
            if resource["intent"] != "order" {
                continue;
            }
            if let Some(open) = prescription(&resource).filter(|p| p.status.is_open()) {
                out.push(open);
            }
        }
    }
    Json(out).into_response()
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum SubmitBody {
    Refill {
        id: String,
        patient: String,
        prior: String,
        #[serde(default)]
        note: Option<String>,
    },
    Care {
        id: String,
        patient: String,
        facility: String,
        what: String,
        #[serde(default)]
        note: Option<String>,
    },
    Appointment {
        id: String,
        patient: String,
        facility: String,
        reason: String,
        #[serde(default)]
        preferred: Option<String>,
        #[serde(default)]
        note: Option<String>,
    },
}

async fn submit(
    State(server): State<FhirServer>,
    subject: Option<Extension<PatientSubject>>,
    Json(body): Json<SubmitBody>,
) -> Response {
    let me = or_return!(patient(subject));
    let (id, patient, asked, note) = match body {
        SubmitBody::Refill {
            id,
            patient,
            prior,
            note,
        } => (id, patient, Asked::Refill { prior }, note),
        SubmitBody::Care {
            id,
            patient,
            facility,
            what,
            note,
        } => (id, patient, Asked::Care { facility, what }, note),
        SubmitBody::Appointment {
            id,
            patient,
            facility,
            reason,
            preferred,
            note,
        } => (
            id,
            patient,
            Asked::Appointment {
                facility,
                reason,
                preferred,
            },
            note,
        ),
    };
    if id.parse::<ResourceId>().is_err() {
        return refused(
            StatusCode::BAD_REQUEST,
            "the id is a client-generated UUIDv7 (ADR 0013)",
        );
    }
    let submission = Submission {
        id,
        patient,
        asked,
        note,
        at: Timestamp::now().to_string(),
    };
    match PatientRequests::new(server.store.clone())
        .submit(&me.account, submission)
        .await
    {
        Ok(proposal) => match view(&proposal) {
            Some(request) => {
                (StatusCode::CREATED, Json(view_json(&request, &proposal))).into_response()
            }
            None => refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
        },
        Err(error) => failed(&error),
    }
}

async fn withdraw(
    State(server): State<FhirServer>,
    subject: Option<Extension<PatientSubject>>,
    Path((kind, id)): Path<(String, String)>,
) -> Response {
    let me = or_return!(patient(subject));
    let reference = format!("{kind}/{id}");
    match PatientRequests::new(server.store.clone())
        .withdraw(&me.account, &reference)
        .await
    {
        Ok(proposal) => match view(&proposal) {
            Some(request) => Json(view_json(&request, &proposal)).into_response(),
            None => refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
        },
        Err(error) => failed(&error),
    }
}

fn staff(subject: Option<Extension<Subject>>) -> Result<Subject, Box<Response>> {
    subject
        .map(|Extension(s)| s)
        .ok_or_else(|| Box::new(refused(StatusCode::UNAUTHORIZED, "sign in")))
}

fn needs(subject: &Subject, permission: &str) -> Result<(), Box<Response>> {
    if subject.permissions.has(permission) {
        Ok(())
    } else {
        Err(Box::new(refused(
            StatusCode::FORBIDDEN,
            &format!("this needs the {permission} permission"),
        )))
    }
}

fn permission_for(kind: RequestKind) -> &'static str {
    match kind {
        RequestKind::Refill => crate::consultation::PRESCRIBE_PERMISSION,
        RequestKind::Care => crate::consultation::ORDER_PERMISSION,
        RequestKind::Appointment => BOOK_PERMISSION,
    }
}

#[derive(Deserialize, Default)]
struct AcceptBody {
    #[serde(default)]
    acknowledge: bool,
    #[serde(default)]
    start: Option<String>,
    #[serde(default)]
    end: Option<String>,
}

fn order_id(proposal: &str) -> ResourceId {
    derive_id(&DerivationName::rule(proposal, ORDER_RULE))
}

/// Writes `order` unless a retried acceptance already did.
async fn write_order(server: &FhirServer, order: &Value) -> Result<(), Box<Response>> {
    let unavailable = || Box::new(refused(StatusCode::SERVICE_UNAVAILABLE, "try again"));
    let kind: ResourceType = order["resourceType"]
        .as_str()
        .unwrap_or_default()
        .parse()
        .map_err(|_| unavailable())?;
    let id: ResourceId = order["id"]
        .as_str()
        .unwrap_or_default()
        .parse()
        .map_err(|_| unavailable())?;
    let mut tx = server.store.begin().await.map_err(|_| unavailable())?;
    match tx
        .write(WriteRequest {
            resource_type: kind,
            id,
            json: RawJson::from_validated(order.to_string()),
            op_id: OpId::new(),
            precondition: Precondition::MustNotExist,
        })
        .await
    {
        Ok(_) => tx.commit().await.map_err(|_| unavailable()),
        Err(WriteError::PreconditionFailed { .. }) => Ok(()),
        Err(_) => Err(unavailable()),
    }
}

async fn accept(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path((kind, id)): Path<(String, String)>,
    body: Option<Json<AcceptBody>>,
) -> Response {
    let subject = or_return!(staff(subject));
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let reference = format!("{kind}/{id}");
    let requests = PatientRequests::new(server.store.clone());
    let (request, proposal) = match requests.pending(&reference, &subject.facility).await {
        Ok(found) => found,
        Err(error) => return failed(&error),
    };
    or_return!(needs(&subject, permission_for(request.kind)));
    let now = Timestamp::now();
    let order = order_id(&reference);
    match request.kind {
        RequestKind::Refill => {
            let concept = &proposal["medication"]["concept"];
            let prescribe = Prescribe {
                id: order.to_string(),
                patient: request.patient.clone(),
                encounter: None,
                facility: subject.facility.clone(),
                medicine: Medicine {
                    text: request.title.clone(),
                    system: concept["coding"][0]["system"].as_str().map(str::to_owned),
                    code: concept["coding"][0]["code"].as_str().map(str::to_owned),
                },
                dosage: proposal["dosageInstruction"][0]["text"]
                    .as_str()
                    .unwrap_or("as before")
                    .to_owned(),
                days: None,
                prescriber: subject.account.clone(),
                at: now,
                based_on: Some(reference.clone()),
            };
            let mut known = found(
                &server,
                "MedicationRequest",
                &[("patient", &request.patient)],
            )
            .await;
            known.extend(
                found(
                    &server,
                    "AllergyIntolerance",
                    &[("patient", &request.patient)],
                )
                .await,
            );
            // The refill's own proposal and the prescription it renews are not duplicates of it.
            let prior = proposal["priorPrescription"]["reference"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            known.retain(|r| {
                r["intent"] != "proposal"
                    && format!("MedicationRequest/{}", r["id"].as_str().unwrap_or_default())
                        != prior
            });
            match Medications::new(server.store.clone())
                .prescribe(
                    &prescribe,
                    &safety_context(&known),
                    &BasicChecks,
                    body.acknowledge,
                )
                .await
            {
                Ok(_) | Err(MedicationError::AlreadyWritten(_)) => {}
                Err(MedicationError::Unacknowledged(warnings)) => {
                    return (StatusCode::CONFLICT, Json(json!({ "error": "safety warnings to acknowledge", "warnings": warnings }))).into_response();
                }
                Err(error) => return refused(StatusCode::UNPROCESSABLE_ENTITY, &error.to_string()),
            }
        }
        RequestKind::Care => {
            let order = json!({
                "resourceType": "ServiceRequest", "id": order.to_string(),
                "meta": { "tag": [{ "system": FACILITY_TAG_SYSTEM, "code": subject.facility }] },
                "status": "active", "intent": "order",
                "code": proposal["code"],
                "subject": { "reference": request.patient },
                "basedOn": [{ "reference": reference }],
                "requester": { "display": subject.account },
                "authoredOn": now.to_string()
            });
            or_return!(write_order(&server, &order).await);
        }
        RequestKind::Appointment => {
            let (Some(start), Some(end)) = (body.start.as_deref(), body.end.as_deref()) else {
                return refused(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "a booking needs its start and end",
                );
            };
            match (start.parse::<Timestamp>(), end.parse::<Timestamp>()) {
                (Ok(s), Ok(e)) if s < e => {}
                _ => {
                    return refused(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "a booking starts before it ends",
                    );
                }
            }
        }
    }
    let (start, end) = (body.start.clone(), body.end.clone());
    let by = subject.account.clone();
    let closed = requests
        .accepted(&reference, &subject.facility, |p| {
            if let (Some(start), Some(end)) = (start, end) {
                p["start"] = json!(start);
                p["end"] = json!(end);
                if let Some(list) = p["participant"].as_array_mut() {
                    list.push(json!({ "actor": { "display": by }, "status": "accepted" }));
                }
            }
        })
        .await;
    match closed {
        Ok(closed) => {
            let mut answer = view(&closed).map_or(Value::Null, |r| view_json(&r, &closed));
            if request.kind != RequestKind::Appointment {
                answer["order"] = json!(format!(
                    "{}/{order}",
                    if request.kind == RequestKind::Refill {
                        "MedicationRequest"
                    } else {
                        "ServiceRequest"
                    }
                ));
            }
            Json(answer).into_response()
        }
        Err(error) => failed(&error),
    }
}

#[derive(Deserialize)]
struct DeclineBody {
    reason: String,
}

async fn decline(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path((kind, id)): Path<(String, String)>,
    Json(body): Json<DeclineBody>,
) -> Response {
    let subject = or_return!(staff(subject));
    let reference = format!("{kind}/{id}");
    let requests = PatientRequests::new(server.store.clone());
    let request = match requests.pending(&reference, &subject.facility).await {
        Ok((request, _)) => request,
        Err(error) => return failed(&error),
    };
    or_return!(needs(&subject, permission_for(request.kind)));
    match requests
        .decline(&reference, &subject.facility, &body.reason)
        .await
    {
        Ok(proposal) => match view(&proposal) {
            Some(request) => Json(view_json(&request, &proposal)).into_response(),
            None => refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
        },
        Err(error) => failed(&error),
    }
}

impl FhirServer {
    /// The patient's request routes, to mount behind `health_auth::patient_layer`.
    pub fn patient_requests_router(&self) -> Router {
        Router::new()
            .route("/requests", get(my_requests).post(submit))
            .route("/requests/{kind}/{id}/withdraw", post(withdraw))
            .route("/prescriptions", get(my_prescriptions))
            .with_state(self.clone())
    }

    /// The staff's request routes, to mount behind `subject_layer`.
    pub fn requests_router(&self) -> Router {
        Router::new()
            .route("/{kind}/{id}/accept", post(accept))
            .route("/{kind}/{id}/decline", post(decline))
            .with_state(self.clone())
    }
}
