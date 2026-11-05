//! The consultation's commands (#104; clinical-workspaces.md § Orders): what a clinician
//! records in an Encounter, as small commands the node turns into FHIR R5. The UI never assembles
//! the resources. Each builder takes the Encounter it belongs to and checks the command:
//!
//! - [`note`]: a consultation note, a final Composition (LOINC 11488-4) signed by its author,
//!   whose narrative is the text, escaped;
//! - [`diagnosis`]: an encounter-diagnosis Condition, active and confirmed (or provisional);
//! - [`service_order`]: a ServiceRequest (a test or procedure) to order;
//! - [`prescription`]: a MedicationRequest to order, with its dosage as written.
//!
//! Every resource names the patient and the Encounter, and is tagged with the Encounter's
//! facility (ADR 0057). Its id is the client's UUIDv7 (ADR 0013), so a retried command writes the
//! same resource.

use health_access_policy::FACILITY_TAG_SYSTEM;
use serde_json::{Value, json};

/// Why a command was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandRefused {
    /// Something the command needs is missing or blank.
    #[error("the command needs {0}")]
    Missing(&'static str),
    /// A value is not one the command takes.
    #[error("{0}")]
    Invalid(String),
}

/// The Encounter a command is recorded in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    /// `Encounter/{id}`.
    pub encounter: String,
    /// `Patient/{id}`.
    pub patient: String,
    /// The Encounter's facility.
    pub facility: String,
    /// The account recording it.
    pub author: String,
    /// When, as an instant.
    pub at: String,
}

/// A coded concept as the client names it: a display, and optionally its code.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Concept {
    /// What a person reads.
    pub display: String,
    /// The code system.
    #[serde(default)]
    pub system: Option<String>,
    /// The code.
    #[serde(default)]
    pub code: Option<String>,
}

impl Concept {
    fn concept(&self) -> Result<Value, CommandRefused> {
        let display = self.display.trim();
        if display.is_empty() {
            return Err(CommandRefused::Missing("what it is"));
        }
        match (&self.system, &self.code) {
            (Some(system), Some(code)) if !system.trim().is_empty() && !code.trim().is_empty() => {
                Ok(
                    json!({ "coding": [{ "system": system, "code": code, "display": display }], "text": display }),
                )
            }
            (None, None) => Ok(json!({ "text": display })),
            _ => Err(CommandRefused::Invalid(
                "a code needs both its system and its code".into(),
            )),
        }
    }
}

fn base(resource_type: &str, id: &str, context: &Context) -> Value {
    json!({
        "resourceType": resource_type,
        "id": id,
        "meta": { "tag": [{ "system": FACILITY_TAG_SYSTEM, "code": context.facility }] },
    })
}

/// `text` as XHTML narrative.
fn narrative(text: &str) -> Value {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', "<br/>");
    json!({
        "status": "generated",
        "div": format!("<div xmlns=\"http://www.w3.org/1999/xhtml\"><p>{escaped}</p></div>")
    })
}

/// A consultation note.
pub fn note(id: &str, context: &Context, text: &str) -> Result<Value, CommandRefused> {
    if text.trim().is_empty() {
        return Err(CommandRefused::Missing("the note's text"));
    }
    let mut note = base("Composition", id, context);
    let fields = json!({
        "status": "final",
        "type": { "coding": [{ "system": "http://loinc.org", "code": "11488-4", "display": "Consult note" }] },
        "subject": [{ "reference": context.patient }],
        "encounter": { "reference": context.encounter },
        "date": context.at,
        "author": [{ "display": context.author }],
        // Saved final, so signed by its author as it is saved (#107).
        "attester": [{
            "mode": { "coding": [{ "system": "http://hl7.org/fhir/composition-attestation-mode", "code": "legal" }] },
            "time": context.at,
            "party": { "display": context.author }
        }],
        "title": "Consultation note",
        "section": [{ "title": "Note", "text": narrative(text.trim()) }]
    });
    merge(&mut note, fields);
    Ok(note)
}

/// An encounter diagnosis; `provisional` until confirmed.
pub fn diagnosis(
    id: &str,
    context: &Context,
    concept: &Concept,
    provisional: bool,
) -> Result<Value, CommandRefused> {
    let mut condition = base("Condition", id, context);
    let fields = json!({
        "clinicalStatus": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active" }] },
        "verificationStatus": { "coding": [{
            "system": "http://terminology.hl7.org/CodeSystem/condition-ver-status",
            "code": if provisional { "provisional" } else { "confirmed" }
        }] },
        "category": [{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/condition-category", "code": "encounter-diagnosis" }] }],
        "code": concept.concept()?,
        "subject": { "reference": context.patient },
        "encounter": { "reference": context.encounter },
        "onsetDateTime": context.at,
        "recordedDate": context.at,
        "participant": [{
            "function": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/provenance-participant-type", "code": "author" }] },
            "actor": { "display": context.author }
        }]
    });
    merge(&mut condition, fields);
    Ok(condition)
}

/// A test or procedure to order.
pub fn service_order(
    id: &str,
    context: &Context,
    concept: &Concept,
    note: Option<&str>,
) -> Result<Value, CommandRefused> {
    let mut order = base("ServiceRequest", id, context);
    let mut fields = json!({
        "status": "active",
        "intent": "order",
        "code": { "concept": concept.concept()? },
        "subject": { "reference": context.patient },
        "encounter": { "reference": context.encounter },
        "authoredOn": context.at,
        "requester": { "display": context.author }
    });
    if let Some(note) = note.map(str::trim).filter(|n| !n.is_empty()) {
        fields["note"] = json!([{ "text": note }]);
    }
    merge(&mut order, fields);
    Ok(order)
}

/// A medication to order, with its dosage as written.
pub fn prescription(
    id: &str,
    context: &Context,
    concept: &Concept,
    dosage: &str,
) -> Result<Value, CommandRefused> {
    if dosage.trim().is_empty() {
        return Err(CommandRefused::Missing("the dosage"));
    }
    let mut request = base("MedicationRequest", id, context);
    let fields = json!({
        "status": "active",
        "intent": "order",
        "medication": { "concept": concept.concept()? },
        "subject": { "reference": context.patient },
        "encounter": { "reference": context.encounter },
        "authoredOn": context.at,
        "requester": { "display": context.author },
        "dosageInstruction": [{ "text": dosage.trim() }]
    });
    merge(&mut request, fields);
    Ok(request)
}

fn merge(into: &mut Value, fields: Value) {
    if let (Some(into), Value::Object(fields)) = (into.as_object_mut(), fields) {
        into.extend(fields);
    }
}
