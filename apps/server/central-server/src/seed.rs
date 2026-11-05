//! The development FHIR seed (#92): definitional resources, such as the Questionnaires the
//! client's forms render, written once at startup from `HEALTH_DEV_FHIR_SEED`, for development
//! and e2e only. Activation (#89) replaces it with the published packages a node has active.
//!
//! The file is a FHIR Bundle. Only definitional resource types are accepted, each must pass
//! tier 1, and each needs a UUID id: one already stored is left as it is, so restarts change
//! nothing.

use std::path::Path;

use health_fhir_validation::validate_structure;
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store, Transaction, WriteError, WriteRequest,
};
use health_surrealdb::SurrealStore;
use serde_json::Value;

/// The resource types a seed may hold: definitions, never records about people.
const DEFINITIONAL: &[&str] = &[
    "Questionnaire",
    "ValueSet",
    "CodeSystem",
    "Library",
    "PlanDefinition",
    "ActivityDefinition",
];

/// Writes the seed at `path` into `store`: the number of resources it added.
pub(crate) async fn run(store: &SurrealStore, path: &Path) -> Result<usize, &'static str> {
    tracing::warn!("writing a development FHIR seed, not published packages");
    let text = std::fs::read_to_string(path).map_err(|_| "cannot read HEALTH_DEV_FHIR_SEED")?;
    let bundle: Value =
        serde_json::from_str(&text).map_err(|_| "HEALTH_DEV_FHIR_SEED is not JSON")?;
    if bundle["resourceType"] != "Bundle" {
        return Err("HEALTH_DEV_FHIR_SEED is not a FHIR Bundle");
    }
    let mut added = 0;
    for entry in bundle["entry"].as_array().into_iter().flatten() {
        let resource = &entry["resource"];
        let kind = resource["resourceType"].as_str().unwrap_or_default();
        if !DEFINITIONAL.contains(&kind) {
            return Err("HEALTH_DEV_FHIR_SEED may hold definitional resources only");
        }
        let json = resource.to_string();
        if !validate_structure(&json).is_empty() {
            return Err("HEALTH_DEV_FHIR_SEED holds a resource that fails validation");
        }
        let resource_type: ResourceType = kind.parse().map_err(|_| "an unknown resource type")?;
        let id = resource["id"]
            .as_str()
            .and_then(|id| id.parse().ok())
            .and_then(|uuid| ResourceId::from_uuid(uuid).ok())
            .ok_or("each seeded resource needs a UUID id")?;
        let mut tx = store
            .begin()
            .await
            .map_err(|_| "the store is unavailable")?;
        let written = tx
            .write(WriteRequest {
                resource_type,
                id,
                json: RawJson::from_validated(json),
                op_id: OpId::new(),
                precondition: Precondition::MustNotExist,
            })
            .await;
        match written {
            Ok(_) => {
                tx.commit()
                    .await
                    .map_err(|_| "the seed could not be written")?;
                added += 1;
            }
            // Already seeded on an earlier start: left as it is.
            Err(WriteError::PreconditionFailed { .. }) => {}
            Err(_) => return Err("the seed could not be written"),
        }
    }
    Ok(added)
}
