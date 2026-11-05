//! Care plans (resource-ownership.md: CarePlan, Goal and CareTeam are care-plans'; #110). The
//! basics:
//!
//! - **A plan** ([`draft`]) for a patient, with its title and what it is for, starts as a draft.
//! - **Its lifecycle** ([`CarePlan::transition`]) follows R5: a draft becomes active (or is
//!   revoked); an active plan is held, completed or revoked; a held one resumes or is revoked;
//!   an open one may be entered in error.
//! - **What it addresses** ([`CarePlan::address`]): the patient's Conditions.
//! - **Its activities** ([`CarePlan::plan`]) are the patient's own requests (R5
//!   `activity.plannedActivityReference`): a ServiceRequest, MedicationRequest, Task,
//!   Appointment, NutritionOrder or DeviceRequest, each once. A plan that has ended takes none.

use health_time::Timestamp;
use serde::{Deserialize, Serialize};

/// The request types a plan's activities may name.
pub const ACTIVITY_TYPES: [&str; 6] = [
    "ServiceRequest",
    "MedicationRequest",
    "Task",
    "Appointment",
    "NutritionOrder",
    "DeviceRequest",
];

/// R5 CarePlan status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanStatus {
    /// Being prepared.
    Draft,
    /// In force.
    Active,
    /// Paused.
    OnHold,
    /// Withdrawn.
    Revoked,
    /// Carried out.
    Completed,
    /// Should never have existed.
    EnteredInError,
}

impl PlanStatus {
    /// The FHIR code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::OnHold => "on-hold",
            Self::Revoked => "revoked",
            Self::Completed => "completed",
            Self::EnteredInError => "entered-in-error",
        }
    }

    /// The status a FHIR code names.
    pub fn from_code(code: &str) -> Option<Self> {
        [
            Self::Draft,
            Self::Active,
            Self::OnHold,
            Self::Revoked,
            Self::Completed,
            Self::EnteredInError,
        ]
        .into_iter()
        .find(|s| s.code() == code)
    }

    /// Whether the plan is still open: drafted, in force or paused.
    pub fn is_open(self) -> bool {
        matches!(self, Self::Draft | Self::Active | Self::OnHold)
    }
}

/// A care plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CarePlan {
    /// Its id (a UUIDv7).
    pub id: String,
    /// The patient, `Patient/{id}`.
    pub patient: String,
    /// The facility.
    pub facility: String,
    /// Its title.
    pub title: String,
    /// What it is for, if written.
    pub description: Option<String>,
    /// Its status.
    pub status: PlanStatus,
    /// When it was drafted.
    pub created: Timestamp,
    /// When it ended, once it has.
    pub ended: Option<Timestamp>,
    /// Who wrote it.
    pub author: String,
    /// The Conditions it addresses, `Condition/{id}`.
    pub addresses: Vec<String>,
    /// Its activities: the patient's requests, `Type/id`.
    pub activities: Vec<String>,
}

/// Why a care-plan step is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanRefused {
    /// Missing its patient, facility, title or author.
    #[error("a care plan needs a patient, a title and who wrote it")]
    Incomplete,
    /// The status cannot change that way.
    #[error("a care plan that is {from} cannot become {to}")]
    Transition {
        /// From.
        from: &'static str,
        /// To.
        to: &'static str,
    },
    /// The plan has ended.
    #[error("the care plan has ended")]
    Ended,
    /// Not a request of a type a plan's activity may name.
    #[error("{0} is not a request a care plan's activity may name")]
    NotARequest(String),
    /// Not a Condition.
    #[error("{0} is not a Condition")]
    NotACondition(String),
}

/// What a draft names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// The id (a UUIDv7).
    pub id: String,
    /// The patient.
    pub patient: String,
    /// The facility.
    pub facility: String,
    /// Its title.
    pub title: String,
    /// What it is for.
    pub description: Option<String>,
    /// Who.
    pub author: String,
    /// When.
    pub at: Timestamp,
}

/// A draft plan.
pub fn draft(request: Draft) -> Result<CarePlan, PlanRefused> {
    if [
        &request.id,
        &request.patient,
        &request.facility,
        &request.title,
        &request.author,
    ]
    .iter()
    .any(|f| f.trim().is_empty())
    {
        return Err(PlanRefused::Incomplete);
    }
    Ok(CarePlan {
        id: request.id,
        patient: request.patient,
        facility: request.facility,
        title: request.title.trim().to_owned(),
        description: request
            .description
            .map(|d| d.trim().to_owned())
            .filter(|d| !d.is_empty()),
        status: PlanStatus::Draft,
        created: request.at,
        ended: None,
        author: request.author,
        addresses: Vec::new(),
        activities: Vec::new(),
    })
}

fn reference_type(reference: &str) -> &str {
    reference.split('/').next().unwrap_or_default()
}

impl CarePlan {
    /// The plan with status `to` from `at`.
    pub fn transition(&self, to: PlanStatus, at: Timestamp) -> Result<Self, PlanRefused> {
        use PlanStatus::{Active, Completed, Draft, EnteredInError, OnHold, Revoked};
        let allowed = matches!(
            (self.status, to),
            (Draft, Active | Revoked)
                | (Active, OnHold | Completed | Revoked)
                | (OnHold, Active | Revoked)
        ) || (to == EnteredInError && self.status.is_open());
        if !allowed {
            return Err(PlanRefused::Transition {
                from: self.status.code(),
                to: to.code(),
            });
        }
        Ok(Self {
            status: to,
            ended: if to.is_open() { None } else { Some(at) },
            ..self.clone()
        })
    }

    /// The plan addressing Condition `condition` too.
    pub fn address(&self, condition: &str) -> Result<Self, PlanRefused> {
        if !self.status.is_open() {
            return Err(PlanRefused::Ended);
        }
        if reference_type(condition) != "Condition" {
            return Err(PlanRefused::NotACondition(condition.to_owned()));
        }
        let mut next = self.clone();
        if !next.addresses.iter().any(|c| c == condition) {
            next.addresses.push(condition.to_owned());
        }
        Ok(next)
    }

    /// The plan with `request` among its activities, once.
    pub fn plan(&self, request: &str) -> Result<Self, PlanRefused> {
        if !self.status.is_open() {
            return Err(PlanRefused::Ended);
        }
        if !ACTIVITY_TYPES.contains(&reference_type(request)) {
            return Err(PlanRefused::NotARequest(request.to_owned()));
        }
        let mut next = self.clone();
        if !next.activities.iter().any(|a| a == request) {
            next.activities.push(request.to_owned());
        }
        Ok(next)
    }
}

#[cfg(test)]
mod tests;
