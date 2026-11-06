//! Dispensing over HTTP (#128; ADR 0035; inventory-and-procurement.md § Dispensing), on the
//! pharmacy routes behind `subject_layer`, for the session's facility, needing
//! [`DISPENSE_PERMISSION`]. **Stock moves only when a dispense is completed.**
//!
//! - `POST /prescriptions/{id}/prepare` `{dispense, store, product, quantity, batches?, reason?}`:
//!   reserves stock for one of this facility's active prescriptions, by first expiry (or the
//!   pharmacist's `batches`, `[{unit, quantity}]`, with a `reason`). Only stock the patient is
//!   eligible for is taken (their active programs, under [`Dispensing::eligibility`]), and only
//!   batches in date past the course plus [`Dispensing::rule`]'s buffer. The MedicationDispense
//!   (`preparation`) is written in the same transaction. Nothing moves.
//! - `POST /dispenses/{id}/complete` `{given?}`: hands over, in full or (with `given`, by
//!   reservation) in part. The dispense movements, the `inventory.dispensed.v1` charge trigger
//!   and the `completed` MedicationDispense are one transaction.
//! - `POST /dispenses/{id}/cancel`: releases the reservations; the MedicationDispense is
//!   `cancelled`. Nothing moves.
//! - `GET /dispenses/{id}`: the MedicationDispense's reservations.

use std::collections::BTreeMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::{FACILITY_TAG_SYSTEM, Subject};
use health_identifiers::ResourceId;
use health_inventory::Policy;
use health_inventory::dispensing::{BatchRule, Eligibility, Need, Pick, Reservation};
use health_inventory_app::{Completion, Inventory, InventoryError, Preparation, Record};
use health_medications_app::prescription;
use health_persistence::{RawJson, ResourceType, Store, Transaction};
use health_time::{SignedDuration, Timestamp};
use jiff::tz::TimeZone;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::FhirServer;
use crate::programs::found;

/// The permission to prepare and hand over dispenses.
pub const DISPENSE_PERMISSION: &str = "pharmacy.dispense";

/// How a node dispenses: the funding-source rules and the batch rule (ADR 0035).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dispensing {
    /// Which program a patient must be in for each funding source's stock.
    pub eligibility: Eligibility,
    /// How far past the course a batch must last.
    pub rule: BatchRule,
}

impl Default for Dispensing {
    /// No funding source restricted, and batches in date 30 days past the course.
    fn default() -> Self {
        Self {
            eligibility: Eligibility::default(),
            rule: BatchRule { buffer_days: 30 },
        }
    }
}

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn failed(error: &InventoryError) -> Response {
    match error {
        InventoryError::NotFound(_) => refused(StatusCode::NOT_FOUND, &error.to_string()),
        InventoryError::Lapsed(_) => refused(StatusCode::CONFLICT, &error.to_string()),
        InventoryError::Busy | InventoryError::Store(_) => {
            refused(StatusCode::SERVICE_UNAVAILABLE, "try again")
        }
        _ => refused(StatusCode::UNPROCESSABLE_ENTITY, &error.to_string()),
    }
}

fn pharmacist(subject: Option<Extension<Subject>>) -> Result<Subject, Box<Response>> {
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(refused(StatusCode::UNAUTHORIZED, "sign in")));
    };
    if !subject.permissions.has(DISPENSE_PERMISSION) {
        return Err(Box::new(refused(
            StatusCode::FORBIDDEN,
            &format!("this needs the {DISPENSE_PERMISSION} permission"),
        )));
    }
    Ok(subject)
}

fn inventory(server: &FhirServer) -> Inventory<health_surrealdb::SurrealStore> {
    Inventory::new(server.store.clone(), Policy::default())
}

fn dispense_type() -> ResourceType {
    "MedicationDispense".parse().expect("a resource type")
}

fn record(id: ResourceId, json: &Value) -> Record {
    Record {
        resource_type: dispense_type(),
        id,
        json: RawJson::from_validated(json.to_string()),
    }
}

fn reservation_json(r: &Reservation) -> Value {
    json!({
        "id": r.id, "unit": r.unit.key(), "batch": r.unit.batch, "expiry": r.unit.expiry,
        "fundingSource": r.unit.funding_source, "quantity": r.quantity,
        "expires": r.expires.to_string(), "state": r.state,
    })
}

/// The patient's active programs, by their EpisodeOfCare's type code.
async fn programs(server: &FhirServer, patient: &str) -> Vec<String> {
    found(server, "EpisodeOfCare", &[("patient", patient)])
        .await
        .iter()
        .filter(|e| e["status"] == "active")
        .filter_map(|e| {
            e["type"][0]["coding"][0]["code"]
                .as_str()
                .map(str::to_owned)
        })
        .collect()
}

#[derive(Deserialize)]
struct Batch {
    unit: String,
    quantity: i64,
}

#[derive(Deserialize)]
struct PrepareBody {
    dispense: String,
    store: String,
    product: String,
    quantity: i64,
    #[serde(default)]
    batches: Option<Vec<Batch>>,
    #[serde(default)]
    reason: Option<String>,
}

async fn prepare(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<PrepareBody>,
) -> Response {
    let subject = match pharmacist(subject) {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let Ok(dispense_id) = body.dispense.parse::<ResourceId>() else {
        return refused(
            StatusCode::BAD_REQUEST,
            "the dispense id is a client-generated UUIDv7 (ADR 0013)",
        );
    };
    // Only this facility's active prescriptions.
    let tag = format!("{FACILITY_TAG_SYSTEM}|{}", subject.facility);
    let Some(resource) = found(&server, "MedicationRequest", &[("_tag", &tag)])
        .await
        .into_iter()
        .find(|r| r["id"] == id.as_str() && r["intent"] == "order")
    else {
        return refused(StatusCode::NOT_FOUND, "no such prescription here");
    };
    let Some(rx) = prescription(&resource).filter(|p| p.status.code() == "active") else {
        return refused(
            StatusCode::CONFLICT,
            "only an active prescription is dispensed",
        );
    };
    let now = Timestamp::now();
    let course_ends = rx
        .authored
        .checked_add(SignedDuration::from_hours(
            24 * i64::from(rx.days.unwrap_or(0)),
        ))
        .unwrap_or(rx.authored)
        .max(now)
        .to_zoned(TimeZone::UTC)
        .date();
    let need = Need {
        product: body.product.trim().to_owned(),
        quantity: body.quantity,
        course_ends,
        programs: programs(&server, &rx.patient).await,
    };
    let inventory = inventory(&server);
    let choice = match body.batches {
        Some(batches) => {
            let Ok(units) = inventory.stock_on_hand(&body.store).await else {
                return refused(StatusCode::SERVICE_UNAVAILABLE, "try again");
            };
            let mut picks = Vec::new();
            for batch in batches {
                let Some(unit) = units.iter().find(|u| u.unit.key() == batch.unit) else {
                    return refused(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "a batch chosen is not in this store",
                    );
                };
                picks.push(Pick {
                    unit: unit.unit.clone(),
                    quantity: batch.quantity,
                });
            }
            Some((picks, body.reason.unwrap_or_default()))
        }
        None => None,
    };
    let reference = format!("MedicationDispense/{dispense_id}");
    let medication = &resource["medication"];
    let dispense = json!({
        "resourceType": "MedicationDispense", "id": dispense_id.to_string(),
        "meta": { "tag": [{ "system": FACILITY_TAG_SYSTEM, "code": subject.facility }] },
        "status": "preparation",
        "medication": medication,
        "subject": { "reference": rx.patient },
        "authorizingPrescription": [{ "reference": format!("MedicationRequest/{id}") }],
        "location": { "reference": body.store },
        "performer": [{ "actor": { "display": subject.account } }],
        "quantity": { "value": body.quantity, "unit": need.product },
        "whenPrepared": now.to_string()
    });
    let preparation = Preparation {
        dispense: reference.clone(),
        store: body.store,
        need,
        choice,
        at: now,
        record: Some(record(dispense_id, &dispense)),
    };
    let rules = server.dispensing.clone();
    match inventory
        .prepare(&preparation, &rules.eligibility, rules.rule)
        .await
    {
        Ok(held) => (
            StatusCode::CREATED,
            Json(json!({
                "dispense": reference,
                "reservations": held.iter().map(reservation_json).collect::<Vec<_>>(),
            })),
        )
            .into_response(),
        Err(error) => failed(&error),
    }
}

/// The MedicationDispense `id` of this facility, as stored.
async fn dispense_of(server: &FhirServer, id: ResourceId, facility: &str) -> Option<Value> {
    let mut tx = server.store.begin().await.ok()?;
    let head = tx.read(&dispense_type(), id).await.ok()??;
    let json: Value = serde_json::from_str(head.json.as_str()).ok()?;
    let tagged = json["meta"]["tag"]
        .as_array()?
        .iter()
        .any(|t| t["system"] == FACILITY_TAG_SYSTEM && t["code"] == facility);
    tagged.then_some(json)
}

#[derive(Deserialize, Default)]
struct CompleteBody {
    #[serde(default)]
    given: Option<BTreeMap<String, i64>>,
}

async fn complete(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    body: Option<Json<CompleteBody>>,
) -> Response {
    let subject = match pharmacist(subject) {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let Ok(dispense_id) = id.parse::<ResourceId>() else {
        return refused(StatusCode::NOT_FOUND, "no such dispense here");
    };
    let Some(mut dispense) = dispense_of(&server, dispense_id, &subject.facility).await else {
        return refused(StatusCode::NOT_FOUND, "no such dispense here");
    };
    let reference = format!("MedicationDispense/{dispense_id}");
    let inventory = inventory(&server);
    let given = match body.and_then(|Json(b)| b.given) {
        Some(given) => given,
        // In full: everything held.
        None => match inventory.reservations(&reference).await {
            Ok(held) => held
                .iter()
                .filter(|r| r.state == health_inventory::dispensing::ReservationState::Held)
                .map(|r| (r.id.clone(), r.quantity))
                .collect(),
            Err(error) => return failed(&error),
        },
    };
    let now = Timestamp::now();
    let total: i64 = given.values().sum();
    dispense["status"] = json!("completed");
    dispense["whenHandedOver"] = json!(now.to_string());
    dispense["quantity"]["value"] = json!(total);
    let completion = Completion {
        dispense: reference.clone(),
        patient: dispense["subject"]["reference"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        given,
        may_emergency_dispense: false,
        actor: subject.account.clone(),
        node: subject.facility.clone(),
        at: now,
        record: Some(record(dispense_id, &dispense)),
    };
    match inventory.complete_dispense(&completion).await {
        Ok(moved) => Json(json!({
            "dispense": reference,
            "status": "completed",
            "given": moved.iter().map(|m| json!({ "unit": m.unit.key(), "batch": m.unit.batch, "quantity": -m.quantity })).collect::<Vec<_>>(),
        }))
        .into_response(),
        Err(error) => failed(&error),
    }
}

async fn cancel(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
) -> Response {
    let subject = match pharmacist(subject) {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let Ok(dispense_id) = id.parse::<ResourceId>() else {
        return refused(StatusCode::NOT_FOUND, "no such dispense here");
    };
    let Some(mut dispense) = dispense_of(&server, dispense_id, &subject.facility).await else {
        return refused(StatusCode::NOT_FOUND, "no such dispense here");
    };
    if dispense["status"] != "preparation" {
        return refused(
            StatusCode::CONFLICT,
            "only a dispense being prepared is cancelled",
        );
    }
    dispense["status"] = json!("cancelled");
    let reference = format!("MedicationDispense/{dispense_id}");
    match inventory(&server)
        .cancel_dispense(
            &reference,
            Timestamp::now(),
            Some(&record(dispense_id, &dispense)),
        )
        .await
    {
        Ok(released) => {
            Json(json!({ "dispense": reference, "status": "cancelled", "released": released }))
                .into_response()
        }
        Err(error) => failed(&error),
    }
}

async fn reservations(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
) -> Response {
    let subject = match pharmacist(subject) {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let Ok(dispense_id) = id.parse::<ResourceId>() else {
        return refused(StatusCode::NOT_FOUND, "no such dispense here");
    };
    let Some(dispense) = dispense_of(&server, dispense_id, &subject.facility).await else {
        return refused(StatusCode::NOT_FOUND, "no such dispense here");
    };
    match inventory(&server)
        .reservations(&format!("MedicationDispense/{dispense_id}"))
        .await
    {
        Ok(held) => Json(json!({
            "status": dispense["status"],
            "reservations": held.iter().map(reservation_json).collect::<Vec<_>>(),
        }))
        .into_response(),
        Err(error) => failed(&error),
    }
}

/// The dispensing routes, merged into the pharmacy's.
pub(crate) fn routes() -> Router<FhirServer> {
    Router::new()
        .route("/prescriptions/{id}/prepare", post(prepare))
        .route("/dispenses/{id}", get(reservations))
        .route("/dispenses/{id}/complete", post(complete))
        .route("/dispenses/{id}/cancel", post(cancel))
}
