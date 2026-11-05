//! The laboratory over HTTP (#109): a laboratory order from the consultation is collected,
//! resulted, verified by a second person and reviewed, each step on its worklist through the lab
//! Workflow Definition (the `ug.moh.lab` reference package). The clinician sees no result before
//! it is verified, and a critical one reaches the clinician's results worklist at stat priority.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{Permissions, Subject};
use health_fhir_search::Unrestricted;
use health_fhir_server::{
    CHECK_IN_PERMISSION, FhirServer, LAB_VERIFY_PERMISSION, LAB_WORK_PERMISSION, ORDER_PERMISSION,
    RESULTS_REVIEW_PERMISSION, WorkflowConfig,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use health_workflow::definition::WorkflowDefinition;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const PATIENT: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000aa";
const VISIT: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000201";
const ORDER: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000c01";
const SPECIMEN: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000d01";

fn lab_workflow() -> WorkflowDefinition {
    let source = include_str!("../../../../packages/reference-packages/sources/lab-test.json");
    serde_json::from_str(source).expect("the lab Workflow Definition")
}

fn subject(account: &str, facility: &str, permissions: &[&str]) -> Extension<Subject> {
    Extension(Subject {
        account: account.into(),
        facility: facility.into(),
        permissions: Permissions(permissions.iter().map(|p| (*p).to_owned()).collect()),
        programs: BTreeSet::new(),
        purpose: Some("TREAT".into()),
        break_glass: None,
    })
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

async fn server() -> FhirServer {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "laboratory-http".into(),
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
    .with_workflows(WorkflowConfig::new(vec![lab_workflow()], "central").expect("config"));
    let request = Request::put(format!("/Patient/{PATIENT}"))
        .header("content-type", "application/fhir+json")
        .body(Body::from(
            json!({ "resourceType": "Patient", "id": PATIENT }).to_string(),
        ))
        .expect("request");
    assert_eq!(
        server
            .clone()
            .router()
            .oneshot(request)
            .await
            .expect("response")
            .status(),
        StatusCode::CREATED
    );
    let reception =
        server
            .reception_router()
            .layer(subject("reception-1", "KAM013", &[CHECK_IN_PERMISSION]));
    let check_in = json!({ "id": VISIT, "patient": format!("Patient/{PATIENT}"), "servicePoint": "consultation" });
    assert_eq!(
        call(&reception, Method::POST, "/check-in", Some(check_in))
            .await
            .0,
        StatusCode::CREATED
    );
    server
}

async fn order(server: &FhirServer, id: &str, laboratory: bool) {
    let consultation =
        server
            .consultation_router()
            .layer(subject("clinician-1", "KAM013", &[ORDER_PERMISSION]));
    let body = json!({ "id": id, "kind": "service", "concept": { "display": "Haemoglobin" }, "laboratory": laboratory });
    let (status, written) = call(
        &consultation,
        Method::POST,
        &format!("/encounters/{VISIT}/orders"),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{written}");
}

/// The open tasks of worklist `key`.
async fn worklist(server: &FhirServer, key: &str) -> Vec<Value> {
    let workflows = server
        .workflow_router()
        .layer(subject("staff", "KAM013", &["workflow.work"]));
    let (status, tasks) = call(&workflows, Method::GET, &format!("/worklists/{key}"), None).await;
    assert_eq!(status, StatusCode::OK, "{tasks}");
    tasks.as_array().cloned().unwrap_or_default()
}

fn hb(value: &str) -> Value {
    json!({ "code": { "text": "Haemoglobin" }, "value": { "kind": "quantity", "value": value, "unit": "g/dL" },
            "reference": { "low": "11.5", "high": "16.5" }, "critical": { "low": "7.0" } })
}

#[tokio::test]
async fn a_result_reaches_the_clinician_only_once_a_second_person_verifies_it() {
    let server = server().await;
    order(&server, ORDER, true).await;
    let context = format!("ServiceRequest/{ORDER}");
    assert_eq!(
        worklist(&server, "lab-collect").await[0]["context"],
        context
    );

    let analyst = server.laboratory_router().layer(subject(
        "analyst-1",
        "KAM013",
        &[LAB_WORK_PERMISSION, LAB_VERIFY_PERMISSION],
    ));
    let clinician = server.laboratory_router().layer(subject(
        "clinician-1",
        "KAM013",
        &[RESULTS_REVIEW_PERMISSION],
    ));
    let (status, collected) = call(
        &analyst,
        Method::POST,
        &format!("/tests/{ORDER}/collect"),
        Some(json!({ "specimen": SPECIMEN, "kind": "Venous blood" })),
    )
    .await;
    assert_eq!(
        (status, collected["stage"].as_str()),
        (StatusCode::OK, Some("collected")),
        "{collected}"
    );
    assert_eq!(worklist(&server, "lab-result").await.len(), 1);

    let (status, resulted) = call(
        &analyst,
        Method::POST,
        &format!("/tests/{ORDER}/result"),
        Some(json!({ "results": [hb("6.4")] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resulted}");
    assert_eq!(resulted["results"][0]["flag"], "critical-low");
    assert_eq!(worklist(&server, "lab-verify").await.len(), 1);

    // Before verification: no results for the clinician, and nothing on their worklist.
    let (status, unreleased) =
        call(&clinician, Method::GET, &format!("/tests/{ORDER}"), None).await;
    assert_eq!(
        (status, unreleased["stage"].as_str()),
        (StatusCode::OK, Some("resulted"))
    );
    assert_eq!(
        unreleased["results"],
        json!([]),
        "unverified results are the laboratory's alone"
    );
    assert!(worklist(&server, "clinician-results").await.is_empty());

    // Not by whoever entered them.
    assert_eq!(
        call(
            &analyst,
            Method::POST,
            &format!("/tests/{ORDER}/verify"),
            None
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let verifier =
        server
            .laboratory_router()
            .layer(subject("analyst-2", "KAM013", &[LAB_VERIFY_PERMISSION]));
    let (status, verified) = call(
        &verifier,
        Method::POST,
        &format!("/tests/{ORDER}/verify"),
        None,
    )
    .await;
    assert_eq!(
        (status, verified["stage"].as_str()),
        (StatusCode::OK, Some("verified")),
        "{verified}"
    );

    let (_, released) = call(&clinician, Method::GET, &format!("/tests/{ORDER}"), None).await;
    assert_eq!(released["results"][0]["entry"]["value"]["value"], "6.4");
    let inbox = worklist(&server, "clinician-results").await;
    assert_eq!(inbox.len(), 1);
    assert_eq!(
        (inbox[0]["context"].as_str(), inbox[0]["priority"].as_str()),
        (Some(context.as_str()), Some("stat")),
        "a critical result, urgently"
    );

    let (status, reviewed) = call(
        &clinician,
        Method::POST,
        &format!("/tests/{ORDER}/review"),
        None,
    )
    .await;
    assert_eq!(
        (status, reviewed["stage"].as_str()),
        (StatusCode::OK, Some("reviewed"))
    );
    assert!(worklist(&server, "clinician-results").await.is_empty());
}

#[tokio::test]
async fn permissions_facilities_and_non_laboratory_orders() {
    let server = server().await;
    order(&server, ORDER, true).await;
    let elsewhere =
        server
            .laboratory_router()
            .layer(subject("analyst-9", "MBR001", &[LAB_WORK_PERMISSION]));
    assert_eq!(
        call(&elsewhere, Method::GET, &format!("/tests/{ORDER}"), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let clinician = server.laboratory_router().layer(subject(
        "clinician-1",
        "KAM013",
        &[RESULTS_REVIEW_PERMISSION],
    ));
    assert_eq!(
        call(
            &clinician,
            Method::POST,
            &format!("/tests/{ORDER}/collect"),
            Some(json!({ "specimen": SPECIMEN, "kind": "Blood" }))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let analyst =
        server
            .laboratory_router()
            .layer(subject("analyst-1", "KAM013", &[LAB_WORK_PERMISSION]));
    assert_eq!(
        call(
            &analyst,
            Method::POST,
            &format!("/tests/{ORDER}/verify"),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN,
        "verifying needs its permission"
    );
    let (status, rejected) = call(
        &analyst,
        Method::POST,
        &format!("/tests/{ORDER}/cancel"),
        Some(json!({ "reason": "ordered in error" })),
    )
    .await;
    assert_eq!(
        (status, rejected["stage"].as_str()),
        (StatusCode::OK, Some("cancelled"))
    );
    assert!(
        worklist(&server, "lab-collect").await.is_empty(),
        "the workflow closed with the test"
    );

    let other = "0190f1a2-7c3d-7e4f-8a5b-000000000c02";
    order(&server, other, false).await;
    assert_eq!(
        call(&analyst, Method::GET, &format!("/tests/{other}"), None)
            .await
            .0,
        StatusCode::NOT_FOUND,
        "not a laboratory order"
    );
}
