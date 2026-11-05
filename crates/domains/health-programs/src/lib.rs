//! Programs (program-designer.md; care-journeys.md; #116): ART, TB, ANC and the others, each a
//! published ProgramPlanDefinition whose enrolment opens an EpisodeOfCare.
//!
//! - **Enrolment** ([`enrol`]) opens an `active` episode for an eligible patient. A patient has at
//!   most one open episode of a program ([`open_guard`]), but may be in several programs at once
//!   (ADR 0042): ART and TB concurrently are two episodes, two guards.
//! - **The lifecycle** ([`Episode::transition`]) follows R5's EpisodeOfCare statuses: planned or
//!   waitlisted, active, on hold, then finished, cancelled or entered in error.
//! - **Transfer** ([`Episode::transfer_out`]) is a hand-over between facilities (write-authority.md):
//!   the old facility finishes its episode, naming where the patient went, and the new one
//!   enrols the patient in an episode of its own that names the old one (`transferred_from`).
//! - **Regimens** come only from approved sources ([`unapproved_regimens`]): every regimen a
//!   program uses cites one of the program's protocol sources.

use health_time::Timestamp;
use serde::{Deserialize, Serialize};

/// A coded concept: what a person reads, and its code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coded {
    /// What a person reads.
    pub text: String,
    /// The code system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// The code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// A program, as its published ProgramPlanDefinition defines it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// Its canonical URL.
    pub url: String,
    /// Its version.
    pub version: String,
    /// The EpisodeOfCare type its enrolment opens.
    pub episode_type: Coded,
    /// Who may enrol: FHIRPath over the patient, if the program restricts it.
    pub eligibility: Option<String>,
    /// The Workflow Definitions (canonicals) an enrolment starts.
    pub workflows: Vec<String>,
    /// The regimens (ActivityDefinition canonicals) it uses.
    pub regimens: Vec<String>,
    /// The approved protocol sources it cites.
    pub sources: Vec<String>,
}

/// R5 EpisodeOfCare status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EpisodeStatus {
    /// Planned, not yet started.
    Planned,
    /// Waiting for a place.
    Waitlist,
    /// Under way.
    Active,
    /// Paused.
    Onhold,
    /// Ended.
    Finished,
    /// Ended before it started.
    Cancelled,
    /// Should never have existed.
    EnteredInError,
}

impl EpisodeStatus {
    /// The FHIR code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Waitlist => "waitlist",
            Self::Active => "active",
            Self::Onhold => "onhold",
            Self::Finished => "finished",
            Self::Cancelled => "cancelled",
            Self::EnteredInError => "entered-in-error",
        }
    }

    /// The status a FHIR code names.
    pub fn from_code(code: &str) -> Option<Self> {
        [
            Self::Planned,
            Self::Waitlist,
            Self::Active,
            Self::Onhold,
            Self::Finished,
            Self::Cancelled,
            Self::EnteredInError,
        ]
        .into_iter()
        .find(|s| s.code() == code)
    }

    /// Whether the episode is still open: its guard is held.
    pub fn is_open(self) -> bool {
        matches!(
            self,
            Self::Planned | Self::Waitlist | Self::Active | Self::Onhold
        )
    }

    fn may_become(self, to: Self) -> bool {
        use EpisodeStatus::{
            Active, Cancelled, EnteredInError, Finished, Onhold, Planned, Waitlist,
        };
        if to == EnteredInError {
            return self.is_open();
        }
        matches!(
            (self, to),
            (Planned | Waitlist, Active | Cancelled)
                | (Planned, Waitlist)
                | (Active, Onhold | Finished)
                | (Onhold, Active | Finished)
        )
    }
}

/// A patient's enrolment in a program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    /// Its id (a UUIDv7).
    pub id: String,
    /// The patient, `Patient/{id}`.
    pub patient: String,
    /// The program's canonical URL.
    pub program: String,
    /// The EpisodeOfCare type.
    pub episode_type: Coded,
    /// The facility that manages it.
    pub facility: String,
    /// Its status.
    pub status: EpisodeStatus,
    /// When it started.
    pub start: Timestamp,
    /// When it ended, once it has.
    #[serde(default)]
    pub end: Option<Timestamp>,
    /// Who enrolled the patient.
    pub by: String,
    /// The episode at another facility this one continues, after a transfer.
    #[serde(default)]
    pub transferred_from: Option<String>,
    /// The facility the patient was transferred to, once they were.
    #[serde(default)]
    pub transferred_to: Option<String>,
}

/// What an enrolment names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enrolment<'a> {
    /// The new episode's id (a UUIDv7).
    pub id: String,
    /// The patient, `Patient/{id}`.
    pub patient: String,
    /// The program.
    pub program: &'a Program,
    /// The facility enrolling the patient.
    pub facility: String,
    /// Whether the patient meets the program's eligibility.
    pub eligible: bool,
    /// Who.
    pub by: String,
    /// When.
    pub at: Timestamp,
    /// The episode a transfer continues, if any.
    pub transferred_from: Option<String>,
}

/// What happened to an episode, on the outbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum ProgramEvent {
    /// A patient was enrolled.
    Enrolled {
        /// The episode.
        episode: String,
        /// The patient.
        patient: String,
        /// The program.
        program: String,
        /// The facility.
        facility: String,
        /// When.
        at: Timestamp,
    },
    /// An episode's status changed.
    StatusChanged {
        /// The episode.
        episode: String,
        /// The new status.
        status: EpisodeStatus,
        /// When.
        at: Timestamp,
    },
    /// A patient left for another facility, which central hands the program over to.
    TransferredOut {
        /// The episode finished here.
        episode: String,
        /// The patient.
        patient: String,
        /// The program.
        program: String,
        /// From.
        from: String,
        /// To.
        to: String,
        /// When.
        at: Timestamp,
    },
}

impl ProgramEvent {
    /// The event's name on the outbox.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Enrolled { .. } => "programs.enrolled.v1",
            Self::StatusChanged { .. } => "programs.status-changed.v1",
            Self::TransferredOut { .. } => "programs.transferred-out.v1",
        }
    }
}

/// Why a program step is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProgramRefused {
    /// An enrolment missing its patient, facility or author.
    #[error("an enrolment needs a patient, a facility and who enrolled them")]
    Incomplete,
    /// The patient does not meet the program's eligibility.
    #[error("the patient is not eligible for {0}")]
    NotEligible(String),
    /// The status cannot change that way.
    #[error("an episode that is {from} cannot become {to}")]
    Transition {
        /// From.
        from: &'static str,
        /// To.
        to: &'static str,
    },
    /// Only an open episode is transferred, to another facility.
    #[error("only an open episode is transferred, to another facility")]
    CannotTransfer,
}

/// The guard key an open episode of `program` for `patient` holds: one at a time per program.
pub fn open_guard(patient: &str, program: &str) -> String {
    format!("open-episode|{patient}|{program}")
}

/// Enrols the patient: an `active` episode, and its [`ProgramEvent::Enrolled`].
pub fn enrol(request: &Enrolment<'_>) -> Result<(Episode, ProgramEvent), ProgramRefused> {
    if [
        &request.id,
        &request.patient,
        &request.facility,
        &request.by,
    ]
    .iter()
    .any(|f| f.trim().is_empty())
    {
        return Err(ProgramRefused::Incomplete);
    }
    if !request.eligible {
        return Err(ProgramRefused::NotEligible(request.program.url.clone()));
    }
    let episode = Episode {
        id: request.id.clone(),
        patient: request.patient.clone(),
        program: request.program.url.clone(),
        episode_type: request.program.episode_type.clone(),
        facility: request.facility.clone(),
        status: EpisodeStatus::Active,
        start: request.at,
        end: None,
        by: request.by.clone(),
        transferred_from: request.transferred_from.clone(),
        transferred_to: None,
    };
    let event = ProgramEvent::Enrolled {
        episode: episode.id.clone(),
        patient: episode.patient.clone(),
        program: episode.program.clone(),
        facility: episode.facility.clone(),
        at: request.at,
    };
    Ok((episode, event))
}

impl Episode {
    /// The episode with status `to` from `at`, and its event.
    pub fn transition(
        &self,
        to: EpisodeStatus,
        at: Timestamp,
    ) -> Result<(Self, ProgramEvent), ProgramRefused> {
        if !self.status.may_become(to) {
            return Err(ProgramRefused::Transition {
                from: self.status.code(),
                to: to.code(),
            });
        }
        let next = Self {
            status: to,
            end: if to.is_open() { self.end } else { Some(at) },
            ..self.clone()
        };
        let event = ProgramEvent::StatusChanged {
            episode: self.id.clone(),
            status: to,
            at,
        };
        Ok((next, event))
    }

    /// The episode finished here because the patient moved to `facility`, and the event central
    /// hands the program over by.
    pub fn transfer_out(
        &self,
        facility: &str,
        at: Timestamp,
    ) -> Result<(Self, ProgramEvent), ProgramRefused> {
        if !self.status.is_open() || facility.trim().is_empty() || facility == self.facility {
            return Err(ProgramRefused::CannotTransfer);
        }
        let next = Self {
            status: EpisodeStatus::Finished,
            end: Some(at),
            transferred_to: Some(facility.to_owned()),
            ..self.clone()
        };
        let event = ProgramEvent::TransferredOut {
            episode: self.id.clone(),
            patient: self.patient.clone(),
            program: self.program.clone(),
            from: self.facility.clone(),
            to: facility.to_owned(),
            at,
        };
        Ok((next, event))
    }
}

/// The regimens of `program` that cite none of its approved sources: each regimen with the
/// sources it cites (`(regimen, cited)`).
pub fn unapproved_regimens(program: &Program, regimens: &[(String, Vec<String>)]) -> Vec<String> {
    regimens
        .iter()
        .filter(|(_, cited)| !cited.iter().any(|c| program.sources.contains(c)))
        .map(|(regimen, _)| regimen.clone())
        .collect()
}

#[cfg(test)]
mod tests;
