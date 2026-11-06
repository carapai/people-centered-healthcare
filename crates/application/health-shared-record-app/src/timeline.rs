//! The longitudinal timeline from FHIR R5 resources (#115; care-journeys.md).
//!
//! - **Which searches** ([`TIMELINE`]): one per type and patient, `{type}?patient=`, across the
//!   patient's linked records ([`crate::linked`]), each under the reader's own search policy, so a
//!   record the policy decision point withholds is never found.
//! - **Mapping** ([`change`]): each resource version becomes a [`Change`]. What is entered in
//!   error, cancelled before it happened, a draft, or a result not yet verified is
//!   [`Change::Gone`]: it is not on the timeline.
//! - **Program context:** `Encounter.episodeOfCare`, else the `workflow-episodeOfCare` extension.
//! - [`timeline`] rebuilds the timeline from what the searches found.

use health_shared_record::timeline::{Change, Entry, Kind, Timeline};
use serde_json::Value;

/// The types on the timeline, searched with `patient=`.
pub const TIMELINE: [&str; 9] = [
    "Encounter",
    "EpisodeOfCare",
    "Observation",
    "ServiceRequest",
    "MedicationRequest",
    "DiagnosticReport",
    "Appointment",
    "Composition",
    "Task",
];

/// The episode-of-care extension (ADR 0042).
pub const EPISODE_EXTENSION: &str =
    "http://hl7.org/fhir/StructureDefinition/workflow-episodeOfCare";

/// The facility tag's system, as `health-access-policy` stamps it.
const FACILITY_TAG: &str = "urn:health:facility";

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

fn episode(resource: &Value) -> Option<String> {
    text(&resource["episodeOfCare"][0]["reference"]).or_else(|| {
        resource["extension"]
            .as_array()?
            .iter()
            .find(|e| e["url"] == EPISODE_EXTENSION)
            .and_then(|e| text(&e["valueReference"]["reference"]))
    })
}

fn facility(resource: &Value) -> Option<String> {
    resource["meta"]["tag"]
        .as_array()?
        .iter()
        .find(|t| t["system"] == FACILITY_TAG)
        .and_then(|t| text(&t["code"]))
}

/// Whether a version of `kind` with `status` stays off the timeline.
fn off(kind: Kind, status: Option<&str>) -> bool {
    match (kind, status) {
        (_, Some("entered-in-error")) => true,
        // Drafts are not yet part of the record.
        (Kind::Note, Some("preliminary")) => true,
        // A result reaches the record once verified (#109).
        (Kind::Result, Some("registered" | "partial" | "preliminary")) => true,
        (Kind::Observation, Some("cancelled" | "registered")) => true,
        (Kind::Medication | Kind::Order, Some("draft")) => true,
        _ => false,
    }
}

/// What a version of `resource` is on the timeline. `None` if it is not a timeline type, or has
/// no id, version or time.
pub fn change(resource: &Value) -> Option<Change> {
    let r = resource;
    let (kind, at, title) = match r["resourceType"].as_str()? {
        "Encounter" => (
            Kind::Visit,
            text(&r["actualPeriod"]["start"]).or_else(|| text(&r["plannedStartDate"])),
            shown(&r["type"][0])
                .or_else(|| shown(&r["serviceType"][0]))
                .or_else(|| shown(&r["class"][0]))
                .unwrap_or_else(|| "Visit".to_owned()),
        ),
        "EpisodeOfCare" => (
            Kind::Program,
            text(&r["period"]["start"]),
            shown(&r["type"][0]).unwrap_or_else(|| "Program".to_owned()),
        ),
        "Observation" => (
            Kind::Observation,
            text(&r["effectiveDateTime"]).or_else(|| text(&r["issued"])),
            shown(&r["code"]).unwrap_or_else(|| "Observation".to_owned()),
        ),
        "ServiceRequest" => (
            Kind::Order,
            text(&r["authoredOn"]),
            shown(&r["code"]).unwrap_or_else(|| "Order".to_owned()),
        ),
        "MedicationRequest" => (
            Kind::Medication,
            text(&r["authoredOn"]),
            shown(&r["medication"]).unwrap_or_else(|| "Medication".to_owned()),
        ),
        "DiagnosticReport" => (
            Kind::Result,
            text(&r["effectiveDateTime"]).or_else(|| text(&r["issued"])),
            shown(&r["code"]).unwrap_or_else(|| "Result".to_owned()),
        ),
        "Appointment" => (
            Kind::Appointment,
            text(&r["start"]).or_else(|| text(&r["created"])),
            text(&r["description"])
                .or_else(|| shown(&r["serviceType"][0]))
                .unwrap_or_else(|| "Appointment".to_owned()),
        ),
        "Composition" => (
            Kind::Note,
            text(&r["date"]),
            text(&r["title"]).unwrap_or_else(|| "Note".to_owned()),
        ),
        "Task" => (
            Kind::Task,
            text(&r["authoredOn"]).or_else(|| text(&r["lastModified"])),
            text(&r["description"])
                .or_else(|| shown(&r["code"]))
                .unwrap_or_else(|| "Task".to_owned()),
        ),
        _ => return None,
    };
    let reference = format!("{}/{}", r["resourceType"].as_str()?, r["id"].as_str()?);
    let version = text(&r["meta"]["versionId"])?;
    let status = text(&r["status"]);
    if off(kind, status.as_deref()) {
        return Some(Change::Gone { reference, version });
    }
    Some(Change::Shown(Entry {
        reference,
        version,
        kind,
        at: at?,
        title,
        status,
        episode: episode(r),
        facility: facility(r),
    }))
}

/// The timeline of what the searches found.
pub fn timeline<'a>(found: impl IntoIterator<Item = &'a Value>) -> Timeline {
    Timeline::rebuild(found.into_iter().filter_map(change))
}
