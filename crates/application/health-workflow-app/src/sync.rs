//! Workflow transitions made away from the instance's authority (#100; workflow-engine.md §
//! Offline and sync; ADR 0020).
//!
//! A desktop in offline fallback runs the engine for the instances in its Sync Scope. Each
//! transition it makes there is a [`Candidate`]: the event, with the instance version and the
//! configuration it was made from. The authority judges it with [`Workflows::apply_candidate`]:
//!
//! - **Its guards are re-evaluated** there: the event is applied through the engine, with the
//!   authority's own guards, as a command would be. One no longer enabled is a conflict.
//! - **It applies** when the instance has not moved since its base, or when it has moved but
//!   every state the candidate's transition left is still active: the change made meanwhile was
//!   elsewhere, such as in another parallel region, so the two commute.
//! - **Anything else is a conflict:** the instance left a state the candidate started from, or
//!   it has ended. The caller opens a Conflict Record (ADR 0020), and nothing is applied.
//!
//! A candidate is applied once: a replay of its `op_id` does nothing.
//!
//! On a replica, [`Workflows::command`] queues each move as a candidate, which sync carries up as
//! an operation. At the authority, sync keeps arriving candidates, and
//! [`Workflows::apply_candidates`] judges them in order, opening a Conflict Record for each that
//! does not apply.

use health_persistence::{Store, Transaction};
use health_workflow::{Event, Guards, InstanceRecord, WorkflowStore};
use serde::{Deserialize, Serialize};

use crate::{Actions, Definitions, WorkflowError, Workflows};

/// A transition made away from the instance's authority, for it to judge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// The operation id: a candidate is applied once.
    pub op_id: String,
    /// The instance.
    pub instance: String,
    /// The instance version it was made from.
    pub base_version: u64,
    /// The configuration it was made from.
    pub base_configuration: Vec<String>,
    /// The states its transition left there: those active before it and not after.
    pub left: Vec<String>,
    /// The event.
    pub event: Event,
    /// Who made it.
    pub actor: String,
    /// Where: the node that made it.
    pub node: String,
}

impl Candidate {
    /// The candidate for `event`, which moved the replica from `before` to `after`, by `actor`
    /// at `node`.
    pub fn of(
        before: &InstanceRecord,
        after: &InstanceRecord,
        event: Event,
        actor: impl Into<String>,
        op_id: impl Into<String>,
        node: impl Into<String>,
    ) -> Self {
        Self {
            op_id: op_id.into(),
            instance: before.id.clone(),
            base_version: before.version,
            base_configuration: before.state.configuration.clone(),
            left: before
                .state
                .configuration
                .iter()
                .filter(|state| !after.state.configuration.contains(state))
                .cloned()
                .collect(),
            event,
            actor: actor.into(),
            node: node.into(),
        }
    }
}

/// What the authority made of a candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Judged {
    /// Applied: the instance as it now stands.
    Applied(InstanceRecord),
    /// Applied before: a replay.
    Replayed,
    /// Not applied: why, for the Conflict Record.
    Conflict(String),
}

impl<S, D, A, G> Workflows<S, D, A, G>
where
    S: Store,
    S::Tx: WorkflowStore,
    D: Definitions,
    A: Actions<S::Tx>,
    G: Guards + Send + Sync,
{
    /// Judges `candidate` at the instance's authority, in one transaction with its actions.
    pub async fn apply_candidate(&self, candidate: &Candidate) -> Result<Judged, WorkflowError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        if !tx
            .claim_event(&candidate_key(&candidate.op_id), now)
            .await?
        {
            return Ok(Judged::Replayed);
        }
        let Some(record) = tx.instance(&candidate.instance).await? else {
            return Ok(Judged::Conflict(format!(
                "instance {} is not held here",
                candidate.instance
            )));
        };
        if !record.state.running {
            return Ok(Judged::Conflict(format!(
                "instance {} has ended",
                candidate.instance
            )));
        }
        if let Some(why) = commutes(&record, candidate) {
            return Ok(Judged::Conflict(why));
        }
        let Some(next) = self
            .advance(
                &mut tx,
                record,
                &candidate.event,
                &candidate.actor,
                &candidate.op_id,
                now,
            )
            .await?
        else {
            return Ok(Judged::Conflict(format!(
                "{} is not enabled at the authority",
                candidate.event.name
            )));
        };
        tx.commit().await?;
        Ok(Judged::Applied(next))
    }
}

impl<S, D, A, G> Workflows<S, D, A, G>
where
    S: Store,
    S::Tx: WorkflowStore,
    D: Definitions,
    A: Actions<S::Tx>,
    G: Guards + Send + Sync,
{
    /// Judges up to `limit` candidates received from other nodes, oldest first: each is applied,
    /// or settled as a conflict with its Conflict Record. A candidate that cannot be read is a
    /// conflict too. Returns each one's operation id and verdict.
    pub async fn apply_candidates(
        &self,
        limit: u32,
    ) -> Result<Vec<(String, Judged)>, WorkflowError> {
        let pending = {
            let mut tx = self.store.begin().await?;
            tx.pending_candidates(limit).await?
        };
        let mut judged = Vec::new();
        for (op_id, json) in pending {
            let verdict = match serde_json::from_str::<Candidate>(&json) {
                Ok(candidate) => self.apply_candidate(&candidate).await?,
                Err(e) => Judged::Conflict(format!("the candidate cannot be read: {e}")),
            };
            let mut tx = self.store.begin().await?;
            let conflict = match &verdict {
                Judged::Conflict(why) => Some(why.as_str()),
                Judged::Applied(_) | Judged::Replayed => None,
            };
            tx.settle_candidate(&op_id, conflict).await?;
            tx.commit().await?;
            judged.push((op_id, verdict));
        }
        Ok(judged)
    }
}

/// Why `candidate` does not commute with `record`, the instance as the authority holds it, if
/// it does not: it moved since the candidate's base and is no longer in a state the candidate's
/// transition left.
pub fn commutes(record: &InstanceRecord, candidate: &Candidate) -> Option<String> {
    if record.version == candidate.base_version {
        return None;
    }
    let left: Vec<&String> = candidate
        .left
        .iter()
        .filter(|state| !record.state.configuration.contains(state))
        .collect();
    if left.is_empty() {
        None
    } else {
        Some(format!(
            "the instance moved on from version {} to {} and is no longer in {}",
            candidate.base_version,
            record.version,
            left.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

pub(crate) fn candidate_key(op_id: &str) -> String {
    format!("candidate|{op_id}")
}
