//! Clinical notes over HTTP (#107; clinical-workspaces.md § Signatures and amendments), behind
//! `subject_layer`, for the session's facility, with [`crate::DOCUMENT_PERMISSION`]. The node
//! builds each Composition (`health_clinical_documentation`); every write goes through the write
//! path's checks, including the signed-note rule.
//!
//! - `POST /notes` `{id, kind, patient, encounter?, sections}`: a draft;
//! - `POST /notes/{id}/edit` `{sections}` and `POST /notes/{id}/cancel`: drafts only;
//! - `POST /notes/{id}/sign`: signed by the session's account, with a Provenance;
//! - `POST /notes/{id}/amend` `{id, relation: "replaces" | "appends", sections}`: a new note,
//!   signed as it is made, relating to the signed original, with a Provenance whose source is
//!   the original's version. The original is never edited;
//! - `GET /patients/{id}/notes`: the patient's notes here, latest first.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::{FACILITY_TAG_SYSTEM, Subject};
use health_clinical_documentation::{Note, NoteKind, NoteRefused, Relation, Section};
use health_clinical_documentation_app::notes::{Act, fhir, note, provenance};
use health_fhir_search::{Handling, Unrestricted, parse_query};
use health_identifiers::ResourceId;
use health_persistence::{Precondition, RawJson, ResourceType, Store, Transaction};
use health_time::Timestamp;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::FhirServer;
use crate::consultation::DOCUMENT_PERMISSION;
use crate::ops::{self, Attempt};

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn note_refused(error: &NoteRefused) -> Response {
    let status = match error {
        NoteRefused::Signed | NoteRefused::SignedNotDeleted | NoteRefused::Closed(_) => {
            StatusCode::CONFLICT
        }
        NoteRefused::NotSigned | NoteRefused::Empty => StatusCode::UNPROCESSABLE_ENTITY,
    };
    refused(status, &error.to_string())
}

fn staff(subject: Option<Extension<Subject>>) -> Result<Subject, Box<Response>> {
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(refused(StatusCode::UNAUTHORIZED, "sign in")));
    };
    if !subject.permissions.has(DOCUMENT_PERMISSION) {
        return Err(Box::new(refused(
            StatusCode::FORBIDDEN,
            &format!("this needs the {DOCUMENT_PERMISSION} permission"),
        )));
    }
    Ok(subject)
}

macro_rules! or_return {
    ($result:expr) => {
        match $result {
            Ok(found) => found,
            Err(response) => return *response,
        }
    };
}

fn kind() -> ResourceType {
    "Composition"
        .parse()
        .expect("Composition is a resource type")
}

fn id(text: &str) -> Result<ResourceId, Box<Response>> {
    text.parse().map_err(|_| {
        Box::new(refused(
            StatusCode::BAD_REQUEST,
            "the id is a client-generated UUIDv7 (ADR 0013)",
        ))
    })
}

fn tagged(resource: &Value, facility: &str) -> bool {
    resource["meta"]["tag"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|t| t["system"] == FACILITY_TAG_SYSTEM && t["code"] == facility)
}

/// Note `id` at the session's facility, with its version id.
async fn load(
    server: &FhirServer,
    subject: &Subject,
    text: &str,
) -> Result<(Note, String), Box<Response>> {
    let not_found = || Box::new(refused(StatusCode::NOT_FOUND, "no such note here"));
    let resource_id = text.parse::<ResourceId>().map_err(|_| not_found())?;
    let mut tx = server
        .store
        .begin()
        .await
        .map_err(|_| Box::new(refused(StatusCode::SERVICE_UNAVAILABLE, "try again")))?;
    let head = tx
        .read(&kind(), resource_id)
        .await
        .ok()
        .flatten()
        .filter(|h| !h.version.deleted)
        .ok_or_else(not_found)?;
    let value: Value = serde_json::from_str(head.json.as_str()).map_err(|_| not_found())?;
    if !tagged(&value, &subject.facility) {
        return Err(not_found());
    }
    let found = note(&value).ok_or_else(not_found)?;
    Ok((found, head.version.version_id.to_string()))
}

/// Writes `written` at `facility` and, for a signature or amendment, its Provenance, in one
/// transaction.
async fn save(
    server: &FhirServer,
    facility: &str,
    written: &Note,
    record: Option<(Act, Option<String>, &str)>,
) -> Response {
    let attempt = async {
        let mut tx = server.store.begin().await?;
        let note_id: ResourceId = written
            .id
            .parse()
            .map_err(|_| Attempt::Fail(crate::outcome::FhirError::bad_request("note id")))?;
        let precondition = match tx.read(&kind(), note_id).await? {
            Some(head) if !head.version.deleted => Precondition::IfMatch(head.version.version_id),
            _ => Precondition::MustNotExist,
        };
        let json = RawJson::from_validated(fhir(written, facility).to_string());
        let (version, stored) = ops::write(&mut tx, &kind(), note_id, &json, precondition).await?;
        crate::references::check_written(server, &mut tx, &stored).await?;
        if let Some((act, source, by)) = record {
            let provenance_id = ResourceId::new();
            let target = format!("Composition/{}/_history/{}", written.id, version.version_id);
            let record = provenance(
                &provenance_id.to_string(),
                act,
                &target,
                source.as_deref(),
                by,
                &Timestamp::now().to_string(),
            );
            let kind: ResourceType = "Provenance"
                .parse()
                .map_err(|_| Attempt::Fail(crate::outcome::FhirError::bad_request("Provenance")))?;
            let (_, stored) = ops::write(
                &mut tx,
                &kind,
                provenance_id,
                &RawJson::from_validated(record.to_string()),
                Precondition::MustNotExist,
            )
            .await?;
            crate::references::check_written(server, &mut tx, &stored).await?;
        }
        tx.commit().await?;
        let value: Value = serde_json::from_str(stored.as_str()).unwrap_or(Value::Null);
        Ok::<_, Attempt>(value)
    };
    match attempt.await {
        Ok(value) => Json(note(&value).map_or(value, |n| json!(n))).into_response(),
        Err(Attempt::Fail(error)) => error.into_response(),
        Err(Attempt::Conflict) => refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
    }
}

#[derive(Deserialize)]
struct DraftBody {
    id: String,
    kind: NoteKind,
    patient: String,
    #[serde(default)]
    encounter: Option<String>,
    sections: Vec<Section>,
}

async fn draft(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<DraftBody>,
) -> Response {
    let subject = or_return!(staff(subject));
    or_return!(id(&body.id));
    if load(&server, &subject, &body.id).await.is_ok() {
        return refused(StatusCode::CONFLICT, "that note exists");
    }
    match Note::draft(
        &body.id,
        body.kind,
        &body.patient,
        body.encounter.as_deref(),
        &subject.account,
        body.sections,
        Timestamp::now(),
    ) {
        Ok(drafted) => {
            let mut response = save(&server, &subject.facility, &drafted, None).await;
            if response.status() == StatusCode::OK {
                *response.status_mut() = StatusCode::CREATED;
            }
            response
        }
        Err(error) => note_refused(&error),
    }
}

#[derive(Deserialize)]
struct EditBody {
    sections: Vec<Section>,
}

async fn edit(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(note_id): Path<String>,
    Json(body): Json<EditBody>,
) -> Response {
    let subject = or_return!(staff(subject));
    let (current, _) = or_return!(load(&server, &subject, &note_id).await);
    match current.edit(body.sections, Timestamp::now()) {
        Ok(next) => save(&server, &subject.facility, &next, None).await,
        Err(error) => note_refused(&error),
    }
}

async fn cancel(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(note_id): Path<String>,
) -> Response {
    let subject = or_return!(staff(subject));
    let (current, _) = or_return!(load(&server, &subject, &note_id).await);
    match current.cancel(Timestamp::now()) {
        Ok(next) => save(&server, &subject.facility, &next, None).await,
        Err(error) => note_refused(&error),
    }
}

async fn sign(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(note_id): Path<String>,
) -> Response {
    let subject = or_return!(staff(subject));
    let (current, _) = or_return!(load(&server, &subject, &note_id).await);
    match current.sign(&subject.account, Timestamp::now()) {
        Ok(next) => {
            save(
                &server,
                &subject.facility,
                &next,
                Some((Act::Signed, None, &subject.account)),
            )
            .await
        }
        Err(error) => note_refused(&error),
    }
}

#[derive(Deserialize)]
struct AmendBody {
    id: String,
    relation: Relation,
    sections: Vec<Section>,
}

async fn amend(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(note_id): Path<String>,
    Json(body): Json<AmendBody>,
) -> Response {
    let subject = or_return!(staff(subject));
    or_return!(id(&body.id));
    let (original, version) = or_return!(load(&server, &subject, &note_id).await);
    match original.amend(
        &body.id,
        body.relation,
        body.sections,
        &subject.account,
        Timestamp::now(),
    ) {
        Ok(amendment) => {
            let source = format!("Composition/{}/_history/{version}", original.id);
            let mut response = save(
                &server,
                &subject.facility,
                &amendment,
                Some((Act::Amended, Some(source), &subject.account)),
            )
            .await;
            if response.status() == StatusCode::OK {
                *response.status_mut() = StatusCode::CREATED;
            }
            response
        }
        Err(error) => note_refused(&error),
    }
}

async fn patient_notes(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(patient): Path<String>,
) -> Response {
    let subject = or_return!(staff(subject));
    let params = vec![
        ("patient".to_owned(), format!("Patient/{patient}")),
        ("_count".to_owned(), "1000".to_owned()),
    ];
    let registry = server.store.registry();
    let Ok(query) = parse_query(&registry, "Composition", &params, Handling::Strict) else {
        return refused(
            StatusCode::NOT_IMPLEMENTED,
            "this node cannot search notes by patient",
        );
    };
    let Ok(mut tx) = server.store.begin().await else {
        return refused(StatusCode::SERVICE_UNAVAILABLE, "try again");
    };
    let Ok(page) = tx.search(&query, &Unrestricted, None).await else {
        return refused(StatusCode::SERVICE_UNAVAILABLE, "try again");
    };
    let mut notes: Vec<Note> = page
        .resources
        .iter()
        .filter_map(|r| serde_json::from_str::<Value>(r.json.as_str()).ok())
        .filter(|v| tagged(v, &subject.facility))
        .filter_map(|v| note(&v))
        .collect();
    notes.sort_by_key(|n| std::cmp::Reverse(n.date));
    Json(notes).into_response()
}

impl FhirServer {
    /// The clinical-notes routes, to mount behind `subject_layer`.
    pub fn notes_router(&self) -> Router {
        Router::new()
            .route("/notes", post(draft))
            .route("/notes/{id}/edit", post(edit))
            .route("/notes/{id}/cancel", post(cancel))
            .route("/notes/{id}/sign", post(sign))
            .route("/notes/{id}/amend", post(amend))
            .route("/patients/{id}/notes", get(patient_notes))
            .with_state(self.clone())
    }
}
