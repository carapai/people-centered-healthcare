//! Prescribing (inventory-and-procurement.md § Dispensing and other consumption;
//! resource-ownership.md; #108). The MedicationRequest is the medications module's.
//!
//! - **Prescribing** ([`prescribe`]) writes an `active` prescription. It has **no stock effect**:
//!   it reserves nothing and moves nothing (ADR 0035: reservations are never made by a
//!   MedicationRequest). It appears on the pharmacy worklist, where dispensing starts.
//! - **Clinical safety checks** ([`SafetyChecks`]) run before a prescription is written. Clinical
//!   decision support is still in the map's fog, so the hook ships with [`BasicChecks`] (the same
//!   medicine already prescribed; a medicine matching a recorded allergy). A warning is not a
//!   refusal: the prescriber acknowledges it, and only then is the prescription written.
//! - **The lifecycle** ([`Prescription::transition`]) follows R5: active and on hold, then
//!   completed, stopped or cancelled; entered in error while open.
//! - **The pharmacy worklist** ([`worklist`]): the facility's open prescriptions, oldest first.

use health_time::Timestamp;
use serde::{Deserialize, Serialize};

/// A medicine as prescribed: what a person reads, and its code if it has one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Medicine {
    /// What a person reads.
    pub text: String,
    /// The code system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// The code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl Medicine {
    /// Whether `other` is the same medicine: the same code, or without codes the same text.
    pub fn same_as(&self, other: &Self) -> bool {
        match (&self.system, &self.code, &other.system, &other.code) {
            (Some(s1), Some(c1), Some(s2), Some(c2)) => s1 == s2 && c1 == c2,
            _ => self.text.trim().eq_ignore_ascii_case(other.text.trim()),
        }
    }
}

/// R5 MedicationRequest status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrescriptionStatus {
    /// To be dispensed and taken.
    Active,
    /// Paused.
    OnHold,
    /// Withdrawn before any was taken.
    Cancelled,
    /// All of it was given.
    Completed,
    /// Stopped part way.
    Stopped,
    /// Should never have existed.
    EnteredInError,
}

impl PrescriptionStatus {
    /// The FHIR code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::OnHold => "on-hold",
            Self::Cancelled => "cancelled",
            Self::Completed => "completed",
            Self::Stopped => "stopped",
            Self::EnteredInError => "entered-in-error",
        }
    }

    /// The status a FHIR code names.
    pub fn from_code(code: &str) -> Option<Self> {
        [
            Self::Active,
            Self::OnHold,
            Self::Cancelled,
            Self::Completed,
            Self::Stopped,
            Self::EnteredInError,
        ]
        .into_iter()
        .find(|s| s.code() == code)
    }

    /// Whether the prescription is still to be dispensed or taken.
    pub fn is_open(self) -> bool {
        matches!(self, Self::Active | Self::OnHold)
    }
}

/// A prescription.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prescription {
    /// Its id (a UUIDv7).
    pub id: String,
    /// The patient, `Patient/{id}`.
    pub patient: String,
    /// The Encounter it was written in, if any.
    pub encounter: Option<String>,
    /// The prescribing facility.
    pub facility: String,
    /// What.
    pub medicine: Medicine,
    /// The dosage, as written.
    pub dosage: String,
    /// The course in days, if stated (FEFO selection needs it at dispensing).
    pub days: Option<u32>,
    /// Its status.
    pub status: PrescriptionStatus,
    /// Who prescribed it.
    pub prescriber: String,
    /// When.
    pub authored: Timestamp,
}

/// What a prescription names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prescribe {
    /// The new prescription's id (a UUIDv7).
    pub id: String,
    /// The patient.
    pub patient: String,
    /// The Encounter, if any.
    pub encounter: Option<String>,
    /// The facility.
    pub facility: String,
    /// What.
    pub medicine: Medicine,
    /// The dosage, as written.
    pub dosage: String,
    /// The course in days, if stated.
    pub days: Option<u32>,
    /// Who.
    pub prescriber: String,
    /// When.
    pub at: Timestamp,
}

/// What happened to a prescription, on the outbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum MedicationEvent {
    /// A prescription was written: the pharmacy worklist follows it.
    Prescribed {
        /// The prescription.
        prescription: String,
        /// The patient.
        patient: String,
        /// The facility.
        facility: String,
        /// When.
        at: Timestamp,
    },
    /// Its status changed.
    StatusChanged {
        /// The prescription.
        prescription: String,
        /// The new status.
        status: PrescriptionStatus,
        /// When.
        at: Timestamp,
    },
}

impl MedicationEvent {
    /// The event's name on the outbox.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Prescribed { .. } => "medications.prescribed.v1",
            Self::StatusChanged { .. } => "medications.status-changed.v1",
        }
    }
}

/// Why a prescribing step is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PrescriptionRefused {
    /// Missing its patient, facility, medicine, dosage or prescriber.
    #[error("a prescription needs a patient, a medicine, a dosage and its prescriber")]
    Incomplete,
    /// A course of no days.
    #[error("a course lasts at least a day")]
    NoCourse,
    /// The status cannot change that way.
    #[error("a prescription that is {from} cannot become {to}")]
    Transition {
        /// From.
        from: &'static str,
        /// To.
        to: &'static str,
    },
}

/// Writes the prescription: `active`, and its [`MedicationEvent::Prescribed`].
pub fn prescribe(
    request: &Prescribe,
) -> Result<(Prescription, MedicationEvent), PrescriptionRefused> {
    if [
        &request.id,
        &request.patient,
        &request.facility,
        &request.medicine.text,
        &request.dosage,
        &request.prescriber,
    ]
    .iter()
    .any(|f| f.trim().is_empty())
    {
        return Err(PrescriptionRefused::Incomplete);
    }
    if request.days == Some(0) {
        return Err(PrescriptionRefused::NoCourse);
    }
    let prescription = Prescription {
        id: request.id.clone(),
        patient: request.patient.clone(),
        encounter: request.encounter.clone(),
        facility: request.facility.clone(),
        medicine: request.medicine.clone(),
        dosage: request.dosage.trim().to_owned(),
        days: request.days,
        status: PrescriptionStatus::Active,
        prescriber: request.prescriber.clone(),
        authored: request.at,
    };
    let event = MedicationEvent::Prescribed {
        prescription: prescription.id.clone(),
        patient: prescription.patient.clone(),
        facility: prescription.facility.clone(),
        at: request.at,
    };
    Ok((prescription, event))
}

impl Prescription {
    /// The prescription with status `to` from `at`, and its event.
    pub fn transition(
        &self,
        to: PrescriptionStatus,
        at: Timestamp,
    ) -> Result<(Self, MedicationEvent), PrescriptionRefused> {
        use PrescriptionStatus::{Active, Cancelled, Completed, EnteredInError, OnHold, Stopped};
        let allowed = matches!(
            (self.status, to),
            (Active, OnHold | Cancelled | Completed | Stopped)
                | (OnHold, Active | Cancelled | Stopped)
        ) || (to == EnteredInError && self.status.is_open());
        if !allowed {
            return Err(PrescriptionRefused::Transition {
                from: self.status.code(),
                to: to.code(),
            });
        }
        let next = Self {
            status: to,
            ..self.clone()
        };
        let event = MedicationEvent::StatusChanged {
            prescription: self.id.clone(),
            status: to,
            at,
        };
        Ok((next, event))
    }
}

/// A clinical safety warning the prescriber must acknowledge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warning {
    /// What kind: `duplicate-therapy`, `allergy`, ….
    pub code: String,
    /// What a person reads.
    pub message: String,
}

/// What the safety checks know of the patient.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Patient {
    /// Their open prescriptions.
    pub prescriptions: Vec<Prescription>,
    /// The substances of their current allergies, as recorded.
    pub allergies: Vec<String>,
}

/// The clinical safety checks hook: decision support plugs in here (the map's CDS fog).
pub trait SafetyChecks: Send + Sync {
    /// The warnings for prescribing `new` to `patient`.
    fn check(&self, new: &Prescription, patient: &Patient) -> Vec<Warning>;
}

/// The checks the platform ships: the same medicine already open, and a medicine whose name
/// contains a recorded allergy's substance.
#[derive(Debug, Clone, Copy, Default)]
pub struct BasicChecks;

impl SafetyChecks for BasicChecks {
    fn check(&self, new: &Prescription, patient: &Patient) -> Vec<Warning> {
        let mut warnings = Vec::new();
        for open in patient
            .prescriptions
            .iter()
            .filter(|p| p.status.is_open() && p.id != new.id)
        {
            if open.medicine.same_as(&new.medicine) {
                warnings.push(Warning {
                    code: "duplicate-therapy".into(),
                    message: format!(
                        "{} is already prescribed: {}",
                        open.medicine.text, open.dosage
                    ),
                });
            }
        }
        let name = new.medicine.text.to_lowercase();
        for substance in &patient.allergies {
            let substance_lower = substance.trim().to_lowercase();
            if !substance_lower.is_empty() && name.contains(&substance_lower) {
                warnings.push(Warning {
                    code: "allergy".into(),
                    message: format!(
                        "the patient is recorded as allergic to {}",
                        substance.trim()
                    ),
                });
            }
        }
        warnings
    }
}

/// The pharmacy worklist of `facility`: its open prescriptions, oldest first.
pub fn worklist(prescriptions: &[Prescription], facility: &str) -> Vec<Prescription> {
    let mut open: Vec<Prescription> = prescriptions
        .iter()
        .filter(|p| p.facility == facility && p.status.is_open())
        .cloned()
        .collect();
    open.sort_by_key(|p| p.authored);
    open
}

#[cfg(test)]
mod tests;
