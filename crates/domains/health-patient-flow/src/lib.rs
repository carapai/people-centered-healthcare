//! Patient flow (clinical-workspaces.md; resource-ownership.md: Queue Entries are this
//! module's; #102): who is waiting at which service point, and in what order.
//!
//! - **Queue entries** ([`QueueEntry`]) follow the encounters module's events: a patient who
//!   starts an Encounter at a service point, or is moved to one, joins its queue. The entry's id
//!   is derived from the Encounter and the service point it joins at (identifiers.md), so a
//!   replayed event never queues anyone twice. Moving on finishes the entry at the service point
//!   the patient left.
//! - **The queue** ([`queue`]) is a service point's open entries: emergencies first, then urgent
//!   care, then everyone else, each by arrival.
//! - **The steps** ([`QueueEntry::step`]): waiting → called (a staff member calls the patient) →
//!   in service → done; a called patient who does not come back is waiting again; a waiting or
//!   called patient may leave without being seen.
//!
//! The queue never changes the Encounter's status: that is the encounters module's alone.

use std::future::Future;

use health_identifiers::{DerivationName, derive_id};
use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;
use serde::{Deserialize, Serialize};

/// The rule a queue entry's id is derived by.
const QUEUE_RULE: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/rule/queue-entry";

/// How soon the patient must be seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Priority {
    /// In arrival order.
    Routine,
    /// Before routine care.
    Urgent,
    /// First.
    Emergency,
}

/// Where a queue entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueueStatus {
    /// Waiting to be called.
    Waiting,
    /// Called to the service point.
    Called,
    /// Being seen.
    InService,
    /// Seen.
    Done,
    /// Left without being seen.
    Left,
}

impl QueueStatus {
    /// Whether the patient is still at the service point.
    pub fn is_open(self) -> bool {
        matches!(self, Self::Waiting | Self::Called | Self::InService)
    }
}

/// A patient in a service point's queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntry {
    /// Its id, derived from the Encounter and the service point it joined at.
    pub id: String,
    /// The Encounter.
    pub encounter: String,
    /// The patient.
    pub patient: String,
    /// The service point.
    pub service_point: String,
    /// How soon the patient must be seen.
    pub priority: Priority,
    /// Where it is.
    pub status: QueueStatus,
    /// When the patient joined the queue.
    pub arrived: Timestamp,
    /// Who called them, and when, once called.
    #[serde(default)]
    pub called: Option<(String, Timestamp)>,
    /// When it closed.
    #[serde(default)]
    pub closed: Option<Timestamp>,
}

/// The id of the entry the Encounter `encounter` holds at `service_point` from `at`.
pub fn entry_id(encounter: &str, service_point: &str, at: Timestamp) -> String {
    derive_id(&DerivationName::rule(
        format!("{encounter}#{service_point}#{}", at.as_second()),
        QUEUE_RULE,
    ))
    .to_string()
}

/// A new entry for a patient of `encounter` arriving at `service_point`.
pub fn arrive(
    encounter: &str,
    patient: &str,
    service_point: &str,
    priority: Priority,
    at: Timestamp,
) -> QueueEntry {
    QueueEntry {
        id: entry_id(encounter, service_point, at),
        encounter: encounter.to_owned(),
        patient: patient.to_owned(),
        service_point: service_point.to_owned(),
        priority,
        status: QueueStatus::Waiting,
        arrived: at,
        called: None,
        closed: None,
    }
}

/// A step in a queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// A staff member calls the patient.
    Call {
        /// Who.
        by: String,
    },
    /// The patient is being seen.
    Serve,
    /// A called patient did not come: waiting again.
    Recall,
    /// Seen.
    Finish,
    /// Left without being seen.
    Leave,
}

/// Why a queue step is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a {status:?} queue entry cannot take that step")]
pub struct QueueRefused {
    /// Where the entry is.
    pub status: QueueStatus,
}

impl QueueEntry {
    /// The entry after `step` at `at`.
    pub fn step(&self, step: &Step, at: Timestamp) -> Result<Self, QueueRefused> {
        use QueueStatus::{Called, Done, InService, Left, Waiting};
        let refused = || QueueRefused {
            status: self.status,
        };
        let mut next = self.clone();
        match (step, self.status) {
            (Step::Call { by }, Waiting) => {
                next.status = Called;
                next.called = Some((by.clone(), at));
            }
            (Step::Serve, Waiting | Called) => next.status = InService,
            (Step::Recall, Called) => {
                next.status = Waiting;
                next.called = None;
            }
            (Step::Finish, Called | InService) => {
                next.status = Done;
                next.closed = Some(at);
            }
            (Step::Leave, Waiting | Called) => {
                next.status = Left;
                next.closed = Some(at);
            }
            _ => return Err(refused()),
        }
        Ok(next)
    }
}

/// The open entries of `service_point`: emergencies first, then urgent, then routine, each by
/// arrival.
pub fn queue<'a>(
    entries: impl IntoIterator<Item = &'a QueueEntry>,
    service_point: &str,
) -> Vec<&'a QueueEntry> {
    let mut open: Vec<&QueueEntry> = entries
        .into_iter()
        .filter(|e| e.service_point == service_point && e.status.is_open())
        .collect();
    open.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then(a.arrived.cmp(&b.arrived))
            .then(a.id.cmp(&b.id))
    });
    open
}

/// Queue entries inside the unit of work's transaction (this module's driven port).
pub trait QueueStore {
    /// Adds an entry; false if one with its id is already kept (a replayed event).
    fn add_entry(
        &mut self,
        entry: &QueueEntry,
    ) -> impl Future<Output = Result<bool, WriteError>> + Send;
    /// Replaces a kept entry.
    fn put_entry(
        &mut self,
        entry: &QueueEntry,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;
    /// One entry.
    fn entry(
        &mut self,
        id: &str,
    ) -> impl Future<Output = Result<Option<QueueEntry>, StoreError>> + Send;
    /// The open entries at a service point.
    fn open_entries(
        &mut self,
        service_point: &str,
    ) -> impl Future<Output = Result<Vec<QueueEntry>, StoreError>> + Send;
    /// An Encounter's open entries.
    fn entries_of_encounter(
        &mut self,
        encounter: &str,
    ) -> impl Future<Output = Result<Vec<QueueEntry>, StoreError>> + Send;
}

#[cfg(test)]
mod tests;
