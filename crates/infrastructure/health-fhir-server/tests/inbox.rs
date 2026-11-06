//! The clinical inbox over HTTP (#110): a clinician's verified results (critical first), their
//! own unsigned notes and patients' requests, each linking its authoritative resource, filtered by
//! the policy decision point as a read of that resource would be.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{FACILITY_TAG_SYSTEM, Pdp, Permissions, Subject};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::{
    APPOINTMENTS_BOOK_PERMISSION, CHECK_IN_PERMISSION, DOCUMENT_PERMISSION, FacilityTag,
    FhirServer, LAB_VERIFY_PERMISSION, LAB_WORK_PERMISSION, ORDER_PERMISSION, PRESCRIBE_PERMISSION,
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

fn registry() -> Registry {
    let definitions = [
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/Resource-tag", "code": "_tag", "base": ["Resource"], "type": "token", "expression": "Resource.meta.tag" }),
    ];
    let catalog = Catalog::from_definitions(definitions.iter());
    let mut registry = Registry::new(jiff::tz::Offset::UTC);
    for base in [
        "Appointment",
        "Composition",
        "MedicationRequest",
        "ServiceRequest",
    ] {
        registry
            .support(&catalog, base, "_tag", ParamState::Active)
            .expect("_tag");
    }
    registry
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

/// A server deciding with the policy decision point, and its unguarded twin for setting up.
async fn servers() -> (FhirServer, FhirServer) {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "inbox".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry());
    let lab: WorkflowDefinition = serde_json::from_str(include_str!(
        "../../../../packages/reference-packages/sources/lab-test.json"
    ))
    .expect("definition");
    let workflows = || WorkflowConfig::new(vec![lab.clone()], "central").expect("config");
    let setup = FhirServer::new(store.clone(), "http://test/fhir", Arc::new(Unrestricted))
        .with_workflows(workflows());
    let guarded = FhirServer::new(store, "http://test/fhir", Arc::new(Unrestricted))
        .with_workflows(workflows())
        .with_access_control(Pdp::new(vec![]), Arc::new(FacilityTag));
    (setup, guarded)
}

async fn put(server: &FhirServer, path: &str, body: Value) {
    let request = Request::put(path)
        .header("content-type", "application/fhir+json")
        .body(Body::from(body.to_string()))
        .expect("request");
    let response = server
        .clone()
        .router()
        .oneshot(request)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CREATED, "{path}");
}

/// A verified critical haemoglobin, a draft and a signed note by clinician-1, a colleague's
/// draft, and a patient's refill request.
async fn setup(server: &FhirServer) {
    put(
        server,
        &format!("/Patient/{PATIENT}"),
        json!({ "resourceType": "Patient", "id": PATIENT }),
    )
    .await;
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
    let consultation =
        server
            .consultation_router()
            .layer(subject("clinician-1", "KAM013", &[ORDER_PERMISSION]));
    let order = json!({ "id": ORDER, "kind": "service", "concept": { "display": "Haemoglobin" } });
    assert_eq!(
        call(
            &consultation,
            Method::POST,
            &format!("/encounters/{VISIT}/orders"),
            Some(order)
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let analyst =
        server
            .laboratory_router()
            .layer(subject("analyst-1", "KAM013", &[LAB_WORK_PERMISSION]));
    let specimen =
        json!({ "specimen": "0190f1a2-7c3d-7e4f-8a5b-000000000d01", "kind": "Venous blood" });
    assert_eq!(
        call(
            &analyst,
            Method::POST,
            &format!("/tests/{ORDER}/collect"),
            Some(specimen)
        )
        .await
        .0,
        StatusCode::OK
    );
    let result = json!({ "results": [{ "code": { "text": "Haemoglobin" }, "value": { "kind": "quantity", "value": "6.4", "unit": "g/dL" }, "critical": { "low": "7.0" } }] });
    assert_eq!(
        call(
            &analyst,
            Method::POST,
            &format!("/tests/{ORDER}/result"),
            Some(result)
        )
        .await
        .0,
        StatusCode::OK
    );
    let verifier =
        server
            .laboratory_router()
            .layer(subject("analyst-2", "KAM013", &[LAB_VERIFY_PERMISSION]));
    assert_eq!(
        call(
            &verifier,
            Method::POST,
            &format!("/tests/{ORDER}/verify"),
            None
        )
        .await
        .0,
        StatusCode::OK
    );

    let note = |id: u32| {
        json!({ "id": format!("0190f1a2-7c3d-7e4f-8a5b-{id:012}"), "kind": "progress", "patient": format!("Patient/{PATIENT}"),
                                 "sections": [{ "title": "Plan", "text": "Review the haemoglobin" }] })
    };
    let mine =
        server
            .notes_router()
            .layer(subject("clinician-1", "KAM013", &[DOCUMENT_PERMISSION]));
    assert_eq!(
        call(&mine, Method::POST, "/notes", Some(note(901))).await.0,
        StatusCode::CREATED
    );
    assert_eq!(
        call(&mine, Method::POST, "/notes", Some(note(902))).await.0,
        StatusCode::CREATED
    );
    assert_eq!(
        call(
            &mine,
            Method::POST,
            "/notes/0190f1a2-7c3d-7e4f-8a5b-000000000902/sign",
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let colleague =
        server
            .notes_router()
            .layer(subject("clinician-2", "KAM013", &[DOCUMENT_PERMISSION]));
    assert_eq!(
        call(&colleague, Method::POST, "/notes", Some(note(903)))
            .await
            .0,
        StatusCode::CREATED
    );

    let refill = "0190f1a2-7c3d-7e4f-8a5b-000000000e01";
    put(server, &format!("/MedicationRequest/{refill}"), json!({
        "resourceType": "MedicationRequest", "id": refill, "status": "active", "intent": "proposal",
        "meta": { "tag": [{ "system": FACILITY_TAG_SYSTEM, "code": "KAM013" }] },
        "medication": { "concept": { "text": "Metformin 500 mg" } },
        "subject": { "reference": format!("Patient/{PATIENT}") }, "authoredOn": "2026-11-05T08:00:00Z"
    }))
    .await;
    let visit = "0190f1a2-7c3d-7e4f-8a5b-000000000e02";
    put(server, &format!("/Appointment/{visit}"), json!({
        "resourceType": "Appointment", "id": visit, "status": "proposed",
        "meta": { "tag": [{ "system": FACILITY_TAG_SYSTEM, "code": "KAM013" }] },
        "description": "Diabetes review",
        "subject": { "reference": format!("Patient/{PATIENT}") },
        "participant": [{ "actor": { "reference": format!("Patient/{PATIENT}") }, "status": "accepted" }],
        "created": "2026-11-05T09:00:00Z"
    }))
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_inbox_links_each_item_to_its_authoritative_resource() {
    let (setup_server, server) = servers().await;
    setup(&setup_server).await;
    let clinician = server.inbox_router().layer(subject(
        "clinician-1",
        "KAM013",
        &["*.rs", RESULTS_REVIEW_PERMISSION, PRESCRIBE_PERMISSION],
    ));
    let (status, inbox) = call(&clinician, Method::GET, "/", None).await;
    assert_eq!(status, StatusCode::OK, "{inbox}");
    let items = inbox.as_array().cloned().unwrap_or_default();
    let kinds: Vec<&str> = items.iter().filter_map(|i| i["kind"].as_str()).collect();
    assert_eq!(kinds, ["result", "documentation", "request"], "{inbox}");
    assert_eq!(
        (items[0]["title"].as_str(), items[0]["priority"].as_str()),
        (Some("Haemoglobin"), Some("stat")),
        "the critical result first"
    );

    // Each reference is the authoritative resource, readable through the FHIR API.
    let fhir = setup_server.clone().router();
    for item in &items {
        let reference = item["reference"].as_str().expect("reference");
        let response = fhir
            .clone()
            .oneshot(
                Request::get(format!("/{reference}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK, "{reference}");
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let resource: Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(
            format!(
                "{}/{}",
                resource["resourceType"].as_str().unwrap_or_default(),
                resource["id"].as_str().unwrap_or_default()
            ),
            reference
        );
    }
    assert!(
        items[0]["reference"]
            .as_str()
            .unwrap_or_default()
            .starts_with("DiagnosticReport/")
    );
    assert_eq!(
        items[1]["reference"], "Composition/0190f1a2-7c3d-7e4f-8a5b-000000000901",
        "my draft, not my signed note nor a colleague's draft"
    );
    assert_eq!(items[2]["title"], "Metformin 500 mg");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn appointment_requests_go_to_whoever_books() {
    let (setup_server, server) = servers().await;
    setup(&setup_server).await;
    let booker = server.inbox_router().layer(subject(
        "booker-1",
        "KAM013",
        &["*.rs", APPOINTMENTS_BOOK_PERMISSION],
    ));
    let (_, inbox) = call(&booker, Method::GET, "/", None).await;
    let items = inbox.as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), 1, "only the appointment request: {inbox}");
    assert_eq!(
        (
            items[0]["kind"].as_str(),
            items[0]["title"].as_str(),
            items[0]["reference"].as_str()
        ),
        (
            Some("request"),
            Some("Diabetes review"),
            Some("Appointment/0190f1a2-7c3d-7e4f-8a5b-000000000e02")
        )
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_policy_decision_point_filters_the_inbox() {
    let (setup_server, server) = servers().await;
    setup(&setup_server).await;
    // Results to review, but no read of DiagnosticReports: the result is left out.
    let narrow = server.inbox_router().layer(subject(
        "clinician-1",
        "KAM013",
        &[RESULTS_REVIEW_PERMISSION, "Composition.rs"],
    ));
    let (_, inbox) = call(&narrow, Method::GET, "/", None).await;
    let kinds: Vec<&str> = inbox
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| i["kind"].as_str())
        .collect();
    assert_eq!(kinds, ["documentation"], "{inbox}");
    // Another facility sees none of it.
    let elsewhere = server.inbox_router().layer(subject(
        "clinician-1",
        "MBR001",
        &["*.rs", RESULTS_REVIEW_PERMISSION, PRESCRIBE_PERMISSION],
    ));
    assert_eq!(call(&elsewhere, Method::GET, "/", None).await.1, json!([]));
    // Signed in, or nothing.
    let anonymous = server.inbox_router();
    assert_eq!(
        call(&anonymous, Method::GET, "/", None).await.0,
        StatusCode::UNAUTHORIZED
    );
}
