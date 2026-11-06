//! The Care Journey from FHIR R5 resources (#115; care-journeys.md).
//!
//! - **Which searches** ([`TYPES`]): one per type and patient, `{type}?patient=`, across the
//!   person's linked records, each under the reader's own search policy. A record the policy
//!   decision point withholds is never found, so it is never on the journey.
//! - **Mapping** ([`read`]):
//!   - EpisodeOfCare are the programs;
//!   - Encounters, Appointments, ServiceRequests (a referral when categorised as one),
//!     DiagnosticReports and MedicationRequests are the steps, in their program by
//!     `episodeOfCare` or the `workflow-episodeOfCare` extension;
//!   - Tasks are the work on them, by `focus`, open until completed, cancelled, failed, rejected
//!     or entered in error (ADR 0042).
//! - **What stays off:** anything entered in error, unverified results, and draft orders and
//!   prescriptions.

use health_access_policy::FACILITY_TAG_SYSTEM;
use health_care_journeys::{Journey, Program, Stage, Step, Work, journey};
use serde_json::Value;

/// The types the journey draws on, searched with `patient=`.
pub const TYPES: [&str; 7] = [
    "EpisodeOfCare",
    "Encounter",
    "Appointment",
    "ServiceRequest",
    "DiagnosticReport",
    "MedicationRequest",
    "Task",
];

/// The episode-of-care extension (ADR 0042).
pub const EPISODE_EXTENSION: &str =
    "http://hl7.org/fhir/StructureDefinition/workflow-episodeOfCare";

/// SNOMED CT's "patient referral", the category that makes a ServiceRequest a referral.
pub const REFERRAL: &str = "3457005";

fn text(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

/// What a person reads of a CodeableConcept, or of a CodeableReference's concept.
fn shown(concept: &Value) -> Option<String> {
    let concept = if concept["concept"].is_object() {
        &concept["concept"]
    } else {
        concept
    };
    text(&concept["text"])
        .or_else(|| text(&concept["coding"][0]["display"]))
        .or_else(|| text(&concept["coding"][0]["code"]))
}

fn reference(r: &Value) -> Option<String> {
    Some(format!(
        "{}/{}",
        r["resourceType"].as_str()?,
        r["id"].as_str()?
    ))
}

fn facility(r: &Value) -> Option<String> {
    r["meta"]["tag"]
        .as_array()?
        .iter()
        .find(|t| t["system"] == FACILITY_TAG_SYSTEM)
        .and_then(|t| text(&t["code"]))
}

fn episode(r: &Value) -> Option<String> {
    text(&r["episodeOfCare"][0]["reference"]).or_else(|| {
        r["extension"]
            .as_array()?
            .iter()
            .find(|e| e["url"] == EPISODE_EXTENSION)
            .and_then(|e| text(&e["valueReference"]["reference"]))
    })
}

fn referral(r: &Value) -> bool {
    r["category"].as_array().into_iter().flatten().any(|c| {
        c["coding"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|coding| coding["code"] == REFERRAL)
    })
}

fn off(stage: Stage, status: Option<&str>) -> bool {
    matches!(
        (stage, status),
        (_, Some("entered-in-error"))
            | (
                Stage::Result,
                Some("registered" | "partial" | "preliminary")
            )
            | (
                Stage::Order | Stage::Referral | Stage::Medication,
                Some("draft")
            )
    )
}

/// The step `r` is, if it is one.
fn step(r: &Value) -> Option<Step> {
    let (stage, at, title) = match r["resourceType"].as_str()? {
        "Encounter" => (
            Stage::Visit,
            text(&r["actualPeriod"]["start"]).or_else(|| text(&r["plannedStartDate"])),
            shown(&r["type"][0])
                .or_else(|| shown(&r["serviceType"][0]))
                .unwrap_or_else(|| "Visit".to_owned()),
        ),
        "Appointment" => (
            Stage::Appointment,
            text(&r["start"]).or_else(|| text(&r["created"])),
            text(&r["description"])
                .or_else(|| shown(&r["serviceType"][0]))
                .unwrap_or_else(|| "Appointment".to_owned()),
        ),
        "ServiceRequest" => (
            if referral(r) {
                Stage::Referral
            } else {
                Stage::Order
            },
            text(&r["authoredOn"]),
            shown(&r["code"]).unwrap_or_else(|| "Order".to_owned()),
        ),
        "DiagnosticReport" => (
            Stage::Result,
            text(&r["effectiveDateTime"]).or_else(|| text(&r["issued"])),
            shown(&r["code"]).unwrap_or_else(|| "Result".to_owned()),
        ),
        "MedicationRequest" => (
            Stage::Medication,
            text(&r["authoredOn"]),
            shown(&r["medication"]).unwrap_or_else(|| "Medication".to_owned()),
        ),
        _ => return None,
    };
    let status = text(&r["status"]);
    if off(stage, status.as_deref()) {
        return None;
    }
    Some(Step {
        reference: reference(r)?,
        version: text(&r["meta"]["versionId"])?,
        stage,
        at: at?,
        title,
        status,
        episode: episode(r),
        facility: facility(r),
        waiting: false,
    })
}

fn program(r: &Value) -> Option<Program> {
    if r["resourceType"] != "EpisodeOfCare" || r["status"] == "entered-in-error" {
        return None;
    }
    Some(Program {
        reference: reference(r)?,
        version: text(&r["meta"]["versionId"])?,
        title: shown(&r["type"][0]).unwrap_or_else(|| "Program".to_owned()),
        status: text(&r["status"]),
        start: text(&r["period"]["start"]),
        facility: facility(r),
    })
}

fn work(r: &Value) -> Option<Work> {
    if r["resourceType"] != "Task" {
        return None;
    }
    let done = matches!(
        r["status"].as_str(),
        Some("completed" | "cancelled" | "failed" | "rejected" | "entered-in-error")
    );
    Some(Work {
        task: reference(r)?,
        version: text(&r["meta"]["versionId"])?,
        focus: text(&r["focus"]["reference"])?,
        open: !done,
    })
}

/// The journey of what the searches found.
pub fn read<'a>(found: impl IntoIterator<Item = &'a Value>) -> Journey {
    let found: Vec<&Value> = found.into_iter().collect();
    journey(
        found.iter().filter_map(|r| program(r)).collect(),
        found.iter().filter_map(|r| step(r)).collect(),
        found.iter().filter_map(|r| work(r)).collect(),
    )
}
