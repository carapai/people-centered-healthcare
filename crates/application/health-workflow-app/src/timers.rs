//! Durable timers and escalation (#98; workflow-engine.md § Timers and escalation; ADR 0044).
//!
//! - **Where they fire:** at the instance's authority node only. A timer whose instance another
//!   node decides, such as an edge-owned instance replicated to a desktop in offline fallback, is
//!   skipped and stays pending.
//! - **When:** the runtime calls [`Workflows::fire_due`] every [`SCHEDULER_INTERVAL`]. The time is
//!   the node clock, checked against the clock floor (offline-auth.md): if the clock reads
//!   before the last known central time, nothing fires.
//! - **Exactly once:** each timer fires in its own transaction, which re-reads it, marks it fired
//!   and runs its event as the workflow principal. A timer already fired, or re-armed for a later
//!   time, is left alone, and a failure keeps it pending for the next run, so restarts neither
//!   lose nor repeat a firing.
//! - **Escalation** is an ordinary timer transition: its typed actions reassign, notify (through
//!   a CommunicationRequest action) or raise priority.

use health_persistence::{Store, Transaction};
use health_time::{SignedDuration, Timestamp};
use health_workflow::{Event, Guards, TimerRecord, TimerStatus, WorkflowStore};

use crate::{Actions, Definitions, PRINCIPAL, WorkflowError, Workflows};

/// How often the runtime checks for due timers.
pub const SCHEDULER_INTERVAL: SignedDuration = SignedDuration::from_secs(30);

/// At most this many timers fire per run; the rest wait for the next.
const BATCH: u32 = 200;

/// The node's clock floor: the last known central time, if any (offline-auth.md § Permission
/// snapshots). Identity & Access keeps it; the runtime adapts it.
pub trait TimeFloor: Send + Sync {
    /// The floor.
    fn floor(&self) -> Option<Timestamp>;
}

/// What a scheduler run did, by `(instance, timer)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fired {
    /// Fired, and the instance moved.
    pub fired: Vec<(String, String)>,
    /// Fired, but the instance had ended or its event enabled nothing.
    pub ignored: Vec<(String, String)>,
    /// Left pending: another node is the instance's authority.
    pub skipped: Vec<(String, String)>,
    /// Left pending to try again: why it failed.
    pub failed: Vec<(String, String, WorkflowError)>,
}

enum Firing {
    Moved,
    Ignored,
    Skipped,
    Gone,
}

impl<S, D, A, G> Workflows<S, D, A, G>
where
    S: Store,
    S::Tx: WorkflowStore,
    D: Definitions,
    A: Actions<S::Tx>,
    G: Guards + Send + Sync,
{
    /// Checks the node clock against `floor` before firing timers.
    #[must_use]
    pub fn with_floor(mut self, floor: std::sync::Arc<dyn TimeFloor>) -> Self {
        self.floor = Some(floor);
        self
    }

    /// The node clock, unless it reads before the clock floor.
    fn checked_now(&self) -> Result<Timestamp, WorkflowError> {
        let local = self.clock.now();
        match self.floor.as_ref().and_then(|f| f.floor()) {
            Some(floor) if local < floor => Err(WorkflowError::ClockBroken { local, floor }),
            _ => Ok(local),
        }
    }

    /// Fires every timer due now whose instance this node decides. Each fires in its own
    /// transaction; one failing does not stop the others.
    pub async fn fire_due(&self) -> Result<Fired, WorkflowError> {
        let now = self.checked_now()?;
        let due = {
            let mut tx = self.store.begin().await?;
            tx.due_timers(now, BATCH).await?
        };
        let mut report = Fired::default();
        for timer in due {
            let key = (timer.instance.clone(), timer.id.clone());
            match self.fire(&timer, now).await {
                Ok(Firing::Moved) => report.fired.push(key),
                Ok(Firing::Ignored) => report.ignored.push(key),
                Ok(Firing::Skipped) => report.skipped.push(key),
                Ok(Firing::Gone) => {}
                Err(error) => report.failed.push((key.0, key.1, error)),
            }
        }
        Ok(report)
    }

    async fn fire(&self, due: &TimerRecord, now: Timestamp) -> Result<Firing, WorkflowError> {
        let mut tx = self.store.begin().await?;
        let current = tx
            .timers(&due.instance)
            .await?
            .into_iter()
            .find(|t| t.id == due.id && t.status == TimerStatus::Pending && t.fire_at <= now);
        let Some(timer) = current else {
            return Ok(Firing::Gone);
        };
        let record = tx
            .instance(&timer.instance)
            .await?
            .ok_or_else(|| WorkflowError::NotFound(timer.instance.clone()))?;
        if record.authority != self.authority {
            return Ok(Firing::Skipped);
        }
        // Marked fired first: if its event re-enters the state, the timer is armed again.
        tx.put_timer(&TimerRecord {
            status: TimerStatus::Fired,
            ..timer.clone()
        })
        .await?;
        let op_id = format!("timer|{}|{}|{}", timer.instance, timer.id, timer.fire_at);
        let moved = if record.state.running {
            self.advance(
                &mut tx,
                record,
                &Event::named(&timer.event),
                PRINCIPAL,
                &op_id,
                now,
            )
            .await?
            .is_some()
        } else {
            false
        };
        tx.commit().await?;
        Ok(if moved {
            Firing::Moved
        } else {
            Firing::Ignored
        })
    }
}
