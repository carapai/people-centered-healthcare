//! Inventory over HTTP for staff (#130; inventory-and-procurement.md), behind `subject_layer`,
//! needing [`STOCK_PERMISSION`]:
//!
//! - `POST /goods-received`: a goods received note, every line a receipt at its unit cost;
//! - `GET /on-hand?store=`: the store's Stock Units, balances and last unit costs;
//! - `GET /reorder?store=&days=`: min/max reorder suggestions from recent consumption;
//! - `POST /gs1`: a scanned or printed GS1 DataMatrix, read into its GTIN, expiry, batch and
//!   serial;
//! - `GET /requisitions?store=`: the requisitions a store asked for or was asked, newest first;
//! - `POST /requisitions`, `POST /requisitions/{id}/decision`: a ward's requisition, and the
//!   pharmacy's approval or refusal;
//! - `POST /requisitions/{id}/fulfil`: the batches picked, dispatched as a transfer to the ward.

use std::str::FromStr;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_inventory::receiving::{
    GoodsReceived, ReceivedLine, ReorderPolicy, RequestedLine, Requisition, parse_gs1,
};
use health_inventory::transfers::TransferLine;
use health_inventory::{Ownership, Policy, Status};
use health_inventory_app::{Inventory, InventoryError, Sender, supply_requests};
use health_time::Timestamp;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::FhirServer;

/// The use-case permission a member of staff's roles must map to, to manage stock.
pub const STOCK_PERMISSION: &str = "stock.manage";

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn failed(error: &InventoryError) -> Response {
    match error {
        InventoryError::NotFound(_) => refused(StatusCode::NOT_FOUND, &error.to_string()),
        InventoryError::Busy | InventoryError::Store(_) => {
            refused(StatusCode::SERVICE_UNAVAILABLE, "try again")
        }
        _ => refused(StatusCode::UNPROCESSABLE_ENTITY, &error.to_string()),
    }
}

/// The member of staff, or a refusal.
fn staff(subject: Option<Extension<Subject>>) -> Result<Subject, Box<Response>> {
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(refused(StatusCode::UNAUTHORIZED, "sign in")));
    };
    if !subject.permissions.has(STOCK_PERMISSION) {
        return Err(Box::new(refused(
            StatusCode::FORBIDDEN,
            "managing stock needs the stock.manage permission",
        )));
    }
    Ok(subject)
}

fn inventory(server: &FhirServer) -> Inventory<health_surrealdb::SurrealStore> {
    Inventory::new(server.store.clone(), Policy::default())
}

fn sender(subject: &Subject) -> Sender {
    Sender {
        actor: subject.account.clone(),
        node: subject.facility.clone(),
    }
}

#[derive(Debug, Deserialize)]
struct LineBody {
    product: String,
    batch: String,
    expiry: String,
    #[serde(default = "government")]
    funding_source: String,
    #[serde(default)]
    consignment: bool,
    quantity: i64,
    #[serde(default = "one")]
    pack_size: u32,
    /// A decimal, e.g. `"1.50"`.
    unit_cost: String,
}

fn government() -> String {
    "government".to_owned()
}

fn one() -> u32 {
    1
}

#[derive(Debug, Deserialize)]
struct NoteBody {
    id: Option<String>,
    store: String,
    supplier: String,
    delivery: String,
    lines: Vec<LineBody>,
}

async fn goods_received(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<NoteBody>,
) -> Response {
    let subject = match staff(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let mut lines = Vec::with_capacity(body.lines.len());
    for (i, line) in body.lines.into_iter().enumerate() {
        let Ok(unit_cost) = Decimal::from_str(line.unit_cost.trim()) else {
            return refused(
                StatusCode::UNPROCESSABLE_ENTITY,
                &format!("line {i}: the unit cost is a number"),
            );
        };
        lines.push(ReceivedLine {
            product: line.product.trim().to_owned(),
            batch: line.batch.trim().to_owned(),
            expiry: line.expiry.trim().to_owned(),
            funding_source: line.funding_source,
            ownership: if line.consignment {
                Ownership::Consignment
            } else {
                Ownership::Owned
            },
            quantity: line.quantity,
            pack_size: line.pack_size,
            unit_cost,
        });
    }
    let note = GoodsReceived {
        id: body
            .id
            .unwrap_or_else(|| format!("grn-{}", uuid::Uuid::now_v7())),
        store: body.store,
        supplier: body.supplier.trim().to_owned(),
        delivery: body.delivery.trim().to_owned(),
        lines,
    };
    match inventory(&server)
        .receive_goods(&note, &sender(&subject), Timestamp::now())
        .await
    {
        Ok(()) => (StatusCode::CREATED, Json(json!({ "id": note.id }))).into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Debug, Deserialize)]
struct StoreQuery {
    store: String,
    days: Option<u32>,
}

async fn on_hand(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Query(query): Query<StoreQuery>,
) -> Response {
    if let Err(response) = staff(subject) {
        return *response;
    }
    match inventory(&server).stock_on_hand(&query.store).await {
        Ok(list) => Json(Value::Array(
            list.iter()
                .map(|item| {
                    json!({
                        "product": item.unit.product,
                        "batch": item.unit.batch,
                        "expiry": item.unit.expiry,
                        "funding_source": item.unit.funding_source,
                        "status": match item.unit.status {
                            Status::Usable => "usable",
                            Status::Quarantine => "quarantine",
                            Status::Restricted => "restricted",
                        },
                        "on_hand": item.balance.on_hand,
                        "available": item.balance.available(),
                        "flagged": item.balance.flagged,
                        "unit_cost": item.unit_cost.map(|c| c.to_string()),
                    })
                })
                .collect(),
        ))
        .into_response(),
        Err(error) => failed(&error),
    }
}

async fn reorder(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Query(query): Query<StoreQuery>,
) -> Response {
    if let Err(response) = staff(subject) {
        return *response;
    }
    let days = query.days.unwrap_or(30).clamp(1, 365);
    match inventory(&server)
        .reorder(
            &query.store,
            ReorderPolicy::default(),
            days,
            Timestamp::now(),
        )
        .await
    {
        Ok(list) => Json(Value::Array(
            list.iter()
                .map(|s| {
                    json!({
                        "product": s.product,
                        "on_hand": s.on_hand,
                        "monthly": s.monthly.to_string(),
                        "min": s.min,
                        "max": s.max,
                        "order": s.order,
                    })
                })
                .collect(),
        ))
        .into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Debug, Deserialize)]
struct Gs1Body {
    data: String,
}

async fn gs1(subject: Option<Extension<Subject>>, Json(body): Json<Gs1Body>) -> Response {
    if let Err(response) = staff(subject) {
        return *response;
    }
    match parse_gs1(&body.data) {
        Ok(read) => Json(json!({
            "gtin": read.gtin,
            "expiry": read.expiry,
            "batch": read.batch,
            "serial": read.serial,
        }))
        .into_response(),
        Err(error) => refused(StatusCode::UNPROCESSABLE_ENTITY, &error.to_string()),
    }
}

#[derive(Debug, Deserialize)]
struct RequisitionBody {
    requesting_store: String,
    supplying_store: String,
    lines: Vec<RequestedLine>,
}

async fn requisition(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<RequisitionBody>,
) -> Response {
    let subject = match staff(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let id = format!("req-{}", uuid::Uuid::now_v7());
    let requisition = match Requisition::new(
        &id,
        &body.requesting_store,
        &body.supplying_store,
        body.lines,
        &subject.account,
    ) {
        Ok(requisition) => requisition,
        Err(error) => return refused(StatusCode::UNPROCESSABLE_ENTITY, &error.to_string()),
    };
    match inventory(&server).request(&requisition).await {
        Ok(()) => (
            StatusCode::CREATED,
            Json(json!({ "id": id, "supplyRequests": supply_requests(&requisition) })),
        )
            .into_response(),
        Err(error) => failed(&error),
    }
}

fn requisition_json(r: &Requisition) -> Value {
    json!({
        "id": r.id,
        "requesting_store": r.requesting_store,
        "supplying_store": r.supplying_store,
        "lines": r.lines.iter().map(|l| json!({ "product": l.product, "quantity": l.quantity })).collect::<Vec<_>>(),
        "status": match r.status {
            health_inventory::receiving::RequisitionStatus::Requested => "requested",
            health_inventory::receiving::RequisitionStatus::Approved => "approved",
            health_inventory::receiving::RequisitionStatus::Fulfilled => "fulfilled",
            health_inventory::receiving::RequisitionStatus::Rejected => "rejected",
        },
        "requested_by": r.requested_by,
        "decided_by": r.decided_by,
        "transfer": r.transfer,
    })
}

async fn requisitions(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Query(query): Query<StoreQuery>,
) -> Response {
    if let Err(response) = staff(subject) {
        return *response;
    }
    match inventory(&server).requisitions(&query.store).await {
        Ok(list) => Json(Value::Array(list.iter().map(requisition_json).collect())).into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Debug, Deserialize)]
struct PickedLine {
    product: String,
    batch: String,
    expiry: String,
    #[serde(default = "government")]
    funding_source: String,
    #[serde(default)]
    consignment: bool,
    quantity: i64,
}

#[derive(Debug, Deserialize)]
struct FulfilBody {
    lines: Vec<PickedLine>,
}

async fn fulfil(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<FulfilBody>,
) -> Response {
    let subject = match staff(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let lines = body
        .lines
        .into_iter()
        .map(|l| TransferLine {
            product: l.product,
            batch: l.batch,
            expiry: l.expiry,
            funding_source: l.funding_source,
            ownership: if l.consignment {
                Ownership::Consignment
            } else {
                Ownership::Owned
            },
            quantity: l.quantity,
        })
        .collect();
    let transfer = format!("transfer-{id}");
    match inventory(&server)
        .fulfil_requisition(&id, &transfer, lines, &sender(&subject), Timestamp::now())
        .await
    {
        Ok((requisition, _)) => Json(requisition_json(&requisition)).into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Debug, Deserialize)]
struct DecisionBody {
    approve: bool,
}

async fn decide(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<DecisionBody>,
) -> Response {
    let subject = match staff(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    match inventory(&server)
        .decide_requisition(&id, body.approve, &subject.account)
        .await
    {
        Ok(requisition) => {
            let mut body = requisition_json(&requisition);
            body["supplyRequests"] = json!(supply_requests(&requisition));
            Json(body).into_response()
        }
        Err(error) => failed(&error),
    }
}

impl FhirServer {
    /// The inventory routes for staff (see the module docs); mount them behind `subject_layer`.
    pub fn inventory_router(&self) -> Router {
        Router::new()
            .route("/goods-received", post(goods_received))
            .route("/on-hand", get(on_hand))
            .route("/reorder", get(reorder))
            .route("/gs1", post(gs1))
            .route("/requisitions", get(requisitions).post(requisition))
            .route("/requisitions/{id}/decision", post(decide))
            .route("/requisitions/{id}/fulfil", post(fulfil))
            .with_state(self.clone())
    }
}
