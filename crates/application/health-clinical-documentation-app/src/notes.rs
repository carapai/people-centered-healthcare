//! The signed-note rule on FHIR resources (#107; synchronization.md: an in-place edit of a signed
//! version is rejected at the receiver). The node's write path asks it before replacing or
//! deleting a Composition or DocumentReference; the Authority Node asks the same when a version
//! arrives by sync.
//!
//! A note's status is a Composition's `status`, or a DocumentReference's `docStatus`. A
//! DocumentReference without one is not a note under this rule.

use health_clinical_documentation::{NoteRefused, NoteStatus, may_delete, may_replace};
use serde_json::Value;

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
