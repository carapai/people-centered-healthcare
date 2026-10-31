//! A facility-scope record names its facility (ADR 0057): the server stamps the session's
//! facility on create, refuses another facility's tag, keeps it through updates, and searches
//! stay within the facility, except central's patient-confined shared reads (ADR 0056).

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{FACILITY_TAG_SYSTEM, NodeScope, Pdp, Permissions, Subject};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::{FacilityTag, FhirServer};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const BASE: &str = "http://test/fhir";

fn registry() -> Registry {
    let definitions = [
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/Resource-tag", "code": "_tag", "base": ["Resource"], "type": "token", "expression": "Resource.meta.tag" }),
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/clinical-patient", "code": "patient", "base": ["Observation"], "type": "reference", "expression": "Observation.subject.where(resolve() is Patient)", "target": ["Patient"] }),
    ];
    let catalog = Catalog::from_definitions(definitions.iter());
    let mut registry = Registry::new(jiff::tz::Offset::UTC);
    for code in ["_tag", "patient"] {
        registry
            .support(&catalog, "Observation", code, ParamState::Active)
            .expect(code);
    }
    registry
}

async fn store() -> SurrealStore {
    let settings = Settings {
        target: Target::Memory,
        namespace: "facility".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry())
}

fn clinician(facility: &str) -> Subject {
    Subject {
        account: format!("clinician-{facility}"),
        facility: facility.into(),
        permissions: Permissions::from(["Observation.cruds", "Patient.crs"]),
        programs: BTreeSet::new(),
        purpose: Some("TREAT".into()),
        break_glass: None,
    }
}

fn app(store: &SurrealStore, scope: NodeScope, subject: &Subject) -> Router {
    FhirServer::new(store.clone(), BASE, Arc::new(Unrestricted))
        .with_access_control(Pdp::default().with_scope(scope), Arc::new(FacilityTag))
        .router()
        .layer(Extension(subject.clone()))
}

async fn send(
    app: &Router,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if body.is_some() {
        request = request.header("content-type", "application/fhir+json");
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

fn facility_of(resource: &Value) -> Option<String> {
    resource["meta"]["tag"].as_array().and_then(|tags| {
        tags.iter()
            .find(|t| t["system"] == FACILITY_TAG_SYSTEM)
            .and_then(|t| t["code"].as_str().map(str::to_owned))
    })
}

fn observation(patient: &str, note: &str) -> Value {
    json!({
        "resourceType": "Observation",
        "status": "final",
        "code": { "text": "weight" },
        "subject": { "reference": format!("Patient/{patient}") },
        "note": [{ "text": note }],
    })
}

async fn notes(app: &Router, query: &str) -> BTreeSet<String> {
    let (status, bundle) = send(app, Method::GET, &format!("/Observation{query}"), None).await;
    assert_eq!(status, StatusCode::OK, "{bundle}");
    bundle["entry"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|e| e["resource"]["note"][0]["text"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_server_stamps_and_keeps_the_records_facility() {
    let store = store().await;
    let f1 = app(&store, NodeScope::Edge, &clinician("F1"));
    let (_, patient) = send(
        &f1,
        Method::POST,
        "/Patient",
        Some(json!({ "resourceType": "Patient" })),
    )
    .await;
    let patient = patient["id"].as_str().expect("id").to_owned();
    // Created without a facility: the session's is stamped.
    let (status, created) = send(
        &f1,
        Method::POST,
        "/Observation",
        Some(observation(&patient, "a")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(facility_of(&created).as_deref(), Some("F1"));
    // Global records carry none.
    let (_, stored) = send(&f1, Method::GET, &format!("/Patient/{patient}"), None).await;
    assert_eq!(facility_of(&stored), None);
    // A tag naming another facility is refused.
    let mut foreign = observation(&patient, "b");
    foreign["meta"] = json!({ "tag": [{ "system": FACILITY_TAG_SYSTEM, "code": "F2" }] });
    let (status, _) = send(&f1, Method::POST, "/Observation", Some(foreign)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // An update that leaves the tag out keeps the facility.
    let id = created["id"].as_str().expect("id").to_owned();
    let mut changed = observation(&patient, "a2");
    changed["id"] = json!(id);
    let request = Request::put(format!("/Observation/{id}"))
        .header("content-type", "application/fhir+json")
        .header(
            "if-match",
            format!(
                "W/\"{}\"",
                created["meta"]["versionId"].as_str().expect("version")
            ),
        )
        .body(Body::from(changed.to_string()))
        .expect("request");
    let response = f1.clone().oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let (_, read) = send(&f1, Method::GET, &format!("/Observation/{id}"), None).await;
    assert_eq!(read["note"][0]["text"], "a2");
    assert_eq!(facility_of(&read).as_deref(), Some("F1"));
    // Another facility never writes it.
    let f2 = app(&store, NodeScope::Central, &clinician("F2"));
    let (status, _) = send(&f2, Method::DELETE, &format!("/Observation/{id}"), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn searches_stay_in_the_facility_except_patient_confined_ones_at_central() {
    let store = store().await;
    let f1 = app(&store, NodeScope::Edge, &clinician("F1"));
    let f2_edge = app(&store, NodeScope::Edge, &clinician("F2"));
    let (_, patient) = send(
        &f1,
        Method::POST,
        "/Patient",
        Some(json!({ "resourceType": "Patient" })),
    )
    .await;
    let patient = patient["id"].as_str().expect("id").to_owned();
    for (app, note) in [(&f1, "at-f1"), (&f2_edge, "at-f2")] {
        let (status, body) = send(
            app,
            Method::POST,
            "/Observation",
            Some(observation(&patient, note)),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    let set = |names: &[&str]| {
        names
            .iter()
            .map(|n| (*n).to_owned())
            .collect::<BTreeSet<_>>()
    };
    // Each facility searches its own records.
    assert_eq!(notes(&f1, "").await, set(&["at-f1"]));
    assert_eq!(notes(&f2_edge, "").await, set(&["at-f2"]));
    assert_eq!(
        notes(&f2_edge, &format!("?patient={patient}")).await,
        set(&["at-f2"])
    );
    // At central, a search confined to the patient reads the shared record; one across
    // patients does not.
    let f2_central = app(&store, NodeScope::Central, &clinician("F2"));
    assert_eq!(
        notes(&f2_central, &format!("?patient={patient}")).await,
        set(&["at-f1", "at-f2"])
    );
    assert_eq!(notes(&f2_central, "").await, set(&["at-f2"]));
}
