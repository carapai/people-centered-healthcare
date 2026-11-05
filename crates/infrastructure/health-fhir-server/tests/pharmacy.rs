//! Prescribing and the pharmacy worklist over HTTP (#108): a prescription from the consultation
//! appears on the pharmacy worklist and stock is unchanged; safety warnings are acknowledged
//! before anything is written; a stopped prescription leaves the worklist.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{Permissions, Subject};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::{
    CHECK_IN_PERMISSION, FhirServer, PHARMACY_VIEW_PERMISSION, PRESCRIBE_PERMISSION,
    STOCK_PERMISSION,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const PATIENT: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000aa";
const VISIT: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000201";

fn registry() -> Registry {
    let definitions = [
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/Resource-tag", "code": "_tag", "base": ["Resource"], "type": "token", "expression": "Resource.meta.tag" }),
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/MedicationRequest-patient", "code": "patient", "base": ["MedicationRequest"], "type": "reference", "expression": "MedicationRequest.subject", "target": ["Patient"] }),
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/AllergyIntolerance-patient", "code": "patient", "base": ["AllergyIntolerance"], "type": "reference", "expression": "AllergyIntolerance.patient", "target": ["Patient"] }),
    ];
    let catalog = Catalog::from_definitions(definitions.iter());
    let mut registry = Registry::new(jiff::tz::Offset::UTC);
    for (base, code) in [
        ("MedicationRequest", "_tag"),
        ("MedicationRequest", "patient"),
        ("AllergyIntolerance", "patient"),
    ] {
        registry
            .support(&catalog, base, code, ParamState::Active)
            .expect(code);
    }
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
        namespace: "pharmacy".into(),
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
    let fhir = server.clone().router();
    let request = Request::put(format!("/Patient/{PATIENT}"))
        .header("content-type", "application/fhir+json")
        .body(Body::from(
            json!({ "resourceType": "Patient", "id": PATIENT }).to_string(),
        ))
        .expect("request");
    assert_eq!(
        fhir.oneshot(request).await.expect("response").status(),
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
    server
}

fn prescription(n: u32, medicine: &str) -> Value {
    json!({ "id": format!("0190f1a2-7c3d-7e4f-8a5b-{n:012}"), "kind": "medication", "concept": { "display": medicine },
            "dosage": "4 tablets twice daily", "days": 3 })
}

async fn on_hand(server: &FhirServer) -> Value {
    let stock = server
        .inventory_router()
        .layer(subject("KAM013", &[STOCK_PERMISSION]));
    let (status, on_hand) = call(
        &stock,
        Method::GET,
        "/on-hand?store=Location%2Fpharmacy",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    on_hand
}

#[tokio::test]
async fn a_prescription_appears_on_the_pharmacy_worklist_and_stock_is_unchanged() {
    let server = server().await;
    let stock = server
        .inventory_router()
        .layer(subject("KAM013", &[STOCK_PERMISSION]));
    let receipt = json!({ "store": "Location/pharmacy", "supplier": "NMS", "delivery": "INV-7",
        "lines": [{ "product": "artemether-lumefantrine", "batch": "AL1", "expiry": "2027-12-31", "quantity": 240, "pack_size": 24, "unit_cost": "0.50" }] });
    assert_eq!(
        call(&stock, Method::POST, "/goods-received", Some(receipt))
            .await
            .0,
        StatusCode::CREATED
    );
    let before = on_hand(&server).await;

    let consultation = server
        .consultation_router()
        .layer(subject("KAM013", &[PRESCRIBE_PERMISSION]));
    let (status, written) = call(
        &consultation,
        Method::POST,
        &format!("/encounters/{VISIT}/orders"),
        Some(prescription(301, "Artemether/lumefantrine 20/120 mg")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{written}");
    assert_eq!(
        (written["resourceType"].as_str(), written["status"].as_str()),
        (Some("MedicationRequest"), Some("active"))
    );
    assert_eq!(
        written["dispenseRequest"]["expectedSupplyDuration"]["value"],
        3
    );

    let pharmacy = server
        .pharmacy_router()
        .layer(subject("KAM013", &[PHARMACY_VIEW_PERMISSION]));
    let (status, worklist) = call(&pharmacy, Method::GET, "/worklist", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(worklist.as_array().map(Vec::len), Some(1), "{worklist}");
    assert_eq!(
        worklist[0]["medicine"]["text"],
        "Artemether/lumefantrine 20/120 mg"
    );
    assert_eq!(worklist[0]["patient"], format!("Patient/{PATIENT}"));
    assert_eq!(
        on_hand(&server).await,
        before,
        "a prescription moves no stock"
    );

    let elsewhere = server
        .pharmacy_router()
        .layer(subject("MBR001", &[PHARMACY_VIEW_PERMISSION]));
    assert_eq!(
        call(&elsewhere, Method::GET, "/worklist", None).await.1,
        json!([])
    );
    let unpermitted = server
        .pharmacy_router()
        .layer(subject("KAM013", &["queues.view"]));
    assert_eq!(
        call(&unpermitted, Method::GET, "/worklist", None).await.0,
        StatusCode::FORBIDDEN
    );

    // Stopped by the prescriber, it leaves the worklist.
    let prescriber = server
        .pharmacy_router()
        .layer(subject("KAM013", &[PRESCRIBE_PERMISSION]));
    let id = written["id"].as_str().expect("id");
    let (status, stopped) = call(
        &prescriber,
        Method::POST,
        &format!("/prescriptions/{id}/status"),
        Some(json!({ "status": "stopped" })),
    )
    .await;
    assert_eq!(
        (status, stopped["status"].as_str()),
        (StatusCode::OK, Some("stopped")),
        "{stopped}"
    );
    assert_eq!(
        call(&pharmacy, Method::GET, "/worklist", None).await.1,
        json!([])
    );
    let foreign = server
        .pharmacy_router()
        .layer(subject("MBR001", &[PRESCRIBE_PERMISSION]));
    assert_eq!(
        call(
            &foreign,
            Method::POST,
            &format!("/prescriptions/{id}/status"),
            Some(json!({ "status": "cancelled" }))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn safety_warnings_are_acknowledged_before_anything_is_written() {
    let server = server().await;
    let allergy = json!({ "resourceType": "AllergyIntolerance", "code": { "text": "Amoxicillin" },
        "clinicalStatus": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/allergyintolerance-clinical", "code": "active" }] },
        "patient": { "reference": format!("Patient/{PATIENT}") } });
    let request = Request::post("/AllergyIntolerance")
        .header("content-type", "application/fhir+json")
        .body(Body::from(allergy.to_string()))
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

    let consultation = server
        .consultation_router()
        .layer(subject("KAM013", &[PRESCRIBE_PERMISSION]));
    let path = format!("/encounters/{VISIT}/orders");
    let (status, warned) = call(
        &consultation,
        Method::POST,
        &path,
        Some(prescription(401, "Amoxicillin 500 mg")),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{warned}");
    assert_eq!(warned["warnings"][0]["code"], "allergy");
    let pharmacy = server
        .pharmacy_router()
        .layer(subject("KAM013", &[PHARMACY_VIEW_PERMISSION]));
    assert_eq!(
        call(&pharmacy, Method::GET, "/worklist", None).await.1,
        json!([]),
        "nothing written"
    );

    let mut acknowledged = prescription(401, "Amoxicillin 500 mg");
    acknowledged["acknowledge"] = json!(true);
    let (status, written) = call(&consultation, Method::POST, &path, Some(acknowledged)).await;
    assert_eq!(status, StatusCode::CREATED, "{written}");
    assert_eq!(written["warnings"][0]["code"], "allergy");

    // The same medicine again is a duplicate to acknowledge.
    let (status, again) = call(
        &consultation,
        Method::POST,
        &path,
        Some(prescription(402, "Amoxicillin 500 mg")),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let codes: Vec<&str> = again["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|w| w["code"].as_str())
        .collect();
    assert!(codes.contains(&"duplicate-therapy"), "{codes:?}");
}
