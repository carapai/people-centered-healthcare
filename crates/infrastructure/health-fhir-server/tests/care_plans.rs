//! Care plans over HTTP (#110): a plan drafted and activated for a patient, addressing their
//! diagnosis and planning their prescription, at its own facility only.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{Permissions, Subject};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::{
    CARE_PLANS_MANAGE_PERMISSION, CARE_PLANS_VIEW_PERMISSION, CHECK_IN_PERMISSION,
    DOCUMENT_PERMISSION, FhirServer, PRESCRIBE_PERMISSION,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const PATIENT: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000aa";
const VISIT: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000201";
const PLAN: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000f01";
const DIAGNOSIS: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000302";
const PRESCRIPTION: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000304";

fn registry() -> Registry {
    let definitions = [
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/Resource-tag", "code": "_tag", "base": ["Resource"], "type": "token", "expression": "Resource.meta.tag" }),
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/MedicationRequest-patient", "code": "patient", "base": ["MedicationRequest"], "type": "reference", "expression": "MedicationRequest.subject", "target": ["Patient"] }),
    ];
    let catalog = Catalog::from_definitions(definitions.iter());
    let mut registry = Registry::new(jiff::tz::Offset::UTC);
    registry
        .support(&catalog, "CarePlan", "_tag", ParamState::Active)
        .expect("_tag");
    registry
        .support(&catalog, "MedicationRequest", "patient", ParamState::Active)
        .expect("patient");
    registry
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
        namespace: "care-plans-http".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let server = FhirServer::new(
        SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry()),
        "http://test/fhir",
        Arc::new(Unrestricted),
    );
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
    let reception = server
        .reception_router()
        .layer(subject("KAM013", &[CHECK_IN_PERMISSION]));
    let check_in = json!({ "id": VISIT, "patient": format!("Patient/{PATIENT}"), "servicePoint": "consultation" });
    assert_eq!(
        call(&reception, Method::POST, "/check-in", Some(check_in))
            .await
            .0,
        StatusCode::CREATED
    );
    let consultation = server.consultation_router().layer(subject(
        "KAM013",
        &[DOCUMENT_PERMISSION, PRESCRIBE_PERMISSION],
    ));
    let diagnosis = json!({ "id": DIAGNOSIS, "concept": { "display": "Hypertension" } });
    assert_eq!(
        call(
            &consultation,
            Method::POST,
            &format!("/encounters/{VISIT}/diagnoses"),
            Some(diagnosis)
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let prescription = json!({ "id": PRESCRIPTION, "kind": "medication", "concept": { "display": "Amlodipine 5 mg" }, "dosage": "Once daily" });
    assert_eq!(
        call(
            &consultation,
            Method::POST,
            &format!("/encounters/{VISIT}/orders"),
            Some(prescription)
        )
        .await
        .0,
        StatusCode::CREATED
    );
    server
}

#[tokio::test]
async fn a_plan_addresses_the_diagnosis_and_plans_the_prescription() {
    let server = server().await;
    let app = server
        .care_plans_router()
        .layer(subject("KAM013", &[CARE_PLANS_MANAGE_PERMISSION]));
    let draft = json!({ "id": PLAN, "patient": format!("Patient/{PATIENT}"), "title": "Hypertension follow-up" });
    let (status, drafted) = call(&app, Method::POST, "/plans", Some(draft.clone())).await;
    assert_eq!(
        (status, drafted["status"].as_str()),
        (StatusCode::CREATED, Some("draft")),
        "{drafted}"
    );
    assert_eq!(
        call(&app, Method::POST, "/plans", Some(draft)).await.0,
        StatusCode::CONFLICT
    );
    let (status, active) = call(
        &app,
        Method::POST,
        &format!("/plans/{PLAN}/status"),
        Some(json!({ "status": "active" })),
    )
    .await;
    assert_eq!(
        (status, active["status"].as_str()),
        (StatusCode::OK, Some("active"))
    );
    let (status, _) = call(
        &app,
        Method::POST,
        &format!("/plans/{PLAN}/addresses"),
        Some(json!({ "condition": format!("Condition/{DIAGNOSIS}") })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, planned) = call(
        &app,
        Method::POST,
        &format!("/plans/{PLAN}/activities"),
        Some(json!({ "request": format!("MedicationRequest/{PRESCRIPTION}") })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{planned}");
    assert_eq!(
        planned["activities"],
        json!([format!("MedicationRequest/{PRESCRIPTION}")])
    );
    assert_eq!(
        planned["addresses"],
        json!([format!("Condition/{DIAGNOSIS}")])
    );

    let viewer = server
        .care_plans_router()
        .layer(subject("KAM013", &[CARE_PLANS_VIEW_PERMISSION]));
    let (status, plans) = call(
        &viewer,
        Method::GET,
        &format!("/patients/{PATIENT}/plans"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(plans.as_array().map(Vec::len), Some(1));
    assert_eq!(plans[0]["title"], "Hypertension follow-up");
}

#[tokio::test]
async fn facilities_permissions_and_foreign_records_are_checked() {
    let server = server().await;
    let app = server
        .care_plans_router()
        .layer(subject("KAM013", &[CARE_PLANS_MANAGE_PERMISSION]));
    let draft = json!({ "id": PLAN, "patient": format!("Patient/{PATIENT}"), "title": "Hypertension follow-up" });
    assert_eq!(
        call(&app, Method::POST, "/plans", Some(draft.clone()))
            .await
            .0,
        StatusCode::CREATED
    );
    let viewer = server
        .care_plans_router()
        .layer(subject("KAM013", &[CARE_PLANS_VIEW_PERMISSION]));
    assert_eq!(
        call(
            &viewer,
            Method::POST,
            &format!("/plans/{PLAN}/status"),
            Some(json!({ "status": "active" }))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let elsewhere = server
        .care_plans_router()
        .layer(subject("MBR001", &[CARE_PLANS_MANAGE_PERMISSION]));
    assert_eq!(
        call(
            &elsewhere,
            Method::POST,
            &format!("/plans/{PLAN}/status"),
            Some(json!({ "status": "active" }))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &elsewhere,
            Method::GET,
            &format!("/patients/{PATIENT}/plans"),
            None
        )
        .await
        .1,
        json!([])
    );
    assert_eq!(
        call(
            &app,
            Method::POST,
            &format!("/plans/{PLAN}/activities"),
            Some(json!({ "request": "MedicationRequest/0190f1a2-7c3d-7e4f-8a5b-000000000999" }))
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a request that is not the patient's"
    );
    assert_eq!(
        call(
            &app,
            Method::POST,
            &format!("/plans/{PLAN}/status"),
            Some(json!({ "status": "closed" }))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut untitled = draft;
    untitled["id"] = json!("0190f1a2-7c3d-7e4f-8a5b-000000000f02");
    untitled["title"] = json!(" ");
    assert_eq!(
        call(&app, Method::POST, "/plans", Some(untitled)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}
