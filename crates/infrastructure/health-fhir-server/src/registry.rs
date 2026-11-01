//! The client registry's operations (#197; ADR 0027; patient-identity.md), for reviewers:
//!
//! | Operation | Route | Parameters |
//! |---|---|---|
//! | `$match` (R5) | `POST /Patient/$match` | `resource` (Patient), `onlyCertainMatches` (boolean), `count` (integer) |
//! | `$register` | `POST /Patient/$register` | `resource` (Patient), `confirmedNew` (boolean) |
//! | `$merge` (R5) | `POST /Patient/$merge` | `source-patient`, `target-patient` (References), `preview` (boolean) |
//! | `$link` | `POST /Patient/{id}/$link` | `golden` (Reference, optional: a new golden record when absent) |
//! | `$unlink` | `POST /Patient/{id}/$unlink` | none |
//!
//! - **`$match`** is for anyone who may search Patients (403 otherwise). It returns the ranked
//!   candidates the subject may see, as a searchset Bundle with `match-grade`. Candidates the
//!   subject may not see are left out, and so is their count.
//! - **`$register`** (with [`FhirServer::with_registration`]) is reception's registration, for
//!   anyone who may create Patients. Until `confirmedNew`, it answers with the candidates the
//!   subject may see and writes nothing (patient-identity.md). Then it creates the provisional
//!   source Patient with a facility MRN and runs registration matching.
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
#[derive(Clone)]
pub(crate) struct RegistryAudit {
    pub(crate) auditor: Option<Arc<Auditor>>,
    pub(crate) agent: Agent,
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
    pub(crate) mrn: Option<health_registries_app::register::MrnSettings>,
}

/// `POST /Patient/$register`.
pub(crate) async fn register(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, FhirError> {
    use health_registries_app::register::Registration;
    use health_registries_app::review::{Registered, RegistryReview};
    let Some(settings) = server.registry.clone() else {
        return Err(FhirError::not_found("the client registry is not enabled"));
    };
    let Some(mrn) = settings.mrn.clone() else {
        return Err(FhirError::not_found(
            "registration is not enabled on this node",
        ));
    };
    let Some(Extension(subject)) = subject else {
        return Err(FhirError::new(
            StatusCode::UNAUTHORIZED,
            "login",
            "registration needs a signed-in user",
        ));
    };
    if !subject.permissions.allows("Patient", Decide::Create) {
        return Err(FhirError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "registration needs Patient create",
        ));
    }
    let facility: health_identifiers::ShortCode = subject.facility.parse().map_err(|_| {
        FhirError::bad_request("the session's facility code cannot be part of an MRN")
    })?;
    let parameters = parameters(&headers, &body)?;
    let mut patient = None;
    let mut confirmed_new = false;
    for parameter in &parameters {
        match parameter["name"].as_str().unwrap_or_default() {
            "resource" => patient = Some(parameter["resource"].clone()),
            "confirmedNew" => {
                confirmed_new = parameter["valueBoolean"]
                    .as_bool()
                    .ok_or_else(|| FhirError::bad_request("confirmedNew must be a boolean"))?;
            }
            other => {
                return Err(FhirError::not_supported(format!(
                    "$register does not support {other}"
                )));
            }
        }
    }
    let patient = patient
        .filter(|p| p["resourceType"] == "Patient")
        .ok_or_else(|| FhirError::bad_request("$register needs a Patient resource"))?;
    // Tiers 1 and 2, as for any write.
    let found = health_fhir_validation::validate_value(&patient);
    if !found.is_empty() {
        return Err(FhirError::invalid_resource(&found));
    }
    server.check_invariants(&patient).await?;
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
    let registry = ClientRegistry::new(server.store.clone(), audit, settings.config.clone());
    let review = RegistryReview::new(
        registry,
        settings.rules.clone(),
        Arc::new(health_time::SystemClock),
    );
    let year: i32 = Timestamp::now()
        .strftime("%Y")
        .to_string()
        .parse()
        .unwrap_or(1970);
    let registered = review
        .register_new(
            &patient,
            confirmed_new,
            &facility,
            &mrn,
            &subject.account,
            year,
        )
        .await?;
    let (status, output) = match registered {
        Registration::Candidates(mut matched) => {
            matched.candidates.retain(|c| {
                crate::access::may_list(&RawJson::from_validated(c.resource.to_string()))
            });
            let candidates = health_registries_app::matching::bundle(&matched);
            (
                StatusCode::OK,
                vec![json!({ "name": "candidates", "resource": candidates })],
            )
        }
        Registration::Registered { id, mrn, outcome } => {
            let (code, golden) = match outcome {
                Registered::AutoLinked { golden } => ("auto-linked", Some(golden)),
                Registered::NewIdentity { golden } => ("new-identity", Some(golden)),
                Registered::Queued { .. } => ("queued", None),
            };
            let mut output = vec![
                json!({ "name": "patient", "valueReference": reference(id) }),
                json!({ "name": "mrn", "valueString": mrn }),
                json!({ "name": "outcome", "valueCode": code }),
            ];
            if let Some(golden) = golden {
                output.push(json!({ "name": "golden", "valueReference": reference(golden) }));
            }
            (StatusCode::CREATED, output)
        }
    };
    let body = json!({ "resourceType": "Parameters", "parameter": output });
    Ok(fhir_json(status, body.to_string()))
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

/// The JSON shape of a trust level.
fn trust_code(trust: health_client_registry::Trust) -> &'static str {
    use health_client_registry::Trust;
    match trust {
        Trust::NiraVerified => "nira-verified",
        Trust::StaffVerified => "staff-verified",
        Trust::StaffEntered => "staff-entered",
        Trust::SelfReported => "self-reported",
    }
}

fn claim_json(claim: &health_client_registry::Claim) -> Value {
    json!({
        "value": claim.value,
        "trust": trust_code(claim.trust),
        "recorded": claim.recorded.to_string(),
        "source": format!("Patient/{}", claim.source),
    })
}

/// The registry review queue, for the review screen (#203). Nest it under the API (e.g.
/// `/api/registry/reviews`) behind the subject layer:
///
/// - `GET /`: pending items, the oldest first, with `overdue` past 7 days. A potential match carries
///   both Patients side by side; a survivorship item, the current and incoming values.
/// - `POST /{id}` with `{"decision": "same-person" | "different-people" | "keep-current" |
///   "take-incoming"}`: carried out, recorded and audited in one transaction.
///
/// Both need a reviewer: the `registry.review` permission and Patient update.
impl FhirServer {
    /// The review queue's routes (see above); empty answers 404 without the client registry.
    pub fn registry_review_router(&self) -> axum::Router {
        axum::Router::new()
            .route("/", axum::routing::get(review_queue))
            .route("/{id}", axum::routing::post(review_decision))
            .with_state(self.clone())
    }
}

type Review =
    health_registries_app::review::RegistryReview<health_surrealdb::SurrealStore, RegistryAudit>;

fn review_use_case(
    server: &FhirServer,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
) -> Result<(Subject, Review), FhirError> {
    let Some(settings) = server.registry.clone() else {
        return Err(FhirError::not_found("the client registry is not enabled"));
    };
    let (subject, registry) = reviewer(server, subject, agent)?;
    Ok((
        subject,
        health_registries_app::review::RegistryReview::new(
            registry,
            settings.rules.clone(),
            Arc::new(health_time::SystemClock),
        ),
    ))
}

/// `GET /` of the review queue.
async fn review_queue(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
) -> Result<Response, FhirError> {
    use health_client_registry::ReviewKind;
    let (_, review) = review_use_case(&server, subject, agent)?;
    let now = Timestamp::now();
    let pending = review.pending(500).await?;
    let mut tx = server.store.begin().await?;
    let patient_type = "Patient"
        .parse()
        .map_err(|_| FhirError::bad_request("Patient"))?;
    let mut items = Vec::with_capacity(pending.len());
    for entry in pending {
        let mut item = json!({
            "id": entry.id,
            "kind": entry.kind.code(),
            "created_at": entry.created_at.to_string(),
            "overdue": entry.overdue(now),
        });
        match &entry.kind {
            ReviewKind::PotentialMatch {
                source,
                candidate,
                golden,
                score,
            } => {
                let mut sides = Vec::with_capacity(2);
                for id in [source, candidate] {
                    let stored = match id.parse::<ResourceId>() {
                        Ok(id) => tx.read(&patient_type, id).await?,
                        Err(_) => None,
                    };
                    sides.push(
                        stored
                            .and_then(|s| serde_json::from_str::<Value>(s.json.as_str()).ok())
                            .unwrap_or(Value::Null),
                    );
                }
                let candidate_json = sides.pop().unwrap_or(Value::Null);
                let source_json = sides.pop().unwrap_or(Value::Null);
                item["golden"] = json!(format!("Patient/{golden}"));
                item["score"] = json!(score);
                item["source"] = source_json;
                item["candidate"] = candidate_json;
            }
            ReviewKind::Survivorship {
                golden,
                field,
                current,
                incoming,
            } => {
                item["golden"] = json!(format!("Patient/{golden}"));
                item["field"] = json!(field.code());
                item["current"] = claim_json(current);
                item["incoming"] = claim_json(incoming);
            }
        }
        items.push(item);
    }
    Ok(fhir_json(StatusCode::OK, Value::Array(items).to_string()))
}

/// `POST /{id}` of the review queue.
async fn review_decision(
    State(server): State<FhirServer>,
    Path(id): Path<String>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
    body: Bytes,
) -> Result<Response, FhirError> {
    let (subject, review) = review_use_case(&server, subject, agent)?;
    let body: Value = serde_json::from_slice(&body)
        .map_err(|_| FhirError::bad_request("the body is not JSON"))?;
    let decision = body["decision"]
        .as_str()
        .and_then(health_client_registry::Decision::from_code)
        .ok_or_else(|| {
            FhirError::bad_request(
                "decision must be same-person, different-people, keep-current or take-incoming",
            )
        })?;
    review.decide(&id, decision, &subject.account).await?;
    let mut response = Response::new(axum::body::Body::empty());
    *response.status_mut() = StatusCode::NO_CONTENT;
    Ok(response)
}
