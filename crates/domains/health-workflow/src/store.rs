//! What the workflow engine persists (ADR 0044; workflow-engine.md § Persistence and
//! transactions), and the port it is persisted through.
//!
//! | Record | Contents |
//! |---|---|
//! | [`InstanceRecord`] | Definition `url\|version`, context, the interpreter's [`Instance`], version, authority node |
//! | [`HistoryEntry`] | Append-only, keyed `[instance, seq]`: event, from- and to-configuration, effects, actor, time, `op_id` |
//! | [`TimerRecord`] | Instance, timer id, `fire_at`, event, status |
//! | inbox | Event id → processed, so a domain event is consumed once |
//!
//! Each instance change is a compare-and-set on its version, and appends the history entry with
//! that version as its `seq`, so the history has no gaps and two concurrent steps cannot both
//! commit.

use std::future::Future;

use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;
use serde::{Deserialize, Serialize};

use crate::interpreter::{Effect, Event, Instance};

/// A workflow instance, as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceRecord {
    /// Its id (a UUID).
    pub id: String,
    /// The definition it runs, `url|version`, pinned for its whole life.
    pub definition: String,
    /// What it is about, e.g. `Encounter/123`.
    pub context: String,
    /// The node that decides its transitions and fires its timers.
    pub authority: String,
    /// The interpreter's state: configuration, data model, microstep sequence.
    pub state: Instance,
    /// Its changes so far: the `seq` of its latest history entry.
    pub version: u64,
}

/// One transition of an instance: appended, never changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// The instance.
    pub instance: String,
    /// Its position: the instance version the transition produced, from 1.
    pub seq: u64,
    /// What happened: the start, a command, a domain event or a cancellation.
    pub event: Event,
    /// The configuration before.
    pub from: Vec<String>,
    /// The configuration after.
    pub to: Vec<String>,
    /// What the transition did, in order.
    pub effects: Vec<Effect>,
    /// Who: a user, or the workflow principal for timers and domain events.
    pub actor: String,
    /// When.
    pub at: Timestamp,
    /// The operation that made it (the command's `op_id`, or the domain event's id).
    pub op_id: String,
}

/// Where a timer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimerStatus {
    /// Waiting to fire.
    Pending,
    /// Fired.
    Fired,
    /// Its state was left first.
    Cancelled,
}

/// A durable timer at the instance's authority (#98 fires them).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimerRecord {
    /// The instance.
    pub instance: String,
    /// The timer id in the definition (`{state}#{index}`).
    pub id: String,
    /// When it fires.
    pub fire_at: Timestamp,
    /// The event it raises.
    pub event: String,
    /// Where it is.
    pub status: TimerStatus,
}

/// Workflow records, inside the unit of work's transaction.
pub trait WorkflowStore {
    /// Adds a new instance; fails if the id is taken.
    fn create_instance(
        &mut self,
        instance: &InstanceRecord,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// One instance.
    fn instance(
        &mut self,
        id: &str,
    ) -> impl Future<Output = Result<Option<InstanceRecord>, StoreError>> + Send;

    /// Replaces the instance if it is still at version `before`: false if another change came
    /// first.
    fn update_instance(
        &mut self,
        instance: &InstanceRecord,
        before: u64,
    ) -> impl Future<Output = Result<bool, WriteError>> + Send;

    /// Appends a history entry; fails if its `[instance, seq]` is taken, so history is never
    /// rewritten.
    fn append_history(
        &mut self,
        entry: &HistoryEntry,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// An instance's history, in order.
    fn instance_history(
        &mut self,
        instance: &str,
    ) -> impl Future<Output = Result<Vec<HistoryEntry>, StoreError>> + Send;

    /// Records a domain event as processed: false if it already was.
    fn claim_event(
        &mut self,
        event_id: &str,
        at: Timestamp,
    ) -> impl Future<Output = Result<bool, WriteError>> + Send;

    /// Adds or replaces a timer.
    fn put_timer(
        &mut self,
        timer: &TimerRecord,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Pending timers due at `at`, earliest first, at most `limit`.
    fn due_timers(
        &mut self,
        at: Timestamp,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<TimerRecord>, StoreError>> + Send;

    /// An instance's timers.
    fn timers(
        &mut self,
        instance: &str,
    ) -> impl Future<Output = Result<Vec<TimerRecord>, StoreError>> + Send;
}
