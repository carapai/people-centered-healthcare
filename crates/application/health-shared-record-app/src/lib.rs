//! The shared record's patient summary from FHIR resources (clinical-workspaces.md; #104).
//!
//! - **Whose records** ([`linked`]): the patient's, and, through the golden record its `refer`
//!   link names, those of every patient record the golden record links (`seealso`): the same
//!   person registered at other facilities (patient-identity.md).
//! - **Which searches** ([`SECTIONS`]): one per section and patient, `{type}?patient=`. The node
//!   runs each under the clinician's own search policy, so a sensitive record the clinician may
//!   not see is never found, and a section the clinician may not search at all is `None`.
//! - **Mapping** ([`record`]): FHIR R5 into the summary's records. A resource missing what a
//!   section needs (an Observation that is not a vital sign, a MedicationRequest without a status)
//!   is left out.

pub mod timeline;

use health_shared_record::{
    Allergy, Coded, Condition, Medication, Program, Record, Summary, Visit, Vital, summarize,
};
use serde_json::Value;

/// The resource type of each section, searched with `patient=`.
pub const SECTIONS: [&str; 6] = [
    "AllergyIntolerance",
    "Condition",
    "MedicationRequest",
    "Observation",
    "Encounter",
    "EpisodeOfCare",
];

/// What the searches found, per section, in [`SECTIONS`] order: `None` where the clinician may
/// not search that type.
pub type Found = [Option<Vec<Value>>; 6];

fn text(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

/// A CodeableConcept as shown.
fn coded(concept: &Value) -> Coded {
    let coding = &concept["coding"][0];
    let shown = text(&coding["display"])
        .or_else(|| text(&concept["text"]))
        .or_else(|| text(&coding["code"]))
        .unwrap_or_default();
    Coded {
        text: shown,
        system: text(&coding["system"]),
        code: text(&coding["code"]),
    }
}

/// The first coding's code of a CodeableConcept.
fn code(concept: &Value) -> Option<String> {
    text(&concept["coding"][0]["code"])
}

fn reference(resource: &Value) -> Option<String> {
    Some(format!(
        "{}/{}",
        resource["resourceType"].as_str()?,
        resource["id"].as_str()?
    ))
}

fn allergy(r: &Value) -> Option<Allergy> {
    Some(Allergy {
        reference: reference(r)?,
        substance: coded(&r["code"]),
        criticality: text(&r["criticality"]),
        clinical_status: code(&r["clinicalStatus"]),
        verification: code(&r["verificationStatus"]),
        recorded: text(&r["recordedDate"]),
    })
}

fn condition(r: &Value) -> Option<Condition> {
    Some(Condition {
        reference: reference(r)?,
        code: coded(&r["code"]),
        clinical_status: code(&r["clinicalStatus"]),
        verification: code(&r["verificationStatus"]),
        onset: text(&r["onsetDateTime"]).or_else(|| text(&r["recordedDate"])),
    })
}

fn medication(r: &Value) -> Option<Medication> {
    Some(Medication {
        reference: reference(r)?,
        medication: coded(&r["medication"]["concept"]),
        status: text(&r["status"])?,
        dosage: text(&r["dosageInstruction"][0]["text"]),
        authored: text(&r["authoredOn"]),
    })
}

fn is_vital(observation: &Value) -> bool {
    observation["category"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|c| c["coding"].as_array().into_iter().flatten())
        .any(|c| c["code"] == "vital-signs")
}

/// A panel's component quantities in one unit, as one value: blood pressure reads `104/66` in
/// `mmHg`. `None` unless every component is a quantity in the same unit.
fn components(r: &Value) -> Option<(String, Option<String>)> {
    let parts = r["component"].as_array().filter(|c| !c.is_empty())?;
    let mut values = Vec::with_capacity(parts.len());
    let mut unit: Option<Option<String>> = None;
    for part in parts {
        let quantity = &part["valueQuantity"];
        let this = text(&quantity["unit"]).or_else(|| text(&quantity["code"]));
        match &unit {
            None => unit = Some(this),
            Some(seen) if *seen == this => {}
            Some(_) => return None,
        }
        values.push(quantity.get("value")?.to_string());
    }
    Some((values.join("/"), unit.flatten()))
}

fn vital(r: &Value) -> Option<Vital> {
    if !is_vital(r) {
        return None;
    }
    let quantity = &r["valueQuantity"];
    let (value, unit) = match quantity.get("value") {
        // The number's own text, so a decimal keeps its scale.
        Some(value) => (
            Some(value.to_string()),
            text(&quantity["unit"]).or_else(|| text(&quantity["code"])),
        ),
        None => match components(r) {
            Some((values, unit)) => (Some(values), unit),
            None => (text(&r["valueString"]), None),
        },
    };
    Some(Vital {
        reference: reference(r)?,
        code: coded(&r["code"]),
        value,
        unit,
        status: text(&r["status"])?,
        effective: text(&r["effectiveDateTime"]).or_else(|| text(&r["effectivePeriod"]["start"])),
    })
}

fn visit(r: &Value) -> Option<Visit> {
    Some(Visit {
        reference: reference(r)?,
        status: text(&r["status"])?,
        class: code(&r["class"][0]),
        start: text(&r["actualPeriod"]["start"]),
        location: text(&r["location"][0]["location"]["display"]),
    })
}

fn program(r: &Value) -> Option<Program> {
    Some(Program {
        reference: reference(r)?,
        kind: coded(&r["type"][0]),
        status: text(&r["status"])?,
        start: text(&r["period"]["start"]),
    })
}

fn section<T>(found: &Option<Vec<Value>>, map: fn(&Value) -> Option<T>) -> Option<Vec<T>> {
    found
        .as_ref()
        .map(|all| all.iter().filter_map(map).collect())
}

/// The records `found` holds.
pub fn record(found: &Found) -> Record {
    Record {
        allergies: section(&found[0], allergy),
        conditions: section(&found[1], condition),
        medications: section(&found[2], medication),
        vitals: section(&found[3], vital),
        visits: section(&found[4], visit),
        programs: section(&found[5], program),
    }
}

/// The patient summary of what the searches `found`.
pub fn summary(found: &Found) -> Summary {
    summarize(record(found))
}

fn links<'a>(patient: &'a Value, kinds: &'a [&str]) -> impl Iterator<Item = String> + 'a {
    patient["link"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(move |l| l["type"].as_str().is_some_and(|t| kinds.contains(&t)))
        .filter_map(|l| text(&l["other"]["reference"]))
        .filter(|r| r.starts_with("Patient/"))
}

/// The golden record `patient` refers to, if it names one.
pub fn golden_of(patient: &Value) -> Option<String> {
    links(patient, &["refer"]).next()
}

/// The patients whose records make up `patient`'s summary: `patient` itself, then every other
/// patient record its golden record links. Each once.
pub fn linked(patient: &Value, golden: Option<&Value>) -> Vec<String> {
    let mut all: Vec<String> = reference(patient).into_iter().collect();
    let others = golden
        .into_iter()
        .flat_map(|g| links(g, &["seealso"]).collect::<Vec<_>>());
    for other in others {
        if !all.contains(&other) {
            all.push(other);
        }
    }
    all
}
