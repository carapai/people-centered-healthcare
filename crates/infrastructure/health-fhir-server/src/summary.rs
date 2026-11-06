//! The patient summary (#104; clinical-workspaces.md): `GET /Patient/{id}/$summary`, the shared
//! record's read model (`health-shared-record`). It answers JSON, not a FHIR resource:
//! `allergies`, `conditions`, `medications`, `vitals`, `visits` and `programs`.
//!
//! - **The patient** must be readable by the clinician (404 otherwise, as a read).
//! - **Whose records:** the patient's and, through the golden record, those of the same person's
//!   other patient records.
//! - **Each section** is a search per patient (`{type}?patient=`) under the clinician's own search
//!   policy, so sensitive labels are the policy decision point's: a record the clinician may not
//!   see is never found. A section the clinician may not search, or the node cannot search, is
//!   `null`.
//! - **Audit:** the records read are audited as a search's are.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use health_audit::Interaction;
use health_fhir_search::{Handling, parse_query};
use health_identifiers::ResourceId;
use health_persistence::{RawJson, ResourceType, Store, StoredResource, Transaction};
use health_shared_record_app::{Found, SECTIONS, golden_of, linked, summary};
use serde_json::Value;

use crate::outcome::FhirError;
use crate::{FhirServer, access, audit};

async fn head(
    tx: &mut health_surrealdb::SurrealTx,
    reference: &str,
) -> Result<Option<StoredResource>, FhirError> {
    let Some((kind, id)) = reference.split_once('/') else {
        return Ok(None);
    };
    let (Ok(kind), Ok(id)) = (kind.parse::<ResourceType>(), id.parse::<ResourceId>()) else {
        return Ok(None);
    };
    Ok(tx.read(&kind, id).await?.filter(|h| !h.version.deleted))
}

/// The patient records that make up `id`'s read models: `Patient/{id}` (readable by the reader,
/// else 404, as a read), then, through its golden record, the same person's other records.
pub(crate) async fn people(
    tx: &mut health_surrealdb::SurrealTx,
    id: &str,
) -> Result<Vec<String>, FhirError> {
    let not_found = || FhirError::not_found(format!("Patient/{id} is not known"));
    let stored = head(tx, &format!("Patient/{id}"))
        .await?
        .ok_or_else(not_found)?;
    access::readable(&stored.json, not_found)?;
    let patient: Value = serde_json::from_str(stored.json.as_str())
        .map_err(|e| FhirError::bad_request(format!("stored JSON: {e}")))?;
    // The golden record is read for its links only.
    let golden = match golden_of(&patient) {
        Some(reference) => head(tx, &reference)
            .await?
            .and_then(|g| serde_json::from_str::<Value>(g.json.as_str()).ok()),
        None => None,
    };
    Ok(linked(&patient, golden.as_ref()))
}

/// Each type's records of `patients` (`{type}?patient=`), under the reader's own search policy:
/// `None` for a type the reader may not search, or the node cannot. `read` collects what was read,
/// for the audit.
pub(crate) async fn searched(
    server: &FhirServer,
    tx: &mut health_surrealdb::SurrealTx,
    patients: &[String],
    types: &[&str],
    read: &mut Vec<RawJson>,
) -> Result<Vec<Option<Vec<Value>>>, FhirError> {
    let registry = server.store.registry();
    let mut sections = Vec::new();
    for resource_type in types {
        let mut all = Vec::new();
        let mut seen = true;
        for reference in patients {
            let params = vec![
                ("patient".to_owned(), reference.clone()),
                ("_count".to_owned(), "1000".to_owned()),
            ];
            let (Ok(query), Ok(policy)) = (
                parse_query(&registry, resource_type, &params, Handling::Strict),
                access::search_policy(server, resource_type, Some(reference.clone())),
            ) else {
                seen = false;
                break;
            };
            let page = tx.search(&query, policy.as_ref(), None).await?;
            for resource in page.resources {
                if let Ok(value) = serde_json::from_str::<Value>(resource.json.as_str()) {
                    all.push(value);
                }
                read.push(resource.json);
            }
        }
        sections.push(seen.then_some(all));
    }
    Ok(sections)
}

/// Audits `read` as a search's reads, and commits if the audit wrote anything.
pub(crate) async fn audited(
    mut tx: health_surrealdb::SurrealTx,
    read: &[RawJson],
) -> Result<(), FhirError> {
    if audit::read(&mut tx, Interaction::Search, read.iter())
        .await
        .map_err(audit::failed)?
    {
        tx.commit().await?;
    }
    Ok(())
}

/// `GET /Patient/{id}/$summary`.
pub(crate) async fn patient_summary(
    State(server): State<FhirServer>,
    Path(id): Path<String>,
) -> Result<Response, FhirError> {
    let mut tx = server.store.begin().await?;
    let patients = people(&mut tx, &id).await?;
    let mut read = Vec::new();
    let sections = searched(&server, &mut tx, &patients, &SECTIONS, &mut read).await?;
    let mut found: Found = Default::default();
    for (slot, section) in found.iter_mut().zip(sections) {
        *slot = section;
    }
    audited(tx, &read).await?;
    let body = serde_json::to_value(summary(&found))
        .map_err(|e| FhirError::bad_request(format!("summary: {e}")))?;
    Ok((StatusCode::OK, axum::Json(body)).into_response())
}
