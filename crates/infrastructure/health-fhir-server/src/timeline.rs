//! The longitudinal patient timeline (#115; care-journeys.md): `GET /Patient/{id}/$timeline`,
//! the shared record's activity timeline (`health-shared-record::timeline`). It answers JSON, not
//! a FHIR resource: `{"entries": [...], "unseen": [types]}`, the newest first.
//!
//! - **The patient** must be readable by the reader (404 otherwise, as a read).
//! - **Whose records:** the patient's and, through the golden record, the same person's other
//!   patient records, at any facility.
//! - **Authorization at read time:** each type is searched under the reader's own search policy,
//!   so the policy decision point decides every entry as it would a search; a record withheld is
//!   never on the timeline. Types the reader may not search are listed in `unseen`.
//! - **Ownership is preserved:** each entry names its authoritative resource, its version and its
//!   facility. The timeline is rebuilt on every read and written nowhere.
//! - **Audit:** the records read are audited as a search's are.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use health_persistence::Store;
use health_shared_record_app::timeline::{TIMELINE, timeline};
use serde_json::json;

use crate::FhirServer;
use crate::outcome::FhirError;
use crate::summary::{audited, people, searched};

/// `GET /Patient/{id}/$timeline`.
pub(crate) async fn patient_timeline(
    State(server): State<FhirServer>,
    Path(id): Path<String>,
) -> Result<Response, FhirError> {
    let mut tx = server.store.begin().await?;
    let patients = people(&mut tx, &id).await?;
    let mut read = Vec::new();
    let sections = searched(&server, &mut tx, &patients, &TIMELINE, &mut read).await?;
    audited(tx, &read).await?;
    let unseen: Vec<&str> = TIMELINE
        .iter()
        .zip(&sections)
        .filter(|(_, section)| section.is_none())
        .map(|(resource_type, _)| *resource_type)
        .collect();
    let built = timeline(sections.iter().flatten().flatten());
    let body = json!({ "entries": built.entries(), "unseen": unseen });
    Ok((StatusCode::OK, axum::Json(body)).into_response())
}
