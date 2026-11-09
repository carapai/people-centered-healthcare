//! The definitions central serves from its active set (#89): each definitional resource in an
//! active package is written into central's store, where clients read it (a Questionnaire by
//! `url|version`, a program's PlanDefinition, a Library) and from where it syncs down to every
//! node in the packages lane.
//!
//! A resource's id is a UUIDv5 of its `url|version`, so each version is its own resource: a
//! rollback activates an earlier one that is still there, and records keep the version they were
//! written against. A resource already written is left as it is, so writing a set again changes
//! nothing. Only central writes them: a node below receives them by sync.

use health_configuration::activation::ActiveSet;
use health_fhir_validation::validate_structure;
use health_files::FsBlobStore;
use health_identifiers::{OpId, ResourceId};
use health_packages::{Files, Limits, unpack};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store as _, Transaction as _, WriteError, WriteRequest,
};
use health_surrealdb::SurrealStore;
use serde_json::Value;

/// The resource types a package's definitions are written as: definitions, never records
/// about people.
pub const DEFINITIONAL: &[&str] = &[
    "ActivityDefinition",
    "CodeSystem",
    "ConceptMap",
    "Library",
    "PlanDefinition",
    "Questionnaire",
    "SearchParameter",
    "StructureDefinition",
    "ValueSet",
];

/// The id a definition is stored under: a UUIDv5 of its `url|version`.
pub fn definition_id(url: &str, version: &str) -> Option<ResourceId> {
    let uuid = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_URL,
        format!("{url}|{version}").as_bytes(),
    );
    ResourceId::from_uuid(uuid).ok()
}

/// The definitional resources in a package's `files`, each with the id it is stored under.
/// Artefacts without a `url` and `version`, and other types, are left out.
pub fn definitions(files: &Files) -> Vec<(ResourceType, ResourceId, Value)> {
    files
        .iter()
        .filter(|(path, _)| {
            path.ends_with(".json") && !path.contains('/') && !path.starts_with('.')
        })
        .filter_map(|(_, bytes)| serde_json::from_slice::<Value>(bytes).ok())
        .filter_map(|mut resource| {
            let kind = resource["resourceType"].as_str()?.to_owned();
            if !DEFINITIONAL.contains(&kind.as_str()) {
                return None;
            }
            let id = definition_id(resource["url"].as_str()?, resource["version"].as_str()?)?;
            resource["id"] = Value::String(id.to_string());
            Some((kind.parse().ok()?, id, resource))
        })
        .collect()
}

/// The files of every package in `active` (staged here, its tarball in `blobs`), by
/// `name@version`.
pub async fn active_files(
    store: &SurrealStore,
    blobs: &FsBlobStore,
    active: &ActiveSet,
) -> Result<Vec<(String, Files)>, String> {
    let mut packages = Vec::with_capacity(active.len());
    for (name, version) in active {
        let id = format!("{name}@{version}");
        let mut tx = store.begin().await.map_err(|e| e.to_string())?;
        let digest = tx
            .staged_digest(&id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("{id} is active but not staged"))?;
        drop(tx);
        let hash: health_files::BlobHash = digest
            .parse()
            .map_err(|_| format!("{id}: {digest} is not a sha-256"))?;
        let local = blobs.clone();
        let tgz = tokio::task::spawn_blocking(move || local.read(&hash))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("{id}: its tarball: {e}"))?;
        let files = unpack(&tgz, Limits::default())
            .map_err(|e| format!("{id}: {e}"))?
            .files;
        packages.push((id, files));
    }
    Ok(packages)
}

/// Writes the definitions of every package in `active` (staged here, its tarball in `blobs`)
/// into `store`: the number newly written.
pub async fn write_definitions(
    store: &SurrealStore,
    blobs: &FsBlobStore,
    active: &ActiveSet,
) -> Result<usize, String> {
    let mut written = 0;
    for (id, files) in active_files(store, blobs, active).await? {
        for (resource_type, resource_id, resource) in definitions(&files) {
            let json = resource.to_string();
            if let Some(issue) = validate_structure(&json).first() {
                return Err(format!(
                    "{id}: a {resource_type} fails validation: {issue:?}"
                ));
            }
            let mut tx = store.begin().await.map_err(|e| e.to_string())?;
            match tx
                .write(WriteRequest {
                    resource_type,
                    id: resource_id,
                    json: RawJson::from_validated(json),
                    op_id: OpId::new(),
                    precondition: Precondition::MustNotExist,
                })
                .await
            {
                Ok(_) => {
                    tx.commit().await.map_err(|e| e.to_string())?;
                    written += 1;
                }
                // Written when this version was first activated: left as it is.
                Err(WriteError::PreconditionFailed { .. }) => {}
                Err(error) => return Err(format!("{id}: {error}")),
            }
        }
    }
    Ok(written)
}
