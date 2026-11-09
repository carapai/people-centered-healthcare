//! Workflows, tasks and worklists over HTTP (#99): a nurse claims and completes triage, the
//! clinician's worklist fills, a supervisor reassigns and escalates, and the permissions and
//! the node's refusal of definitions it cannot run.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{Permissions, Subject};
use health_fhir_search::Unrestricted;
use health_fhir_server::{
    FhirServer, WORKFLOW_PERMISSION, WORKFLOW_SUPERVISE_PERMISSION, WorkflowConfig,
    WorkflowConfigError,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use health_workflow::definition::WorkflowDefinition;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const OPD: &str = "https://health.go.ug/fhir/Library/opd-visit|1.0.0";

/// The OPD definition, as a node's definitions file holds it.
fn opd() -> Value {
    json!({
        "url": "https://health.go.ug/fhir/Library/opd-visit",
        "version": "1.0.0",
        "statechart": {
            "states": [
                {
                    "id": "triage",
                    "kind": "atomic",
                    "task": { "role": "nurse", "worklist": "opd-triage" },
                    "transitions": [{ "events": ["triaged"], "targets": ["consultation"] }]
                },
                {
                    "id": "consultation",
                    "kind": "atomic",
                    "task": { "role": "clinician", "worklist": "opd-consult", "priority": "urgent" },
                    "transitions": [
                        { "events": ["seen"], "targets": ["done"] },
                        { "events": ["referred"], "targets": ["done"] }
                    ]
                },
                { "id": "done", "kind": "final" }
            ]
        }
    })
}

fn definition(value: Value) -> WorkflowDefinition {
    serde_json::from_value(value).expect("a definition")
}

async fn server() -> FhirServer {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "workflow-http".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    FhirServer::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        "http://test/fhir",
        Arc::new(Unrestricted),
    )
    .with_workflows(WorkflowConfig::new(vec![definition(opd())], "central").expect("config"))
}

fn as_(server: &FhirServer, account: &str, permissions: &[&str]) -> Router {
    server.workflow_router().layer(Extension(Subject {
        account: account.into(),
        facility: "KAM013".into(),
        permissions: Permissions(permissions.iter().map(|p| (*p).to_owned()).collect()),
        programs: BTreeSet::new(),
        purpose: Some("TREAT".into()),
        break_glass: None,
    }))
}

async fn send(app: &Router, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let response = app
        .clone()
        .oneshot(
            request
                .body(body.map_or(Body::empty(), |b| Body::from(b.to_string())))
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn field<'a>(tasks: &'a Value, name: &str) -> Vec<&'a Value> {
    tasks
        .as_array()
        .expect("tasks")
        .iter()
        .map(|t| &t[name])
        .collect()
}

#[tokio::test]
async fn a_visit_moves_from_the_triage_worklist_to_the_consultation_worklist() {
    let server = server().await;
    let nurse = as_(&server, "nurse-1", &[WORKFLOW_PERMISSION]);
    let other = as_(&server, "nurse-2", &[WORKFLOW_PERMISSION]);

    let (status, instance) = send(
        &nurse,
        "POST",
        "/instances",
        Some(json!({ "definition": OPD, "context": "Encounter/e-1", "opId": "op-1" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{instance}");
    assert_eq!(instance["configuration"], json!(["triage"]));
    let id = instance["id"].as_str().expect("id").to_owned();

    let (status, listed) = send(&nurse, "GET", "/definitions", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed[0]["definition"], OPD);
    assert_eq!(
        listed[0]["tasks"],
        json!([
            { "state": "consultation", "role": "clinician", "worklist": "opd-consult" },
            { "state": "triage", "role": "nurse", "worklist": "opd-triage" }
        ])
    );

    let (status, triage) = send(&nurse, "GET", "/worklists/opd-triage", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(field(&triage, "status"), [&json!("ready")]);
    assert_eq!(field(&triage, "events"), [&json!(["triaged"])]);
    assert_eq!(field(&triage, "instance"), [&json!(id)]);

    let claim = format!("/instances/{id}/tasks/triage/claim");
    let (status, _) = send(&nurse, "POST", &claim, Some(json!({ "opId": "op-2" }))).await;
    assert_eq!(status, StatusCode::OK);
    let (status, refused) = send(&other, "POST", &claim, Some(json!({ "opId": "op-3" }))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    let (_, triage) = send(&nurse, "GET", "/worklists/opd-triage", None).await;
    assert_eq!(field(&triage, "owner"), [&json!("nurse-1")]);

    let (status, moved) = send(
        &nurse,
        "POST",
        &format!("/instances/{id}/events"),
        Some(json!({ "event": "triaged", "opId": "op-4" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(moved["configuration"], json!(["consultation"]));
    let (_, triage) = send(&nurse, "GET", "/worklists/opd-triage", None).await;
    assert_eq!(triage, json!([]));
    let (_, consult) = send(&nurse, "GET", "/worklists/opd-consult", None).await;
    assert_eq!(field(&consult, "priority"), [&json!("urgent")]);
    assert_eq!(field(&consult, "events"), [&json!(["seen", "referred"])]);

    let (_, replay) = send(
        &nurse,
        "POST",
        &format!("/instances/{id}/events"),
        Some(json!({ "event": "triaged", "opId": "op-4" })),
    )
    .await;
    assert_eq!(replay, json!({ "replayed": true }));

    let (_, tasks) = send(&nurse, "GET", &format!("/instances/{id}/tasks"), None).await;
    assert_eq!(
        field(&tasks, "status"),
        [&json!("completed"), &json!("ready")]
    );
    assert_eq!(
        field(&tasks, "events")[0],
        &json!([]),
        "a closed task offers nothing"
    );
}

#[tokio::test]
async fn supervising_needs_its_permission() {
    let server = server().await;
    let nurse = as_(&server, "nurse-1", &[WORKFLOW_PERMISSION]);
    let lead = as_(
        &server,
        "lead-1",
        &[WORKFLOW_PERMISSION, WORKFLOW_SUPERVISE_PERMISSION],
    );
    let nobody = as_(&server, "clerk-1", &[]);

    let (status, _) = send(&nobody, "GET", "/worklists/opd-triage", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (_, instance) = send(
        &nurse,
        "POST",
        "/instances",
        Some(json!({ "definition": OPD, "context": "Encounter/e-1", "opId": "op-1" })),
    )
    .await;
    let id = instance["id"].as_str().expect("id").to_owned();
    let path = |command: &str| format!("/instances/{id}/tasks/triage/{command}");

    let (status, _) = send(
        &nurse,
        "POST",
        &path("reassign"),
        Some(json!({ "to": "nurse-3", "opId": "op-2" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(
        &lead,
        "POST",
        &path("reassign"),
        Some(json!({ "to": "nurse-3", "opId": "op-3" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &lead,
        "POST",
        &path("escalate"),
        Some(json!({ "priority": "stat", "opId": "op-4" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, triage) = send(&nurse, "GET", "/worklists/opd-triage", None).await;
    assert_eq!(field(&triage, "owner"), [&json!("nurse-3")]);
    assert_eq!(field(&triage, "priority"), [&json!("stat")]);

    let (status, _) = send(
        &lead,
        "POST",
        &path("escalate"),
        Some(json!({ "priority": "whenever", "opId": "op-5" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = send(
        &nurse,
        "POST",
        &format!("/instances/{id}/cancel"),
        Some(json!({ "reason": "left", "opId": "op-6" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(
        &lead,
        "POST",
        &format!("/instances/{id}/cancel"),
        Some(json!({ "reason": "left", "opId": "op-7" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, triage) = send(&nurse, "GET", "/worklists/opd-triage", None).await;
    assert_eq!(triage, json!([]));
}

#[tokio::test]
async fn a_node_refuses_definitions_it_cannot_run() {
    let mut guarded = opd();
    guarded["statechart"]["states"][0]["transitions"][0]["guard"] = json!("%encounter.exists()");
    assert!(matches!(
        WorkflowConfig::new(vec![definition(guarded)], "central"),
        Err(WorkflowConfigError::Unsupported(_))
    ));
    let mut broken = opd();
    broken["statechart"]["states"][0]["task"]["worklist"] = json!("");
    assert!(matches!(
        WorkflowConfig::new(vec![definition(broken)], "central"),
        Err(WorkflowConfigError::Problem { .. })
    ));

    let server = server().await;
    let nurse = as_(&server, "nurse-1", &[WORKFLOW_PERMISSION]);
    let (status, _) = send(
        &nurse,
        "POST",
        "/instances",
        Some(json!({ "definition": "urn:unknown|1", "context": "Encounter/e-1", "opId": "op-1" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn activation_replaces_the_definitions_every_holder_runs() {
    let config = WorkflowConfig::new(Vec::new(), "central").expect("empty");
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "workflow-replace".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let server = FhirServer::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        "http://test/fhir",
        Arc::new(Unrestricted),
    )
    .with_workflows(config.clone());
    let nurse = as_(&server, "nurse-1", &[WORKFLOW_PERMISSION]);
    let (_, listed) = send(&nurse, "GET", "/definitions", None).await;
    assert_eq!(listed.as_array().map(Vec::len), Some(0));

    // The kept clone replaces what the server runs.
    assert_eq!(config.replace(vec![definition(opd())]), Ok(1));
    let (_, listed) = send(&nurse, "GET", "/definitions", None).await;
    assert_eq!(listed.as_array().map(Vec::len), Some(1));

    // Definitions that cannot run leave the running ones in place.
    let mut guarded = opd();
    guarded["statechart"]["states"][0]["transitions"][0]["guard"] = json!("%encounter.exists()");
    assert!(config.replace(vec![definition(guarded)]).is_err());
    let (_, listed) = send(&nurse, "GET", "/definitions", None).await;
    assert_eq!(listed.as_array().map(Vec::len), Some(1));
}
