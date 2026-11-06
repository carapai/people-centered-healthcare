//! Dispensing over HTTP (#128): stock moves only when a dispense is completed. Preparing
//! reserves by first expiry, writing the MedicationDispense; completing (in full or in part)
//! moves stock; cancelling moves nothing; donor stock goes only to enrolled patients.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{Permissions, Subject};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::{
    CHECK_IN_PERMISSION, DISPENSE_PERMISSION, Dispensing, FhirServer, PRESCRIBE_PERMISSION,
    STOCK_PERMISSION,
};
use health_inventory::dispensing::{BatchRule, Eligibility};
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
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/EpisodeOfCare-patient", "code": "patient", "base": ["EpisodeOfCare"], "type": "reference", "expression": "EpisodeOfCare.patient", "target": ["Patient"] }),
    ];
    let catalog = Catalog::from_definitions(definitions.iter());
    let mut registry = Registry::new(jiff::tz::Offset::UTC);
    for (base, code) in [
        ("MedicationRequest", "_tag"),
        ("MedicationRequest", "patient"),
        ("AllergyIntolerance", "patient"),
        ("EpisodeOfCare", "patient"),
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

async fn server(namespace: &str) -> FhirServer {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: namespace.into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let server = FhirServer::new(
        SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry()),
        "http://test/fhir",
        Arc::new(Unrestricted),
    )
    .with_dispensing(Dispensing {
        eligibility: Eligibility::new([("pepfar", "hiv")]),
        rule: BatchRule { buffer_days: 30 },
    });
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

const DISPENSE: &str = "0190f1a2-7c3d-7e4f-8a5b-00000000d001";

/// Receives `lines` into the pharmacy, and prescribes `medicine`: the prescription's id.
async fn stocked(server: &FhirServer, lines: Value, medicine: &str) -> String {
    let stock = server
        .inventory_router()
        .layer(subject("KAM013", &[STOCK_PERMISSION]));
    let receipt = json!({ "store": "Location/pharmacy", "supplier": "NMS", "delivery": "INV-7", "lines": lines });
    let (status, body) = call(&stock, Method::POST, "/goods-received", Some(receipt)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let consultation = server
        .consultation_router()
        .layer(subject("KAM013", &[PRESCRIBE_PERMISSION]));
    let (status, written) = call(
        &consultation,
        Method::POST,
        &format!("/encounters/{VISIT}/orders"),
        Some(prescription(301, medicine)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{written}");
    written["id"].as_str().expect("id").to_owned()
}

/// Each batch's on-hand quantity.
async fn quantities(server: &FhirServer) -> Vec<(String, i64)> {
    on_hand(server)
        .await
        .as_array()
        .into_iter()
        .flatten()
        .map(|u| {
            (
                format!(
                    "{} {}",
                    u["batch"].as_str().unwrap_or_default(),
                    u["status"].as_str().unwrap_or_default()
                ),
                u["on_hand"].as_i64().unwrap_or_default(),
            )
        })
        .collect()
}

fn line(batch: &str, expiry: &str, funding: &str, quantity: i64) -> Value {
    json!({ "product": "artemether-lumefantrine", "batch": batch, "expiry": expiry, "funding_source": funding,
            "quantity": quantity, "pack_size": 24, "unit_cost": "0.50" })
}

fn pharmacist(server: &FhirServer) -> Router {
    server
        .pharmacy_router()
        .layer(subject("KAM013", &[DISPENSE_PERMISSION]))
}

async fn dispense(server: &FhirServer) -> Value {
    let fhir = server.clone().router();
    let response = fhir
        .oneshot(
            Request::get(format!("/MedicationDispense/{DISPENSE}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("json")
}

fn prepare_body(quantity: i64) -> Value {
    json!({ "dispense": DISPENSE, "store": "Location/pharmacy", "product": "artemether-lumefantrine", "quantity": quantity })
}

#[tokio::test]
async fn stock_moves_only_when_the_dispense_is_completed() {
    let server = server("dispense-moves").await;
    let rx = stocked(
        &server,
        json!([
            line("SOON", "2027-06-30", "government", 24),
            line("LATE", "2027-12-31", "government", 240)
        ]),
        "Artemether/lumefantrine 20/120 mg",
    )
    .await;
    let before = quantities(&server).await;
    let pharmacy = pharmacist(&server);

    // Preparing reserves by first expiry, and writes the MedicationDispense; nothing moves.
    let (status, prepared) = call(
        &pharmacy,
        Method::POST,
        &format!("/prescriptions/{rx}/prepare"),
        Some(prepare_body(30)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{prepared}");
    let held: Vec<(String, i64)> = prepared["reservations"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| {
            (
                r["batch"].as_str().unwrap_or_default().to_owned(),
                r["quantity"].as_i64().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(held, [("SOON".to_owned(), 24), ("LATE".to_owned(), 6)]);
    assert_eq!(
        quantities(&server).await,
        before,
        "preparing moves no stock"
    );
    let prepared_dispense = dispense(&server).await;
    assert_eq!(prepared_dispense["status"], "preparation");
    assert_eq!(
        prepared_dispense["authorizingPrescription"][0]["reference"],
        format!("MedicationRequest/{rx}")
    );

    // Handed over in full: now, and only now, stock moves.
    let (status, done) = call(
        &pharmacy,
        Method::POST,
        &format!("/dispenses/{DISPENSE}/complete"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    let mut after = quantities(&server).await;
    after.sort();
    assert_eq!(
        after,
        [
            ("LATE usable".to_owned(), 234),
            ("SOON usable".to_owned(), 0)
        ]
    );
    let completed = dispense(&server).await;
    assert_eq!(
        (
            completed["status"].as_str(),
            completed["quantity"]["value"].as_i64()
        ),
        (Some("completed"), Some(30))
    );
    assert!(completed["whenHandedOver"].is_string());
    // Once.
    assert_eq!(
        call(
            &pharmacy,
            Method::POST,
            &format!("/dispenses/{DISPENSE}/complete"),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_partial_dispense_moves_what_was_given_and_a_cancelled_one_nothing() {
    let server = server("dispense-partial").await;
    let rx = stocked(
        &server,
        json!([line("SOON", "2027-06-30", "government", 24)]),
        "Artemether/lumefantrine 20/120 mg",
    )
    .await;
    let pharmacy = pharmacist(&server);
    let (_, prepared) = call(
        &pharmacy,
        Method::POST,
        &format!("/prescriptions/{rx}/prepare"),
        Some(prepare_body(12)),
    )
    .await;
    let reservation = prepared["reservations"][0]["id"]
        .as_str()
        .expect("id")
        .to_owned();
    let (status, done) = call(
        &pharmacy,
        Method::POST,
        &format!("/dispenses/{DISPENSE}/complete"),
        Some(json!({ "given": { reservation: 5 } })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(quantities(&server).await, [("SOON usable".to_owned(), 19)]);
    assert_eq!(dispense(&server).await["quantity"]["value"], 5);

    // Another dispense, cancelled: nothing moves, and its stock is free again.
    let other = "0190f1a2-7c3d-7e4f-8a5b-00000000d002";
    let (status, _) = call(
        &pharmacy,
        Method::POST,
        &format!("/prescriptions/{rx}/prepare"),
        Some(json!({ "dispense": other, "store": "Location/pharmacy", "product": "artemether-lumefantrine", "quantity": 19 })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, cancelled) = call(
        &pharmacy,
        Method::POST,
        &format!("/dispenses/{other}/cancel"),
        None,
    )
    .await;
    assert_eq!(
        (status, cancelled["status"].as_str()),
        (StatusCode::OK, Some("cancelled")),
        "{cancelled}"
    );
    assert_eq!(quantities(&server).await, [("SOON usable".to_owned(), 19)]);
    let (_, again) = call(
        &pharmacy,
        Method::POST,
        &format!("/prescriptions/{rx}/prepare"),
        Some(json!({ "dispense": "0190f1a2-7c3d-7e4f-8a5b-00000000d003", "store": "Location/pharmacy", "product": "artemether-lumefantrine", "quantity": 19 })),
    )
    .await;
    assert_eq!(again["reservations"][0]["quantity"], 19, "{again}");
}

#[tokio::test]
async fn donor_stock_goes_only_to_patients_in_its_program() {
    let server = server("dispense-donor").await;
    let rx = stocked(
        &server,
        json!([line("DONOR", "2027-06-30", "pepfar", 60)]),
        "Tenofovir/lamivudine/dolutegravir",
    )
    .await;
    let pharmacy = pharmacist(&server);
    let (status, refused) = call(
        &pharmacy,
        Method::POST,
        &format!("/prescriptions/{rx}/prepare"),
        Some(prepare_body(30)),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");

    // Enrolled in the HIV program, the patient receives it.
    let fhir = server.clone().router();
    let episode = json!({ "resourceType": "EpisodeOfCare", "status": "active",
        "type": [{ "coding": [{ "code": "hiv" }] }], "patient": { "reference": format!("Patient/{PATIENT}") } });
    let response = fhir
        .oneshot(
            Request::post("/EpisodeOfCare")
                .header("content-type", "application/fhir+json")
                .body(Body::from(episode.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let (status, prepared) = call(
        &pharmacy,
        Method::POST,
        &format!("/prescriptions/{rx}/prepare"),
        Some(prepare_body(30)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{prepared}");
    assert_eq!(prepared["reservations"][0]["batch"], "DONOR");
}

#[tokio::test]
async fn only_a_pharmacist_of_this_facility_dispenses() {
    let server = server("dispense-who").await;
    let rx = stocked(
        &server,
        json!([line("SOON", "2027-06-30", "government", 24)]),
        "Artemether/lumefantrine 20/120 mg",
    )
    .await;
    let viewer = server
        .pharmacy_router()
        .layer(subject("KAM013", &["pharmacy.view"]));
    assert_eq!(
        call(
            &viewer,
            Method::POST,
            &format!("/prescriptions/{rx}/prepare"),
            Some(prepare_body(1))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let elsewhere = server
        .pharmacy_router()
        .layer(subject("MBR001", &[DISPENSE_PERMISSION]));
    assert_eq!(
        call(
            &elsewhere,
            Method::POST,
            &format!("/prescriptions/{rx}/prepare"),
            Some(prepare_body(1))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let pharmacy = pharmacist(&server);
    assert_eq!(
        call(
            &pharmacy,
            Method::POST,
            &format!("/prescriptions/{rx}/prepare"),
            Some(prepare_body(1))
        )
        .await
        .0,
        StatusCode::CREATED
    );
    assert_eq!(
        call(
            &elsewhere,
            Method::POST,
            &format!("/dispenses/{DISPENSE}/complete"),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(quantities(&server).await, [("SOON usable".to_owned(), 24)]);
}
