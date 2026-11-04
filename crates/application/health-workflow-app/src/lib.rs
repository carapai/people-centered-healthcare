//! The workflow application (workflow-engine.md § Persistence and transactions; ADRs 0043, 0044):
//! runs workflow instances through the pure interpreter and persists every step.
//!
//! - **User commands** ([`Workflows::start`], [`Workflows::command`], [`Workflows::cancel`]) run
//!   the transition, its typed actions, its timers and its history entry in **one transaction**.
//!   If an action fails, nothing is kept.
//! - **Domain events** from other modules ([`Workflows::deliver`]) are consumed after their own
//!   commit, through the inbox, one transaction each. The inbox is keyed by event id, so a
//!   replayed event is a no-op; commands are keyed by their `op_id` the same way.
//! - **History** is appended once per transition at `[instance, version]`, and each change is a
//!   compare-and-set on the instance version, so two concurrent steps cannot both commit.
//! - **Actions** are carried out through the [`Actions`] port, inside the transaction, keyed by
//!   the UUIDv5 of instance, microstep and index, so the target context can make them
//!   idempotent.
//! - **Compensation.** Cancelling an instance runs, in reverse, the compensations of the actions
//!   it completed: a compensating action through the port, or, for actions undone by a person or
//!   not at all, a note on the cancellation's history entry (`human-task.N`, `not-undone.N`).
//!
//! Sub-workflow invocations are refused until the engine supports them, so a definition that
//! uses one fails closed. Firing timers is #98; this records them.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use health_persistence::{Store, StoreError, Transaction, WriteError};
use health_time::{Clock, Timestamp};
use health_workflow::{
    DefinitionError, Effect, Event, Guards, HistoryEntry, InstanceRecord, Machine, TimerRecord,
    TimerStatus, Value, WorkflowStore,
};
use health_workflow_actions::{ActionDescriptor, Compensation};
use uuid::Uuid;

/// Who acts for domain events and timers: the workflow principal, whose permissions are
/// approved with the definition (ADR 0043).
pub const PRINCIPAL: &str = "workflow-principal";

/// The name of the event a cancellation records.
pub const CANCELLED: &str = "workflow.cancelled";

/// The name of the event a start records.
pub const STARTED: &str = "workflow.started";

/// The namespace of the engine's UUIDv5s: instance ids and action keys.
const NAMESPACE: Uuid = Uuid::from_u128(0x6f1c_2d0e_9b7a_4c51_8e3f_0a4d_5b6c_7e8f);

/// The compiled definitions this node runs, by `url|version`.
pub trait Definitions: Send + Sync {
    /// The definition, if this node has it.
    fn machine(&self, definition: &str) -> Option<Arc<Machine>>;
}

impl Definitions for BTreeMap<String, Arc<Machine>> {
    fn machine(&self, definition: &str) -> Option<Arc<Machine>> {
        self.get(definition).cloned()
    }
}

/// One typed action to carry out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionCall {
    /// The instance.
    pub instance: String,
    /// The action, `<id>@<major>`.
    pub action: String,
    /// Its parameters.
    pub params: BTreeMap<String, Value>,
    /// Its idempotency key: the same call always has the same key.
    pub key: Uuid,
    /// Who it is done for: the user, or [`PRINCIPAL`].
    pub actor: String,
}

/// An action that could not be carried out.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ActionFailed(pub String);

/// Carries out typed actions in the workflow's transaction: each Bounded Context's use cases,
/// behind the actions its contract crate declares.
pub trait Actions<Tx>: Send + Sync {
    /// Carries out `call`, writing through `tx`.
    fn execute(
        &self,
        tx: &mut Tx,
        call: &ActionCall,
    ) -> impl Future<Output = Result<(), ActionFailed>> + Send;
}

/// What a command or event did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It ran: the instance as it now is.
    Applied(InstanceRecord),
    /// Its `op_id` or event id was already processed: nothing was done.
    Replayed,
    /// A domain event for an instance that has ended, or that enabled no transition: consumed,
    /// and nothing else done.
    Ignored,
}

/// Why a workflow command failed. Nothing was kept.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkflowError {
    /// This node does not have the definition.
    #[error("the definition {0} is not active here")]
    UnknownDefinition(String),
    /// No such instance.
    #[error("workflow instance {0} is not known")]
    NotFound(String),
    /// The instance has ended.
    #[error("workflow instance {0} has ended")]
    Ended(String),
    /// The command enabled no transition from where the instance is.
    #[error("{0} is not possible from the instance's current state")]
    NotEnabled(String),
    /// The stored instance does not fit its definition.
    #[error(transparent)]
    Definition(#[from] DefinitionError),
    /// An action this node does not know.
    #[error("unknown action {0}")]
    UnknownAction(String),
    /// An action failed.
    #[error("{action} failed: {reason}")]
    Action {
        /// The action.
        action: String,
        /// Why.
        reason: String,
    },
    /// Something the engine does not do yet.
    #[error("{0} are not supported yet")]
    Unsupported(&'static str),
    /// Another change to the instance came first; try again.
    #[error("the workflow instance changed meanwhile; try again")]
    Conflict,
    /// The store failed.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for WorkflowError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for WorkflowError {
    fn from(error: WriteError) -> Self {
        Self::Store(error.to_string())
    }
}

/// The idempotency key of action `index` of microstep `sequence` of `instance`.
pub fn action_key(instance: &str, sequence: u64, index: u32) -> Uuid {
    Uuid::new_v5(
        &NAMESPACE,
        format!("action|{instance}|{sequence}|{index}").as_bytes(),
    )
}

/// The id of the instance started by command `op_id`.
pub fn instance_id(op_id: &str) -> String {
    Uuid::new_v5(&NAMESPACE, format!("instance|{op_id}").as_bytes()).to_string()
}

fn command_key(op_id: &str) -> String {
    format!("command|{op_id}")
}

fn event_key(event_id: &str) -> String {
    format!("event|{event_id}")
}

/// The workflow use cases at this node.
pub struct Workflows<S, D, A, G> {
    store: S,
    definitions: D,
    actions: A,
    guards: G,
    clock: Arc<dyn Clock>,
    catalog: Vec<ActionDescriptor>,
    authority: String,
}

impl<S, D, A, G> Workflows<S, D, A, G>
where
    S: Store,
    S::Tx: WorkflowStore,
    D: Definitions,
    A: Actions<S::Tx>,
    G: Guards + Send + Sync,
{
    /// The use cases at node `authority`, carrying out the actions of `catalog`.
    pub fn new(
        store: S,
        definitions: D,
        actions: A,
        guards: G,
        clock: Arc<dyn Clock>,
        catalog: Vec<ActionDescriptor>,
        authority: &str,
    ) -> Self {
        Self {
            store,
            definitions,
            actions,
            guards,
            clock,
            catalog,
            authority: authority.to_owned(),
        }
    }

    fn machine(&self, definition: &str) -> Result<Arc<Machine>, WorkflowError> {
        self.definitions
            .machine(definition)
            .ok_or_else(|| WorkflowError::UnknownDefinition(definition.to_owned()))
    }

    fn descriptor(&self, reference: &str) -> Result<&ActionDescriptor, WorkflowError> {
        self.catalog
            .iter()
            .find(|d| d.reference() == reference)
            .ok_or_else(|| WorkflowError::UnknownAction(reference.to_owned()))
    }

    /// Starts an instance of `definition` about `context`. Replaying `op_id` does nothing.
    pub async fn start(
        &self,
        definition: &str,
        context: &str,
        actor: &str,
        op_id: &str,
    ) -> Result<Outcome, WorkflowError> {
        let machine = self.machine(definition)?;
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        if !tx.claim_event(&command_key(op_id), now).await? {
            return Ok(Outcome::Replayed);
        }
        let (state, effects) = machine.start(&self.guards);
        let record = InstanceRecord {
            id: instance_id(op_id),
            definition: definition.to_owned(),
            context: context.to_owned(),
            authority: self.authority.clone(),
            state,
            version: 1,
        };
        self.carry_out(&mut tx, &record, &effects, actor, now)
            .await?;
        tx.create_instance(&record).await?;
        tx.append_history(&HistoryEntry {
            instance: record.id.clone(),
            seq: 1,
            event: Event::named(STARTED),
            from: Vec::new(),
            to: record.state.configuration.clone(),
            effects,
            actor: actor.to_owned(),
            at: now,
            op_id: op_id.to_owned(),
        })
        .await?;
        tx.commit().await?;
        Ok(Outcome::Applied(record))
    }

    /// A user's command: `event` on `instance`, in one transaction with its actions. Replaying
    /// `op_id` does nothing.
    pub async fn command(
        &self,
        instance: &str,
        event: &Event,
        actor: &str,
        op_id: &str,
    ) -> Result<Outcome, WorkflowError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        if !tx.claim_event(&command_key(op_id), now).await? {
            return Ok(Outcome::Replayed);
        }
        let record = tx
            .instance(instance)
            .await?
            .ok_or_else(|| WorkflowError::NotFound(instance.to_owned()))?;
        if !record.state.running {
            return Err(WorkflowError::Ended(instance.to_owned()));
        }
        let Some(next) = self
            .advance(&mut tx, record, event, actor, op_id, now)
            .await?
        else {
            return Err(WorkflowError::NotEnabled(event.name.clone()));
        };
        tx.commit().await?;
        Ok(Outcome::Applied(next))
    }

    /// A domain event from another module, consumed after its commit: delivered once per
    /// `event_id`, acting as the workflow principal.
    pub async fn deliver(
        &self,
        event_id: &str,
        instance: &str,
        event: &Event,
    ) -> Result<Outcome, WorkflowError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        if !tx.claim_event(&event_key(event_id), now).await? {
            return Ok(Outcome::Replayed);
        }
        let record = tx
            .instance(instance)
            .await?
            .ok_or_else(|| WorkflowError::NotFound(instance.to_owned()))?;
        let outcome = if record.state.running {
            self.advance(&mut tx, record, event, PRINCIPAL, event_id, now)
                .await?
                .map_or(Outcome::Ignored, Outcome::Applied)
        } else {
            Outcome::Ignored
        };
        tx.commit().await?;
        Ok(outcome)
    }

    /// Cancels `instance`: compensates its completed actions in reverse, cancels its timers and
    /// ends it, recording `reason`. Replaying `op_id` does nothing.
    pub async fn cancel(
        &self,
        instance: &str,
        reason: &str,
        actor: &str,
        op_id: &str,
    ) -> Result<Outcome, WorkflowError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        if !tx.claim_event(&command_key(op_id), now).await? {
            return Ok(Outcome::Replayed);
        }
        let record = tx
            .instance(instance)
            .await?
            .ok_or_else(|| WorkflowError::NotFound(instance.to_owned()))?;
        if !record.state.running {
            return Err(WorkflowError::Ended(instance.to_owned()));
        }

        let mut next = InstanceRecord {
            version: record.version + 1,
            ..record.clone()
        };
        next.state.sequence += 1;
        next.state.configuration.clear();
        next.state.running = false;
        let mut event = Event::named(CANCELLED);
        event
            .data
            .insert("reason".to_owned(), Value::Text(reason.to_owned()));
        let mut effects = Vec::new();

        let completed: Vec<(String, BTreeMap<String, Value>)> = tx
            .instance_history(instance)
            .await?
            .into_iter()
            .flat_map(|entry| entry.effects)
            .filter_map(|effect| match effect {
                Effect::Execute { action, params, .. } => Some((action, params)),
                _ => None,
            })
            .collect();
        let (mut human, mut none, mut index) = (0, 0, 0_u32);
        for (action, params) in completed.into_iter().rev() {
            let descriptor = self.descriptor(&action)?;
            match descriptor.compensation {
                Compensation::Action(id) => {
                    let undo = self
                        .catalog
                        .iter()
                        .filter(|d| d.id == id)
                        .max_by_key(|d| (d.major == descriptor.major, d.major))
                        .ok_or_else(|| WorkflowError::UnknownAction(id.to_owned()))?
                        .reference();
                    let call = ActionCall {
                        instance: instance.to_owned(),
                        action: undo.clone(),
                        params: params.clone(),
                        key: action_key(instance, next.state.sequence, index),
                        actor: actor.to_owned(),
                    };
                    self.actions.execute(&mut tx, &call).await.map_err(|e| {
                        WorkflowError::Action {
                            action: undo.clone(),
                            reason: e.0,
                        }
                    })?;
                    effects.push(Effect::Execute {
                        action: undo,
                        params,
                        key: (next.state.sequence, index),
                    });
                    index += 1;
                }
                Compensation::HumanTask => {
                    event
                        .data
                        .insert(format!("human-task.{human}"), Value::Text(action));
                    human += 1;
                }
                Compensation::None => {
                    event
                        .data
                        .insert(format!("not-undone.{none}"), Value::Text(action));
                    none += 1;
                }
            }
        }
        for timer in tx.timers(instance).await? {
            if timer.status == TimerStatus::Pending {
                effects.push(Effect::CancelTimer {
                    id: timer.id.clone(),
                });
                tx.put_timer(&TimerRecord {
                    status: TimerStatus::Cancelled,
                    ..timer
                })
                .await?;
            }
        }

        self.record(&mut tx, &record, &next, event, effects, actor, op_id, now)
            .await?;
        tx.commit().await?;
        Ok(Outcome::Applied(next))
    }

    /// Steps `record` with `event` and keeps the result, unless it enabled nothing.
    async fn advance(
        &self,
        tx: &mut S::Tx,
        record: InstanceRecord,
        event: &Event,
        actor: &str,
        op_id: &str,
        now: Timestamp,
    ) -> Result<Option<InstanceRecord>, WorkflowError> {
        let machine = self.machine(&record.definition)?;
        let (state, effects) = machine.step(record.state.clone(), event, &self.guards)?;
        if state == record.state && effects.is_empty() {
            return Ok(None);
        }
        let next = InstanceRecord {
            state,
            version: record.version + 1,
            ..record.clone()
        };
        self.carry_out(tx, &next, &effects, actor, now).await?;
        self.record(
            tx,
            &record,
            &next,
            event.clone(),
            effects,
            actor,
            op_id,
            now,
        )
        .await?;
        Ok(Some(next))
    }

    /// Appends the history entry of `before` → `next` and stores `next`, if nothing changed the
    /// instance meanwhile.
    #[allow(clippy::too_many_arguments)]
    async fn record(
        &self,
        tx: &mut S::Tx,
        before: &InstanceRecord,
        next: &InstanceRecord,
        event: Event,
        effects: Vec<Effect>,
        actor: &str,
        op_id: &str,
        now: Timestamp,
    ) -> Result<(), WorkflowError> {
        tx.append_history(&HistoryEntry {
            instance: next.id.clone(),
            seq: next.version,
            event,
            from: before.state.configuration.clone(),
            to: next.state.configuration.clone(),
            effects,
            actor: actor.to_owned(),
            at: now,
            op_id: op_id.to_owned(),
        })
        .await
        .map_err(|_| WorkflowError::Conflict)?;
        if !tx.update_instance(next, before.version).await? {
            return Err(WorkflowError::Conflict);
        }
        Ok(())
    }

    /// Carries out a step's effects in the transaction.
    async fn carry_out(
        &self,
        tx: &mut S::Tx,
        record: &InstanceRecord,
        effects: &[Effect],
        actor: &str,
        now: Timestamp,
    ) -> Result<(), WorkflowError> {
        for effect in effects {
            match effect {
                Effect::Execute {
                    action,
                    params,
                    key: (sequence, index),
                } => {
                    self.descriptor(action)?;
                    let call = ActionCall {
                        instance: record.id.clone(),
                        action: action.clone(),
                        params: params.clone(),
                        key: action_key(&record.id, *sequence, *index),
                        actor: actor.to_owned(),
                    };
                    self.actions
                        .execute(tx, &call)
                        .await
                        .map_err(|e| WorkflowError::Action {
                            action: action.clone(),
                            reason: e.0,
                        })?;
                }
                Effect::StartTimer { id, after, event } => {
                    let fire_at = now
                        .checked_add(*after)
                        .map_err(|e| WorkflowError::Store(e.to_string()))?;
                    tx.put_timer(&TimerRecord {
                        instance: record.id.clone(),
                        id: id.clone(),
                        fire_at,
                        event: event.clone(),
                        status: TimerStatus::Pending,
                    })
                    .await?;
                }
                Effect::CancelTimer { id } => {
                    let pending = tx
                        .timers(&record.id)
                        .await?
                        .into_iter()
                        .find(|t| &t.id == id && t.status == TimerStatus::Pending);
                    if let Some(timer) = pending {
                        tx.put_timer(&TimerRecord {
                            status: TimerStatus::Cancelled,
                            ..timer
                        })
                        .await?;
                    }
                }
                Effect::StartInvoke { .. } | Effect::CancelInvoke { .. } => {
                    return Err(WorkflowError::Unsupported("sub-workflow invocations"));
                }
                Effect::Completed => {}
            }
        }
        Ok(())
    }
}
