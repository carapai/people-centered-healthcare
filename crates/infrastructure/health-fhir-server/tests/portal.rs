//! Self-registration over HTTP against the real client registry (#111): a new patient is a
//! provisional, patient-reported source Patient that is never auto-linked, and a phone account
//! resembling an existing Patient goes to review with nothing linked.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use std::collections::BTreeSet;

use health_access_policy::{PatientSubject, Permissions, Subject};
use health_audit::{Agent, Auditor};
use health_client_registry::matching::MatchingRules;
use health_fhir_search::{Registry, Unrestricted};
use health_fhir_server::{FhirServer, REVIEW_PERMISSION};
use health_registries_app::fhir::{ECID_SYSTEM, RegistryConfig};
use health_registries_app::matching::RegistryBlocking;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const NIN: &str = "urn:health:synthetic-data/nin";
const BASE: &str = "http://test/fhir";
const PHONE: &str = "+256772123456";

fn config() -> RegistryConfig {
    RegistryConfig {
        nin_system: NIN.into(),
    }
}

fn rules() -> MatchingRules {
    MatchingRules::platform_default(NIN, &[NIN, ECID_SYSTEM])
}

async fn store() -> SurrealStore {
    let settings = Settings {
        target: Target::Memory,
        namespace: "portal-http".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
        .with_registry(Registry::new(jiff::tz::Offset::UTC))
        .with_blocking(Arc::new(RegistryBlocking::new(rules(), config())))
}

fn server(store: &SurrealStore) -> FhirServer {
    FhirServer::new(store.clone(), BASE, Arc::new(Unrestricted))
        .with_audit(Auditor::new("test central", Vec::new()))
        .with_client_registry(config(), rules())
}

/// The portal as `patient_layer` serves it: the patient is a request extension.
fn portal(store: &SurrealStore, account: Option<&str>) -> Router {
    let router = server(store).portal_router();
    match account {
        Some(account) => router.layer(Extension(PatientSubject {
            account: account.into(),
            phone: PHONE.into(),
        })),
        None => router,
    }
}

/// The review queue as a registry reviewer's request reaches it.
fn reviews(store: &SurrealStore) -> Router {
    server(store)
        .registry_review_router()
        .layer(Extension(Agent {
            display: "account clerk-1".into(),
            ..Agent::unauthenticated()
        }))
        .layer(Extension(Subject {
            account: "clerk-1".into(),
            facility: "KLA01".into(),
            permissions: Permissions(
                ["Patient.crus", REVIEW_PERMISSION]
                    .iter()
                    .map(|p| (*p).to_owned())
                    .collect(),
            ),
            programs: BTreeSet::new(),
            purpose: Some("TREAT".into()),
            break_glass: None,
        }))
}

/// An existing Patient like the one the tests register, and its id.
async fn existing(store: &SurrealStore) -> String {
    let (status, existing) = send(
        &server(store).router(),
        "POST",
        "/Patient",
        Some(json!({
            "resourceType": "Patient",
            "name": [{ "family": "Nakato", "given": ["Grace"] }],
            "birthDate": "1990-04-12",
            "gender": "female",
            "telecom": [{ "system": "phone", "value": PHONE }],
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{existing}");
    existing["id"].as_str().expect("id").to_owned()
}

async fn send(app: &Router, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if body.is_some() {
        request = request.header("content-type", "application/fhir+json");
    }
    let body = body.map_or(Body::empty(), |b| Body::from(b.to_string()));
    let response = app
        .clone()
        .oneshot(request.body(body).expect("request"))
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

fn response(given: &str, family: &str) -> Value {
    let text =
        |link: &str, value: &str| json!({ "linkId": link, "answer": [{ "valueString": value }] });
    json!({
        "resourceType": "QuestionnaireResponse",
        "questionnaire": health_patient_engagement_app::QUESTIONNAIRE,
        "status": "completed",
        "item": [
            text("given", given),
            text("family", family),
            { "linkId": "birthDate", "answer": [{ "valueDate": "1990-04-12" }] },
            { "linkId": "gender", "answer": [{ "valueCoding": { "code": "female" } }] },
        ],
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_patient_is_self_registered_and_sees_only_what_they_submitted() {
    let store = store().await;
    let app = portal(&store, Some("acct-1"));
    let (status, _) = send(&app, "GET", "/registration", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, body) = send(
        &app,
        "POST",
        "/registration",
        Some(response("Grace", "Nakato")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["status"], "completed");
    assert_eq!(body["access"], "self-submitted");
    let id = body["patient"].as_str().expect("patient").to_owned();

    let admin = server(&store).router();
    let (status, patient) = send(&admin, "GET", &format!("/Patient/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    let tags: Vec<&str> = patient["meta"]["tag"]
        .as_array()
        .expect("tags")
        .iter()
        .filter_map(|t| t["code"].as_str())
        .collect();
    assert!(
        tags.contains(&"provisional") && tags.contains(&"patient-reported"),
        "{tags:?}"
    );
    assert!(patient["link"].is_null(), "never auto-linked: {patient}");
    assert!(patient["identifier"].is_null(), "no MRN");
    assert_eq!(patient["telecom"][0]["value"], PHONE);

    let (status, again) = send(&app, "GET", "/registration", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again, body);
    let (status, _) = send(
        &app,
        "POST",
        "/registration",
        Some(response("Grace", "Nakato")),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_phone_account_resembling_an_existing_patient_goes_to_review() {
    let store = store().await;
    let admin = server(&store).router();
    existing(&store).await;

    let app = portal(&store, Some("acct-2"));
    let (status, body) = send(
        &app,
        "POST",
        "/registration",
        Some(response("Grace", "Nakato")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["status"], "review");
    assert_eq!(body["access"], "none");
    assert!(body["patient"].is_null());

    let (_, found) = send(&admin, "GET", "/Patient?_count=10", None).await;
    assert_eq!(
        found["entry"].as_array().map(Vec::len),
        Some(1),
        "no new Patient"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_portal_needs_a_patient_and_a_valid_response() {
    let store = store().await;
    let (status, _) = send(
        &portal(&store, None),
        "POST",
        "/registration",
        Some(response("A", "B")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let mut bad = response("Grace", "Nakato");
    bad["item"].as_array_mut().expect("items").remove(0);
    let (status, outcome) = send(
        &portal(&store, Some("acct-3")),
        "POST",
        "/registration",
        Some(bad),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(outcome["resourceType"], "OperationOutcome");
}

/// Submits a lookalike registration and returns its queue item.
async fn queued(store: &SurrealStore, account: &str) -> (String, Value) {
    let id = existing(store).await;
    let (_, body) = send(
        &portal(store, Some(account)),
        "POST",
        "/registration",
        Some(response("Grace", "Nakato")),
    )
    .await;
    assert_eq!(body["status"], "review");
    let (status, queue) = send(&reviews(store), "GET", "/", None).await;
    assert_eq!(status, StatusCode::OK);
    let item = queue
        .as_array()
        .expect("items")
        .iter()
        .find(|i| i["kind"] == "registration-request")
        .cloned()
        .expect("queued");
    (id, item)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reviewer_links_the_account_after_checking_its_holder_in_person() {
    let store = store().await;
    let (existing_id, item) = queued(&store, "acct-4").await;
    assert_eq!(item["candidates"][0]["id"], existing_id.as_str());
    assert_eq!(item["response"]["resourceType"], "QuestionnaireResponse");
    let path = format!("/{}", item["id"].as_str().expect("id"));

    let (status, _) = send(
        &reviews(&store),
        "POST",
        &path,
        Some(json!({ "decision": "link-patient", "patient": existing_id })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "no in-person check"
    );
    let (status, _) = send(
        &reviews(&store),
        "POST",
        &path,
        Some(json!({ "decision": "same-person" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, body) = send(
        &reviews(&store),
        "POST",
        &path,
        Some(json!({ "decision": "link-patient", "patient": format!("Patient/{existing_id}"), "document": "national-id" })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (_, mine) = send(
        &portal(&store, Some("acct-4")),
        "GET",
        "/registration",
        None,
    )
    .await;
    assert_eq!(mine["status"], "completed");
    assert_eq!(mine["patient"], existing_id.as_str());
    assert_eq!(mine["access"], "records");

    let (_, queue) = send(&reviews(&store), "GET", "/", None).await;
    assert_eq!(queue, json!([]), "decided");
    let (status, _) = send(
        &reviews(&store),
        "POST",
        &path,
        Some(json!({ "decision": "create-patient" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reviewer_may_create_a_new_patient_for_the_account() {
    let store = store().await;
    let (existing_id, item) = queued(&store, "acct-5").await;
    let path = format!("/{}", item["id"].as_str().expect("id"));
    let (status, _) = send(
        &reviews(&store),
        "POST",
        &path,
        Some(json!({ "decision": "create-patient" })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, mine) = send(
        &portal(&store, Some("acct-5")),
        "GET",
        "/registration",
        None,
    )
    .await;
    assert_eq!(mine["status"], "completed");
    assert_eq!(mine["access"], "self-submitted");
    assert_ne!(mine["patient"], existing_id.as_str());
}
