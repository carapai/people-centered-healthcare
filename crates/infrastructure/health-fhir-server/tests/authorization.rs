//! The authorization matrix through the HTTP API (#192): every interaction is decided by the
//! policy decision point for the request's subject, over SurrealDB (memory engine). Permissions,
//! facility isolation, sensitivity labels, purpose of use and break-glass.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use health_access_policy::{
    BTG, BreakGlassPolicy, FACILITY_TAG_SYSTEM, Label, LabelRule, NodeScope, Pdp, Permissions,
    Subject,
};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::{FacilityTag, FhirServer};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const ACT: &str = "http://terminology.hl7.org/CodeSystem/v3-ActCode";
const BASE: &str = "http://test/fhir";
fn registry() -> Registry {
    // The R5 definitions of the parameters this test uses.
    let definitions = [
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/Resource-id", "code": "_id", "base": ["Resource"], "type": "token", "expression": "Resource.id" }),
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/Resource-tag", "code": "_tag", "base": ["Resource"], "type": "token", "expression": "Resource.meta.tag" }),
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/Resource-security", "code": "_security", "base": ["Resource"], "type": "token", "expression": "Resource.meta.security" }),
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/clinical-patient", "code": "patient", "base": ["Observation"], "type": "reference", "expression": "Observation.subject.where(resolve() is Patient)", "target": ["Patient"] }),
    ];
    let catalog = Catalog::from_definitions(definitions.iter());
    let mut registry = Registry::new(jiff::tz::Offset::UTC);
    for code in ["_id", "_security", "_tag", "patient"] {
        registry
            .support(&catalog, "Observation", code, ParamState::Active)
            .expect(code);
    }
    registry
        .support(&catalog, "Patient", "_id", ParamState::Active)
        .expect("_id");
    registry
}

async fn store() -> SurrealStore {
    let settings = Settings {
        target: Target::Memory,
        namespace: "pdp".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry())
}

fn pdp() -> Pdp {
    Pdp::new(vec![
        LabelRule {
            label: Label::new(ACT, "HIV"),
            program: "hiv".into(),
        },
        LabelRule {
            label: Label::new(ACT, "PSY"),
            program: "mental-health".into(),
        },
    ])
}

fn subject(permissions: Permissions, programs: &[&str]) -> Subject {
    Subject {
        account: "u1".into(),
        facility: "F1".into(),
        permissions,
        programs: programs.iter().map(|p| (*p).to_owned()).collect(),
        purpose: Some("TREAT".into()),
        break_glass: None,
    }
}

/// One request as `subject` (none: unauthenticated).
async fn send(
    router: &Router,
    subject: Option<&Subject>,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if body.is_some() {
        request = request.header("content-type", "application/fhir+json");
    }
    if let Some(subject) = subject {
        request = request.extension(subject.clone());
    }
    let request = request
        .body(body.map_or(Body::empty(), |b| Body::from(b.to_string())))
        .expect("request");
    let response = router.clone().oneshot(request).await.expect("response");
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

/// The `note` names of a Bundle's resources.
fn names(bundle: &Value) -> BTreeSet<String> {
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

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

struct Fixture {
    router: Router,
    p1: String,
    /// Observation ids by name.
    ids: std::collections::BTreeMap<String, String>,
}

async fn fixture() -> Fixture {
    fixture_at(NodeScope::Edge).await
}

async fn fixture_at(scope: NodeScope) -> Fixture {
    let store = store().await;
    let admin = FhirServer::new(store.clone(), BASE, Arc::new(Unrestricted)).router();
    let mut patients = Vec::new();
    for _ in 0..2 {
        let (_, p) = send(
            &admin,
            None,
            Method::POST,
            "/Patient",
            Some(json!({ "resourceType": "Patient" })),
        )
        .await;
        patients.push(p["id"].as_str().expect("id").to_owned());
    }
    let (p1, p2) = (patients[0].clone(), patients[1].clone());
    let mut ids = std::collections::BTreeMap::new();
    for (name, patient, labels, facility) in [
        ("plain", &p1, json!([]), None),
        ("hiv", &p1, json!([{ "system": ACT, "code": "HIV" }]), None),
        ("psy", &p1, json!([{ "system": ACT, "code": "PSY" }]), None),
        (
            "normal-conf",
            &p1,
            json!([{ "system": "http://terminology.hl7.org/CodeSystem/v3-Confidentiality", "code": "N" }]),
            None,
        ),
        (
            "hiv-other-patient",
            &p2,
            json!([{ "system": ACT, "code": "HIV" }]),
            None,
        ),
        ("at-f1", &p1, json!([]), Some("F1")),
        ("at-f2", &p1, json!([]), Some("F2")),
    ] {
        let mut observation = json!({
            "resourceType": "Observation",
            "status": "final",
            "code": { "text": "test" },
            "subject": { "reference": format!("Patient/{patient}") },
            "note": [{ "text": name }],
        });
        if labels.as_array().is_some_and(|l| !l.is_empty()) {
            observation["meta"] = json!({ "security": labels });
        }
        // Observations are facility-scope (ADR 0057): F1's unless the row says otherwise.
        observation["meta"]["tag"] =
            json!([{ "system": FACILITY_TAG_SYSTEM, "code": facility.unwrap_or("F1") }]);
        let (status, body) = send(
            &admin,
            None,
            Method::POST,
            "/Observation",
            Some(observation),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        ids.insert(name.to_owned(), body["id"].as_str().expect("id").to_owned());
    }
    let router = FhirServer::new(store, BASE, Arc::new(Unrestricted))
        .with_access_control(pdp().with_scope(scope), Arc::new(FacilityTag))
        .router();
    Fixture { router, p1, ids }
}

impl Fixture {
    async fn search(&self, subject: &Subject, query: &str) -> (StatusCode, BTreeSet<String>) {
        let (status, bundle) = send(
            &self.router,
            Some(subject),
            Method::GET,
            &format!("/Observation{query}"),
            None,
        )
        .await;
        (status, names(&bundle))
    }

    async fn read(&self, subject: &Subject, name: &str) -> StatusCode {
        send(
            &self.router,
            Some(subject),
            Method::GET,
            &format!("/Observation/{}", self.ids[name]),
            None,
        )
        .await
        .0
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_route_serves_fhir_data_without_a_subject() {
    let f = fixture().await;
    for path in [
        format!("/Observation/{}", f.ids["plain"]),
        "/Observation".to_owned(),
        "/_history".to_owned(),
        format!("/Patient/{}/_history", f.p1),
    ] {
        let (status, outcome) = send(&f.router, None, Method::GET, &path, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
        assert_eq!(outcome["issue"][0]["code"], "login");
    }
    let (status, _) = send(
        &f.router,
        None,
        Method::POST,
        "/",
        Some(json!({ "resourceType": "Bundle", "type": "batch", "entry": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // The CapabilityStatement stays public.
    let (status, _) = send(&f.router, None, Method::GET, "/metadata", None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn labels_and_purpose_of_use_restrict_searches_and_reads() {
    let f = fixture().await;
    // F2's record stays out of F1's searches (ADR 0057).
    let everything = set(&["plain", "normal-conf", "at-f1"]);

    // A clinician without program assignments sees no labelled record, in any search.
    let clinician = subject(Permissions::from(["*.rs"]), &[]);
    assert_eq!(
        f.search(&clinician, "").await,
        (StatusCode::OK, everything.clone())
    );
    assert_eq!(
        f.search(&clinician, &format!("?patient={}", f.p1)).await.1,
        everything
    );
    // Asking for the label explicitly does not get round the filter.
    assert_eq!(
        f.search(&clinician, &format!("?_security={ACT}|HIV"))
            .await
            .1,
        set(&[])
    );
    // A denied read is a 404: the record's existence is not disclosed.
    assert_eq!(f.read(&clinician, "hiv").await, StatusCode::NOT_FOUND);
    assert_eq!(f.read(&clinician, "plain").await, StatusCode::OK);

    // An HIV counsellor sees HIV records too, but not mental-health ones.
    let counsellor = subject(Permissions::from(["Observation.rs"]), &["hiv"]);
    let (_, seen) = f.search(&counsellor, "").await;
    assert!(seen.contains("hiv") && seen.contains("hiv-other-patient") && !seen.contains("psy"));
    assert_eq!(f.read(&counsellor, "hiv").await, StatusCode::OK);
    assert_eq!(f.read(&counsellor, "psy").await, StatusCode::NOT_FOUND);

    // Without a declared purpose of use, the program assignment does not open them.
    let undeclared = Subject {
        purpose: None,
        ..counsellor
    };
    assert!(!f.search(&undeclared, "").await.1.contains("hiv"));
    assert_eq!(f.read(&undeclared, "hiv").await, StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permissions_gate_every_interaction() {
    let f = fixture().await;
    // No search permission: 403, before anything is searched.
    let cashier = subject(Permissions::from(["Invoice.rs"]), &[]);
    let (status, _) = f.search(&cashier, "").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(f.read(&cashier, "plain").await, StatusCode::NOT_FOUND);
    let (status, _) = send(
        &f.router,
        Some(&cashier),
        Method::GET,
        "/Observation/_history",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Read-only: creates, updates and deletes are 403.
    let reader = subject(Permissions::from(["*.rs"]), &[]);
    let observation = json!({
        "resourceType": "Observation",
        "status": "final",
        "code": { "text": "new" },
        "subject": { "reference": format!("Patient/{}", f.p1) },
    });
    let (status, _) = send(
        &f.router,
        Some(&reader),
        Method::POST,
        "/Observation",
        Some(observation.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(
        &f.router,
        Some(&reader),
        Method::DELETE,
        &format!("/Observation/{}", f.ids["plain"]),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // A transaction with one forbidden entry writes nothing.
    let (status, _) = send(
        &f.router,
        Some(&reader),
        Method::POST,
        "/",
        Some(json!({ "resourceType": "Bundle", "type": "transaction", "entry": [
            { "fullUrl": "urn:uuid:5d9f1a52-6a4b-4c38-9f7e-7a1d4e0b2c11", "resource": observation, "request": { "method": "POST", "url": "Observation" } },
        ] })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A writer may create, and labelled records it cannot open stay closed to it.
    let writer = subject(Permissions::from(["Observation.cruds"]), &[]);
    let (status, body) = send(
        &f.router,
        Some(&writer),
        Method::POST,
        "/Observation",
        Some(observation),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, _) = send(
        &f.router,
        Some(&writer),
        Method::DELETE,
        &format!("/Observation/{}", f.ids["hiv"]),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn facility_scope_records_are_used_at_their_facility_only() {
    let f = fixture().await;
    let clinician = subject(Permissions::from(["Observation.cruds", "Patient.rs"]), &[]);
    assert_eq!(f.read(&clinician, "at-f1").await, StatusCode::OK);
    assert_eq!(f.read(&clinician, "at-f2").await, StatusCode::NOT_FOUND);
    // Another facility's record is never written here: its authority stays there.
    let (_, head) = send(
        &f.router,
        Some(&Subject {
            facility: "F2".into(),
            ..clinician.clone()
        }),
        Method::GET,
        &format!("/Observation/{}", f.ids["at-f2"]),
        None,
    )
    .await;
    assert_eq!(head["note"][0]["text"], "at-f2");
    let (status, _) = send(
        &f.router,
        Some(&clinician),
        Method::DELETE,
        &format!("/Observation/{}", f.ids["at-f2"]),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // History: the record's own history is a 404, and type history leaves it out.
    let (status, _) = send(
        &f.router,
        Some(&clinician),
        Method::GET,
        &format!("/Observation/{}/_history", f.ids["at-f2"]),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, history) = send(
        &f.router,
        Some(&clinician),
        Method::GET,
        "/Observation/_history",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let seen = names(&history);
    assert!(seen.contains("at-f1") && !seen.contains("at-f2") && !seen.contains("hiv"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn break_glass_opens_one_patient_only() {
    let f = fixture().await;
    let mut glass = subject(Permissions::from(["*.rs", "break-glass"]), &[]);
    let p1 = format!("Patient/{}", f.p1);
    glass.break_glass = Some(
        BreakGlassPolicy::default()
            .grant(&glass, &p1, BTG, "emergency", Timestamp::now())
            .expect("grant")
            .0,
    );
    // Reads of that patient's sensitive and other-facility records open.
    assert_eq!(f.read(&glass, "hiv").await, StatusCode::OK);
    assert_eq!(f.read(&glass, "psy").await, StatusCode::OK);
    assert_eq!(f.read(&glass, "at-f2").await, StatusCode::OK);
    // Not another patient's.
    assert_eq!(
        f.read(&glass, "hiv-other-patient").await,
        StatusCode::NOT_FOUND
    );
    // A search confined to the patient sees everything of theirs; one across patients does not
    // rely on break-glass.
    let (_, confined) = f.search(&glass, &format!("?patient={}", f.p1)).await;
    assert_eq!(
        confined,
        set(&["plain", "hiv", "psy", "normal-conf", "at-f1", "at-f2"])
    );
    let (_, across) = f.search(&glass, "").await;
    assert!(!across.contains("hiv"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn central_shares_patient_confined_reads_across_facilities() {
    // ADR 0056: at central, another facility's record about a patient may be read, but it is
    // never listed across patients or written.
    let f = fixture_at(NodeScope::Central).await;
    let clinician = subject(Permissions::from(["Observation.cruds"]), &[]);
    assert_eq!(f.read(&clinician, "at-f2").await, StatusCode::OK);
    let (status, _) = send(
        &f.router,
        Some(&clinician),
        Method::GET,
        &format!("/Observation/{}/_history", f.ids["at-f2"]),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &f.router,
        Some(&clinician),
        Method::DELETE,
        &format!("/Observation/{}", f.ids["at-f2"]),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (_, history) = send(
        &f.router,
        Some(&clinician),
        Method::GET,
        "/Observation/_history",
        None,
    )
    .await;
    let seen = names(&history);
    assert!(seen.contains("at-f1") && !seen.contains("at-f2"));
    // Labels still apply to shared reads.
    assert_eq!(f.read(&clinician, "hiv").await, StatusCode::NOT_FOUND);
}
