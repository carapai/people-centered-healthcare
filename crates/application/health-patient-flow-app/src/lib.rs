//! Patient flow at the facility authority (#102; clinical-workspaces.md): queue entries kept from
//! the encounters module's events, and the queue steps.
//!
//! - **Following events** ([`Flow::follow`]), each in one transaction, consumed after the
//!   encounter's commit:
//!   - `Started`: the patient joins the service point's queue; an emergency is seen first;
//!   - `Moved`: the patient's entry where they were is finished, and they join the queue where
//!     they are sent;
//!   - an ended Encounter closes its open entries: done if the patient was called or being seen,
//!     else left.
//!
//!   Entry ids are derived from the event, so a replayed event changes nothing.
//! - **The queue** ([`Flow::queue`]) and its steps ([`Flow::step`]): call, serve, recall, finish
//!   and leave. The queue never changes the Encounter's status.

use health_encounters::{Class, EncounterEvent};
use health_patient_flow::{
    Priority, QueueEntry, QueueRefused, QueueStatus, QueueStore, Step, arrive, queue,
};
use health_persistence::{Store, StoreError, Transaction, WriteError};
use health_time::Timestamp;

/// Why a patient-flow step failed. Nothing was kept.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FlowError {
    /// The step does not fit the entry.
    #[error(transparent)]
    Refused(#[from] QueueRefused),
    /// No such entry.
    #[error("queue entry {0} is not known")]
    NotFound(String),
    /// The store failed, or a concurrent change came first; try again.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for FlowError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for FlowError {
    fn from(error: WriteError) -> Self {
        Self::Store(error.to_string())
    }
}

fn priority(class: Class) -> Priority {
    match class {
        Class::Emergency => Priority::Emergency,
        Class::Ambulatory | Class::Inpatient => Priority::Routine,
    }
}

/// The patient-flow use cases over `store`.
pub struct Flow<S> {
    store: S,
}

impl<S> Flow<S>
where
    S: Store,
    S::Tx: QueueStore,
{
    /// The use cases over `store`.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Closes the Encounter's open entries at `service_point` (any, if `None`) at `at`.
    async fn close(
        tx: &mut S::Tx,
        encounter: &str,
        service_point: Option<&str>,
        at: Timestamp,
    ) -> Result<(), FlowError> {
        for entry in tx.entries_of_encounter(encounter).await? {
            if !entry.status.is_open() || service_point.is_some_and(|sp| sp != entry.service_point)
            {
                continue;
            }
            let step = if entry.status == QueueStatus::Waiting {
                Step::Leave
            } else {
                Step::Finish
            };
            tx.put_entry(&entry.step(&step, at)?).await?;
        }
        Ok(())
    }

    /// Follows one encounter event; a replay changes nothing.
    pub async fn follow(&self, event: &EncounterEvent) -> Result<(), FlowError> {
        let mut tx = self.store.begin().await?;
        match event {
            EncounterEvent::Started {
                encounter,
                patient,
                service_point,
                class,
                at,
            } => {
                let entry = arrive(encounter, patient, service_point, priority(*class), *at);
                tx.add_entry(&entry).await?;
            }
            EncounterEvent::Moved {
                encounter,
                patient,
                from,
                to,
                class,
                at,
            } => {
                let entry = arrive(encounter, patient, to, priority(*class), *at);
                if tx.add_entry(&entry).await? {
                    Self::close(&mut tx, encounter, Some(from), *at).await?;
                }
            }
            EncounterEvent::StatusChanged {
                encounter,
                status,
                at,
            } => {
                if status.is_final() {
                    Self::close(&mut tx, encounter, None, *at).await?;
                }
            }
        }
        tx.commit().await?;
        Ok(())
    }

    /// The queue at `service_point`: emergencies first, then by arrival.
    pub async fn queue(&self, service_point: &str) -> Result<Vec<QueueEntry>, FlowError> {
        let mut tx = self.store.begin().await?;
        let open = tx.open_entries(service_point).await?;
        Ok(queue(&open, service_point).into_iter().cloned().collect())
    }

    /// Takes `step` on entry `id` at `at`.
    pub async fn step(
        &self,
        id: &str,
        step: &Step,
        at: Timestamp,
    ) -> Result<QueueEntry, FlowError> {
        let mut tx = self.store.begin().await?;
        let entry = tx
            .entry(id)
            .await?
            .ok_or_else(|| FlowError::NotFound(id.to_owned()))?;
        let next = entry.step(step, at)?;
        tx.put_entry(&next).await?;
        tx.commit().await?;
        Ok(next)
    }
}
