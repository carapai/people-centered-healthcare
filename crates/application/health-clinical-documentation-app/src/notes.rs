//! The signed-note rule on FHIR resources (#107; synchronization.md: an in-place edit of a signed
//! version is rejected at the receiver). The node's write path asks it before replacing or
//! deleting a Composition or DocumentReference; the Authority Node asks the same when a version
//! arrives by sync.
//!
//! A note's status is a Composition's `status`, or a DocumentReference's `docStatus`. A
//! DocumentReference without one is not a note under this rule.
//!
//! [`fhir`] and [`note`] map a note to its Composition and back, and [`provenance`] records its
//! signature or amendment.

use health_access_policy::FACILITY_TAG_SYSTEM;
use health_clinical_documentation::{
    Note, NoteKind, NoteRefused, NoteStatus, RelatesTo, Relation, Section, Signature, may_delete,
    may_replace,
};
use serde_json::{Value, json};

/// The note status of `resource`, if it is a Composition or a DocumentReference with one.
pub fn status_of(resource: &Value) -> Option<NoteStatus> {
    let field = match resource["resourceType"].as_str()? {
        "Composition" => "status",
        "DocumentReference" => "docStatus",
        _ => return None,
    };
    NoteStatus::from_code(resource[field].as_str()?)
}

/// `resource` without `meta` and its status: what a note says.
fn content(resource: &Value) -> Value {
    let mut copy = resource.clone();
    if let Some(object) = copy.as_object_mut() {
        for field in ["meta", "status", "docStatus"] {
            object.remove(field);
        }
    }
    copy
}

/// Whether `next` may replace `current`, the stored version of the same resource.
pub fn check_replace(current: &Value, next: &Value) -> Result<(), NoteRefused> {
    let Some(was) = status_of(current) else {
        return Ok(());
    };
    // A next version without a status is no longer the note it was: never in place.
    let Some(becomes) = status_of(next) else {
        return may_replace(was, NoteStatus::Preliminary, false);
    };
    may_replace(was, becomes, content(current) == content(next))
}

/// Whether `current` may be deleted.
pub fn check_delete(current: &Value) -> Result<(), NoteRefused> {
    status_of(current).map_or(Ok(()), may_delete)
}

const KINDS: [NoteKind; 6] = [
    NoteKind::Progress,
    NoteKind::Consultation,
    NoteKind::Admission,
    NoteKind::Procedure,
    NoteKind::Discharge,
    NoteKind::Letter,
];

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', "<br/>")
}

fn unescape(html: &str) -> String {
    html.replace("<br/>", "\n")
        .replace("&quot;", "\"")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
}

const OPEN: &str = "<div xmlns=\"http://www.w3.org/1999/xhtml\"><p>";
const CLOSE: &str = "</p></div>";

/// The text of a section's narrative, as [`fhir`] wrote it.
fn text_of(div: &str) -> Option<String> {
    Some(unescape(div.strip_prefix(OPEN)?.strip_suffix(CLOSE)?))
}

/// The FHIR R5 Composition of `note`, at `facility`.
pub fn fhir(note: &Note, facility: &str) -> Value {
    let (code, display) = note.kind.loinc();
    let mut composition = json!({
        "resourceType": "Composition",
        "id": note.id,
        "meta": { "tag": [{ "system": FACILITY_TAG_SYSTEM, "code": facility }] },
        "status": note.status.code(),
        "type": { "coding": [{ "system": "http://loinc.org", "code": code, "display": display }] },
        "subject": [{ "reference": note.patient }],
        "date": note.date.to_string(),
        "author": [{ "display": note.author }],
        "title": display,
        "section": note.sections.iter().map(|s| json!({
            "title": s.title,
            "text": { "status": "generated", "div": format!("{OPEN}{}{CLOSE}", escape(s.text.trim())) }
        })).collect::<Vec<_>>()
    });
    if let Some(encounter) = &note.encounter {
        composition["encounter"] = json!({ "reference": encounter });
    }
    if let Some(signed) = &note.signed {
        composition["attester"] = json!([{
            "mode": { "coding": [{ "system": "http://hl7.org/fhir/composition-attestation-mode", "code": "legal" }] },
            "time": signed.at.to_string(),
            "party": { "display": signed.by }
        }]);
    }
    if let Some(RelatesTo {
        relation,
        note: original,
    }) = &note.relates_to
    {
        let kind = match relation {
            Relation::Replaces => "replaces",
            Relation::Appends => "appends",
        };
        composition["relatesTo"] = json!([{ "type": kind, "resourceReference": { "reference": format!("Composition/{original}") } }]);
    }
    composition
}

/// The note a Composition written by [`fhir`] holds.
pub fn note(composition: &Value) -> Option<Note> {
    if composition["resourceType"] != "Composition" {
        return None;
    }
    let code = composition["type"]["coding"][0]["code"].as_str()?;
    let kind = KINDS.into_iter().find(|k| k.loinc().0 == code)?;
    let sections = composition["section"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| {
            Some(Section {
                title: s["title"].as_str()?.to_owned(),
                text: text_of(s["text"]["div"].as_str()?)?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let signed = composition["attester"][0]["time"]
        .as_str()
        .and_then(|t| t.parse().ok())
        .map(|at| Signature {
            by: composition["attester"][0]["party"]["display"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            at,
        });
    let relates_to = composition["relatesTo"][0]["resourceReference"]["reference"]
        .as_str()
        .and_then(|r| r.strip_prefix("Composition/"))
        .and_then(|original| {
            let relation = match composition["relatesTo"][0]["type"].as_str()? {
                "replaces" => Relation::Replaces,
                "appends" => Relation::Appends,
                _ => return None,
            };
            Some(RelatesTo {
                relation,
                note: original.to_owned(),
            })
        });
    Some(Note {
        id: composition["id"].as_str()?.to_owned(),
        kind,
        patient: composition["subject"][0]["reference"].as_str()?.to_owned(),
        encounter: composition["encounter"]["reference"]
            .as_str()
            .map(str::to_owned),
        author: composition["author"][0]["display"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        date: composition["date"].as_str()?.parse().ok()?,
        status: NoteStatus::from_code(composition["status"].as_str()?)?,
        sections,
        signed,
        relates_to,
    })
}

/// What a Provenance records of a note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// The note was signed.
    Signed,
    /// The note amends or appends to another, whose version is the Provenance's source.
    Amended,
}

/// The Provenance of signing or amending a note: `target` is the note's version
/// (`Composition/{id}/_history/{vid}`), `source` the original's for an amendment.
pub fn provenance(
    id: &str,
    act: Act,
    target: &str,
    source: Option<&str>,
    by: &str,
    at: &str,
) -> Value {
    let activity = match act {
        Act::Signed => {
            json!({ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/v3-DocumentCompletion", "code": "LA", "display": "legally authenticated" }] })
        }
        Act::Amended => {
            json!({ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/v3-DataOperation", "code": "UPDATE", "display": "revise" }] })
        }
    };
    let mut provenance = json!({
        "resourceType": "Provenance",
        "id": id,
        "target": [{ "reference": target }],
        "recorded": at,
        "activity": activity,
        "agent": [{
            "type": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/provenance-participant-type", "code": if act == Act::Signed { "attester" } else { "author" } }] },
            "who": { "display": by }
        }]
    });
    if let Some(source) = source {
        provenance["entity"] = json!([{ "role": "revision", "what": { "reference": source } }]);
    }
    provenance
}
