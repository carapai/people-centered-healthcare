//! The shared record's patient summary (clinical-workspaces.md § What the workspaces are built on;
//! #104): a read model over the patient's records, through the golden record, that a clinician
//! sees first. The records come in already filtered by what the clinician may see: sensitive
//! labels are the policy decision point's, and a record withheld never reaches the summary.
//!
//! [`summarize`] keeps what is current and orders it:
//! - **allergies** that are not resolved, refuted or entered in error, the high-criticality
//!   ones first;
//! - **conditions** that are active, recurring or relapsing, and not refuted or entered in
//!   error, the latest onset first;
//! - **medications** that are active or on hold, the latest first;
//! - **vitals:** the latest reading of each code, leaving out cancelled and entered-in-error
//!   ones;
//! - **visits:** the latest [`VISITS`] Encounters, leaving out those entered in error;
//! - **programs:** the active and waitlisted EpisodesOfCare.
//!
//! Dates are FHIR `date` or `dateTime` text in UTC, so their text orders them.

use serde::Serialize;

/// How many visits the summary shows.
pub const VISITS: usize = 10;

/// A coded concept, as shown: its text, and the code it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Coded {
    /// What a person reads: the display, else the text, else the code.
    pub text: String,
    /// The code system.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// The code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// An AllergyIntolerance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Allergy {
    /// `AllergyIntolerance/{id}`.
    pub reference: String,
    /// The substance.
    pub substance: Coded,
    /// `low`, `high` or `unable-to-assess`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub criticality: Option<String>,
    /// The clinical status code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clinical_status: Option<String>,
    /// The verification status code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<String>,
    /// When it was recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recorded: Option<String>,
}

/// A Condition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    /// `Condition/{id}`.
    pub reference: String,
    /// What it is.
    pub code: Coded,
    /// The clinical status code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clinical_status: Option<String>,
    /// The verification status code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<String>,
    /// When it began.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onset: Option<String>,
}

/// A MedicationRequest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Medication {
    /// `MedicationRequest/{id}`.
    pub reference: String,
    /// The medication.
    pub medication: Coded,
    /// Its status code.
    pub status: String,
    /// The dosage, as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dosage: Option<String>,
    /// When it was written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authored: Option<String>,
}

/// A vital-signs Observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vital {
    /// `Observation/{id}`.
    pub reference: String,
    /// What was measured.
    pub code: Coded,
    /// The value, as exact text (`61.50`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Its unit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Its status code.
    pub status: String,
    /// When.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective: Option<String>,
}

/// An Encounter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Visit {
    /// `Encounter/{id}`.
    pub reference: String,
    /// Its status code.
    pub status: String,
    /// Its class code (`AMB`, `EMER`, `IMP`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    /// When it started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// Where: the location's display.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

/// An EpisodeOfCare: the patient's enrolment in a program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Program {
    /// `EpisodeOfCare/{id}`.
    pub reference: String,
    /// The program.
    pub kind: Coded,
    /// Its status code.
    pub status: String,
    /// When it started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
}

/// The records a summary is drawn from. A section the clinician may not see at all is `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    /// AllergyIntolerances.
    pub allergies: Option<Vec<Allergy>>,
    /// Conditions.
    pub conditions: Option<Vec<Condition>>,
    /// MedicationRequests.
    pub medications: Option<Vec<Medication>>,
    /// Vital-signs Observations.
    pub vitals: Option<Vec<Vital>>,
    /// Encounters.
    pub visits: Option<Vec<Visit>>,
    /// EpisodesOfCare.
    pub programs: Option<Vec<Program>>,
}

/// The patient summary. A section the clinician may not see is `null`, not empty.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Current allergies.
    pub allergies: Option<Vec<Allergy>>,
    /// Current conditions.
    pub conditions: Option<Vec<Condition>>,
    /// Current medications.
    pub medications: Option<Vec<Medication>>,
    /// The latest reading of each vital sign.
    pub vitals: Option<Vec<Vital>>,
    /// The latest visits.
    pub visits: Option<Vec<Visit>>,
    /// Current programs.
    pub programs: Option<Vec<Program>>,
}

fn is(value: Option<&String>, codes: &[&str]) -> bool {
    value.is_some_and(|v| codes.contains(&v.as_str()))
}

/// Latest first; undated last.
fn latest_first<T>(items: &mut [T], date: impl Fn(&T) -> Option<&String>) {
    items.sort_by(|a, b| date(b).cmp(&date(a)));
}

/// The summary of `record`.
pub fn summarize(record: Record) -> Summary {
    let allergies = record.allergies.map(|mut all| {
        all.retain(|a| {
            !is(a.clinical_status.as_ref(), &["inactive", "resolved"])
                && !is(a.verification.as_ref(), &["refuted", "entered-in-error"])
        });
        latest_first(&mut all, |a| a.recorded.as_ref());
        // Stable: high criticality first, each group still latest first.
        all.sort_by_key(|a| a.criticality.as_deref() != Some("high"));
        all
    });
    let conditions = record.conditions.map(|mut all| {
        all.retain(|c| {
            is(
                c.clinical_status.as_ref(),
                &["active", "recurrence", "relapse"],
            ) && !is(c.verification.as_ref(), &["refuted", "entered-in-error"])
        });
        latest_first(&mut all, |c| c.onset.as_ref());
        all
    });
    let medications = record.medications.map(|mut all| {
        all.retain(|m| matches!(m.status.as_str(), "active" | "on-hold"));
        latest_first(&mut all, |m| m.authored.as_ref());
        all
    });
    let vitals = record.vitals.map(|mut all| {
        all.retain(|v| !matches!(v.status.as_str(), "cancelled" | "entered-in-error"));
        latest_first(&mut all, |v| v.effective.as_ref());
        let mut seen: Vec<(Option<String>, Option<String>, String)> = Vec::new();
        all.retain(|v| {
            let key = (
                v.code.system.clone(),
                v.code.code.clone(),
                v.code.text.clone(),
            );
            if seen.contains(&key) {
                false
            } else {
                seen.push(key);
                true
            }
        });
        all
    });
    let visits = record.visits.map(|mut all| {
        all.retain(|v| v.status != "entered-in-error");
        latest_first(&mut all, |v| v.start.as_ref());
        all.truncate(VISITS);
        all
    });
    let programs = record.programs.map(|mut all| {
        all.retain(|p| matches!(p.status.as_str(), "active" | "waitlist"));
        latest_first(&mut all, |p| p.start.as_ref());
        all
    });
    Summary {
        allergies,
        conditions,
        medications,
        vitals,
        visits,
        programs,
    }
}

#[cfg(test)]
mod tests;
