//! Inbound integrations (ADR 0051; fhir-r5-model.md § Validation): integrations, pulled feeds and
//! R4 adapters after conversion. No external payload that fails validation is lost:
//!
//! - **Tier 1 fails** (not JSON, or not structurally R5): the payload is refused to the caller
//!   with an OperationOutcome and kept, exactly as received, in the **Inbound Dead Letter**.
//! - **Tiers 2–3 fail**, or the store refuses the write (an unresolved reference, a conflict):
//!   the resource goes to the **quarantine** review queue instead of the record.
//! - Otherwise it is written through the [`Sink`].
//!
//! Reviewers list pending entries, [`Inbound::replay`] one after a fix (it is validated again
//! from tier 1 and written), or [`Inbound::discard`] it. Every receipt, listing, replay, discard
//! and purge is audited in the same transaction. Entries are purged when their integration's
//! retention ends (90 days by default). At rest they are protected like the rest of the node's
//! data: full-disk encryption and encrypted backups (security.md).

use std::future::Future;
use std::sync::Arc;

use health_audit::{Agent, Auditor, Entity, Interaction};
use health_fhir_validation::{
    FhirPathEngine, Issue, ProfileValidator, validate_invariants, validate_structure,
};
use health_persistence::{Store, StoreError, Transaction, WriteError};
use health_surrealdb::{InboundEntry, InboundKind, InboundStatus, SurrealStore, SurrealTx};
use health_time::{Clock, SignedDuration, Timestamp};
use serde_json::{Value, json};

/// One external source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Integration {
    /// Its id, e.g. `dhis2-tracker` or `lab-analyser-1`.
    pub id: String,
    /// How long its kept payloads are retained.
    pub retention: SignedDuration,
}

impl Integration {
    /// An integration with the default 90-day retention.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            retention: SignedDuration::from_hours(90 * 24),
        }
    }
}

/// Why the sink did not write a resource.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SinkError {
    /// The store refused this resource (4xx): it is quarantined with these issues.
    #[error("the store refused the resource")]
    Refused(Vec<String>),
    /// The store is unavailable (5xx): nothing is kept, and the caller retries.
    #[error("the store is unavailable: {0}")]
    Unavailable(String),
}

/// Where validated resources go: the node's write path (its FHIR API or use cases).
pub trait Sink: Send + Sync {
    /// Writes `resource`, which passed tiers 1–3.
    fn write(&self, resource: &Value) -> impl Future<Output = Result<(), SinkError>> + Send;
}

/// What happened to a payload.
#[derive(Debug, Clone, PartialEq)]
pub enum Received {
    /// Validated and written.
    Accepted,
    /// Refused to the caller (this OperationOutcome) and kept in the dead letter.
    DeadLettered {
        /// The entry id.
        id: String,
        /// The answer for the caller.
        outcome: Value,
    },
    /// Kept in quarantine for review.
    Quarantined {
        /// The entry id.
        id: String,
        /// The problems, as an OperationOutcome.
        outcome: Value,
    },
}

/// What a replay did.
#[derive(Debug, Clone, PartialEq)]
pub enum Replayed {
    /// It passed and was written; the entry is resolved.
    Written,
    /// It still fails; the entry stays pending with these problems.
    StillFailing(Value),
}

/// An inbound failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InboundError {
    /// The store or sink is unavailable; retry.
    #[error("unavailable: {0}")]
    Unavailable(String),
    /// No pending entry with that id.
    #[error("no pending inbound entry {0}")]
    NotPending(String),
}

impl From<StoreError> for InboundError {
    fn from(error: StoreError) -> Self {
        Self::Unavailable(error.to_string())
    }
}

impl From<WriteError> for InboundError {
    fn from(error: WriteError) -> Self {
        Self::Unavailable(error.to_string())
    }
}

/// A validation stage's problems.
enum Stage {
    Structure(Vec<String>),
    Content(Vec<String>),
}

fn lines(issues: &[Issue]) -> Vec<String> {
    issues
        .iter()
        .map(|i| {
            format!(
                "{}: {}: {}",
                i.location,
                i.code.as_str(),
                i.message.replace('\n', " ")
            )
        })
        .collect()
}

fn outcome(code: &str, issues: &[String]) -> Value {
    json!({
        "resourceType": "OperationOutcome",
        "issue": issues.iter().map(|i| json!({ "severity": "error", "code": code, "diagnostics": i })).collect::<Vec<_>>(),
    })
}

fn entity(id: &str) -> Entity {
    Entity {
        what: format!("urn:health:inbound:{id}"),
        labels: Vec::new(),
    }
}

/// The inbound pipeline for one node.
pub struct Inbound<S> {
    store: SurrealStore,
    auditor: Auditor,
    engine: Arc<dyn FhirPathEngine>,
    profiles: Option<Arc<dyn ProfileValidator>>,
    sink: S,
    clock: Arc<dyn Clock>,
}

impl<S: Sink> Inbound<S> {
    /// A pipeline that writes through `sink`, keeping failures in `store` and auditing with
    /// `auditor`.
    pub fn new(
        store: SurrealStore,
        auditor: Auditor,
        engine: Arc<dyn FhirPathEngine>,
        sink: S,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            store,
            auditor,
            engine,
            profiles: None,
            sink,
            clock,
        }
    }

    /// Also checks tier 3 against the profiles a resource claims.
    #[must_use]
    pub fn with_profiles(mut self, profiles: Arc<dyn ProfileValidator>) -> Self {
        self.profiles = Some(profiles);
        self
    }

    /// Tiers 1–3: the parsed resource, or the stage that failed.
    async fn validate(&self, payload: &str) -> Result<Value, Stage> {
        let structure = validate_structure(payload);
        if !structure.is_empty() {
            return Err(Stage::Structure(lines(&structure)));
        }
        let resource: Value = serde_json::from_str(payload)
            .map_err(|e| Stage::Structure(vec![format!(": structure: {e}")]))?;
        let invariants = validate_invariants(self.engine.as_ref(), &resource).await;
        if !invariants.is_empty() {
            return Err(Stage::Content(lines(&invariants)));
        }
        if let Some(validator) = &self.profiles {
            let claimed: Vec<String> = resource["meta"]["profile"]
                .as_array()
                .map(|p| {
                    p.iter()
                        .filter_map(|u| u.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            if !claimed.is_empty() {
                let found = validator.validate(&resource, &claimed).await;
                if !found.is_empty() {
                    return Err(Stage::Content(lines(&found)));
                }
            }
        }
        Ok(resource)
    }

    /// Validates and writes one payload from `integration`, keeping it when it fails.
    pub async fn receive(
        &self,
        integration: &Integration,
        payload: &str,
    ) -> Result<Received, InboundError> {
        let agent = Agent::system(&format!("integration {}", integration.id));
        let (kind, issues, code) = match self.validate(payload).await {
            Err(Stage::Structure(issues)) => (InboundKind::DeadLetter, issues, "structure"),
            Err(Stage::Content(issues)) => (InboundKind::Quarantine, issues, "invariant"),
            Ok(resource) => match self.sink.write(&resource).await {
                Ok(()) => return Ok(Received::Accepted),
                Err(SinkError::Refused(issues)) => (InboundKind::Quarantine, issues, "processing"),
                Err(SinkError::Unavailable(reason)) => {
                    return Err(InboundError::Unavailable(reason));
                }
            },
        };
        let now = self.clock.now();
        let id = uuid::Uuid::now_v7().to_string();
        let entry = InboundEntry {
            id: id.clone(),
            kind,
            integration: integration.id.clone(),
            received_at: now,
            retain_until: now.checked_add(integration.retention).unwrap_or(now),
            payload: payload.to_owned(),
            issues: issues.clone(),
            status: InboundStatus::Pending,
            resolved_at: None,
            note: None,
        };
        let mut tx = self.store.begin().await?;
        tx.put_inbound(&entry).await?;
        self.audit(&mut tx, &agent, Interaction::Create, &[entity(&id)], now)
            .await?;
        tx.commit().await?;
        let outcome = outcome(code, &issues);
        Ok(match kind {
            InboundKind::DeadLetter => Received::DeadLettered { id, outcome },
            InboundKind::Quarantine => Received::Quarantined { id, outcome },
        })
    }

    async fn audit(
        &self,
        tx: &mut SurrealTx,
        agent: &Agent,
        interaction: Interaction,
        entities: &[Entity],
        now: Timestamp,
    ) -> Result<(), InboundError> {
        let event = self
            .auditor
            .event(agent, interaction, entities, None, now, None);
        self.auditor.append(tx, &event).await?;
        Ok(())
    }

    /// Pending entries of `kind`, oldest first, for a reviewer; the listing is audited.
    pub async fn pending(
        &self,
        kind: InboundKind,
        reviewer: &Agent,
        limit: usize,
    ) -> Result<Vec<InboundEntry>, InboundError> {
        let mut tx = self.store.begin().await?;
        let entries = tx.inbound_pending(kind, limit).await?;
        let named: Vec<Entity> = entries.iter().map(|e| entity(&e.id)).collect();
        self.audit(
            &mut tx,
            reviewer,
            Interaction::Search,
            &named,
            self.clock.now(),
        )
        .await?;
        tx.commit().await?;
        Ok(entries)
    }

    /// Replays a pending entry: its payload, or `corrected` if the reviewer fixed it, is
    /// validated again from tier 1 and written. On success the entry is resolved; otherwise it
    /// stays pending.
    pub async fn replay(
        &self,
        id: &str,
        corrected: Option<&str>,
        reviewer: &Agent,
    ) -> Result<Replayed, InboundError> {
        let mut tx = self.store.begin().await?;
        let entry = tx
            .inbound(id)
            .await?
            .filter(|e| e.status == InboundStatus::Pending)
            .ok_or_else(|| InboundError::NotPending(id.to_owned()))?;
        let payload = corrected.unwrap_or(&entry.payload);
        let failing = match self.validate(payload).await {
            Err(Stage::Structure(issues)) => Some(outcome("structure", &issues)),
            Err(Stage::Content(issues)) => Some(outcome("invariant", &issues)),
            Ok(resource) => match self.sink.write(&resource).await {
                Ok(()) => None,
                Err(SinkError::Refused(issues)) => Some(outcome("processing", &issues)),
                Err(SinkError::Unavailable(reason)) => {
                    return Err(InboundError::Unavailable(reason));
                }
            },
        };
        let now = self.clock.now();
        let result = match failing {
            Some(problems) => {
                self.audit(&mut tx, reviewer, Interaction::Read, &[entity(id)], now)
                    .await?;
                Replayed::StillFailing(problems)
            }
            None => {
                let note = corrected.map(|_| "replayed with a correction".to_owned());
                if !tx
                    .resolve_inbound(id, InboundStatus::Replayed, now, note)
                    .await?
                {
                    return Err(InboundError::NotPending(id.to_owned()));
                }
                self.audit(&mut tx, reviewer, Interaction::Update, &[entity(id)], now)
                    .await?;
                Replayed::Written
            }
        };
        tx.commit().await?;
        Ok(result)
    }

    /// Discards a pending entry with the reviewer's reason. It is kept, resolved, until its
    /// retention ends.
    pub async fn discard(
        &self,
        id: &str,
        reason: &str,
        reviewer: &Agent,
    ) -> Result<(), InboundError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        if !tx
            .resolve_inbound(id, InboundStatus::Discarded, now, Some(reason.to_owned()))
            .await?
        {
            return Err(InboundError::NotPending(id.to_owned()));
        }
        self.audit(&mut tx, reviewer, Interaction::Update, &[entity(id)], now)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Deletes the entries whose retention has ended (a background job), auditing the purge.
    pub async fn purge(&self) -> Result<Vec<String>, InboundError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let purged = tx.purge_inbound(now).await?;
        if !purged.is_empty() {
            let named: Vec<Entity> = purged.iter().map(|id| entity(id)).collect();
            self.audit(
                &mut tx,
                &Agent::system("inbound retention"),
                Interaction::Delete,
                &named,
                now,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(purged)
    }
}
