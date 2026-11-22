//! The facility registry's places as FHIR (ADR 0094): each place is an Organization, linked to
//! the one above it by `partOf`, and a facility with a position also has a Location holding it.
//!
//! - An Organization's id is derived from its place code ([`place_id`]), so a place is read by
//!   its code without a search. It carries the code as an identifier ([`PLACE_SYSTEM`]; a
//!   facility's is its NHFR identifier, also under [`NHFR_SYSTEM`]), its level as a type coding
//!   ([`PLACE_LEVEL_SYSTEM`]), and the NHFR's level of care and ownership as further codings.
//! - [`places`] reads the hierarchy; [`apply_import`] writes an NHFR import's plan in one
//!   transaction, each place's write audited, a place written over only at the version read.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use health_facility_registry::{
    AREAS_CONTENT, AREAS_TYPE, AdministrativeAreas, ImportPlan, Place, PlaceLevel, PlaceStore,
    Places, location_id, place_id,
};
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store, StoreError, Transaction, WriteRequest,
};
use serde_json::{Value, json};

use crate::{AuditTrail, RegistryError};

/// The identifier system of a place's code.
pub const PLACE_SYSTEM: &str = "urn:health:place";
/// The identifier system of a facility's NHFR identifier.
pub const NHFR_SYSTEM: &str = "urn:health:nhfr";
/// The code system of a place's level (`facility`, `district`, `region`, `national`).
pub const PLACE_LEVEL_SYSTEM: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/CodeSystem/place-level";
/// The code system of the NHFR's level of care (`HC III`, `Hospital`, …).
pub const CARE_LEVEL_SYSTEM: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/CodeSystem/nhfr-care-level";
/// The code system of the NHFR's ownership (`Government`, `PNFP`, …).
pub const OWNERSHIP_SYSTEM: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/CodeSystem/nhfr-ownership";

/// Whether `resource` is a package's administrative areas: a `Library` of type [`AREAS_TYPE`].
pub fn is_administrative_areas(resource: &Value) -> bool {
    resource["resourceType"] == "Library"
        && resource["type"]["coding"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|c| c["code"] == AREAS_TYPE)
}

/// The regions and districts of an administrative-areas `Library`: its first attachment,
/// [`AREAS_CONTENT`] inline as base64 JSON.
pub fn administrative_areas(resource: &Value) -> Result<AdministrativeAreas, String> {
    let content = &resource["content"][0];
    if content["contentType"] != AREAS_CONTENT {
        return Err(format!("the areas' content is not {AREAS_CONTENT}"));
    }
    let data = content["data"]
        .as_str()
        .ok_or("the areas' content is not inline")?;
    let bytes = STANDARD
        .decode(data)
        .map_err(|e| format!("the areas' content is not base64: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("the areas' content is not valid: {e}"))
}

fn coding(system: &str, code: &str) -> Value {
    json!({ "coding": [{ "system": system, "code": code }] })
}

/// The Organization of `place`.
pub fn organization_json(place: &Place) -> Value {
    let mut identifier = vec![json!({ "system": PLACE_SYSTEM, "value": place.code })];
    let mut types = vec![coding(PLACE_LEVEL_SYSTEM, place.level.code())];
    if place.level == PlaceLevel::Facility {
        identifier.push(json!({ "system": NHFR_SYSTEM, "value": place.code }));
    }
    if let Some(level) = &place.care_level {
        types.push(coding(CARE_LEVEL_SYSTEM, level));
    }
    if let Some(ownership) = &place.ownership {
        types.push(coding(OWNERSHIP_SYSTEM, ownership));
    }
    let mut organization = json!({
        "resourceType": "Organization",
        "id": place_id(&place.code).to_string(),
        "identifier": identifier,
        "active": place.active,
        "type": types,
        "name": place.name,
    });
    if let Some(parent) = &place.parent {
        organization["partOf"] = json!({
            "reference": format!("Organization/{}", place_id(parent)),
            "identifier": { "system": PLACE_SYSTEM, "value": parent },
        });
    }
    organization
}

/// The Location holding `place`'s coordinates, when it has them.
pub fn location_json(place: &Place) -> Option<Value> {
    let position = place.coordinates.as_ref()?;
    let degrees = |text: &str| text.parse::<serde_json::Number>().ok().map(Value::Number);
    Some(json!({
        "resourceType": "Location",
        "id": location_id(&place.code).to_string(),
        "identifier": [{ "system": PLACE_SYSTEM, "value": place.code }],
        "status": if place.active { "active" } else { "inactive" },
        "name": place.name,
        "mode": "instance",
        "position": {
            "latitude": degrees(&position.latitude)?,
            "longitude": degrees(&position.longitude)?,
        },
        "managingOrganization": { "reference": format!("Organization/{}", place_id(&place.code)) },
    }))
}

fn code_in(types: &Value, system: &str) -> Option<String> {
    types.as_array()?.iter().find_map(|t| {
        t["coding"]
            .as_array()?
            .iter()
            .find(|c| c["system"] == system)?["code"]
            .as_str()
            .map(str::to_owned)
    })
}

/// The level of a stored Organization, when it is a place.
pub fn level_of(organization: &Value) -> Option<PlaceLevel> {
    code_in(&organization["type"], PLACE_LEVEL_SYSTEM)?
        .parse()
        .ok()
}

/// The place a stored Organization is, when it is one. Its coordinates are on its Location and
/// are not read back here.
pub fn place_of(organization: &Value) -> Option<Place> {
    let level = level_of(organization)?;
    let code = organization["identifier"]
        .as_array()?
        .iter()
        .find(|i| i["system"] == PLACE_SYSTEM)?["value"]
        .as_str()?
        .to_owned();
    Some(Place {
        code,
        name: organization["name"].as_str().unwrap_or_default().to_owned(),
        level,
        parent: organization["partOf"]["identifier"]["value"]
            .as_str()
            .map(str::to_owned),
        active: organization["active"].as_bool().unwrap_or(true),
        care_level: code_in(&organization["type"], CARE_LEVEL_SYSTEM),
        ownership: code_in(&organization["type"], OWNERSHIP_SYSTEM),
        coordinates: None,
    })
}

/// The registry's places, read in `tx`, with each facility's coordinates from its Location.
pub async fn places<T: PlaceStore + Transaction>(tx: &mut T) -> Result<Places, StoreError> {
    let mut found = Vec::new();
    for json in tx.organizations().await? {
        let Some(mut place) = serde_json::from_str::<Value>(&json)
            .ok()
            .as_ref()
            .and_then(place_of)
        else {
            continue;
        };
        if place.level == PlaceLevel::Facility {
            place.coordinates = coordinates(tx, &place.code).await?;
        }
        found.push(place);
    }
    Ok(Places::new(found))
}

async fn coordinates<T: Transaction>(
    tx: &mut T,
    code: &str,
) -> Result<Option<health_facility_registry::Coordinates>, StoreError> {
    let Ok(id) = ResourceId::from_uuid(location_id(code)) else {
        return Ok(None);
    };
    let Some(stored) = tx.read(&location_type()?, id).await? else {
        return Ok(None);
    };
    if stored.version.deleted {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(stored.json.as_str()).unwrap_or(Value::Null);
    let text = |v: &Value| match v {
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    };
    Ok(text(&value["position"]["latitude"])
        .zip(text(&value["position"]["longitude"]))
        .and_then(|(lat, lon)| health_facility_registry::Coordinates::new(&lat, &lon)))
}

fn organization_type() -> Result<ResourceType, StoreError> {
    "Organization"
        .parse()
        .map_err(|_| StoreError::UnknownResourceType("Organization".into()))
}

fn location_type() -> Result<ResourceType, StoreError> {
    "Location"
        .parse()
        .map_err(|_| StoreError::UnknownResourceType("Location".into()))
}

/// Writes `json` as `resource_type`/`id` over whatever version is there now.
async fn put<T: Transaction, A: AuditTrail<T>>(
    tx: &mut T,
    audit: &A,
    actor: &str,
    resource_type: ResourceType,
    id: uuid::Uuid,
    json: &Value,
) -> Result<(), RegistryError> {
    let id = ResourceId::from_uuid(id).map_err(|e| RegistryError::Store(e.to_string()))?;
    let precondition = match tx.read(&resource_type, id).await? {
        Some(head) => Precondition::IfMatch(head.version.version_id),
        None => Precondition::MustNotExist,
    };
    tx.write(WriteRequest {
        resource_type: resource_type.clone(),
        id,
        json: RawJson::from_validated(json.to_string()),
        op_id: OpId::new(),
        precondition,
    })
    .await?;
    let written = tx
        .read(&resource_type, id)
        .await?
        .ok_or_else(|| RegistryError::Store("a written place is missing".into()))?;
    audit
        .written(tx, actor, "nhfr-import", &written.json)
        .await?;
    Ok(())
}

/// Applies an NHFR import's `plan` in one transaction of `store`, as `actor`: every place it
/// writes, as an Organization, and each facility's Location where it has coordinates. Returns
/// how many places were written. A place changed by someone else since it was read fails the
/// whole import with [`RegistryError::Conflict`].
pub async fn apply_import<S, A>(
    store: &S,
    audit: &A,
    actor: &str,
    plan: &ImportPlan,
) -> Result<usize, RegistryError>
where
    S: Store,
    A: AuditTrail<S::Tx>,
{
    let mut tx = store.begin().await?;
    let writes = plan.writes();
    for place in &writes {
        put(
            &mut tx,
            audit,
            actor,
            organization_type()?,
            place_id(&place.code),
            &organization_json(place),
        )
        .await?;
        if let Some(location) = location_json(place) {
            put(
                &mut tx,
                audit,
                actor,
                location_type()?,
                location_id(&place.code),
                &location,
            )
            .await?;
        }
    }
    tx.commit().await?;
    Ok(writes.len())
}
