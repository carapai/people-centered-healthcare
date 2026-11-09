//! `POST /Questionnaire/{id}/$populate` (SDC; questionnaire-extraction.md § Population; #94): a
//! new response to the Questionnaire with what this node knows answered, for the user to check.
//! Nothing is written.
//!
//! The body is Parameters with `subject` (a `valueReference` to the Patient) and any `context`s
//! (each with parts `name`, e.g. `encounter`, and `content`, a `valueReference`). Population
//! respects the node's policy decision point: the Questionnaire and each launch context are read
//! as the caller would read them (one they may not read is not found), and the Observations it
//! draws on are the patient's the caller may search. The answer is Parameters with `response`,
//! and an `issues` OperationOutcome-free `candidates` part per item that offers answers.

use std::collections::BTreeMap;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use health_fhir_extraction::populate::{Launch, PopulateError, populate as run};
use health_fhir_search::{Handling, parse_query};
use health_identifiers::ResourceId;
use health_persistence::{ResourceType, Store, Transaction};
use health_time::Timestamp;
use serde_json::{Value, json};

use crate::FhirServer;
use crate::outcome::FhirError;

/// The most Observations of the patient population looks at.
const OBSERVATIONS: usize = 500;

/// `Type/id` from a reference.
fn target(reference: &str) -> Option<(ResourceType, ResourceId)> {
    let (kind, id) = reference.split_once('/')?;
    Some((kind.parse().ok()?, id.parse().ok()?))
}

async fn read_as_caller(server: &FhirServer, reference: &str) -> Result<Value, FhirError> {
    let not_found = || FhirError::not_found(format!("{reference} is not found"));
    let (kind, id) = target(reference)
        .ok_or_else(|| FhirError::bad_request(format!("{reference} is not a Type/id reference")))?;
    let mut tx = server.store.begin().await?;
    let stored = tx
        .read(&kind, id)
        .await?
        .filter(|s| !s.version.deleted)
        .ok_or_else(not_found)?;
    crate::access::readable(&stored.json, not_found)?;
    serde_json::from_str(stored.json.as_str())
        .map_err(|_| FhirError::bad_request(format!("{reference} is not JSON")))
}

/// The patient's Observations the caller may search; none when they may search none.
async fn observations(server: &FhirServer, patient: &str) -> Result<Vec<Value>, FhirError> {
    let Ok(policy) = crate::access::search_policy(server, "Observation", Some(patient.to_owned()))
    else {
        return Ok(Vec::new());
    };
    let params = vec![
        ("patient".to_owned(), patient.to_owned()),
        ("_count".to_owned(), OBSERVATIONS.to_string()),
    ];
    let Ok(query) = parse_query(
        &server.store.registry(),
        "Observation",
        &params,
        Handling::Strict,
    ) else {
        return Ok(Vec::new());
    };
    let mut tx = server.store.begin().await?;
    let page = tx.search(&query, policy.as_ref(), None).await?;
    Ok(page
        .resources
        .iter()
        .filter_map(|found| serde_json::from_str::<Value>(found.json.as_str()).ok())
        .collect())
}

pub(crate) async fn populate(
    State(server): State<FhirServer>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, FhirError> {
    crate::http::require_json_body(&headers)?;
    let parameters: Value = serde_json::from_slice(&body)
        .map_err(|e| FhirError::bad_request(format!("the body is not JSON: {e}")))?;
    if parameters["resourceType"] != "Parameters" {
        return Err(FhirError::bad_request("the body is Parameters"));
    }
    let parts = parameters["parameter"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let questionnaire = read_as_caller(&server, &format!("Questionnaire/{id}")).await?;
    let mut contexts = BTreeMap::new();
    let mut patient = None;
    if let Some(subject) = parts
        .iter()
        .find(|p| p["name"] == "subject")
        .and_then(|p| p["valueReference"]["reference"].as_str())
    {
        if !subject.starts_with("Patient/") {
            return Err(FhirError::bad_request("the subject is a Patient"));
        }
        contexts.insert(
            "patient".to_owned(),
            read_as_caller(&server, subject).await?,
        );
        patient = Some(subject.to_owned());
    }
    for context in parts.iter().filter(|p| p["name"] == "context") {
        let part = |name: &str| {
            context["part"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|p| p["name"] == name)
                .cloned()
        };
        let (Some(name), Some(reference)) = (
            part("name").and_then(|p| p["valueString"].as_str().map(str::to_owned)),
            part("content")
                .and_then(|p| p["valueReference"]["reference"].as_str().map(str::to_owned)),
        ) else {
            return Err(FhirError::bad_request(
                "each context has a name and a content reference",
            ));
        };
        contexts.insert(name, read_as_caller(&server, &reference).await?);
    }
    let observations = match &patient {
        Some(patient) => observations(&server, patient).await?,
        None => Vec::new(),
    };
    let launch = Launch {
        contexts,
        observations,
        now: Some(Timestamp::now()),
    };
    let populated = run(server.fhirpath.as_ref(), &questionnaire, &launch)
        .await
        .map_err(|e| match e {
            PopulateError::MissingContext(_) => FhirError::bad_request(e.to_string()),
            PopulateError::Expression { .. } => FhirError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "processing",
                e.to_string(),
            ),
        })?;
    let mut out = vec![json!({ "name": "response", "resource": populated.response })];
    for (link, answers) in populated.candidates {
        out.push(json!({
            "name": "candidates",
            "part": std::iter::once(json!({ "name": "linkId", "valueString": link }))
                .chain(answers.into_iter().map(|a| json!({ "name": "answer", "value": a })))
                .collect::<Vec<_>>()
        }));
    }
    let body = json!({ "resourceType": "Parameters", "parameter": out });
    Ok(crate::http::fhir_json(StatusCode::OK, body.to_string()))
}
