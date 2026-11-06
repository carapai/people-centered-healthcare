//! The Care Journey (#115; care-journeys.md; MASTER_PROMPT §61): `GET /Patient/{id}/$journey`,
//! a patient-centred projection of the person's care (`health-care-journeys`). It answers JSON,
//! not a FHIR resource: `{"stretches": [...], "waiting": [...], "unseen": [types]}`.
//!
//! - **The patient** must be readable by the reader (404 otherwise, as a read).
//! - **Whose records:** the patient's and, through the golden record, the same person's other
//!   patient records, at any facility.
//! - **Authorization at read time:** each type is searched under the reader's own search policy,
//!   so a record the policy decision point withholds is never on the journey. A step in a program
//!   the reader may not see falls under care outside any program. Types the reader may not search
//!   are listed in `unseen`.
//! - **Workflow:** what waits comes from Tasks (ADR 0042), never from clinical status.
//! - **Ownership is preserved:** each step names its authoritative resource, version and facility.
//!   The journey is rebuilt on every read and written nowhere.
//! - **Audit:** the records read are audited as a search's are.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use health_care_journeys_app::{TYPES, read};
use health_persistence::Store;
use serde_json::json;

use crate::FhirServer;
use crate::outcome::FhirError;
use crate::summary::{audited, people, searched};

/// `GET /Patient/{id}/$journey`.
pub(crate) async fn patient_journey(
    State(server): State<FhirServer>,
    Path(id): Path<String>,
) -> Result<Response, FhirError> {
    let mut tx = server.store.begin().await?;
    let patients = people(&mut tx, &id).await?;
    let mut found = Vec::new();
    let sections = searched(&server, &mut tx, &patients, &TYPES, &mut found).await?;
    audited(tx, &found).await?;
    let unseen: Vec<&str> = TYPES
        .iter()
        .zip(&sections)
        .filter(|(_, section)| section.is_none())
        .map(|(resource_type, _)| *resource_type)
        .collect();
    let journey = read(sections.iter().flatten().flatten());
    let body =
        json!({ "stretches": journey.stretches, "waiting": journey.waiting(), "unseen": unseen });
    Ok((StatusCode::OK, axum::Json(body)).into_response())
}
