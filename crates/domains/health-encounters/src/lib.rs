//! Encounters (clinical-workspaces.md; resource-ownership.md; #102): check-in at a service point
//! and the Encounter lifecycle. The Encounter is this module's: its status is the real clinical
//! status and changes only here, never through a workflow or a queue (workflow-engine.md: workflow
//! state is never written into a clinical status). Queues live in patient-flow, which follows the
//! [`EncounterEvent`]s this module emits.
//!
//! - **Check-in** ([`check_in`]) starts an `in-progress` Encounter at a service point. One
//!   patient has at most one open Encounter at a service point: [`open_guard`] is the guard key
//!   the application claims with it, and releases when it ends.
//! - **The lifecycle** ([`Encounter::transition`]) follows R5's status codes:
//!   planned → in-progress or cancelled; in-progress → on-hold, discharged, completed or
//!   discontinued; on-hold → in-progress or discontinued; discharged → completed; anything not
//!   yet ended → entered-in-error. Completed, cancelled, discontinued and entered-in-error are
//!   final.
//! - **Moving** ([`Encounter::move_to`]) sends the patient to another service point (triage, then
//!   consultation), which patient-flow follows with a queue entry there.

use std::future::Future;

use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;
use serde::{Deserialize, Serialize};

/// FHIR R5 Encounter status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EncounterStatus {
    /// Planned, not yet started.
    Planned,
    /// Under way: the patient has arrived.
    InProgress,
    /// Paused.
    OnHold,
    /// Discharged, with follow-up still to complete.
    Discharged,
    /// Finished.
    Completed,
    /// Cancelled before it started.
    Cancelled,
    /// Stopped before it was finished.
    Discontinued,
    /// Should never have existed.
    EnteredInError,
}

impl EncounterStatus {
    /// The FHIR code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::InProgress => "in-progress",
            Self::OnHold => "on-hold",
            Self::Discharged => "discharged",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Discontinued => "discontinued",
            Self::EnteredInError => "entered-in-error",
        }
    }

    /// Whether the Encounter has ended: its open guard is released.
    pub fn is_final(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Discontinued | Self::EnteredInError
        )
    }

    /// Whether an Encounter in this status may move to `to`: the allowed-transition table that
    /// sync also applies to concurrent edits (ADR 0020).
    pub fn may_become(self, to: Self) -> bool {
        use EncounterStatus::{
            Cancelled, Completed, Discharged, Discontinued, EnteredInError, InProgress, OnHold,
            Planned,
        };
        if to == EnteredInError {
            return !self.is_final();
        }
        matches!(
            (self, to),
            (Planned, InProgress | Cancelled)
                | (InProgress, OnHold | Discharged | Completed | Discontinued)
                | (OnHold, InProgress | Discontinued)
                | (Discharged, Completed)
        )
    }
}

/// The kind of Encounter (FHIR `class`, v3 ActCode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    /// Outpatient: `AMB`.
    Ambulatory,
    /// Emergency: `EMER`.
    Emergency,
    /// Inpatient: `IMP`.
    Inpatient,
}

impl Class {
    /// The v3 ActCode.
    pub fn code(self) -> &'static str {
        match self {
            Self::Ambulatory => "AMB",
            Self::Emergency => "EMER",
            Self::Inpatient => "IMP",
        }
    }
}

/// An Encounter, as this module keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Encounter {
    /// Its id (a UUIDv7).
    pub id: String,
    /// The patient, `Patient/{id}`.
    pub patient: String,
    /// The facility.
    pub facility: String,
    /// Where the patient is now: a service point.
    pub service_point: String,
    /// Its kind.
    pub class: Class,
    /// Its status.
    pub status: EncounterStatus,
    /// When it started.
    pub started: Timestamp,
    /// When it ended, once final.
    #[serde(default)]
    pub ended: Option<Timestamp>,
    /// Who checked the patient in.
    pub by: String,
}

/// What a check-in names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckIn {
    /// The new Encounter's id (a UUIDv7).
    pub id: String,
    /// The patient, `Patient/{id}`.
    pub patient: String,
    /// The facility.
    pub facility: String,
    /// The service point the patient arrives at, e.g. `triage`.
    pub service_point: String,
    /// Its kind.
    pub class: Class,
    /// Who.
    pub by: String,
    /// When.
    pub at: Timestamp,
}

/// What happened to an Encounter, for patient-flow and the other modules that follow it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum EncounterEvent {
    /// The patient arrived at a service point.
    Started {
        /// The Encounter.
        encounter: String,
        /// The patient.
        patient: String,
        /// Where.
        service_point: String,
        /// Its kind (an emergency is seen first).
        class: Class,
        /// When.
        at: Timestamp,
    },
    /// The patient was sent to another service point.
    Moved {
        /// The Encounter.
        encounter: String,
        /// The patient.
        patient: String,
        /// From.
        from: String,
        /// To.
        to: String,
        /// Its kind.
        class: Class,
        /// When.
        at: Timestamp,
    },
    /// The Encounter's status changed.
    StatusChanged {
        /// The Encounter.
        encounter: String,
        /// The new status.
        status: EncounterStatus,
        /// When.
        at: Timestamp,
    },
}

impl EncounterEvent {
    /// The event's name on the outbox, e.g. `encounters.started.v1`.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Started { .. } => "encounters.started.v1",
            Self::Moved { .. } => "encounters.moved.v1",
            Self::StatusChanged { .. } => "encounters.status-changed.v1",
        }
    }
}

/// Why an Encounter step is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EncounterRefused {
    /// A check-in missing its patient, facility, service point or author.
    #[error("a check-in needs a patient, a facility, a service point and who checked them in")]
    Incomplete,
    /// The status cannot change that way.
    #[error("an Encounter that is {from} cannot become {to}")]
    Transition {
        /// From.
        from: &'static str,
        /// To.
        to: &'static str,
    },
    /// Only an Encounter under way moves, and not to where it already is.
    #[error("only an Encounter in progress moves, to another service point")]
    CannotMove,
}

/// The guard key one open Encounter of `patient` at `service_point` holds.
pub fn open_guard(patient: &str, service_point: &str) -> String {
    format!("open-encounter|{patient}|{service_point}")
}

/// Checks the patient in: an `in-progress` Encounter, and its [`EncounterEvent::Started`].
pub fn check_in(request: &CheckIn) -> Result<(Encounter, EncounterEvent), EncounterRefused> {
    let blank = |text: &str| text.trim().is_empty();
    if [
        &request.id,
        &request.patient,
        &request.facility,
        &request.service_point,
        &request.by,
    ]
    .iter()
    .any(|field| blank(field))
    {
        return Err(EncounterRefused::Incomplete);
    }
    let encounter = Encounter {
        id: request.id.clone(),
        patient: request.patient.clone(),
        facility: request.facility.clone(),
        service_point: request.service_point.clone(),
        class: request.class,
        status: EncounterStatus::InProgress,
        started: request.at,
        ended: None,
        by: request.by.clone(),
    };
    let event = EncounterEvent::Started {
        encounter: encounter.id.clone(),
        patient: encounter.patient.clone(),
        service_point: encounter.service_point.clone(),
        class: encounter.class,
        at: request.at,
    };
    Ok((encounter, event))
}

impl Encounter {
    /// The Encounter with status `to` at `at`, and its event.
    pub fn transition(
        &self,
        to: EncounterStatus,
        at: Timestamp,
    ) -> Result<(Self, EncounterEvent), EncounterRefused> {
        if !self.status.may_become(to) {
            return Err(EncounterRefused::Transition {
                from: self.status.code(),
                to: to.code(),
            });
        }
        let next = Self {
            status: to,
            ended: if to.is_final() { Some(at) } else { self.ended },
            ..self.clone()
        };
        let event = EncounterEvent::StatusChanged {
            encounter: self.id.clone(),
            status: to,
            at,
        };
        Ok((next, event))
    }

    /// The Encounter at `service_point` from `at`, and its event.
    pub fn move_to(
        &self,
        service_point: &str,
        at: Timestamp,
    ) -> Result<(Self, EncounterEvent), EncounterRefused> {
        if self.status != EncounterStatus::InProgress
            || service_point.trim().is_empty()
            || service_point == self.service_point
        {
            return Err(EncounterRefused::CannotMove);
        }
        let event = EncounterEvent::Moved {
            encounter: self.id.clone(),
            patient: self.patient.clone(),
            from: self.service_point.clone(),
            to: service_point.to_owned(),
            class: self.class,
            at,
        };
        let next = Self {
            service_point: service_point.to_owned(),
            ..self.clone()
        };
        Ok((next, event))
    }
}

/// Encounters inside the unit of work's transaction (this module's driven port). The FHIR
/// Encounter is the application's projection of what is kept here.
pub trait EncounterStore {
    /// Adds or replaces an Encounter.
    fn put_encounter(
        &mut self,
        encounter: &Encounter,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;
    /// One Encounter.
    fn encounter(
        &mut self,
        id: &str,
    ) -> impl Future<Output = Result<Option<Encounter>, StoreError>> + Send;
}

#[cfg(test)]
mod tests;
