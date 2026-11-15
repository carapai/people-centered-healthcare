//! Equipment as FHIR R5 Device resources (ADR 0064; resource-ownership.md: Device of category
//! *equipment* is the equipment module's). The facility's edge is the only writer.
//!
//! An [`Equipment`] unit is a Device:
//! - `id`: the unit's identity, from its Product and serial ([`health_equipment::identity`]);
//! - `meta.tag`: its facility ([`FACILITY_TAG_SYSTEM`]), so it syncs as facility-scope;
//! - `category`: `equipment` in [`DEVICE_CATEGORY`];
//! - `identifier`: its Product's code ([`PRODUCT_SYSTEM`]);
//! - `serialNumber`, `displayName` (the Product's name) and `location`;
//! - `availabilityStatus`: its [`Status`] in [`STATUS_SYSTEM`], and `status` `inactive` once
//!   retired;
//! - the [`RECEIVED`] extension: the goods received note it arrived on;
//! - `note`: its latest change, by whom and when. Its versions (`_history`) are its maintenance
//!   history;
//! - the [`TRANSFER`] extension while it is in transit to another facility: `from`, `to`, `by`
//!   and `at`;
//! - the [`ACQUISITION`] extension: what it cost (`cost`) and when it was received (`at`).
//!
//! [`device`] writes one, [`equipment`] reads one back, and [`register`] creates one in the
//! caller's transaction, refusing a unit already registered. [`Equipments::change`] applies a
//! facility's change to one of its units.
//!
//! **Transfers between facilities.** Central hands a dispatched unit over to the facility it goes
//! to (ADR 0064): [`Equipments::dispatch`] when the dispatch is made at central, in the same
//! transaction, and [`handed_over`] when an edge's dispatch reaches central by sync. As the edge
//! pushes its outbox in order, everything it wrote of the unit before the dispatch is at central
//! by then.
//!
//! **Inventory accounting** (#453): [`equipment_postings`] gives the postings of a change to a
//! unit, at its cost: its receipt (equipment against supplier payables), its retirement (written
//! off), and its hand-over to another facility (its cost moved between the two facilities). Book
//! value is a projection ([`Equipment::book_value`]) and is not posted.

use std::str::FromStr;

use health_access_policy::FACILITY_TAG_SYSTEM;
use health_equipment::{Acquisition, Change, Equipment, EquipmentRefused, Status, Transfer};
use health_financial_ledger::{
    Currency, Dimensions, EQUIPMENT, EQUIPMENT_WRITE_OFF, Line, Money, PAYABLE, Posting, Side,
};
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store, StoreError, Transaction, WriteError, WriteRequest,
};
use health_time::Timestamp;
use rust_decimal::Decimal;
use serde_json::{Value, json};

/// The platform category system of a FHIR Device (ADR 0064).
pub const DEVICE_CATEGORY: &str = "urn:health:device-category";
/// The identifier system of a product's code (as the product registry writes it).
pub const PRODUCT_SYSTEM: &str = "urn:health:product";
/// The code system of a unit's operational status.
pub const STATUS_SYSTEM: &str = "urn:health:equipment-status";
/// The extension naming the goods received note a unit arrived on.
pub const RECEIVED: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/equipment-received";
/// The extension of a unit's acquisition: its cost and when it was received.
pub const ACQUISITION: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/equipment-acquisition";
/// The extension of a unit's transfer to another facility, while it is in transit.
pub const TRANSFER: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/equipment-transfer";

/// `unit` as a Device.
#[must_use]
pub fn device(unit: &Equipment) -> Value {
    let mut resource = json!({
        "resourceType": "Device",
        "id": unit.id.to_string(),
        "meta": { "tag": [{ "system": FACILITY_TAG_SYSTEM, "code": unit.facility }] },
        "identifier": [{ "system": PRODUCT_SYSTEM, "value": unit.product }],
        "displayName": unit.name,
        "status": if unit.status == Status::Retired { "inactive" } else { "active" },
        "availabilityStatus": {
            "coding": [{ "system": STATUS_SYSTEM, "code": unit.status.code() }]
        },
        "serialNumber": unit.serial,
        "category": [{ "coding": [{ "system": DEVICE_CATEGORY, "code": "equipment" }] }],
        "location": { "reference": unit.location },
    });
    let mut extensions = Vec::new();
    if let Some(note) = &unit.received {
        extensions.push(json!({ "url": RECEIVED, "valueString": note }));
    }
    if let Some(transfer) = &unit.transfer {
        extensions.push(json!({
            "url": TRANSFER,
            "extension": [
                { "url": "from", "valueString": transfer.from },
                { "url": "to", "valueString": transfer.to },
                { "url": "by", "valueString": transfer.by },
                { "url": "at", "valueDateTime": transfer.at.to_string() },
            ],
        }));
    }
    if let Some(acquired) = &unit.acquired
        && let Ok(cost) = serde_json::Number::from_str(&acquired.cost.normalize().to_string())
    {
        extensions.push(json!({
            "url": ACQUISITION,
            "extension": [
                { "url": "cost", "valueDecimal": cost },
                { "url": "at", "valueDateTime": acquired.at.to_string() },
            ],
        }));
    }
    if !extensions.is_empty() {
        resource["extension"] = Value::Array(extensions);
    }
    if let Some(change) = &unit.change {
        resource["note"] = json!([{
            "authorString": change.by,
            "time": change.at.to_string(),
            "text": change.note,
        }]);
    }
    resource
}

/// The Equipment a Device is, if it is one: category *equipment*, with its Product, serial,
/// facility and location.
#[must_use]
pub fn equipment(resource: &Value) -> Option<Equipment> {
    if resource["resourceType"] != "Device" || resource["status"] == "entered-in-error" {
        return None;
    }
    let is_equipment = resource["category"].as_array()?.iter().any(|c| {
        c["coding"].as_array().is_some_and(|codings| {
            codings
                .iter()
                .any(|k| k["system"] == DEVICE_CATEGORY && k["code"] == "equipment")
        })
    });
    if !is_equipment {
        return None;
    }
    let product = resource["identifier"]
        .as_array()?
        .iter()
        .find(|i| i["system"] == PRODUCT_SYSTEM)?["value"]
        .as_str()?;
    let facility = resource["meta"]["tag"]
        .as_array()?
        .iter()
        .find(|t| t["system"] == FACILITY_TAG_SYSTEM)?["code"]
        .as_str()?;
    let status = resource["availabilityStatus"]["coding"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["system"] == STATUS_SYSTEM)
        .find_map(|c| Status::from_code(c["code"].as_str()?))
        .unwrap_or(Status::Available);
    Some(Equipment {
        id: resource["id"].as_str()?.parse().ok()?,
        product: product.to_owned(),
        name: resource["displayName"]
            .as_str()
            .unwrap_or(product)
            .to_owned(),
        serial: resource["serialNumber"].as_str()?.to_owned(),
        facility: facility.to_owned(),
        location: resource["location"]["reference"].as_str()?.to_owned(),
        status,
        received: extension(resource, RECEIVED)
            .and_then(|e| e["valueString"].as_str())
            .map(str::to_owned),
        acquired: extension(resource, ACQUISITION).and_then(|e| {
            let part = |name: &str| e["extension"].as_array()?.iter().find(|p| p["url"] == name);
            Some(Acquisition {
                cost: Decimal::from_str(&part("cost")?["valueDecimal"].as_number()?.to_string())
                    .ok()?,
                at: part("at")?["valueDateTime"].as_str()?.parse().ok()?,
            })
        }),
        transfer: extension(resource, TRANSFER).and_then(|e| {
            let part = |name: &str| {
                e["extension"]
                    .as_array()?
                    .iter()
                    .find(|p| p["url"] == name)
                    .and_then(|p| p["valueString"].as_str().or(p["valueDateTime"].as_str()))
            };
            Some(Transfer {
                from: part("from")?.to_owned(),
                to: part("to")?.to_owned(),
                by: part("by")?.to_owned(),
                at: part("at")?.parse().ok()?,
            })
        }),
        change: resource["note"]
            .as_array()
            .and_then(|notes| notes.last())
            .and_then(|n| {
                Some(Change {
                    note: n["text"].as_str()?.to_owned(),
                    by: n["authorString"].as_str()?.to_owned(),
                    at: n["time"].as_str()?.parse().ok()?,
                })
            }),
    })
}

/// The extension `url` of `resource`, if it has it.
fn extension<'a>(resource: &'a Value, url: &str) -> Option<&'a Value> {
    resource["extension"]
        .as_array()?
        .iter()
        .find(|e| e["url"] == url)
}

/// The Device as central hands it over, if it is a unit dispatched to another facility: the
/// same unit, now the receiving facility's (ADR 0064). `None` for anything else.
#[must_use]
pub fn handed_over(resource: &Value) -> Option<Value> {
    let mut unit = equipment(resource)?;
    unit.hand_over().then(|| device(&unit))
}

/// The inventory-accounting posting of a unit changing from `before` (`None` when it was just
/// registered) to `after`, at its cost in `currency`, if the change moves value (ADR 0064):
/// - **received:** equipment, against supplier payables, at the receiving facility;
/// - **retired:** its cost written off;
/// - **handed over** to another facility: its cost out of the sender and into the receiver.
///
/// `None` for any other change, for a unit whose cost is not known, and at zero cost.
#[must_use]
pub fn equipment_postings(
    before: Option<&Equipment>,
    after: &Equipment,
    currency: Currency,
    at: Timestamp,
) -> Option<Posting> {
    let amount = Money::rounded(after.acquired?.cost, currency);
    if amount.is_zero() {
        return None;
    }
    let line = |account: &str, side: Side, facility: &str| Line {
        account: account.to_owned(),
        side,
        amount,
        dimensions: Dimensions {
            facility: facility.to_owned(),
            department: None,
            service: None,
            program: None,
            funding_source: None,
            fund: None,
            encounter: None,
            price_versions: std::collections::BTreeSet::new(),
        },
        original: None,
    };
    let unit = format!("{} {}", after.product, after.serial);
    let (event, description, lines) = match before {
        None => (
            "received",
            format!("Equipment received: {unit}"),
            vec![
                line(EQUIPMENT, Side::Debit, &after.facility),
                line(PAYABLE, Side::Credit, &after.facility),
            ],
        ),
        Some(before) if before.status != Status::Retired && after.status == Status::Retired => (
            "retired",
            format!("Equipment retired: {unit}"),
            vec![
                line(EQUIPMENT_WRITE_OFF, Side::Debit, &after.facility),
                line(EQUIPMENT, Side::Credit, &after.facility),
            ],
        ),
        Some(before) if before.facility != after.facility => (
            "handed-over",
            format!(
                "Equipment handed over from {} to {}: {unit}",
                before.facility, after.facility
            ),
            vec![
                line(EQUIPMENT, Side::Debit, &after.facility),
                line(EQUIPMENT, Side::Credit, &before.facility),
            ],
        ),
        Some(_) => return None,
    };
    Some(Posting {
        id: format!("equipment-{}-{event}-{}", after.id, at.as_second()),
        at,
        description,
        lines,
        reverses: None,
    })
}

/// Why a unit could not be registered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegisterError {
    /// It is not a valid unit.
    #[error(transparent)]
    Refused(#[from] EquipmentRefused),
    /// A unit of that Product with that serial is already registered.
    #[error("{product} with serial number {serial} is already registered")]
    Registered {
        /// The Product.
        product: String,
        /// The serial number.
        serial: String,
    },
    /// The write failed.
    #[error(transparent)]
    Write(#[from] WriteError),
}

/// Creates `unit` as a Device in `tx`. Its id follows from its Product and serial, so a unit
/// registered before is refused.
pub async fn register<T: Transaction>(tx: &mut T, unit: &Equipment) -> Result<(), RegisterError> {
    unit.validate()?;
    let id = ResourceId::from_uuid(unit.id).map_err(|_| {
        RegisterError::Refused(EquipmentRefused::Missing("id from its product and serial"))
    })?;
    let resource_type = "Device"
        .parse()
        .map_err(|_| WriteError::Invalid("the Device type".into()))?;
    tx.write(WriteRequest {
        resource_type,
        id,
        json: RawJson::from_validated(device(unit).to_string()),
        op_id: OpId::new(),
        precondition: Precondition::MustNotExist,
    })
    .await
    .map(|_| ())
    .map_err(|error| match error {
        WriteError::PreconditionFailed { .. } => RegisterError::Registered {
            product: unit.product.clone(),
            serial: unit.serial.clone(),
        },
        other => RegisterError::Write(other),
    })
}

/// Why a change to a unit was not made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EquipmentError {
    /// No unit of Equipment has that id.
    #[error("no equipment {0}")]
    NotFound(String),
    /// Another facility holds it: only its facility changes it (ADR 0064).
    #[error("{0} is held by another facility")]
    OtherFacility(String),
    /// The change breaks a rule.
    #[error(transparent)]
    Refused(#[from] EquipmentRefused),
    /// Someone changed it at the same time; read it again.
    #[error("the equipment changed meanwhile; try again")]
    Conflict,
    /// The store failed.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for EquipmentError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for EquipmentError {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::Conflict | WriteError::PreconditionFailed { .. } => Self::Conflict,
            other => Self::Store(other.to_string()),
        }
    }
}

/// The equipment use cases over `store`.
pub struct Equipments<S> {
    store: S,
}

impl<S: Store> Equipments<S> {
    /// The use cases over `store`.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// The unit `id`, if there is one.
    pub async fn get(&self, id: ResourceId) -> Result<Option<Equipment>, EquipmentError> {
        let mut tx = self.store.begin().await?;
        Ok(read(&mut tx, id).await?.map(|(unit, _)| unit))
    }

    /// Applies `apply` to the unit `id` held by `facility`, as a new version `If-Match` the one
    /// it was read at, and returns the unit as changed.
    pub async fn change(
        &self,
        id: ResourceId,
        facility: &str,
        apply: impl FnOnce(&mut Equipment) -> Result<(), EquipmentRefused>,
    ) -> Result<Equipment, EquipmentError> {
        let mut tx = self.store.begin().await?;
        let (mut unit, version) = read(&mut tx, id)
            .await?
            .ok_or_else(|| EquipmentError::NotFound(id.to_string()))?;
        if unit.facility != facility {
            return Err(EquipmentError::OtherFacility(unit.serial));
        }
        apply(&mut unit)?;
        unit.validate()?;
        write(&mut tx, id, &unit, version).await?;
        tx.commit().await?;
        Ok(unit)
    }

    /// Dispatches the unit `id` held by `facility` to facility `to`, and as central hands it over
    /// to `to` in the same transaction (ADR 0064): two versions, the dispatch and the hand-over.
    /// Returns the unit as `to` now holds it, in transit until it receives it.
    pub async fn dispatch(
        &self,
        id: ResourceId,
        facility: &str,
        to: &str,
        by: &str,
        at: Timestamp,
    ) -> Result<Equipment, EquipmentError> {
        let mut tx = self.store.begin().await?;
        let (mut unit, version) = read(&mut tx, id)
            .await?
            .ok_or_else(|| EquipmentError::NotFound(id.to_string()))?;
        if unit.facility != facility {
            return Err(EquipmentError::OtherFacility(unit.serial));
        }
        unit.dispatch(to, by, at)?;
        let dispatched = write(&mut tx, id, &unit, version).await?;
        unit.hand_over();
        write(&mut tx, id, &unit, dispatched).await?;
        tx.commit().await?;
        Ok(unit)
    }
}

/// Writes `unit` as the next version of `id`, `If-Match` `version`, and returns the new one.
async fn write<T: Transaction>(
    tx: &mut T,
    id: ResourceId,
    unit: &Equipment,
    version: OpId,
) -> Result<OpId, EquipmentError> {
    let written = tx
        .write(WriteRequest {
            resource_type: device_type()?,
            id,
            json: RawJson::from_validated(device(unit).to_string()),
            op_id: OpId::new(),
            precondition: Precondition::IfMatch(version),
        })
        .await?;
    Ok(written.version_id)
}

fn device_type() -> Result<ResourceType, EquipmentError> {
    "Device"
        .parse()
        .map_err(|_| EquipmentError::Store("the Device type".into()))
}

/// The unit `id` and the version it was read at, if it is Equipment.
async fn read<T: Transaction>(
    tx: &mut T,
    id: ResourceId,
) -> Result<Option<(Equipment, OpId)>, EquipmentError> {
    let Some(head) = tx.read(&device_type()?, id).await? else {
        return Ok(None);
    };
    if head.version.deleted {
        return Ok(None);
    }
    let resource: Value = serde_json::from_str(head.json.as_str())
        .map_err(|e| EquipmentError::Store(e.to_string()))?;
    Ok(equipment(&resource).map(|unit| (unit, head.version.version_id)))
}
