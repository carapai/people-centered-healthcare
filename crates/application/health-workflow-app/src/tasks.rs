//! Task commands and worklists (#99; workflow-engine.md § Worklists and queues; ADR 0002).
//!
//! Claiming, releasing, reassigning and escalating a task are workflow commands, never direct
//! Task edits: each sets the reserved `task.<state>.*` fields of the instance's data model,
//! appends a history entry (`task.claimed`, `task.released`, `task.reassigned`,
//! `task.escalated`) and rebuilds the instance's Tasks, in one transaction. Completing a task is
//! an ordinary [`Workflows::command`] that leaves its state.
//!
//! - **Claim:** the user becomes `Task.owner`. Claiming again refreshes the claim; a claim idle
//!   for the task's timeout (15 minutes by default) lapses, and anyone may claim it.
//! - **Release:** only the claim's holder.
//! - **Reassign:** to another user, whatever the claim. Who may is the caller's policy decision.
//! - **Escalate:** only to a more urgent priority.
//!
//! Permission checks are the caller's: these commands run after the policy decision point.

use std::collections::BTreeMap;

use health_persistence::{Store, Transaction};
use health_time::Timestamp;
use health_workflow::worklists::{
    TaskView, claim, claimed_field, human_tasks, owner_field, priority, priority_field, project,
};
use health_workflow::{
    DataModel, Event, Guards, HumanTask, InstanceRecord, Priority, Value, WorkflowStore,
};

use crate::{Actions, Definitions, Outcome, WorkflowError, Workflows, command_key};

/// A task command's change to the data model: given the task, the live claim, the data model
/// and the time; it may add to the history entry's event data.
type Change<'a> = Box<
    dyn FnOnce(
            &HumanTask,
            Option<(String, Timestamp)>,
            &mut DataModel,
            &mut BTreeMap<String, Value>,
            Timestamp,
        ) -> Result<(), WorkflowError>
        + Send
        + 'a,
>;

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

impl<S, D, A, G> Workflows<S, D, A, G>
where
    S: Store,
    S::Tx: WorkflowStore,
    D: Definitions,
    A: Actions<S::Tx>,
    G: Guards + Send + Sync,
{
    /// Claims the task at `state` for `user`, or refreshes their claim.
    pub async fn claim(
        &self,
        instance: &str,
        state: &str,
        user: &str,
        op_id: &str,
    ) -> Result<Outcome, WorkflowError> {
        let change: Change<'_> = Box::new(move |_, held, data, event, now| {
            if let Some((owner, _)) = held.filter(|(owner, _)| owner != user) {
                return Err(WorkflowError::Claimed(owner));
            }
            data.insert(owner_field(state), text(user));
            data.insert(claimed_field(state), Value::Int(now.as_second()));
            event.insert("owner".into(), text(user));
            Ok(())
        });
        self.task_command(instance, state, user, op_id, "task.claimed", change)
            .await
    }

    /// Releases `user`'s claim on the task at `state`.
    pub async fn release(
        &self,
        instance: &str,
        state: &str,
        user: &str,
        op_id: &str,
    ) -> Result<Outcome, WorkflowError> {
        let change: Change<'_> = Box::new(move |_, held, data, _, _| {
            if held.is_none_or(|(owner, _)| owner != user) {
                return Err(WorkflowError::NotOwner(user.to_owned()));
            }
            data.remove(&owner_field(state));
            data.remove(&claimed_field(state));
            Ok(())
        });
        self.task_command(instance, state, user, op_id, "task.released", change)
            .await
    }

    /// Gives the task at `state` to `to`, as decided by `by`.
    pub async fn reassign(
        &self,
        instance: &str,
        state: &str,
        to: &str,
        by: &str,
        op_id: &str,
    ) -> Result<Outcome, WorkflowError> {
        let change: Change<'_> = Box::new(move |_, held, data, event, now| {
            data.insert(owner_field(state), text(to));
            data.insert(claimed_field(state), Value::Int(now.as_second()));
            if let Some((owner, _)) = held {
                event.insert("from".into(), text(&owner));
            }
            event.insert("owner".into(), text(to));
            Ok(())
        });
        self.task_command(instance, state, by, op_id, "task.reassigned", change)
            .await
    }

    /// Raises the priority of the task at `state` to `to`.
    pub async fn escalate(
        &self,
        instance: &str,
        state: &str,
        to: Priority,
        by: &str,
        op_id: &str,
    ) -> Result<Outcome, WorkflowError> {
        let change: Change<'_> = Box::new(move |task, _, data, event, _| {
            let current = match data.get(&priority_field(state)) {
                Some(Value::Text(code)) => priority(code).unwrap_or(task.priority),
                _ => task.priority,
            }
            .max(task.priority);
            if to <= current {
                return Err(WorkflowError::NotEscalation(to.code().to_owned()));
            }
            data.insert(priority_field(state), text(to.code()));
            event.insert("priority".into(), text(to.code()));
            Ok(())
        });
        self.task_command(instance, state, by, op_id, "task.escalated", change)
            .await
    }

    /// Every Task of `instance`, rebuilt from its state and history.
    pub async fn tasks(&self, instance: &str) -> Result<Vec<TaskView>, WorkflowError> {
        let mut tx = self.store.begin().await?;
        let record = tx
            .instance(instance)
            .await?
            .ok_or_else(|| WorkflowError::NotFound(instance.to_owned()))?;
        let compiled = self.compiled(&record.definition)?;
        let history = tx.instance_history(instance).await?;
        Ok(project(
            &compiled.chart,
            &record,
            &history,
            self.clock.now(),
        ))
    }

    /// The open tasks on `worklist` now, most urgent first, then by arrival.
    pub async fn worklist(&self, worklist: &str) -> Result<Vec<TaskView>, WorkflowError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        Ok(tx
            .open_tasks(worklist)
            .await?
            .iter()
            .map(|task| task.as_at(now))
            .collect())
    }

    async fn task_command(
        &self,
        instance: &str,
        state: &str,
        actor: &str,
        op_id: &str,
        name: &str,
        change: Change<'_>,
    ) -> Result<Outcome, WorkflowError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        if !tx.claim_event(&command_key(op_id), now).await? {
            return Ok(Outcome::Replayed);
        }
        let record: InstanceRecord = tx
            .instance(instance)
            .await?
            .ok_or_else(|| WorkflowError::NotFound(instance.to_owned()))?;
        if !record.state.running {
            return Err(WorkflowError::Ended(instance.to_owned()));
        }
        let compiled = self.compiled(&record.definition)?;
        let tasks = human_tasks(&compiled.chart);
        let task = tasks
            .get(state)
            .filter(|_| record.state.configuration.iter().any(|s| s == state))
            .ok_or_else(|| WorkflowError::NoTask(state.to_owned()))?;
        let held = claim(&record, state, task.claim_timeout, now);

        let mut next = InstanceRecord {
            version: record.version + 1,
            ..record.clone()
        };
        let mut event = Event::named(name);
        event.data.insert("state".into(), text(state));
        change(task, held, &mut next.state.data, &mut event.data, now)?;
        self.record(
            &mut tx,
            &record,
            &mut next,
            event,
            Vec::new(),
            actor,
            op_id,
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(Outcome::Applied(next))
    }
}
