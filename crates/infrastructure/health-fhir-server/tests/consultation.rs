//! The consultation's commands over HTTP (#104): a clinician records a note, a diagnosis, a test
//! order and a prescription in a visit in progress at their facility; retries write nothing new,
//! and permissions, facilities and the visit's status are checked.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{Permissions, Subject};
use health_fhir_search::Unrestricted;
use health_fhir_server::{
    CHECK_IN_PERMISSION, DOCUMENT_PERMISSION, ENCOUNTERS_MANAGE_PERMISSION, FhirServer,
    ORDER_PERMISSION, PRESCRIBE_PERMISSION,
};
use health_identifiers::{OpId, ResourceId};
use health_persistence::{Precondition, RawJson, Store, Transaction, WriteRequest};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const PATIENT: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000aa";
const VISIT: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000201";

async fn store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "consultation".into(),
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

fn layer(router: Router, facility: &str, permissions: &[&str]) -> Router {
    router.layer(Extension(Subject {
        account: "clinician-1".into(),
        facility: facility.into(),
        permissions: Permissions(permissions.iter().map(|p| (*p).to_owned()).collect()),
        programs: BTreeSet::new(),
        purpose: Some("TREAT".into()),
        break_glass: None,
    }))
}

fn consultation(store: &SurrealStore, facility: &str, permissions: &[&str]) -> Router {
    layer(
        FhirServer::new(store.clone(), "http://test/fhir", Arc::new(Unrestricted))
            .consultation_router(),
        facility,
        permissions,
    )
}

async fn send(app: &Router, path: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
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

async fn checked_in(store: &SurrealStore) {
    let reception = layer(
        FhirServer::new(store.clone(), "http://test/fhir", Arc::new(Unrestricted))
            .reception_router(),
        "KAM013",
        &[CHECK_IN_PERMISSION, ENCOUNTERS_MANAGE_PERMISSION],
    );
    let (status, body) =
        send(&reception, "/check-in", json!({ "id": VISIT, "patient": format!("Patient/{PATIENT}"), "servicePoint": "consultation" })).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

fn id(n: u32) -> String {
    format!("0190f1a2-7c3d-7e4f-8a5b-{n:012}")
}

#[tokio::test]
async fn a_clinician_records_the_consultation() {
    let store = store().await;
    checked_in(&store).await;
    let clinician = consultation(
        &store,
        "KAM013",
        &[DOCUMENT_PERMISSION, ORDER_PERMISSION, PRESCRIBE_PERMISSION],
    );
    let path = |what: &str| format!("/encounters/{VISIT}/{what}");

    let (status, composition) = send(
        &clinician,
        &path("notes"),
        json!({ "id": id(301), "text": "Fever for 3 days." }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{composition}");
    assert_eq!(composition["resourceType"], "Composition");
    assert_eq!(
        composition["encounter"]["reference"],
        format!("Encounter/{VISIT}")
    );
    // A retried command answers what it wrote.
    let (status, again) = send(
        &clinician,
        &path("notes"),
        json!({ "id": id(301), "text": "Fever for 3 days." }),
    )
    .await;
    assert_eq!(
        (status, &again["meta"]["versionId"]),
        (StatusCode::OK, &composition["meta"]["versionId"])
    );

    let (status, condition) =
        send(&clinician, &path("diagnoses"), json!({ "id": id(302), "concept": { "display": "Malaria", "system": "http://snomed.info/sct", "code": "61462000" } })).await;
    assert_eq!(status, StatusCode::CREATED, "{condition}");
    assert_eq!(
        condition["subject"]["reference"],
        format!("Patient/{PATIENT}")
    );
    assert_eq!(condition["meta"]["tag"][0]["code"], "KAM013");

    let (status, test) = send(
        &clinician,
        &path("orders"),
        json!({ "id": id(303), "kind": "service", "concept": { "display": "Malaria RDT" } }),
    )
    .await;
    assert_eq!(
        (status, test["resourceType"].as_str()),
        (StatusCode::CREATED, Some("ServiceRequest")),
        "{test}"
    );
    let (status, prescription) = send(
        &clinician,
        &path("orders"),
        json!({ "id": id(304), "kind": "medication", "concept": { "display": "Artemether/lumefantrine 20/120 mg" }, "dosage": "4 tablets twice daily for 3 days" }),
    )
    .await;
    assert_eq!(
        (status, prescription["resourceType"].as_str()),
        (StatusCode::CREATED, Some("MedicationRequest")),
        "{prescription}"
    );

    // Incomplete commands and ids that are not UUIDv7s are refused.
    assert_eq!(
        send(
            &clinician,
            &path("notes"),
            json!({ "id": id(305), "text": " " })
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        send(&clinician, &path("orders"), json!({ "id": id(306), "kind": "medication", "concept": { "display": "Paracetamol" }, "dosage": "" })).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        send(
            &clinician,
            &path("notes"),
            json!({ "id": "6f1c1f0e-3b7a-4c55-9d2e-1f0a2b3c4d5e", "text": "x" })
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn permissions_facilities_and_the_visit_status_are_checked() {
    let store = store().await;
    checked_in(&store).await;
    let path = |what: &str| format!("/encounters/{VISIT}/{what}");
    let order = json!({ "id": id(401), "kind": "medication", "concept": { "display": "Paracetamol" }, "dosage": "1 g" });

    let nurse = consultation(&store, "KAM013", &[DOCUMENT_PERMISSION, ORDER_PERMISSION]);
    assert_eq!(
        send(&nurse, &path("orders"), order.clone()).await.0,
        StatusCode::FORBIDDEN,
        "prescribing needs its permission"
    );
    let elsewhere = consultation(
        &store,
        "MBR001",
        &[DOCUMENT_PERMISSION, PRESCRIBE_PERMISSION],
    );
    assert_eq!(
        send(&elsewhere, &path("orders"), order.clone()).await.0,
        StatusCode::NOT_FOUND,
        "another facility's visit"
    );
    let clinician = consultation(
        &store,
        "KAM013",
        &[DOCUMENT_PERMISSION, PRESCRIBE_PERMISSION],
    );
    let unknown = format!("/encounters/{}/notes", id(999));
    assert_eq!(
        send(&clinician, &unknown, json!({ "id": id(402), "text": "x" }))
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    // The visit completed: nothing more is recorded in it.
    let reception = layer(
        FhirServer::new(store.clone(), "http://test/fhir", Arc::new(Unrestricted))
            .reception_router(),
        "KAM013",
        &[ENCOUNTERS_MANAGE_PERMISSION],
    );
    assert_eq!(
        send(
            &reception,
            &format!("/encounters/{VISIT}/status"),
            json!({ "status": "completed" })
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        send(&clinician, &path("orders"), order).await.0,
        StatusCode::CONFLICT
    );
}
