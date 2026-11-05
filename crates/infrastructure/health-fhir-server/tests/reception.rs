//! Reception and queues over HTTP (#102): a receptionist checks a patient in and the patient
//! appears in the triage queue, where a nurse calls and sees them; permissions and the session's
//! facility bound every route.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{Permissions, Subject};
use health_fhir_search::Unrestricted;
use health_fhir_server::{
    CHECK_IN_PERMISSION, ENCOUNTERS_MANAGE_PERMISSION, FhirServer, QUEUE_VIEW_PERMISSION,
    QUEUE_WORK_PERMISSION,
};
use health_identifiers::{OpId, ResourceId};
use health_persistence::{Precondition, RawJson, Store, Transaction, WriteRequest};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const PATIENT: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000aa";
const E1: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000201";
const E2: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000202";

async fn store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "reception-http".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock));
    let mut tx = store.begin().await.expect("begin");
    tx.write(WriteRequest {
        resource_type: "Patient".parse().expect("type"),
        id: PATIENT.parse::<ResourceId>().expect("id"),
        json: RawJson::from_validated(
            json!({ "resourceType": "Patient", "id": PATIENT }).to_string(),
        ),
        op_id: OpId::new(),
        precondition: Precondition::MustNotExist,
    })
    .await
    .expect("patient");
    tx.commit().await.expect("commit");
    store
}

fn app(store: &SurrealStore, facility: &str, permissions: &[&str]) -> Router {
    FhirServer::new(store.clone(), "http://test/fhir", Arc::new(Unrestricted))
        .reception_router()
        .layer(Extension(Subject {
            account: "staff-1".into(),
            facility: facility.into(),
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

fn check_in(id: &str) -> Value {
    json!({ "id": id, "patient": format!("Patient/{PATIENT}"), "servicePoint": "triage" })
}

#[tokio::test]
async fn a_checked_in_patient_appears_in_the_triage_queue_and_is_called_and_seen() {
    let store = store().await;
    let reception = app(
        &store,
        "KAM013",
        &[CHECK_IN_PERMISSION, QUEUE_VIEW_PERMISSION],
    );
    let nurse = app(
        &store,
        "KAM013",
        &[
            QUEUE_VIEW_PERMISSION,
            QUEUE_WORK_PERMISSION,
            ENCOUNTERS_MANAGE_PERMISSION,
        ],
    );

    let (status, encounter) = send(&reception, "POST", "/check-in", Some(check_in(E1))).await;
    assert_eq!(status, StatusCode::CREATED, "{encounter}");
    assert_eq!(encounter["status"], "in-progress");
    assert_eq!(encounter["servicePoint"], "KAM013/triage");
    assert_eq!(
        send(&reception, "POST", "/check-in", Some(check_in(E2)))
            .await
            .0,
        StatusCode::CONFLICT,
        "one open encounter per patient at a service point"
    );

    let (status, queue) = send(&reception, "GET", "/queues/triage", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(queue.as_array().map(Vec::len), Some(1));
    assert_eq!(queue[0]["patient"], format!("Patient/{PATIENT}"));
    assert_eq!(queue[0]["status"], "waiting");
    let entry = queue[0]["id"].as_str().expect("entry id").to_owned();

    assert_eq!(
        send(&reception, "POST", &format!("/queue/{entry}/call"), None)
            .await
            .0,
        StatusCode::FORBIDDEN,
        "calling needs queues.work"
    );
    let (status, called) = send(&nurse, "POST", &format!("/queue/{entry}/call"), None).await;
    assert_eq!(status, StatusCode::OK, "{called}");
    assert_eq!(called["status"], "called");
    assert_eq!(
        send(&nurse, "POST", &format!("/queue/{entry}/serve"), None)
            .await
            .1["status"],
        "in-service"
    );
    assert_eq!(
        send(&nurse, "POST", &format!("/queue/{entry}/call"), None)
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a patient being seen is not called again"
    );

    // Sent on to consultation: the triage entry closes and one opens there.
    let (status, moved) = send(
        &nurse,
        "POST",
        &format!("/encounters/{E1}/move"),
        Some(json!({ "servicePoint": "consultation" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(
        send(&nurse, "GET", "/queues/triage", None).await.1,
        json!([])
    );
    assert_eq!(
        send(&nurse, "GET", "/queues/consultation", None)
            .await
            .1
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    let (status, done) = send(
        &nurse,
        "POST",
        &format!("/encounters/{E1}/status"),
        Some(json!({ "status": "completed" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(
        send(&nurse, "GET", "/queues/consultation", None).await.1,
        json!([])
    );
}

#[tokio::test]
async fn requests_are_checked_and_facilities_kept_apart() {
    let store = store().await;
    let reception = app(
        &store,
        "KAM013",
        &[CHECK_IN_PERMISSION, QUEUE_VIEW_PERMISSION],
    );
    let elsewhere = app(
        &store,
        "MBR001",
        &[QUEUE_VIEW_PERMISSION, QUEUE_WORK_PERMISSION],
    );

    let nobody = app(&store, "KAM013", &[]);
    assert_eq!(
        send(&nobody, "POST", "/check-in", Some(check_in(E1)))
            .await
            .0,
        StatusCode::FORBIDDEN,
        "checking in needs encounters.checkin"
    );
    let mut v4 = check_in(E1);
    v4["id"] = json!("6f1c1f0e-3b7a-4c55-9d2e-1f0a2b3c4d5e");
    assert_eq!(
        send(&reception, "POST", "/check-in", Some(v4)).await.0,
        StatusCode::BAD_REQUEST,
        "ids are UUIDv7"
    );
    let mut unknown = check_in(E1);
    unknown["patient"] = json!("Patient/0190f1a2-7c3d-7e4f-8a5b-0000000000bb");
    assert_eq!(
        send(&reception, "POST", "/check-in", Some(unknown)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "no such patient"
    );
    let mut nested = check_in(E1);
    nested["servicePoint"] = json!("MBR001/triage");
    assert_eq!(
        send(&reception, "POST", "/check-in", Some(nested)).await.0,
        StatusCode::BAD_REQUEST,
        "service points are named per facility"
    );

    assert_eq!(
        send(&reception, "POST", "/check-in", Some(check_in(E1)))
            .await
            .0,
        StatusCode::CREATED
    );
    let entry = send(&reception, "GET", "/queues/triage", None).await.1[0]["id"]
        .as_str()
        .expect("entry")
        .to_owned();
    assert_eq!(
        send(&elsewhere, "GET", "/queues/triage", None).await.1,
        json!([]),
        "another facility's triage queue"
    );
    assert_eq!(
        send(&elsewhere, "POST", &format!("/queue/{entry}/call"), None)
            .await
            .0,
        StatusCode::NOT_FOUND,
        "another facility's entry is not stepped"
    );
    assert_eq!(
        send(&elsewhere, "POST", &format!("/queue/{entry}/dance"), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
