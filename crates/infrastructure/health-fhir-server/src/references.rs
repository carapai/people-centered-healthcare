//! Reference integrity (persistence.md § Reference integrity), checked in application code
//! inside the write transaction, after the writes it covers:
//!
//! | Reference | Rule here |
//! |---|---|
//! | `#id` (contained) | not checked |
//! | an absolute URL on another server | allowed |
//! | `urn:…` | refused: it only makes sense inside a Bundle, where it was rewritten |
//! | `Type/id`, or an absolute URL on this server | must name a live resource, unless [`ReferenceScope`] says the target may be out of scope |
//!
//! A delete is refused (409) while a live record still references the resource.

use axum::http::StatusCode;
use health_fhir_search::normalize::ReferenceKey;
use health_identifiers::ResourceId;
use health_persistence::{RawJson, ResourceType, Transaction};
use health_surrealdb::{SurrealTx, ownership};
use serde_json::Value;

use crate::FhirServer;
use crate::ops::Attempt;
use crate::outcome::FhirError;

/// Which reference targets must resolve on this node. Global replicas and in-scope facility
/// records must; records outside the node's scope (another facility's Encounter in a referral)
/// may be dangling, to be checked at central. Until node scope exists, [`AllMustResolve`] is
/// the only policy.
pub trait ReferenceScope: Send + Sync {
    /// Whether a reference to `target_type/target_id` must resolve locally.
    fn must_resolve(&self, target_type: &ResourceType, target_id: ResourceId) -> bool;
}

/// Every local reference must resolve.
#[derive(Debug, Clone, Copy, Default)]
pub struct AllMustResolve;

impl ReferenceScope for AllMustResolve {
    fn must_resolve(&self, _target_type: &ResourceType, _target_id: ResourceId) -> bool {
        true
    }
}

fn unresolved(reference: &str, why: &str) -> Attempt {
    FhirError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "not-found",
        format!("reference {reference} {why}"),
    )
    .into()
}

fn collect<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                match (key.as_str(), child) {
                    ("reference", Value::String(reference)) => out.push(reference),
                    _ => collect(child, out),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect(item, out)),
        _ => {}
    }
}

/// Checks the references of a stored resource version.
pub(crate) async fn check_written(
    server: &FhirServer,
    tx: &mut SurrealTx,
    json: &RawJson,
) -> Result<(), Attempt> {
    let value: Value = serde_json::from_str(json.as_str())
        .map_err(|e| Attempt::Fail(FhirError::bad_request(format!("stored JSON: {e}"))))?;
    let mut references = Vec::new();
    collect(&value, &mut references);
    references.sort_unstable();
    references.dedup();
    for reference in references {
        if reference.starts_with('#') {
            continue;
        }
        if reference.starts_with("urn:") {
            return Err(unresolved(reference, "is not a resource on this server"));
        }
        let local = reference
            .strip_prefix(server.base.as_ref())
            .map(|r| r.trim_start_matches('/'));
        let is_external = local.is_none() && reference.contains("://");
        if is_external {
            continue;
        }
        let key = ReferenceKey::parse(local.unwrap_or(reference));
        if key.target_type.is_empty() || ownership::resource(&key.target_type).is_none() {
            return Err(unresolved(
                reference,
                "does not name a resource type stored here",
            ));
        }
        let target_type: ResourceType = key
            .target_type
            .parse()
            .map_err(|_| unresolved(reference, "does not name a resource type stored here"))?;
        let Ok(target_id) = key.target_id.parse::<ResourceId>() else {
            return Err(unresolved(reference, "does not resolve"));
        };
        if !server.references.must_resolve(&target_type, target_id) {
            continue;
        }
        match tx.read(&target_type, target_id).await? {
            Some(head) if !head.version.deleted => {}
            Some(_) => return Err(unresolved(reference, "names a deleted resource")),
            None => return Err(unresolved(reference, "does not resolve")),
        }
    }
    Ok(())
}

/// Refuses a delete (409) while a live record still references the resource.
pub(crate) async fn check_deletable(
    tx: &mut SurrealTx,
    resource_type: &ResourceType,
    id: ResourceId,
) -> Result<(), Attempt> {
    if let Some((source_type, source_id)) =
        tx.referrers(resource_type, id, 1).await?.into_iter().next()
    {
        return Err(FhirError::new(
            StatusCode::CONFLICT,
            "conflict",
            format!("{resource_type}/{id} is still referenced by {source_type}/{source_id}; correct records with entered-in-error instead of deleting"),
        )
        .into());
    }
    Ok(())
}
