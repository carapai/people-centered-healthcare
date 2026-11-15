//! Inventory over HTTP for staff (#130; inventory-and-procurement.md), behind `subject_layer`,
//! needing [`STOCK_PERMISSION`]:
//!
//! - `POST /goods-received`: a goods received note, every Stock line a receipt at its unit cost,
//!   and every line of a Product tracked as Equipment one Device per serial (ADR 0064);
//! - `GET /on-hand?store=`: the store's Stock Units, balances and last unit costs;
//! - `GET /reorder?store=&days=`: min/max reorder suggestions from recent consumption;
//! - `POST /gs1`: a scanned or printed GS1 DataMatrix, read into its GTIN, expiry, batch and
//!   serial;
//! - `GET /requisitions?store=`: the requisitions a store asked for or was asked, newest first;
//! - `POST /requisitions`, `POST /requisitions/{id}/decision`: a ward's requisition, and the
//!   pharmacy's approval or refusal;
//! - `POST /requisitions/{id}/fulfil`: the batches picked, dispatched as a transfer to the ward;
//! - `GET /products`, `POST /products` `{id?, code, name, baseUnit, packSizes, synonyms?,
//!   medicines?, gtin?, active?, equipment?}`: the product master (#128 follow-up), each product a
//!   Medication, its code unique and unchanging, and stocked or tracked as Equipment for good;
//! - `GET /equipment?location=`: the facility's Equipment (ADR 0064), each a Device;
//! - `POST /equipment/{id}/status` `{status, note}` and `POST /equipment/{id}/move` `{location}`:
//!   a change to one of its units, kept as a new version (its maintenance history is
//!   `Device/{id}/_history`). Only the facility that holds a unit changes it;
//! - `POST /equipment/{id}/dispatch` `{to}`: a unit sent to facility `to`. Central hands it over
//!   at once, so `to` holds it, in transit, and the sender no longer does;
//! - `POST /equipment/{id}/receive` `{location}`: a unit in transit received by the facility it
//!   was handed over to.

use std::collections::BTreeMap;
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
use health_inventory_app::{
    EquipmentIntake, IntakeError, Inventory, InventoryError, ReceivedUnit, Sender, supply_requests,
};
use health_persistence::Transaction;
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

/// The equipment module behind receiving (ADR 0064): the Products tracked as Equipment, by
/// code with their names, and each received unit registered as a Device.
struct Equipment {
    tracked: BTreeMap<String, String>,
}

impl Equipment {
    /// The Products tracked as Equipment in the product master.
    async fn of(server: &FhirServer) -> Self {
        let tracked = health_product_registry_app::products(
            &crate::programs::found(server, "Medication", &[]).await,
        )
        .into_iter()
        .filter(|p| p.equipment)
        .map(|p| (p.code, p.name))
        .collect();
        Self { tracked }
    }
}

impl<T: Transaction> EquipmentIntake<T> for Equipment {
    fn tracked(&self, product: &str) -> bool {
        self.tracked.contains_key(product)
    }

    async fn register(&self, tx: &mut T, unit: ReceivedUnit) -> Result<(), IntakeError> {
        let name = self
            .tracked
            .get(&unit.product)
            .map_or(unit.product.as_str(), String::as_str);
        let equipment = health_equipment::Equipment::received(
            &unit.product,
            name,
            &unit.serial,
            &unit.facility,
            &unit.location,
            &unit.note,
        )
        .map_err(|e| IntakeError::Refused(format!("serial {}: {e}", unit.serial)))?
        .with_acquisition(unit.unit_cost, unit.received_at);
        health_equipment_app::register(tx, &equipment)
            .await
            .map_err(|error| match error {
                health_equipment_app::RegisterError::Write(write) => IntakeError::Write(write),
                refused => IntakeError::Refused(refused.to_string()),
            })
    }
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
    /// Empty for Equipment, which is received by serial instead.
    #[serde(default)]
    batch: String,
    #[serde(default)]
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
    /// One serial number or asset tag per unit, for a Product tracked as Equipment (ADR 0064).
    #[serde(default)]
    serials: Vec<String>,
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
            serials: line.serials.iter().map(|s| s.trim().to_owned()).collect(),
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
    let equipment = Equipment::of(&server).await;
    match inventory(&server)
        .receive_goods_with(&note, &sender(&subject), Timestamp::now(), &equipment)
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
            .route("/products", get(list_products).post(save_product))
            .route("/equipment", get(list_equipment))
            .route("/equipment/{id}/status", post(equipment_status))
            .route("/equipment/{id}/move", post(equipment_move))
            .route("/equipment/{id}/dispatch", post(equipment_dispatch))
            .route("/equipment/{id}/receive", post(equipment_receive))
            .with_state(self.clone())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProductBody {
    id: Option<String>,
    code: String,
    name: String,
    base_unit: String,
    #[serde(default)]
    pack_sizes: Vec<u32>,
    #[serde(default)]
    synonyms: Vec<String>,
    #[serde(default)]
    medicines: Vec<health_product_registry::Coding>,
    #[serde(default)]
    gtin: Option<String>,
    #[serde(default = "active")]
    active: bool,
    /// Tracked as Equipment, one record per serial, instead of stocked (ADR 0064).
    #[serde(default)]
    equipment: bool,
    /// For Equipment: the years it depreciates over (#453).
    #[serde(default)]
    useful_life_years: Option<u32>,
}

fn active() -> bool {
    true
}

fn product_json(id: &str, product: &health_product_registry::Product) -> Value {
    json!({
        "id": id, "code": product.code, "name": product.name, "baseUnit": product.base_unit,
        "packSizes": product.pack_sizes, "synonyms": product.synonyms, "medicines": product.medicines,
        "gtin": product.gtin, "active": product.active, "equipment": product.equipment,
        "usefulLifeYears": product.useful_life_years,
    })
}

async fn list_products(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
) -> Response {
    if let Err(response) = staff(subject) {
        return *response;
    }
    let mut list: Vec<Value> = crate::programs::found(&server, "Medication", &[])
        .await
        .iter()
        .filter_map(|m| {
            let product = health_product_registry_app::product(m)?;
            Some(product_json(m["id"].as_str()?, &product))
        })
        .collect();
    list.sort_by(|a, b| a["code"].as_str().cmp(&b["code"].as_str()));
    Json(list).into_response()
}

async fn save_product(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<ProductBody>,
) -> Response {
    if let Err(response) = staff(subject) {
        return *response;
    }
    let id = match body
        .id
        .as_deref()
        .map(str::parse::<health_identifiers::ResourceId>)
    {
        Some(Ok(id)) => id,
        Some(Err(_)) => return refused(StatusCode::BAD_REQUEST, "the id is a UUIDv7 (ADR 0013)"),
        None => health_identifiers::ResourceId::new(),
    };
    let product = health_product_registry::Product {
        code: body.code.trim().to_owned(),
        name: body.name.trim().to_owned(),
        gtin: body
            .gtin
            .map(|g| g.trim().to_owned())
            .filter(|g| !g.is_empty()),
        base_unit: body.base_unit.trim().to_owned(),
        pack_sizes: body.pack_sizes,
        medicines: body.medicines,
        synonyms: body
            .synonyms
            .iter()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect(),
        active: body.active,
        equipment: body.equipment,
        useful_life_years: body.useful_life_years,
    };
    match health_product_registry_app::Products::new(server.store.clone())
        .save(id, &product)
        .await
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(product_json(&id.to_string(), &product)),
        )
            .into_response(),
        Err(health_product_registry_app::ProductError::Store(_)) => {
            refused(StatusCode::SERVICE_UNAVAILABLE, "try again")
        }
        Err(error @ health_product_registry_app::ProductError::Conflict) => {
            refused(StatusCode::CONFLICT, &error.to_string())
        }
        Err(error) => refused(StatusCode::UNPROCESSABLE_ENTITY, &error.to_string()),
    }
}

/// `unit` as the API gives it, with its book value when it is known.
fn equipment_json(unit: &health_equipment::Equipment, book_value: Option<Decimal>) -> Value {
    json!({
        "id": unit.id.to_string(), "product": unit.product, "name": unit.name,
        "serial": unit.serial, "location": unit.location, "status": unit.status.code(),
        "received": unit.received,
        "change": unit.change.as_ref().map(|c| json!({
            "note": c.note, "by": c.by, "at": c.at.to_string(),
        })),
        "transfer": unit.transfer.as_ref().map(|t| json!({
            "from": t.from, "to": t.to, "by": t.by, "at": t.at.to_string(),
        })),
        "cost": unit.acquired.map(|a| a.cost.normalize().to_string()),
        "bookValue": book_value.map(|v| v.normalize().to_string()),
    })
}

#[derive(Debug, Deserialize)]
struct EquipmentQuery {
    location: Option<String>,
}

async fn list_equipment(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Query(query): Query<EquipmentQuery>,
) -> Response {
    let subject = match staff(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let tag = format!(
        "{}|{}",
        health_access_policy::FACILITY_TAG_SYSTEM,
        subject.facility
    );
    let mut list: Vec<health_equipment::Equipment> =
        crate::programs::found(&server, "Device", &[("_tag", tag.as_str())])
            .await
            .iter()
            .filter_map(health_equipment_app::equipment)
            .filter(|unit| unit.facility == subject.facility)
            .filter(|unit| query.location.as_ref().is_none_or(|l| *l == unit.location))
            .collect();
    list.sort_by(|a, b| (&a.name, &a.serial).cmp(&(&b.name, &b.serial)));
    // Book value is a projection from each Product's useful life (#453).
    let lives: BTreeMap<String, u32> = health_product_registry_app::products(
        &crate::programs::found(&server, "Medication", &[]).await,
    )
    .into_iter()
    .filter_map(|p| Some((p.code, p.useful_life_years?)))
    .collect();
    let now = Timestamp::now();
    Json(
        list.iter()
            .map(|unit| {
                let value = unit.book_value(lives.get(&unit.product).copied(), now);
                equipment_json(unit, value)
            })
            .collect::<Vec<_>>(),
    )
    .into_response()
}

#[derive(Debug, Deserialize)]
struct StatusBody {
    status: String,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Deserialize)]
struct MoveBody {
    location: String,
}

/// Applies `apply` to the unit `id` held by the member of staff's facility.
async fn change_equipment(
    server: &FhirServer,
    subject: Option<Extension<Subject>>,
    id: &str,
    apply: impl FnOnce(
        &mut health_equipment::Equipment,
        &str,
        Timestamp,
    ) -> Result<(), health_equipment::EquipmentRefused>,
) -> Response {
    let subject = match staff(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let Ok(id) = id.parse::<health_identifiers::ResourceId>() else {
        return refused(StatusCode::NOT_FOUND, "no such equipment");
    };
    let by = subject.account.clone();
    answer(
        health_equipment_app::Equipments::new(server.store.clone())
            .change(id, &subject.facility, |unit| {
                apply(unit, &by, Timestamp::now())
            })
            .await,
    )
}

/// The unit as changed, or why it was not.
fn answer(
    changed: Result<health_equipment::Equipment, health_equipment_app::EquipmentError>,
) -> Response {
    use health_equipment_app::EquipmentError as E;
    match changed {
        Ok(unit) => Json(equipment_json(&unit, None)).into_response(),
        Err(error) => {
            let status = match &error {
                E::NotFound(_) => StatusCode::NOT_FOUND,
                E::OtherFacility(_) => StatusCode::FORBIDDEN,
                E::Conflict => StatusCode::CONFLICT,
                E::Store(_) => return refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
                E::Refused(_) => StatusCode::UNPROCESSABLE_ENTITY,
            };
            refused(status, &error.to_string())
        }
    }
}

async fn equipment_status(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<StatusBody>,
) -> Response {
    let Some(status) = health_equipment::Status::from_code(body.status.trim()) else {
        return refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "status is available, faulty, under-maintenance or retired",
        );
    };
    change_equipment(&server, subject, &id, |unit, by, at| {
        unit.set_status(status, &body.note, by, at)
    })
    .await
}

async fn equipment_move(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<MoveBody>,
) -> Response {
    change_equipment(&server, subject, &id, |unit, by, at| {
        unit.move_to(&body.location, by, at)
    })
    .await
}

#[derive(Debug, Deserialize)]
struct DispatchBody {
    to: String,
}

/// Dispatches a unit to another facility. The inventory routes are served by central, the
/// authority that hands a dispatched unit over (ADR 0064), so it is handed over at once.
async fn equipment_dispatch(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<DispatchBody>,
) -> Response {
    let subject = match staff(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let Ok(id) = id.parse::<health_identifiers::ResourceId>() else {
        return refused(StatusCode::NOT_FOUND, "no such equipment");
    };
    answer(
        health_equipment_app::Equipments::new(server.store.clone())
            .dispatch(
                id,
                &subject.facility,
                &body.to,
                &subject.account,
                Timestamp::now(),
            )
            .await,
    )
}

async fn equipment_receive(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<MoveBody>,
) -> Response {
    change_equipment(&server, subject, &id, |unit, by, at| {
        unit.receive(&body.location, by, at)
    })
    .await
}
