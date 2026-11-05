//! Completing a QuestionnaireResponse (#93; questionnaire-extraction.md; ADR 0022, 0023), on the
//! write path behind [`FhirServer::with_extraction`]:
//!
//! - **On submission** ([`choose`]): a `completed` or `amended` response that records no mapping
//!   gets the newest Extraction Mapping for its Questionnaire version, found among the node's
//!   `Library?type=extraction-mapping`. A Questionnaire with no mapping produces nothing.
//! - **After the response is written** ([`complete`]), in its transaction:
//!   - extraction with the recorded mapping (`Library?url=&version=`), each planned resource
//!     accepted by its Owner's use case;
//!   - the plan reconciled with the last run, read from the response's Provenance;
//!   - each written or retracted resource goes through the write path like any other (tiers
//!     1–2, the facility stamp, audit, reference checks);
//!   - a seed resource the amendment would change is left as it is, and
//!     `clinical-documentation.source-form-amended.v1` goes on the outbox for its review;
//!   - the Provenance gets a new version naming every target.
//!
//!   Any failure fails the whole write: the response stays as it was (all or nothing).

use std::collections::BTreeMap;

use axum::http::StatusCode;
use health_clinical_documentation_app::{
    Run, Step, admit, choose as choose_mapping, key, provenance, provenance_id, reconcile,
    record_mapping, recorded_mapping, targets,
};
use health_fhir_extraction::{Mapping, extract};
use health_fhir_search::{Handling, Unrestricted, parse_query};
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    OutboxEntry, OutboxPayload, Precondition, RawJson, ResourceType, Store, Transaction,
};
use health_surrealdb::SurrealTx;
use health_time::Timestamp;
use serde_json::{Value, json};

use crate::FhirServer;
use crate::ops::{self, Attempt};
use crate::outcome::FhirError;

fn refused(diagnostics: impl Into<String>) -> FhirError {
    FhirError::new(StatusCode::UNPROCESSABLE_ENTITY, "processing", diagnostics)
}

fn is_submitted(value: &Value) -> bool {
    value["resourceType"] == "QuestionnaireResponse"
        && matches!(value["status"].as_str(), Some("completed" | "amended"))
}

/// The Libraries `params` finds, as mappings; those that are not usable mappings are skipped.
async fn mappings(
    server: &FhirServer,
    tx: &mut SurrealTx,
    params: &[(&str, &str)],
) -> Result<Vec<Mapping>, FhirError> {
    let params: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    let registry = server.store.registry();
    let query = parse_query(&registry, "Library", &params, Handling::Strict)
        .map_err(|_| FhirError::not_supported("this node cannot look up Extraction Mappings"))?;
    let page = tx.search(&query, &Unrestricted, None).await?;
    Ok(page
        .resources
        .iter()
        .filter_map(|found| serde_json::from_str::<Value>(found.json.as_str()).ok())
        .filter_map(|library| Mapping::from_library(&library).ok())
        .collect())
}

/// Records on a submitted `response` the mapping it is extracted with, unless it records one.
pub(crate) async fn choose(server: &FhirServer, response: &mut Value) -> Result<(), FhirError> {
    if !server.extraction || !is_submitted(response) || recorded_mapping(response).is_some() {
        return Ok(());
    }
    let Some(questionnaire) = response["questionnaire"].as_str().map(str::to_owned) else {
        return Ok(());
    };
    let mut tx = server.store.begin().await?;
    let found = mappings(server, &mut tx, &[("type", "extraction-mapping")]).await?;
    if let Some(mapping) = choose_mapping(&found, &questionnaire) {
        record_mapping(response, &mapping.canonical());
    }
    Ok(())
}

/// The head of `Type/id`, unless it is deleted, with its version id.
async fn head(
    tx: &mut SurrealTx,
    resource_type: &ResourceType,
    id: ResourceId,
) -> Result<Option<(Value, String)>, Attempt> {
    Ok(tx
        .read(resource_type, id)
        .await?
        .filter(|h| !h.version.deleted)
        .and_then(|h| {
            serde_json::from_str(h.json.as_str())
                .ok()
                .map(|v| (v, h.version.version_id.to_string()))
        }))
}

fn kind(name: &str) -> Result<ResourceType, Attempt> {
    name.parse()
        .map_err(|_| Attempt::Fail(refused(format!("{name} is not a resource type"))))
}

/// Writes `resource` as the next version of `Type/id` through the write path, returning
/// `Type/id/_history/vid`.
async fn put(
    server: &FhirServer,
    tx: &mut SurrealTx,
    resource_type: &str,
    id: ResourceId,
    resource: &Value,
) -> Result<String, Attempt> {
    let found = health_fhir_validation::validate_value(resource);
    if !found.is_empty() {
        return Err(Attempt::Fail(FhirError::invalid_resource(&found)));
    }
    server.check_invariants(resource).await?;
    let kind = kind(resource_type)?;
    let precondition = match tx.read(&kind, id).await? {
        Some(existing) => Precondition::IfMatch(existing.version.version_id),
        None => Precondition::MustNotExist,
    };
    let json = RawJson::from_validated(resource.to_string());
    let (version, stored) = ops::write(tx, &kind, id, &json, precondition).await?;
    crate::references::check_written(server, tx, &stored).await?;
    Ok(format!(
        "{}/_history/{}",
        key(resource_type, id),
        version.version_id
    ))
}

/// Extracts the stored `response`, if it is a submitted response with a mapping, and writes what
/// it produces in the same transaction.
pub(crate) async fn complete(
    server: &FhirServer,
    tx: &mut SurrealTx,
    stored: &RawJson,
) -> Result<(), Attempt> {
    if !server.extraction {
        return Ok(());
    }
    let Ok(response) = serde_json::from_str::<Value>(stored.as_str()) else {
        return Ok(());
    };
    if !is_submitted(&response) {
        return Ok(());
    }
    let Some(canonical) = recorded_mapping(&response).map(str::to_owned) else {
        return Ok(());
    };
    let Some((url, version)) = canonical.split_once('|') else {
        return Err(Attempt::Fail(refused(format!(
            "the recorded mapping {canonical} names no version"
        ))));
    };
    let mapping = mappings(server, tx, &[("url", url), ("version", version)])
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| {
            Attempt::Fail(refused(format!(
                "the Extraction Mapping {canonical} is not known here"
            )))
        })?;
    let response_id: ResourceId = response["id"]
        .as_str()
        .and_then(|id| id.parse().ok())
        .ok_or_else(|| Attempt::Fail(refused("the response has no resource id")))?;
    let plan = extract(server.fhirpath.as_ref(), &response, &mapping)
        .await
        .map_err(|e| Attempt::Fail(refused(e.to_string())))?;
    for planned in &plan {
        admit(planned).map_err(|e| Attempt::Fail(refused(e.to_string())))?;
    }

    // The last run's targets, from the response's Provenance, and every resource's head.
    let provenance_kind = kind("Provenance")?;
    let provenance_id = provenance_id(response_id);
    let previous = head(tx, &provenance_kind, provenance_id)
        .await?
        .map(|(p, _)| targets(&p))
        .unwrap_or_default();
    let mut current = BTreeMap::new();
    let mut versions = BTreeMap::new();
    let named =
        plan.iter()
            .map(|p| (p.resource_type.clone(), p.id))
            .chain(previous.iter().filter_map(|k| {
                let (t, id) = k.split_once('/')?;
                Some((t.to_owned(), id.parse().ok()?))
            }));
    for (resource_type, id) in named {
        if let Some((resource, version)) = head(tx, &kind(&resource_type)?, id).await? {
            let here = key(&resource_type, id);
            versions.insert(here.clone(), format!("{here}/_history/{version}"));
            current.insert(here, resource);
        }
    }

    let mut touched = BTreeMap::new();
    for step in reconcile(&plan, &current, &previous, &mapping) {
        match step {
            Step::Write {
                resource_type,
                id,
                resource,
            }
            | Step::Retract {
                resource_type,
                id,
                resource,
            } => {
                let written = put(server, tx, &resource_type, id, &resource).await?;
                touched.insert(key(&resource_type, id), written);
            }
            Step::Review { resource_type, id } => {
                let body = json!({
                    "response": format!("QuestionnaireResponse/{response_id}"),
                    "target": key(&resource_type, id),
                });
                tx.append_outbox(OutboxEntry {
                    op_id: OpId::new(),
                    payload: OutboxPayload::DomainEvent {
                        name: "clinical-documentation.source-form-amended.v1".into(),
                        body: RawJson::from_validated(body.to_string()),
                    },
                })
                .await?;
            }
        }
    }

    // Every target of this run: those written or retracted, and those left as they were.
    let mut named: Vec<String> = plan
        .iter()
        .map(|p| key(&p.resource_type, p.id))
        .chain(previous)
        .collect();
    named.dedup();
    let mut all: Vec<String> = Vec::new();
    for k in named {
        if let Some(versioned) = touched.get(&k).or_else(|| versions.get(&k))
            && !all.contains(versioned)
        {
            all.push(versioned.clone());
        }
    }
    if all.is_empty() {
        return Ok(());
    }
    let response_version = response["meta"]["versionId"].as_str().unwrap_or_default();
    let recorded = Timestamp::now().to_string();
    let record = provenance(
        &Run {
            response: &response,
            response_version,
            mapping: &canonical,
            targets: &all,
            recorded: &recorded,
        },
        response_id,
    );
    put(server, tx, "Provenance", provenance_id, &record).await?;
    Ok(())
}

impl FhirServer {
    /// This server, completing every submitted QuestionnaireResponse (#93): extraction with the
    /// Extraction Mapping for its Questionnaire, the extracted resources and their Provenance
    /// written with the response, all or nothing. It needs [`Self::with_questionnaire_checks`],
    /// and the node's search must support Library `type`, `url` and `version`.
    #[must_use]
    pub fn with_extraction(mut self) -> Self {
        self.extraction = true;
        self
    }
}
