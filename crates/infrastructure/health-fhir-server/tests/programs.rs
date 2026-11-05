//! Programs over HTTP (#116): a patient enrolled in ART and TB concurrently, ART's Workflow
//! Definition started about the episode, eligibility decided by the program, and the status and
//! transfer of an enrolment at its own facility only.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{Permissions, Subject};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::{
    FhirServer, PROGRAMS_ENROL_PERMISSION, PROGRAMS_MANAGE_PERMISSION, PROGRAMS_VIEW_PERMISSION,
    WorkflowConfig,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use health_workflow::definition::WorkflowDefinition;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const ACTIVE: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000a1";
const INACTIVE: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000a2";
const ART: &str = "https://health.go.ug/fhir/PlanDefinition/art-program";
const TB: &str = "https://health.go.ug/fhir/PlanDefinition/tb-program";

fn registry() -> Registry {
    let definitions = [
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/PlanDefinition-type", "code": "type", "base": ["PlanDefinition"], "type": "token", "expression": "PlanDefinition.type" }),
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/EpisodeOfCare-patient", "code": "patient", "base": ["EpisodeOfCare"], "type": "reference", "expression": "EpisodeOfCare.patient", "target": ["Patient"] }),
    ];
    let catalog = Catalog::from_definitions(definitions.iter());
    let mut registry = Registry::new(jiff::tz::Offset::UTC);
    registry
        .support(&catalog, "PlanDefinition", "type", ParamState::Active)
        .expect("type");
    registry
        .support(&catalog, "EpisodeOfCare", "patient", ParamState::Active)
        .expect("patient");
    registry
}

fn plan(
    id: &str,
    url: &str,
    episode_type: &str,
    eligibility: Option<&str>,
    workflow: Option<&str>,
) -> Value {
    let mut plan = json!({
        "resourceType": "PlanDefinition", "id": id, "status": "active", "url": url, "version": "1.0.0",
        "type": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/plan-definition-type", "code": "clinical-protocol" }] },
        "extension": [{ "url": "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/program-episode-type", "valueCodeableConcept": { "text": episode_type } }]
    });
    if let Some(expression) = eligibility {
        plan["extension"].as_array_mut().expect("extensions").push(json!({
            "url": "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/program-eligibility",
            "valueExpression": { "language": "text/fhirpath", "expression": expression }
        }));
    }
    if let Some(workflow) = workflow {
        plan["relatedArtifact"] = json!([{ "type": "composed-of", "resource": workflow }]);
    }
    plan
}

fn follow_up() -> WorkflowDefinition {
    serde_json::from_value(json!({
        "url": "https://health.go.ug/fhir/Library/art-follow-up", "version": "1.0.0",
        "statechart": { "states": [
            { "id": "first-visit", "kind": "atomic", "task": { "role": "clinician", "worklist": "art-follow-up" },
              "transitions": [{ "events": ["seen"], "targets": ["done"] }] },
            { "id": "done", "kind": "final" }
        ] }
    }))
    .expect("definition")
}

async fn server() -> FhirServer {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "programs-http".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry());
    let server = FhirServer::new(store, "http://test/fhir", Arc::new(Unrestricted))
        .with_workflows(WorkflowConfig::new(vec![follow_up()], "central").expect("config"));
    let admin = server.clone().router();
    for (path, body) in [
        (
            format!("/Patient/{ACTIVE}"),
            json!({ "resourceType": "Patient", "id": ACTIVE, "active": true }),
        ),
        (
            format!("/Patient/{INACTIVE}"),
            json!({ "resourceType": "Patient", "id": INACTIVE, "active": false }),
        ),
        (
            "/PlanDefinition/0190f1a2-7c3d-7e4f-8a5b-0000000000b1".to_owned(),
            plan(
                "0190f1a2-7c3d-7e4f-8a5b-0000000000b1",
                ART,
                "HIV care",
                Some("%patient.active"),
                Some("https://health.go.ug/fhir/Library/art-follow-up"),
            ),
        ),
        (
            "/PlanDefinition/0190f1a2-7c3d-7e4f-8a5b-0000000000b2".to_owned(),
            plan(
                "0190f1a2-7c3d-7e4f-8a5b-0000000000b2",
                TB,
                "TB treatment",
                None,
                None,
            ),
        ),
    ] {
        let (status, created) = call(&admin, Method::PUT, &path, Some(body)).await;
        assert_eq!(status, StatusCode::CREATED, "{path}: {created}");
    }
    server
}

fn subject(facility: &str, permissions: &[&str]) -> Extension<Subject> {
    Extension(Subject {
        account: "clinician-1".into(),
        facility: facility.into(),
        permissions: Permissions(permissions.iter().map(|p| (*p).to_owned()).collect()),
        programs: BTreeSet::new(),
        purpose: Some("TREAT".into()),
        break_glass: None,
    })
}

fn programs(server: &FhirServer, facility: &str, permissions: &[&str]) -> Router {
    server
        .programs_router()
        .layer(subject(facility, permissions))
}

async fn call(
    app: &Router,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
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

fn enrolment(id: u32, patient: &str, program: &str) -> Value {
    json!({ "id": format!("0190f1a2-7c3d-7e4f-8a5b-{id:012}"), "patient": format!("Patient/{patient}"), "program": program })
}

const ALL: [&str; 3] = [
    PROGRAMS_VIEW_PERMISSION,
    PROGRAMS_ENROL_PERMISSION,
    PROGRAMS_MANAGE_PERMISSION,
];

#[tokio::test]
async fn a_patient_is_enrolled_in_art_and_tb_concurrently_and_arts_workflow_starts() {
    let server = server().await;
    let app = programs(&server, "KAM013", &ALL);
    let (status, listed) = call(&app, Method::GET, "/definitions", None).await;
    assert_eq!(status, StatusCode::OK);
    let mut urls: Vec<&str> = listed
        .as_array()
        .expect("list")
        .iter()
        .filter_map(|p| p["url"].as_str())
        .collect();
    urls.sort_unstable();
    assert_eq!(urls, [ART, TB]);

    let (status, art) = call(
        &app,
        Method::POST,
        "/enrolments",
        Some(enrolment(501, ACTIVE, ART)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{art}");
    assert_eq!(
        art["workflows"],
        json!(["https://health.go.ug/fhir/Library/art-follow-up|1.0.0"])
    );
    let (status, tb) = call(
        &app,
        Method::POST,
        "/enrolments",
        Some(enrolment(502, ACTIVE, TB)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{tb}");
    assert_eq!(
        call(
            &app,
            Method::POST,
            "/enrolments",
            Some(enrolment(503, ACTIVE, ART))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );

    let (status, episodes) = call(
        &app,
        Method::GET,
        &format!("/patients/{ACTIVE}/enrolments"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(episodes.as_array().map(Vec::len), Some(2), "{episodes}");

    // ART's follow-up runs about the episode: its first task is on the worklist.
    let worklists = server
        .workflow_router()
        .layer(subject("KAM013", &["workflow.work"]));
    let (status, tasks) = call(&worklists, Method::GET, "/worklists/art-follow-up", None).await;
    assert_eq!(status, StatusCode::OK, "{tasks}");
    assert_eq!(
        tasks[0]["context"],
        format!("EpisodeOfCare/{}", art["id"].as_str().expect("id"))
    );
}

#[tokio::test]
async fn eligibility_permissions_and_facilities_are_checked() {
    let server = server().await;
    let app = programs(&server, "KAM013", &ALL);
    let (status, refused) = call(
        &app,
        Method::POST,
        "/enrolments",
        Some(enrolment(601, INACTIVE, ART)),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");
    assert!(
        refused["error"]
            .as_str()
            .unwrap_or_default()
            .contains("not eligible")
    );
    assert_eq!(
        call(
            &app,
            Method::POST,
            "/enrolments",
            Some(enrolment(602, INACTIVE, TB))
        )
        .await
        .0,
        StatusCode::CREATED,
        "TB takes everyone"
    );
    assert_eq!(
        call(
            &app,
            Method::POST,
            "/enrolments",
            Some(enrolment(
                603,
                ACTIVE,
                "https://health.go.ug/fhir/PlanDefinition/none"
            ))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let mut v4 = enrolment(604, ACTIVE, TB);
    v4["id"] = json!("6f1c1f0e-3b7a-4c55-9d2e-1f0a2b3c4d5e");
    assert_eq!(
        call(&app, Method::POST, "/enrolments", Some(v4)).await.0,
        StatusCode::BAD_REQUEST
    );
    let viewer = programs(&server, "KAM013", &[PROGRAMS_VIEW_PERMISSION]);
    assert_eq!(
        call(
            &viewer,
            Method::POST,
            "/enrolments",
            Some(enrolment(605, ACTIVE, TB))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );

    // Status and transfer, at the enrolment's own facility only.
    let (_, art) = call(
        &app,
        Method::POST,
        "/enrolments",
        Some(enrolment(606, ACTIVE, ART)),
    )
    .await;
    let id = art["id"].as_str().expect("id").to_owned();
    let elsewhere = programs(&server, "MBR001", &ALL);
    assert_eq!(
        call(
            &elsewhere,
            Method::POST,
            &format!("/enrolments/{id}/status"),
            Some(json!({ "status": "onhold" }))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &elsewhere,
            Method::GET,
            &format!("/patients/{ACTIVE}/enrolments"),
            None
        )
        .await
        .1,
        json!([])
    );
    let (status, held) = call(
        &app,
        Method::POST,
        &format!("/enrolments/{id}/status"),
        Some(json!({ "status": "onhold" })),
    )
    .await;
    assert_eq!(
        (status, held["status"].as_str()),
        (StatusCode::OK, Some("onhold")),
        "{held}"
    );
    assert_eq!(
        call(
            &app,
            Method::POST,
            &format!("/enrolments/{id}/status"),
            Some(json!({ "status": "closed" }))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            Method::POST,
            &format!("/enrolments/{id}/status"),
            Some(json!({ "status": "planned" }))
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, left) = call(
        &app,
        Method::POST,
        &format!("/enrolments/{id}/transfer"),
        Some(json!({ "facility": "MBR001" })),
    )
    .await;
    assert_eq!(
        (status, left["transferredTo"].as_str()),
        (StatusCode::OK, Some("MBR001")),
        "{left}"
    );

    // The receiving facility enrols the patient in an episode continuing this one.
    let mut arriving = enrolment(607, ACTIVE, ART);
    arriving["transferredFrom"] = json!(format!("EpisodeOfCare/{id}"));
    let (status, continued) = call(&elsewhere, Method::POST, "/enrolments", Some(arriving)).await;
    assert_eq!(status, StatusCode::CREATED, "{continued}");
    assert_eq!(continued["transferredFrom"], format!("EpisodeOfCare/{id}"));
}
