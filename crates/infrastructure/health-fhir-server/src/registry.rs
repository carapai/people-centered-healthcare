//! The client registry's operations (#197; ADR 0027; patient-identity.md), for reviewers:
//!
//! | Operation | Route | Parameters |
//! |---|---|---|
//! | `$match` (R5) | `POST /Patient/$match` | `resource` (Patient), `onlyCertainMatches` (boolean), `count` (integer) |
//! | `$merge` (R5) | `POST /Patient/$merge` | `source-patient`, `target-patient` (References), `preview` (boolean) |
//! | `$link` | `POST /Patient/{id}/$link` | `golden` (Reference, optional: a new golden record when absent) |
//! | `$unlink` | `POST /Patient/{id}/$unlink` | none |
//!
//! - **`$match`** is for anyone who may search Patients (403 otherwise). It returns the ranked
//!   candidates the subject may see, as a searchset Bundle with `match-grade`. Candidates the
//!   subject may not see are left out, and so is their count.
//! - **Who:** for the other operations, the reviewer is the request's `Subject` (from the verified token). Without one the
//!   answer is 401. The subject needs the [`REVIEW_PERMISSION`] use-case permission and
//!   `Patient` update.
//! - **Merges:**
//!   - `preview=true` returns the plan and writes nothing.
//!   - Without it, the plan is approved by the reviewer and carried out.
//!   - A merge is for two sources of the same facility, the reviewer's own (403 otherwise):
//!     the session's facility code is the managing Organization's id or one of its identifier
//!     values.
//!   - Records of different facilities are refused (422).
//! - **Audit and review:** every version written is audited in its transaction, naming the
//!   reviewer. A link's survivorship disagreements go to the registry review queue.

use std::sync::Arc;

use axum::Extension;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use health_access_policy::{Interaction as Decide, Subject};
use health_audit::{Agent, Auditor, Entity, Interaction};
use health_client_registry::MergeRefused;
use health_identifiers::ResourceId;
use health_persistence::{RawJson, Store, Transaction, WriteError};
use health_registries_app::{AuditTrail, ClientRegistry, RegistryError};
use health_surrealdb::SurrealTx;
use health_time::Timestamp;
use serde_json::{Value, json};

use crate::FhirServer;
use crate::http::{fhir_json, require_json_body};
use crate::outcome::{FhirError, operation_outcome};

/// The use-case permission a reviewer's roles must map to.
pub const REVIEW_PERMISSION: &str = "registry.review";

/// The audit trail of one reviewer's request, over the server's auditor.
pub(crate) struct RegistryAudit {
    auditor: Option<Arc<Auditor>>,
    agent: Agent,
}

impl RegistryAudit {
    async fn append(
        &self,
        tx: &mut SurrealTx,
        entity: Entity,
        patient: Option<String>,
    ) -> Result<(), WriteError> {
        let Some(auditor) = &self.auditor else {
            return Ok(());
        };
        let event = auditor.event(
            &self.agent,
            Interaction::Update,
            &[entity],
            patient.as_deref(),
            Timestamp::now(),
            None,
        );
        auditor.append(tx, &event).await.map(|_| ())
    }
}

impl AuditTrail<SurrealTx> for RegistryAudit {
    async fn written(
        &self,
        tx: &mut SurrealTx,
        _actor: &str,
        _reason: &str,
        json: &RawJson,
    ) -> Result<(), WriteError> {
        let value: Value = serde_json::from_str(json.as_str()).unwrap_or(Value::Null);
        let Some(entity) = Entity::of(&value) else {
            return Ok(());
        };
        self.append(tx, entity, crate::access::patient_of(&value))
            .await
    }

    async fn recorded(
        &self,
        tx: &mut SurrealTx,
        _actor: &str,
        _reason: &str,
        what: &str,
    ) -> Result<(), WriteError> {
        let entity = Entity {
            what: what.to_owned(),
            labels: Vec::new(),
        };
        self.append(tx, entity, None).await
    }
}

type Registry = ClientRegistry<health_surrealdb::SurrealStore, RegistryAudit>;

/// The registry's configuration and Matching Rules.
pub(crate) struct RegistrySettings {
    pub(crate) config: health_registries_app::fhir::RegistryConfig,
    pub(crate) rules: health_client_registry::matching::MatchingRules,
}

/// The most candidates `$match` returns; the default is 10.
const MAX_MATCHES: usize = 100;

/// `POST /Patient/$match`.
pub(crate) async fn matches(
    State(server): State<FhirServer>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, FhirError> {
    let Some(settings) = server.registry.clone() else {
        return Err(FhirError::not_found("the client registry is not enabled"));
    };
    // The same permission as a Patient search (403 without it).
    crate::access::search_policy(&server, "Patient", None)?;
    let parameters = parameters(&headers, &body)?;
    let mut query = None;
    let mut only_certain = false;
    let mut count = 10;
    for parameter in &parameters {
        match parameter["name"].as_str().unwrap_or_default() {
            "resource" => query = Some(parameter["resource"].clone()),
            "onlyCertainMatches" => {
                only_certain = parameter["valueBoolean"].as_bool().ok_or_else(|| {
                    FhirError::bad_request("onlyCertainMatches must be a boolean")
                })?;
            }
            "count" => {
                count = parameter["valueInteger"]
                    .as_u64()
                    .and_then(|c| usize::try_from(c).ok())
                    .filter(|c| *c > 0)
                    .ok_or_else(|| FhirError::bad_request("count must be a positive integer"))?
                    .min(MAX_MATCHES);
            }
            other => {
                return Err(FhirError::not_supported(format!(
                    "$match does not support {other}"
                )));
            }
        }
    }
    let query = query
        .filter(|q| q["resourceType"] == "Patient")
        .ok_or_else(|| FhirError::bad_request("$match needs a Patient resource"))?;
    let matcher = health_registries_app::matching::Matcher::new(
        server.store.clone(),
        settings.rules.clone(),
        settings.config.clone(),
    );
    // Every candidate a block yields, so filtering cannot hide better ones behind `count`.
    let mut matched = matcher.find(&query, usize::MAX, only_certain).await?;
    matched
        .candidates
        .retain(|c| crate::access::may_list(&RawJson::from_validated(c.resource.to_string())));
    matched.candidates.truncate(count);
    let mut tx = server.store.begin().await?;
    let served: Vec<RawJson> = matched
        .candidates
        .iter()
        .map(|c| RawJson::from_validated(c.resource.to_string()))
        .collect();
    if crate::audit::read(&mut tx, Interaction::Search, served.iter())
        .await
        .map_err(crate::audit::failed)?
    {
        tx.commit().await?;
    }
    Ok(fhir_json(
        StatusCode::OK,
        health_registries_app::matching::bundle(&matched).to_string(),
    ))
}

/// The reviewer of this request and the registry acting for them.
fn reviewer(
    server: &FhirServer,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
) -> Result<(Subject, Registry), FhirError> {
    let Some(config) = server.registry.clone() else {
        return Err(FhirError::not_found("the client registry is not enabled"));
    };
    let Some(Extension(subject)) = subject else {
        return Err(FhirError::new(
            StatusCode::UNAUTHORIZED,
            "login",
            "registry operations need a signed-in reviewer",
        ));
    };
    if !subject.permissions.has(REVIEW_PERMISSION)
        || !subject.permissions.allows("Patient", Decide::Update)
    {
        return Err(FhirError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "registry operations need the registry reviewer's permission",
        ));
    }
    let agent = agent.map_or_else(
        || Agent {
            display: format!("account {}", subject.account),
            ..Agent::unauthenticated()
        },
        |Extension(agent)| agent,
    );
    let audit = RegistryAudit {
        auditor: server.audit.clone(),
        agent,
    };
    Ok((
        subject,
        ClientRegistry::new(server.store.clone(), audit, config.config.clone()),
    ))
}

impl From<RegistryError> for FhirError {
    fn from(error: RegistryError) -> Self {
        let message = error.to_string();
        match error {
            RegistryError::NotFound(_) | RegistryError::NoSuchReview(_) => Self::not_found(message),
            RegistryError::WrongKind(_) => Self::bad_request(message),
            RegistryError::AlreadyLinked { .. } | RegistryError::Conflict => {
                Self::new(StatusCode::CONFLICT, "conflict", message)
            }
            RegistryError::Link(_) | RegistryError::Merge(_) | RegistryError::Review(_) => {
                Self::new(StatusCode::UNPROCESSABLE_ENTITY, "business-rule", message)
            }
            RegistryError::Store(_) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "transient",
                "the store is unavailable",
            ),
        }
    }
}

/// The `parameter` entries of a Parameters body.
fn parameters(headers: &HeaderMap, body: &Bytes) -> Result<Vec<Value>, FhirError> {
    if body.is_empty() {
        return Ok(Vec::new());
    }
    require_json_body(headers)?;
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| FhirError::bad_request(format!("the body is not JSON: {e}")))?;
    if value["resourceType"] != "Parameters" {
        return Err(FhirError::bad_request(
            "the body is not a Parameters resource",
        ));
    }
    Ok(value["parameter"].as_array().cloned().unwrap_or_default())
}

/// The Patient id a `Patient/{id}` Reference parameter names.
fn patient_parameter(parameters: &[Value], name: &str) -> Result<Option<ResourceId>, FhirError> {
    let Some(parameter) = parameters.iter().find(|p| p["name"] == name) else {
        return Ok(None);
    };
    parameter["valueReference"]["reference"]
        .as_str()
        .and_then(|r| r.strip_prefix("Patient/"))
        .and_then(|id| id.parse().ok())
        .map(Some)
        .ok_or_else(|| FhirError::bad_request(format!("{name} must be a Patient reference")))
}

fn id_of(text: &str) -> Result<ResourceId, FhirError> {
    text.parse()
        .map_err(|_| FhirError::not_found(format!("Patient/{text} is not known")))
}

fn reference(id: impl std::fmt::Display) -> Value {
    json!({ "reference": format!("Patient/{id}") })
}

/// Whether the records' managing Organization (`Organization/{id}`) is the session's facility:
/// the facility code is the Organization's id or one of its identifier values.
async fn own_facility(
    server: &FhirServer,
    organization: &str,
    facility: &str,
) -> Result<bool, FhirError> {
    let Some(id) = organization.strip_prefix("Organization/") else {
        return Ok(false);
    };
    if id == facility {
        return Ok(true);
    }
    let Ok(id) = id.parse::<ResourceId>() else {
        return Ok(false);
    };
    let organization_type = "Organization"
        .parse()
        .map_err(|_| FhirError::bad_request("Organization"))?;
    let mut tx = server.store.begin().await?;
    let Some(stored) = tx.read(&organization_type, id).await? else {
        return Ok(false);
    };
    let json: Value = serde_json::from_str(stored.json.as_str()).unwrap_or(Value::Null);
    Ok(json["identifier"]
        .as_array()
        .is_some_and(|ids| ids.iter().any(|i| i["value"] == facility)))
}

/// `POST /Patient/$merge`.
pub(crate) async fn merge(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, FhirError> {
    let (subject, registry) = reviewer(&server, subject, agent)?;
    let parameters = parameters(&headers, &body)?;
    for parameter in &parameters {
        let name = parameter["name"].as_str().unwrap_or_default();
        if !matches!(name, "source-patient" | "target-patient" | "preview") {
            return Err(FhirError::not_supported(format!(
                "$merge does not support {name} yet"
            )));
        }
    }
    let required = |name: &str| {
        patient_parameter(&parameters, name)?
            .ok_or_else(|| FhirError::bad_request(format!("$merge needs {name}")))
    };
    let (source, target) = (required("source-patient")?, required("target-patient")?);
    let preview = parameters
        .iter()
        .find(|p| p["name"] == "preview")
        .and_then(|p| p["valueBoolean"].as_bool())
        .unwrap_or(false);
    let plan = registry.merge_preview(source, target).await?;
    // A reviewer merges records of their own facility only.
    if !own_facility(&server, &plan.facility, &subject.facility).await? {
        return Err(FhirError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            MergeRefused::DifferentFacility.to_string(),
        ));
    }
    let outcome = if preview {
        operation_outcome(
            "information",
            "informational",
            &format!(
                "Preview: Patient/{source} would become inactive and replaced by Patient/{target}; nothing was written"
            ),
        )
    } else {
        registry.merge(&plan.approve(&subject.account)).await?;
        operation_outcome(
            "information",
            "informational",
            &format!("Patient/{source} was merged into Patient/{target}"),
        )
    };
    let mut output = vec![
        json!({ "name": "outcome", "resource": outcome }),
        json!({ "name": "source-patient", "valueReference": reference(source) }),
        json!({ "name": "target-patient", "valueReference": reference(target) }),
    ];
    if !preview {
        let mut tx = server.store.begin().await?;
        if let Some(stored) = tx
            .read(
                &"Patient"
                    .parse()
                    .map_err(|_| FhirError::bad_request("Patient"))?,
                target,
            )
            .await?
        {
            let result: Value = serde_json::from_str(stored.json.as_str()).unwrap_or(Value::Null);
            output.push(json!({ "name": "result", "resource": result }));
        }
    }
    let body = json!({ "resourceType": "Parameters", "parameter": output });
    Ok(fhir_json(StatusCode::OK, body.to_string()))
}

/// `POST /Patient/{id}/$link`.
pub(crate) async fn link(
    State(server): State<FhirServer>,
    Path(id): Path<String>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, FhirError> {
    let (subject, registry) = reviewer(&server, subject, agent)?;
    let source = id_of(&id)?;
    let golden = patient_parameter(&parameters(&headers, &body)?, "golden")?;
    let linked = registry
        .link_reviewed(source, golden, &subject.account, Timestamp::now())
        .await?;
    let body = json!({ "resourceType": "Parameters", "parameter": [
        { "name": "golden", "valueReference": reference(linked.golden) },
        { "name": "created", "valueBoolean": linked.created },
        { "name": "review", "valueInteger": linked.review.len() },
    ] });
    Ok(fhir_json(StatusCode::OK, body.to_string()))
}

/// `POST /Patient/{id}/$unlink`.
pub(crate) async fn unlink(
    State(server): State<FhirServer>,
    Path(id): Path<String>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
) -> Result<Response, FhirError> {
    let (subject, registry) = reviewer(&server, subject, agent)?;
    let golden = registry.unlink(id_of(&id)?, &subject.account).await?;
    let body = json!({ "resourceType": "Parameters", "parameter": [
        { "name": "golden", "valueReference": reference(golden) },
    ] });
    Ok(fhir_json(StatusCode::OK, body.to_string()))
}
