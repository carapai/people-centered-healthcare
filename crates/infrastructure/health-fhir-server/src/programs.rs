//! Programs over HTTP (#116; program-designer.md), behind `subject_layer`, for the session's
//! facility. The definitions are the node's ProgramPlanDefinitions (`PlanDefinition?type=
//! clinical-protocol`).
//!
//! - `GET /definitions` needs [`VIEW_PERMISSION`]: the programs, with their episode type;
//! - `POST /enrolments` `{id, patient, program, transferredFrom?}` needs [`ENROL_PERMISSION`]: the patient
//!   enrolled if eligible (422 otherwise, 409 when already in the program), and the program's
//!   Workflow Definitions started with the episode as their context (`EpisodeOfCare/{id}`);
//! - `GET /patients/{id}/enrolments` needs [`VIEW_PERMISSION`]: the patient's episodes here;
//! - `POST /enrolments/{id}/status` `{status}` and `POST /enrolments/{id}/transfer`
//!   `{facility}` need [`MANAGE_PERMISSION`].
//!
//! A patient may be in several programs at once; an episode of another facility is not found.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::{FACILITY_TAG_SYSTEM, Subject};
use health_fhir_search::{Handling, Unrestricted, parse_query};
use health_identifiers::ResourceId;
use health_persistence::{ResourceType, Store, Transaction};
use health_programs::{Enrolment, Episode, EpisodeStatus, Program};
use health_programs_app::{ProgramError, Programs, eligible, episode, program};
use health_time::Timestamp;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::FhirServer;

/// The permission to see programs and enrolments.
pub const VIEW_PERMISSION: &str = "programs.view";
/// The permission to enrol patients.
pub const ENROL_PERMISSION: &str = "programs.enrol";
/// The permission to change and transfer enrolments.
pub const MANAGE_PERMISSION: &str = "programs.manage";

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn staff(subject: Option<Extension<Subject>>, permission: &str) -> Result<Subject, Box<Response>> {
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(refused(StatusCode::UNAUTHORIZED, "sign in")));
    };
    if !subject.permissions.has(permission) {
        return Err(Box::new(refused(
            StatusCode::FORBIDDEN,
            &format!("this needs the {permission} permission"),
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

fn failed(error: &ProgramError) -> Response {
    let status = match error {
        ProgramError::Refused(_) | ProgramError::Eligibility { .. } => {
            StatusCode::UNPROCESSABLE_ENTITY
        }
        ProgramError::AlreadyEnrolled(_) => StatusCode::CONFLICT,
        ProgramError::NotFound(_) | ProgramError::NotAProgram(_) => StatusCode::NOT_FOUND,
        ProgramError::Store(_) => return refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
    };
    refused(status, &error.to_string())
}

/// The resources a search on this node finds, unrestricted: the routes filter by facility.
async fn search(
    server: &FhirServer,
    resource_type: &str,
    params: &[(&str, &str)],
) -> Result<Vec<Value>, Box<Response>> {
    let params: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .chain([("_count".to_owned(), "1000".to_owned())])
        .collect();
    let registry = server.store.registry();
    let query = parse_query(&registry, resource_type, &params, Handling::Strict).map_err(|_| {
        Box::new(refused(
            StatusCode::NOT_IMPLEMENTED,
            &format!("this node cannot search {resource_type} that way"),
        ))
    })?;
    let unavailable = |_| Box::new(refused(StatusCode::SERVICE_UNAVAILABLE, "try again"));
    let mut tx = server.store.begin().await.map_err(unavailable)?;
    let page = tx
        .search(&query, &Unrestricted, None)
        .await
        .map_err(|_| Box::new(refused(StatusCode::SERVICE_UNAVAILABLE, "try again")))?;
    Ok(page
        .resources
        .iter()
        .filter_map(|r| serde_json::from_str(r.json.as_str()).ok())
        .collect())
}

async fn definitions(server: &FhirServer) -> Result<Vec<Program>, Box<Response>> {
    Ok(
        search(server, "PlanDefinition", &[("type", "clinical-protocol")])
            .await?
            .iter()
            .filter_map(|plan| program(plan).ok())
            .collect(),
    )
}

fn program_json(program: &Program) -> Value {
    json!({
        "url": program.url,
        "version": program.version,
        "episodeType": program.episode_type,
        "workflows": program.workflows,
    })
}

async fn list_definitions(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
) -> Response {
    or_return!(staff(subject, VIEW_PERMISSION));
    let programs = or_return!(definitions(&server).await);
    Json(programs.iter().map(program_json).collect::<Vec<_>>()).into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnrolBody {
    id: String,
    patient: String,
    program: String,
    #[serde(default)]
    transferred_from: Option<String>,
}

async fn enrol(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<EnrolBody>,
) -> Response {
    let subject = or_return!(staff(subject, ENROL_PERMISSION));
    if body.id.parse::<ResourceId>().is_err() {
        return refused(
            StatusCode::BAD_REQUEST,
            "the id is a client-generated UUIDv7 (ADR 0013)",
        );
    }
    let programs = or_return!(definitions(&server).await);
    let Some(chosen) = programs.iter().find(|p| p.url == body.program) else {
        return refused(StatusCode::NOT_FOUND, "no such program here");
    };
    // The patient, whose eligibility the program decides.
    let patient = async {
        let id = body
            .patient
            .strip_prefix("Patient/")?
            .parse::<ResourceId>()
            .ok()?;
        let kind: ResourceType = "Patient".parse().ok()?;
        let mut tx = server.store.begin().await.ok()?;
        let head = tx.read(&kind, id).await.ok().flatten()?;
        (!head.version.deleted)
            .then(|| serde_json::from_str::<Value>(head.json.as_str()).ok())
            .flatten()
    };
    let Some(patient) = patient.await else {
        return refused(StatusCode::UNPROCESSABLE_ENTITY, "no such patient");
    };
    let is_eligible = match eligible(server.fhirpath.as_ref(), chosen, &patient).await {
        Ok(found) => found,
        Err(error) => return failed(&error),
    };
    let request = Enrolment {
        id: body.id,
        patient: body.patient,
        program: chosen,
        facility: subject.facility.clone(),
        eligible: is_eligible,
        by: subject.account.clone(),
        at: Timestamp::now(),
        transferred_from: body.transferred_from,
    };
    let (enrolled, _) = match Programs::new(server.store.clone()).enrol(&request).await {
        Ok(done) => done,
        Err(error) => return failed(&error),
    };
    // The program's workflows, about the episode. A retry starts nothing twice.
    let context = format!("EpisodeOfCare/{}", enrolled.id);
    let mut started = Vec::new();
    for workflow in &chosen.workflows {
        let op_id = format!("{}#{workflow}", enrolled.id);
        match crate::workflow::start_for(&server, workflow, &context, &subject.account, &op_id)
            .await
        {
            Ok(Some(key)) => started.push(key),
            Ok(None) => {}
            Err(reason) => {
                tracing::warn!(error.kind = "workflow-start-failed", %reason, "a program workflow did not start");
            }
        }
    }
    let mut answer = json!(enrolled);
    answer["workflows"] = json!(started);
    (StatusCode::CREATED, Json(answer)).into_response()
}

fn here(resource: &Value, facility: &str) -> bool {
    resource["meta"]["tag"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|t| t["system"] == FACILITY_TAG_SYSTEM && t["code"] == facility)
}

async fn enrolments(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(patient): Path<String>,
) -> Response {
    let subject = or_return!(staff(subject, VIEW_PERMISSION));
    let reference = format!("Patient/{patient}");
    let found = or_return!(search(&server, "EpisodeOfCare", &[("patient", &reference)]).await);
    let episodes: Vec<Episode> = found
        .iter()
        .filter(|r| here(r, &subject.facility))
        .filter_map(episode)
        .collect();
    Json(episodes).into_response()
}

/// Episode `id`, if it is this facility's.
async fn ours(server: &FhirServer, subject: &Subject, id: &str) -> Result<(), Box<Response>> {
    match Programs::new(server.store.clone()).episode(id).await {
        Ok(found) if found.facility == subject.facility => Ok(()),
        Ok(_) => Err(Box::new(refused(
            StatusCode::NOT_FOUND,
            "no such enrolment here",
        ))),
        Err(error) => Err(Box::new(failed(&error))),
    }
}

#[derive(Deserialize)]
struct StatusBody {
    status: String,
}

async fn status(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<StatusBody>,
) -> Response {
    let subject = or_return!(staff(subject, MANAGE_PERMISSION));
    or_return!(ours(&server, &subject, &id).await);
    let Some(to) = EpisodeStatus::from_code(&body.status) else {
        return refused(StatusCode::BAD_REQUEST, "not an EpisodeOfCare status");
    };
    match Programs::new(server.store.clone())
        .transition(&id, to, Timestamp::now())
        .await
    {
        Ok((next, _)) => Json(next).into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Deserialize)]
struct TransferBody {
    facility: String,
}

async fn transfer(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<TransferBody>,
) -> Response {
    let subject = or_return!(staff(subject, MANAGE_PERMISSION));
    or_return!(ours(&server, &subject, &id).await);
    match Programs::new(server.store.clone())
        .transfer_out(&id, &body.facility, Timestamp::now())
        .await
    {
        Ok((next, _)) => Json(next).into_response(),
        Err(error) => failed(&error),
    }
}

impl FhirServer {
    /// The program routes, to mount behind `subject_layer`.
    pub fn programs_router(&self) -> Router {
        Router::new()
            .route("/definitions", get(list_definitions))
            .route("/enrolments", post(enrol))
            .route("/patients/{id}/enrolments", get(enrolments))
            .route("/enrolments/{id}/status", post(status))
            .route("/enrolments/{id}/transfer", post(transfer))
            .with_state(self.clone())
    }
}

/// The R5 core extension naming the EpisodeOfCare a record belongs to.
pub(crate) const WORKFLOW_EPISODE: &str =
    "http://hl7.org/fhir/StructureDefinition/workflow-episodeOfCare";

/// The EpisodeOfCare a record names with [`WORKFLOW_EPISODE`], if any.
pub(crate) fn episode_of(resource: &Value) -> Option<Value> {
    resource["extension"]
        .as_array()?
        .iter()
        .find(|e| e["url"] == WORKFLOW_EPISODE)
        .cloned()
}

/// The resources a search finds, or none when the node cannot search that way.
async fn found(server: &FhirServer, resource_type: &str, params: &[(&str, &str)]) -> Vec<Value> {
    let params: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .chain([("_count".to_owned(), "1000".to_owned())])
        .collect();
    let registry = server.store.registry();
    let Ok(query) = parse_query(&registry, resource_type, &params, Handling::Strict) else {
        return Vec::new();
    };
    let Ok(mut tx) = server.store.begin().await else {
        return Vec::new();
    };
    tx.search(&query, &Unrestricted, None)
        .await
        .map(|page| {
            page.resources
                .iter()
                .filter_map(|r| serde_json::from_str(r.json.as_str()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Program context on a submitted QuestionnaireResponse (questionnaire-extraction.md § Program
/// context): when its form belongs to programs (their definitions list it), the patient's one
/// open episode of those programs here is recorded on it with `workflow-episodeOfCare`. With
/// several, the clinician chooses (422 until the response names one); with none, nothing is
/// recorded. A response that already names its episode keeps it.
pub(crate) async fn episode_context(
    server: &FhirServer,
    response: &mut Value,
) -> Result<(), crate::outcome::FhirError> {
    if episode_of(response).is_some() {
        return Ok(());
    }
    let (Some(form), Some(patient)) = (
        response["questionnaire"]
            .as_str()
            .map(|q| q.split('|').next().unwrap_or(q).to_owned()),
        response["subject"]["reference"].as_str().map(str::to_owned),
    ) else {
        return Ok(());
    };
    let programs: Vec<String> = found(server, "PlanDefinition", &[("type", "clinical-protocol")])
        .await
        .iter()
        .filter_map(|plan| program(plan).ok())
        .filter(|p| {
            p.questionnaires
                .iter()
                .any(|q| q.split('|').next().unwrap_or(q) == form)
        })
        .map(|p| p.url)
        .collect();
    if programs.is_empty() {
        return Ok(());
    }
    let facility = crate::access::session_facility();
    let open: Vec<Episode> = found(server, "EpisodeOfCare", &[("patient", &patient)])
        .await
        .iter()
        .filter_map(episode)
        .filter(|e| e.status.is_open() && programs.contains(&e.program))
        .filter(|e| facility.as_ref().is_none_or(|f| *f == e.facility))
        .collect();
    match open.as_slice() {
        [] => Ok(()),
        [one] => {
            let named = json!({ "url": WORKFLOW_EPISODE, "valueReference": { "reference": format!("EpisodeOfCare/{}", one.id) } });
            match response["extension"].as_array_mut() {
                Some(list) => list.push(named),
                None => response["extension"] = json!([named]),
            }
            Ok(())
        }
        _ => Err(crate::outcome::FhirError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "processing",
            "the patient has several open episodes of this form's programs: name one with workflow-episodeOfCare",
        )),
    }
}
