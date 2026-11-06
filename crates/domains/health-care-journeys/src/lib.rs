//! The Care Journey (care-journeys.md; MASTER_PROMPT §61; #115): a longitudinal, patient-centred
//! projection of a person's care. It is a **read model**: it owns nothing, every step names its
//! authoritative resource and version, and it is rebuilt from those resources whenever it is read
//! (resource-ownership.md rule 8).
//!
//! [`journey`] arranges the person's care:
//! - **by program:** one [`Stretch`] per [`Program`] (EpisodeOfCare), the earliest start first,
//!   then one for care outside any program (or in one the reader may not see);
//! - **steps** (visits, appointments, orders, referrals, results, prescriptions) the earliest
//!   first, then by reference;
//! - **what waits:** a step is waiting while [`Work`] on it is open. Workflow state comes from Task
//!   projections, never from a step's clinical status (ADR 0042).
//!
//! It is a pure function of its inputs: the same resources, in any order or given twice, make the
//! same journey. A resource given in two versions counts once, at its newer version (version ids
//! are UUIDv7 operation ids, whose text orders them). Times are FHIR `dateTime` text in UTC, so
//! their text orders them.

use std::collections::BTreeMap;

use serde::Serialize;

/// What kind of step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// A visit (Encounter).
    Visit,
    /// An appointment.
    Appointment,
    /// An order (ServiceRequest).
    Order,
    /// A referral (ServiceRequest categorised as one).
    Referral,
    /// A result (DiagnosticReport).
    Result,
    /// A prescription (MedicationRequest).
    Medication,
}

/// Time in a program: an EpisodeOfCare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Program {
    /// `EpisodeOfCare/{id}`.
    pub reference: String,
    /// Its `meta.versionId`.
    pub version: String,
    /// What a person reads: the program.
    pub title: String,
    /// Its status code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// When it started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// The facility whose record it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facility: Option<String>,
}

/// One step of the journey.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    /// The authoritative resource, `Type/id`.
    pub reference: String,
    /// Its `meta.versionId`.
    pub version: String,
    /// What kind of step.
    pub stage: Stage,
    /// When it happened, or was made.
    pub at: String,
    /// What a person reads.
    pub title: String,
    /// Its status code, as its resource has it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The program it belongs to, `EpisodeOfCare/{id}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode: Option<String>,
    /// The facility whose record it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facility: Option<String>,
    /// Whether work on it is still open.
    pub waiting: bool,
}

/// Workflow on a step: a Task and its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Work {
    /// `Task/{id}`.
    pub task: String,
    /// Its `meta.versionId`.
    pub version: String,
    /// The step it is about, `Type/id`.
    pub focus: String,
    /// Whether it is still to be done.
    pub open: bool,
}

/// The steps of one program, or of care outside any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stretch {
    /// The program; `None` for care outside any program the reader sees.
    pub program: Option<Program>,
    /// Its steps, the earliest first.
    pub steps: Vec<Step>,
}

/// A person's care journey.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Journey {
    /// By program, the earliest start first; care outside any program last.
    pub stretches: Vec<Stretch>,
}

/// Each item once, at its newest version.
fn newest<T>(items: Vec<T>, key: impl Fn(&T) -> (&str, &str)) -> BTreeMap<String, T> {
    let mut kept: BTreeMap<String, T> = BTreeMap::new();
    for item in items {
        let (reference, version) = key(&item);
        let reference = reference.to_owned();
        let newer = kept
            .get(&reference)
            .is_none_or(|held| version > key(held).1);
        if newer {
            kept.insert(reference, item);
        }
    }
    kept
}

/// The journey of `programs`, `steps` and the `work` on them.
pub fn journey(programs: Vec<Program>, steps: Vec<Step>, work: Vec<Work>) -> Journey {
    let programs = newest(programs, |p| (&p.reference, &p.version));
    let work = newest(work, |w| (&w.task, &w.version));
    let mut steps: Vec<Step> = newest(steps, |s| (&s.reference, &s.version))
        .into_values()
        .map(|mut step| {
            step.waiting = work.values().any(|w| w.open && w.focus == step.reference);
            step
        })
        .collect();
    steps.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.reference.cmp(&b.reference)));

    let mut by_program: BTreeMap<String, Vec<Step>> = BTreeMap::new();
    let mut outside = Vec::new();
    for step in steps {
        match step.episode.as_ref().filter(|e| programs.contains_key(*e)) {
            Some(episode) => by_program.entry(episode.clone()).or_default().push(step),
            None => outside.push(step),
        }
    }
    let mut programs: Vec<Program> = programs.into_values().collect();
    programs.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then_with(|| a.reference.cmp(&b.reference))
    });
    let mut stretches: Vec<Stretch> = programs
        .into_iter()
        .map(|program| Stretch {
            steps: by_program.remove(&program.reference).unwrap_or_default(),
            program: Some(program),
        })
        .collect();
    if !outside.is_empty() {
        stretches.push(Stretch {
            program: None,
            steps: outside,
        });
    }
    Journey { stretches }
}

impl Journey {
    /// The steps still waiting on work, the earliest first.
    pub fn waiting(&self) -> Vec<&Step> {
        let mut waiting: Vec<&Step> = self
            .stretches
            .iter()
            .flat_map(|s| &s.steps)
            .filter(|s| s.waiting)
            .collect();
        waiting.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.reference.cmp(&b.reference)));
        waiting
    }
}

#[cfg(test)]
mod tests;
