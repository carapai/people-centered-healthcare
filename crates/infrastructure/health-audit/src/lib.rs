//! The audit module (resource-ownership.md rule 7; security.md; write-authority.md: AuditEvent is
//! append-only at its origin node and synced up).
//!
//! - **Only this module writes AuditEvent.** [`Auditor::append`] creates one inside the caller's
//!   transaction, so the audited write and its event commit together or not at all. Nothing
//!   updates or deletes them.
//! - **What is audited:** every write (create, update, delete), and every read of a record that
//!   carries a configured sensitivity label or that relied on break-glass.
//! - **The event** is an R5 AuditEvent: the `rest` category, the interaction, `action`, the
//!   outcome, the agent (who, purpose of use, break-glass), the observing node, the entities with
//!   their version and security labels, and the patient.
//! - **Provenance** is owned by its target's Owner and written in the Owner's transaction;
//!   [`provenance`] builds it.

use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    Precondition, RawJson, StoreError, Transaction, WriteError, WriteRequest,
};
use health_time::Timestamp;
use serde_json::{Value, json};

/// The resource type this module owns.
pub const AUDIT_EVENT: &str = "AuditEvent";

/// Who acted: from the verified token and the request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    /// A reference to the user (`Practitioner/…`, `PractitionerRole/…`) or the account, if known.
    pub who: Option<String>,
    /// How the agent is shown when there is no reference.
    pub display: String,
    /// The client application.
    pub client: Option<String>,
    /// The network address the request came from.
    pub network: Option<String>,
    /// Declared v3 PurposeOfUse codes.
    pub purpose: Vec<String>,
    /// Whether the access relied on break-glass (the PDP's obligation).
    pub break_glass: bool,
}

impl Agent {
    /// A request with no authenticated user (only while routes are not yet behind
    /// authorization; recorded so such access is visible).
    pub fn unauthenticated() -> Self {
        Self {
            who: None,
            display: "unauthenticated request".into(),
            client: None,
            network: None,
            purpose: Vec::new(),
            break_glass: false,
        }
    }

    /// A job the node runs itself (reindex, purge).
    pub fn system(job: &str) -> Self {
        Self {
            display: format!("system: {job}"),
            ..Self::unauthenticated()
        }
    }
}

/// The audited interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interaction {
    /// `create`.
    Create,
    /// `read`.
    Read,
    /// `vread`.
    Vread,
    /// `update`.
    Update,
    /// `delete`.
    Delete,
    /// `search-type`.
    Search,
    /// `history`.
    History,
}

impl Interaction {
    fn code(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Read => "read",
            Self::Vread => "vread",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Search => "search-type",
            Self::History => "history",
        }
    }

    /// The AuditEvent `action` (C, R, U, D, E).
    fn action(self) -> &'static str {
        match self {
            Self::Create => "C",
            Self::Read | Self::Vread | Self::History => "R",
            Self::Update => "U",
            Self::Delete => "D",
            Self::Search => "E",
        }
    }
}

/// One record the event is about.
#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    /// `Type/id`, or `Type/id/_history/version` when a version is known.
    pub what: String,
    /// The record's `meta.security` codings, copied so a reviewer sees why it was sensitive.
    pub labels: Vec<Value>,
}

impl Entity {
    /// The entity for a stored resource's JSON (its type, id, version and labels).
    pub fn of(json: &Value) -> Option<Self> {
        let resource_type = json["resourceType"].as_str()?;
        let id = json["id"].as_str()?;
        let what = match json["meta"]["versionId"].as_str() {
            Some(version) => format!("{resource_type}/{id}/_history/{version}"),
            None => format!("{resource_type}/{id}"),
        };
        let labels = json["meta"]["security"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Some(Self { what, labels })
    }
}

/// Builds and appends AuditEvents for one node.
#[derive(Debug, Clone)]
pub struct Auditor {
    observer: String,
    sensitive: Vec<(String, String)>,
}

impl Auditor {
    /// An auditor for the node shown as `observer`, treating records that carry any of
    /// `sensitive` (system, code) labels as sensitive.
    pub fn new(observer: impl Into<String>, sensitive: Vec<(String, String)>) -> Self {
        Self {
            observer: observer.into(),
            sensitive,
        }
    }

    /// Whether `json` carries a configured sensitivity label.
    pub fn is_sensitive(&self, json: &Value) -> bool {
        json["meta"]["security"].as_array().is_some_and(|labels| {
            labels.iter().any(|label| {
                self.sensitive.iter().any(|(system, code)| {
                    label["system"] == system.as_str() && label["code"] == code.as_str()
                })
            })
        })
    }

    /// Whether a read of `records` must be audited: any sensitive record, or break-glass.
    pub fn must_audit_read<'a>(
        &self,
        agent: &Agent,
        records: impl IntoIterator<Item = &'a Value>,
    ) -> bool {
        agent.break_glass || records.into_iter().any(|json| self.is_sensitive(json))
    }

    /// The R5 AuditEvent for `interaction` by `agent` on `entities`, at `recorded`.
    /// `failure` is the outcome's detail when the interaction failed.
    pub fn event(
        &self,
        agent: &Agent,
        interaction: Interaction,
        entities: &[Entity],
        patient: Option<&str>,
        recorded: Timestamp,
        failure: Option<&str>,
    ) -> Value {
        let mut who = json!({ "display": agent.display });
        if let Some(reference) = &agent.who {
            who["reference"] = json!(reference);
        }
        let mut requestor = json!({
            "type": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/provenance-participant-type", "code": "author" }] },
            "who": who,
            "requestor": true,
        });
        if let Some(network) = &agent.network {
            requestor["networkString"] = json!(network);
        }
        let mut agents = vec![requestor];
        if let Some(client) = &agent.client {
            agents.push(json!({ "who": { "display": client }, "requestor": false }));
        }
        let mut event = json!({
            "resourceType": AUDIT_EVENT,
            "category": [{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/audit-event-type", "code": "rest", "display": "RESTful Operation" }] }],
            "code": { "coding": [{ "system": "http://hl7.org/fhir/restful-interaction", "code": interaction.code() }] },
            "action": interaction.action(),
            "severity": if failure.is_some() { "warning" } else { "informational" },
            "recorded": recorded.to_string(),
            "outcome": {
                "code": { "system": "http://terminology.hl7.org/CodeSystem/audit-event-outcome", "code": if failure.is_some() { "4" } else { "0" } },
            },
            "agent": agents,
            "source": { "observer": { "display": self.observer } },
        });
        if let Some(detail) = failure {
            event["outcome"]["detail"] = json!([{ "text": detail }]);
        }
        let mut authorization: Vec<Value> = agent
            .purpose
            .iter()
            .map(|code| json!({ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/v3-ActReason", "code": code }] }))
            .collect();
        if agent.break_glass {
            // offline-auth.md § Break-glass: BTG in authorization, and the event labelled.
            authorization.push(json!({ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/v3-ActReason", "code": "BTG", "display": "break the glass" }] }));
            event["meta"] = json!({ "security": [{ "system": "http://terminology.hl7.org/CodeSystem/v3-ActReason", "code": "BTG" }] });
        }
        if !authorization.is_empty() {
            event["authorization"] = json!(authorization);
        }
        if let Some(patient) = patient {
            event["patient"] = json!({ "reference": patient });
        }
        if !entities.is_empty() {
            event["entity"] = json!(
                entities
                    .iter()
                    .map(|e| {
                        let mut entity = json!({ "what": { "reference": e.what } });
                        if !e.labels.is_empty() {
                            entity["securityLabel"] = json!(
                                e.labels
                                    .iter()
                                    .map(|l| json!({ "coding": [l] }))
                                    .collect::<Vec<_>>()
                            );
                        }
                        entity
                    })
                    .collect::<Vec<_>>()
            );
        }
        event
    }

    /// Appends `event` in `tx`: a new AuditEvent that can never be overwritten.
    pub async fn append<T: Transaction>(
        &self,
        tx: &mut T,
        event: &Value,
    ) -> Result<ResourceId, WriteError> {
        let id = ResourceId::new();
        let resource_type = AUDIT_EVENT.parse().map_err(|_| {
            WriteError::Store(StoreError::Unavailable(
                "AuditEvent is not a resource type".into(),
            ))
        })?;
        tx.write(WriteRequest {
            resource_type,
            id,
            json: RawJson::from_validated(event.to_string()),
            op_id: OpId::new(),
            precondition: Precondition::MustNotExist,
        })
        .await?;
        Ok(id)
    }
}

/// A Provenance for a derived write (extraction, identity survivorship, pricing versions), to be
/// written by the target's Owner in the same transaction as the target (resource-ownership.md
/// rule 7). `targets` and `sources` are `Type/id/_history/version` references; `activity` is a
/// v3-DataOperation code (`CREATE`, `UPDATE`, …).
pub fn provenance(
    targets: &[String],
    sources: &[String],
    agent: &Agent,
    activity: &str,
    recorded: Timestamp,
) -> Value {
    let mut who = json!({ "display": agent.display });
    if let Some(reference) = &agent.who {
        who["reference"] = json!(reference);
    }
    let mut provenance = json!({
        "resourceType": "Provenance",
        "target": targets.iter().map(|t| json!({ "reference": t })).collect::<Vec<_>>(),
        "recorded": recorded.to_string(),
        "activity": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/v3-DataOperation", "code": activity }] },
        "agent": [{ "who": who }],
    });
    if !sources.is_empty() {
        provenance["entity"] = json!(
            sources
                .iter()
                .map(|s| json!({ "role": "source", "what": { "reference": s } }))
                .collect::<Vec<_>>()
        );
    }
    provenance
}

#[cfg(test)]
mod tests {
    use health_fhir_validation::validate_structure;

    use super::*;

    const ACT: &str = "http://terminology.hl7.org/CodeSystem/v3-ActCode";

    fn auditor() -> Auditor {
        Auditor::new("edge KAM013-E1", vec![(ACT.into(), "HIV".into())])
    }

    fn now() -> Timestamp {
        Timestamp::from_second(1_800_000_000).unwrap_or(Timestamp::UNIX_EPOCH)
    }

    #[test]
    fn events_are_valid_r5_and_carry_who_what_and_why() {
        let record = json!({ "resourceType": "Observation", "id": "o1", "meta": { "versionId": "v1", "security": [{ "system": ACT, "code": "HIV" }] } });
        let entity = Entity::of(&record).expect("entity");
        assert_eq!(entity.what, "Observation/o1/_history/v1");
        let agent = Agent {
            who: Some("Practitioner/p9".into()),
            display: "Dr Nakato".into(),
            client: Some("health-client".into()),
            network: Some("10.0.0.7".into()),
            purpose: vec!["TREAT".into()],
            break_glass: true,
        };
        let event = auditor().event(
            &agent,
            Interaction::Read,
            &[entity],
            Some("Patient/x1"),
            now(),
            None,
        );
        assert!(
            validate_structure(&event.to_string()).is_empty(),
            "{:?}",
            validate_structure(&event.to_string())
        );
        assert_eq!(event["action"], "R");
        assert_eq!(event["agent"][0]["who"]["reference"], "Practitioner/p9");
        assert_eq!(event["agent"][0]["networkString"], "10.0.0.7");
        assert_eq!(event["authorization"][1]["coding"][0]["code"], "BTG");
        assert_eq!(event["meta"]["security"][0]["code"], "BTG");
        assert_eq!(
            event["entity"][0]["securityLabel"][0]["coding"][0]["code"],
            "HIV"
        );
        assert_eq!(event["patient"]["reference"], "Patient/x1");

        let failed = auditor().event(
            &Agent::unauthenticated(),
            Interaction::Delete,
            &[],
            None,
            now(),
            Some("409 referenced"),
        );
        assert!(validate_structure(&failed.to_string()).is_empty());
        assert_eq!(
            (
                failed["outcome"]["code"]["code"].clone(),
                failed["severity"].clone()
            ),
            (json!("4"), json!("warning"))
        );
        assert!(failed.get("authorization").is_none());
    }

    #[test]
    fn only_sensitive_or_break_glass_reads_are_audited() {
        let auditor = auditor();
        let plain = json!({ "resourceType": "Observation", "id": "o1" });
        let hiv = json!({ "resourceType": "Observation", "id": "o2", "meta": { "security": [{ "system": ACT, "code": "HIV" }] } });
        let other = json!({ "resourceType": "Observation", "id": "o3", "meta": { "security": [{ "system": ACT, "code": "NOTSENSITIVE" }] } });
        let agent = Agent::system("test");
        assert!(!auditor.must_audit_read(&agent, [&plain, &other]));
        assert!(auditor.must_audit_read(&agent, [&plain, &hiv]));
        assert!(auditor.must_audit_read(
            &Agent {
                break_glass: true,
                ..agent
            },
            [&plain]
        ));
    }

    #[test]
    fn provenance_is_valid_r5() {
        let p = provenance(
            &["Observation/o1/_history/v1".into()],
            &["QuestionnaireResponse/q1/_history/v3".into()],
            &Agent::system("extraction"),
            "CREATE",
            now(),
        );
        assert!(
            validate_structure(&p.to_string()).is_empty(),
            "{:?}",
            validate_structure(&p.to_string())
        );
        assert_eq!(p["entity"][0]["role"], "source");
    }
}
