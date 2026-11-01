//! Self-registration over HTTP against the real client registry (#111): a new patient is a
//! provisional, patient-reported source Patient that is never auto-linked, and a phone account
//! resembling an existing Patient goes to review with nothing linked.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::PatientSubject;
use health_audit::Auditor;
use health_client_registry::matching::MatchingRules;
use health_fhir_search::{Registry, Unrestricted};
use health_fhir_server::FhirServer;
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
    let (status, existing) = send(
        &admin,
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
