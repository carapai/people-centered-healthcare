//! Patient requests over HTTP (#114): a patient asks for a refill, a test and an appointment from
//! the portal; **none becomes an order without staff action**. Staff accept (the refill through
//! medications and its safety checks, the test as an order, the appointment booked) or decline
//! with a reason; the patient may withdraw while pending.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{PatientSubject, Permissions, Subject};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::{
    APPOINTMENTS_BOOK_PERMISSION, CHECK_IN_PERMISSION, FhirServer, ORDER_PERMISSION,
    PRESCRIBE_PERMISSION,
};
use health_patient_engagement_app::account_person;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const MINE: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000a1";
const THEIRS: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000a2";
const VISIT: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000201";
const PRESCRIPTION: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000304";
const ACCOUNT: &str = "phone-account-1";

fn registry() -> Registry {
    let mut definitions = Vec::new();
    for (base, path) in [
        ("MedicationRequest", "subject"),
        ("ServiceRequest", "subject"),
        ("Appointment", "subject"),
        ("AllergyIntolerance", "patient"),
    ] {
        definitions.push(json!({ "resourceType": "SearchParameter", "url": format!("http://hl7.org/fhir/SearchParameter/{base}-patient"), "code": "patient",
            "base": [base], "type": "reference", "expression": format!("{base}.{path}"), "target": ["Patient"] }));
    }
    let catalog = Catalog::from_definitions(definitions.iter());
    let mut registry = Registry::new(jiff::tz::Offset::UTC);
    for base in [
        "MedicationRequest",
        "ServiceRequest",
        "Appointment",
        "AllergyIntolerance",
    ] {
        registry
            .support(&catalog, base, "patient", ParamState::Active)
            .expect("patient");
    }
    registry
}

fn staff(facility: &str, permissions: &[&str]) -> Extension<Subject> {
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

async fn put(server: &FhirServer, path: &str, body: Value) {
    let request = Request::put(path)
        .header("content-type", "application/fhir+json")
        .body(Body::from(body.to_string()))
        .expect("request");
    assert_eq!(
        server
            .clone()
            .router()
            .oneshot(request)
            .await
            .expect("response")
            .status(),
        StatusCode::CREATED,
        "{path}"
    );
}

async fn server() -> FhirServer {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "requests".into(),
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
    for patient in [MINE, THEIRS] {
        put(
            &server,
            &format!("/Patient/{patient}"),
            json!({ "resourceType": "Patient", "id": patient }),
        )
        .await;
    }
    // The phone account, linked to MINE at level3.
    let person = account_person(ACCOUNT).to_string();
    put(&server, &format!("/Person/{person}"), json!({
        "resourceType": "Person", "id": person,
        "identifier": [{ "system": "urn:health:login-account", "value": ACCOUNT }],
        "link": [{ "target": { "reference": format!("Patient/{MINE}") }, "assurance": "level3" }]
    }))
    .await;
    let reception = server
        .reception_router()
        .layer(staff("KAM013", &[CHECK_IN_PERMISSION]));
    let check_in = json!({ "id": VISIT, "patient": format!("Patient/{MINE}"), "servicePoint": "consultation" });
    assert_eq!(
        call(&reception, Method::POST, "/check-in", Some(check_in))
            .await
            .0,
        StatusCode::CREATED
    );
    let consultation = server
        .consultation_router()
        .layer(staff("KAM013", &[PRESCRIBE_PERMISSION]));
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

fn portal(server: &FhirServer) -> Router {
    server
        .patient_requests_router()
        .layer(Extension(PatientSubject {
            account: ACCOUNT.into(),
            phone: "+256700000001".into(),
        }))
}

fn id(n: u32) -> String {
    format!("0190f1a2-7c3d-7e4f-8a5b-{n:012}")
}

/// The patient's orders: MedicationRequests and ServiceRequests whose intent is `order`.
async fn orders(server: &FhirServer) -> usize {
    let mut count = 0;
    for kind in ["MedicationRequest", "ServiceRequest"] {
        let request = Request::get(format!("/{kind}?patient=Patient/{MINE}"))
            .body(Body::empty())
            .expect("request");
        let response = server
            .clone()
            .router()
            .oneshot(request)
            .await
            .expect("response");
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let bundle: Value = serde_json::from_slice(&bytes).expect("bundle");
        count += bundle["entry"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| e["resource"]["intent"] == "order")
            .count();
    }
    count
}

#[tokio::test]
async fn no_request_becomes_an_order_without_staff_action() {
    let server = server().await;
    let portal = portal(&server);
    let (status, prescriptions) = call(&portal, Method::GET, "/prescriptions", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(prescriptions[0]["id"], PRESCRIPTION);

    let refill = json!({ "kind": "refill", "id": id(501), "patient": format!("Patient/{MINE}"), "prior": format!("MedicationRequest/{PRESCRIPTION}") });
    let (status, asked) = call(&portal, Method::POST, "/requests", Some(refill)).await;
    assert_eq!(
        (status, asked["state"].as_str()),
        (StatusCode::CREATED, Some("pending")),
        "{asked}"
    );
    let care = json!({ "kind": "care", "id": id(502), "patient": format!("Patient/{MINE}"), "facility": "KAM013", "what": "Blood sugar test" });
    assert_eq!(
        call(&portal, Method::POST, "/requests", Some(care)).await.0,
        StatusCode::CREATED
    );
    let visit = json!({ "kind": "appointment", "id": id(503), "patient": format!("Patient/{MINE}"), "facility": "KAM013", "reason": "Blood pressure review", "preferred": "2026-11-20" });
    assert_eq!(
        call(&portal, Method::POST, "/requests", Some(visit))
            .await
            .0,
        StatusCode::CREATED
    );
    assert_eq!(
        orders(&server).await,
        1,
        "only the original prescription: no request is an order yet"
    );

    // Staff accept the refill: a prescription based on the proposal; then the test, then a booking.
    let requests = server.requests_router().layer(staff(
        "KAM013",
        &[
            PRESCRIBE_PERMISSION,
            ORDER_PERMISSION,
            APPOINTMENTS_BOOK_PERMISSION,
        ],
    ));
    let (status, accepted) = call(
        &requests,
        Method::POST,
        &format!("/MedicationRequest/{}/accept", id(501)),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{accepted}");
    assert_eq!(accepted["state"], "accepted");
    let order = accepted["order"].as_str().expect("order").to_owned();
    let response = server
        .clone()
        .router()
        .oneshot(
            Request::get(format!("/{order}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let prescribed: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(
        (
            prescribed["intent"].as_str(),
            prescribed["basedOn"][0]["reference"].as_str()
        ),
        (
            Some("order"),
            Some(format!("MedicationRequest/{}", id(501)).as_str())
        )
    );
    assert_eq!(
        call(
            &requests,
            Method::POST,
            &format!("/MedicationRequest/{}/accept", id(501)),
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "accepted once"
    );

    assert_eq!(
        call(
            &requests,
            Method::POST,
            &format!("/ServiceRequest/{}/accept", id(502)),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        orders(&server).await,
        3,
        "two orders, each by a member of staff"
    );
    assert_eq!(
        call(
            &requests,
            Method::POST,
            &format!("/Appointment/{}/accept", id(503)),
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a booking needs its time"
    );
    let (status, booked) = call(
        &requests,
        Method::POST,
        &format!("/Appointment/{}/accept", id(503)),
        Some(json!({ "start": "2026-11-20T09:00:00Z", "end": "2026-11-20T09:20:00Z" })),
    )
    .await;
    assert_eq!(
        (status, booked["state"].as_str(), booked["start"].as_str()),
        (
            StatusCode::OK,
            Some("accepted"),
            Some("2026-11-20T09:00:00Z")
        ),
        "{booked}"
    );

    let (_, mine) = call(&portal, Method::GET, "/requests", None).await;
    let states: Vec<&str> = mine
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["state"].as_str())
        .collect();
    assert_eq!(states, ["accepted", "accepted", "accepted"], "{mine}");
}

#[tokio::test]
async fn requests_are_the_patients_own_and_declined_or_withdrawn_while_pending() {
    let server = server().await;
    let portal = portal(&server);
    let theirs = json!({ "kind": "care", "id": id(601), "patient": format!("Patient/{THEIRS}"), "facility": "KAM013", "what": "X-ray" });
    assert_eq!(
        call(&portal, Method::POST, "/requests", Some(theirs))
            .await
            .0,
        StatusCode::FORBIDDEN,
        "not linked"
    );
    let blank = json!({ "kind": "care", "id": id(602), "patient": format!("Patient/{MINE}"), "facility": "KAM013", "what": " " });
    assert_eq!(
        call(&portal, Method::POST, "/requests", Some(blank))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let bogus = json!({ "kind": "refill", "id": id(603), "patient": format!("Patient/{MINE}"), "prior": format!("MedicationRequest/{}", id(999)) });
    assert_eq!(
        call(&portal, Method::POST, "/requests", Some(bogus))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let care = |n: u32| json!({ "kind": "care", "id": id(n), "patient": format!("Patient/{MINE}"), "facility": "KAM013", "what": "Blood sugar test" });
    assert_eq!(
        call(&portal, Method::POST, "/requests", Some(care(604)))
            .await
            .0,
        StatusCode::CREATED
    );
    assert_eq!(
        call(&portal, Method::POST, "/requests", Some(care(605)))
            .await
            .0,
        StatusCode::CREATED
    );

    let pharmacist = server
        .requests_router()
        .layer(staff("KAM013", &[PRESCRIBE_PERMISSION]));
    assert_eq!(
        call(
            &pharmacist,
            Method::POST,
            &format!("/ServiceRequest/{}/accept", id(604)),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN,
        "orders need orders.request"
    );
    let elsewhere = server
        .requests_router()
        .layer(staff("MBR001", &[ORDER_PERMISSION]));
    assert_eq!(
        call(
            &elsewhere,
            Method::POST,
            &format!("/ServiceRequest/{}/accept", id(604)),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let staff_here = server
        .requests_router()
        .layer(staff("KAM013", &[ORDER_PERMISSION]));
    assert_eq!(
        call(
            &staff_here,
            Method::POST,
            &format!("/ServiceRequest/{}/decline", id(604)),
            Some(json!({ "reason": " " }))
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, declined) = call(
        &staff_here,
        Method::POST,
        &format!("/ServiceRequest/{}/decline", id(604)),
        Some(json!({ "reason": "Tested last week" })),
    )
    .await;
    assert_eq!(
        (
            status,
            declined["state"].as_str(),
            declined["reason"].as_str()
        ),
        (StatusCode::OK, Some("closed"), Some("Tested last week"))
    );
    assert_eq!(
        call(
            &portal,
            Method::POST,
            &format!("/requests/ServiceRequest/{}/withdraw", id(604)),
            None
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "no longer pending"
    );
    let (status, withdrawn) = call(
        &portal,
        Method::POST,
        &format!("/requests/ServiceRequest/{}/withdraw", id(605)),
        None,
    )
    .await;
    assert_eq!(
        (status, withdrawn["state"].as_str()),
        (StatusCode::OK, Some("closed"))
    );
    assert_eq!(orders(&server).await, 1, "nothing ordered");
}
