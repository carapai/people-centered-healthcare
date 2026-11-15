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
//! - the [`RECEIVED`] extension: the goods received note it arrived on.
//!
//! [`device`] writes one, [`equipment`] reads one back, and [`register`] creates one in the
//! caller's transaction, refusing a unit already registered.

use health_access_policy::FACILITY_TAG_SYSTEM;
use health_equipment::{Equipment, EquipmentRefused, Status};
use health_identifiers::{OpId, ResourceId};
use health_persistence::{Precondition, RawJson, Transaction, WriteError, WriteRequest};
use serde_json::{Value, json};

/// The platform category system of a FHIR Device (ADR 0064).
pub const DEVICE_CATEGORY: &str = "urn:health:device-category";
/// The identifier system of a product's code (as the product registry writes it).
pub const PRODUCT_SYSTEM: &str = "urn:health:product";
/// The code system of a unit's operational status.
pub const STATUS_SYSTEM: &str = "urn:health:equipment-status";
/// The extension naming the goods received note a unit arrived on.
pub const RECEIVED: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/equipment-received";

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
    if let Some(note) = &unit.received {
        resource["extension"] = json!([{ "url": RECEIVED, "valueString": note }]);
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
        received: resource["extension"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|e| e["url"] == RECEIVED)
            .and_then(|e| e["valueString"].as_str())
            .map(str::to_owned),
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
