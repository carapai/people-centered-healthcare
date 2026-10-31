//! The client registry's reviewer operations over HTTP (#197): `$merge` with preview and
//! approval, `$link` and `$unlink`, each version audited naming the reviewer, and survivorship
//! disagreements queued for review.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{Pdp, Permissions, Subject};
use health_audit::{Agent, Auditor};
use health_client_registry::ReviewQueue;
use health_client_registry::matching::MatchingRules;
use health_fhir_search::{Registry, Unrestricted};
use health_fhir_server::{FhirServer, NoFacilityScope, REVIEW_PERMISSION};
use health_persistence::Store;
use health_registries_app::fhir::{ECID_SYSTEM, RegistryConfig};
use health_registries_app::matching::RegistryBlocking;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const NIN: &str = "urn:health:synthetic-data/nin";
const BASE: &str = "http://test/fhir";

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
        namespace: "registry-http".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
        .with_registry(Registry::new(jiff::tz::Offset::UTC))
        // Source Patients carry their blocking keys, as at central.
        .with_blocking(Arc::new(RegistryBlocking::new(rules(), config())))
}

fn subject(account: &str, facility: &str, permissions: &[&str]) -> Subject {
    Subject {
        account: account.into(),
        facility: facility.into(),
        permissions: Permissions(permissions.iter().map(|p| (*p).to_owned()).collect()),
        programs: BTreeSet::new(),
        purpose: Some("TREAT".into()),
        break_glass: None,
    }
}

fn reviewer(facility: &str) -> Subject {
    subject("reviewer-1", facility, &["Patient.crus", REVIEW_PERMISSION])
}

/// The server as the subject layer serves it: the subject and agent are request extensions.
fn app(store: &SurrealStore, subject: Option<&Subject>) -> Router {
    let router = FhirServer::new(store.clone(), BASE, Arc::new(Unrestricted))
        .with_access_control(Pdp::default(), Arc::new(NoFacilityScope))
        .with_audit(Auditor::new("test central", Vec::new()))
        .with_client_registry(config(), rules())
        .router();
    match subject {
        Some(subject) => router
            .layer(Extension(Agent {
                display: format!("account {}", subject.account),
                ..Agent::unauthenticated()
            }))
            .layer(Extension(subject.clone())),
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

/// The facility `code`'s Organization id, created on first use.
async fn organization(store: &SurrealStore, code: &str) -> String {
    let admin = FhirServer::new(store.clone(), BASE, Arc::new(Unrestricted)).router();
    let (_, found) = send(&admin, "GET", "/Organization?_count=100", None).await;
    if let Some(id) = found["entry"].as_array().and_then(|entries| {
        entries
            .iter()
            .find(|e| e["resource"]["identifier"][0]["value"] == code)
            .and_then(|e| e["resource"]["id"].as_str().map(str::to_owned))
    }) {
        return id;
    }
    let (status, body) = send(
        &admin,
        "POST",
        "/Organization",
        Some(json!({
            "resourceType": "Organization",
            "identifier": [{ "system": "urn:health:facility", "value": code }],
            "name": code,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().expect("id").to_owned()
}

/// A source Patient registered at `facility`.
async fn source(store: &SurrealStore, facility: &str, family: &str, phone: &str) -> String {
    sourced(store, facility, family, phone, "staff-entered").await
}

/// A source Patient registered at `facility`, its values trusted as `trust`.
async fn sourced(
    store: &SurrealStore,
    facility: &str,
    family: &str,
    phone: &str,
    trust: &str,
) -> String {
    let facility = organization(store, facility).await;
    let admin = FhirServer::new(store.clone(), BASE, Arc::new(Unrestricted)).router();
    let (status, body) = send(
        &admin,
        "POST",
        "/Patient",
        Some(json!({
            "resourceType": "Patient",
            "meta": { "tag": [{ "system": "urn:health:registry:trust", "code": trust }] },
            "name": [{ "family": family, "given": ["Grace"] }],
            "gender": "female",
            "telecom": [{ "system": "phone", "value": phone }],
            "managingOrganization": { "reference": format!("Organization/{facility}") },
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().expect("id").to_owned()
}

async fn patient(store: &SurrealStore, id: &str) -> Value {
    let admin = FhirServer::new(store.clone(), BASE, Arc::new(Unrestricted)).router();
    send(&admin, "GET", &format!("/Patient/{id}"), None).await.1
}

/// The AuditEvents so far that name `reviewer` as their agent.
async fn audited_by(store: &SurrealStore, reviewer: &str) -> Vec<Value> {
    let reader = FhirServer::new(store.clone(), BASE, Arc::new(Unrestricted)).router();
    let (status, bundle) = send(&reader, "GET", "/AuditEvent?_count=1000", None).await;
    assert_eq!(status, StatusCode::OK, "{bundle}");
    bundle["entry"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .map(|e| e["resource"].clone())
                .filter(|e| {
                    e["agent"]
                        .to_string()
                        .contains(&format!("account {reviewer}"))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn merge_body(source: &str, target: &str, preview: bool) -> Value {
    json!({ "resourceType": "Parameters", "parameter": [
        { "name": "source-patient", "valueReference": { "reference": format!("Patient/{source}") } },
        { "name": "target-patient", "valueReference": { "reference": format!("Patient/{target}") } },
        { "name": "preview", "valueBoolean": preview },
    ] })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reviewer_previews_then_approves_a_same_facility_merge() {
    let store = store().await;
    let a = source(&store, "KAM013", "Nakato", "0772000001").await;
    let b = source(&store, "KAM013", "Nakato", "0772000001").await;
    let other = source(&store, "WAK002", "Nakato", "0772000001").await;
    let app = app(&store, Some(&reviewer("KAM013")));

    // The preview writes nothing.
    let (status, preview) = send(
        &app,
        "POST",
        "/Patient/$merge",
        Some(merge_body(&a, &b, true)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(
        preview["parameter"][0]["resource"]["issue"][0]["severity"],
        "information"
    );
    assert_ne!(patient(&store, &a).await["active"], false);
    assert!(audited_by(&store, "reviewer-1").await.is_empty());

    // Approved: the reviewer from the token approves, and both versions are audited.
    let (status, merged) = send(
        &app,
        "POST",
        "/Patient/$merge",
        Some(merge_body(&a, &b, false)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{merged}");
    let retired = patient(&store, &a).await;
    assert_eq!(retired["active"], false);
    assert!(
        retired["link"].to_string().contains("replaced-by"),
        "{retired}"
    );
    let result = merged["parameter"]
        .as_array()
        .and_then(|p| p.iter().find(|p| p["name"] == "result"))
        .expect("result");
    assert!(result["resource"]["link"].to_string().contains("replaces"));
    assert_eq!(audited_by(&store, "reviewer-1").await.len(), 2);

    // Records of different facilities are never merged.
    let (status, outcome) = send(
        &app,
        "POST",
        "/Patient/$merge",
        Some(merge_body(&b, &other, true)),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{outcome}");
    // Nor are another facility's records, by this facility's reviewer.
    let c = source(&store, "WAK002", "Achen", "0772000002").await;
    let (status, _) = send(
        &app,
        "POST",
        "/Patient/$merge",
        Some(merge_body(&other, &c, true)),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn link_and_unlink_are_audited_and_queue_survivorship_review() {
    let store = store().await;
    let a = source(&store, "KAM013", "Nakato", "0772000001").await;
    // B's phone is self-reported, so it loses to A's and goes to review.
    let b = sourced(&store, "KAM013", "Nakato", "0772999999", "self-reported").await;
    let app = app(&store, Some(&reviewer("KAM013")));

    // Linking with no golden record creates one.
    let (status, linked) = send(&app, "POST", &format!("/Patient/{a}/$link"), None).await;
    assert_eq!(status, StatusCode::OK, "{linked}");
    assert_eq!(linked["parameter"][1]["valueBoolean"], true);
    let golden = linked["parameter"][0]["valueReference"]["reference"]
        .as_str()
        .expect("golden")
        .to_owned();
    // Linking a second source whose phone disagrees queues a survivorship review item.
    let (status, second) = send(
        &app,
        "POST",
        &format!("/Patient/{b}/$link"),
        Some(json!({ "resourceType": "Parameters", "parameter": [
            { "name": "golden", "valueReference": { "reference": golden } },
        ] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    let queued = second["parameter"][2]["valueInteger"]
        .as_u64()
        .expect("review");
    assert!(queued > 0, "the phones disagree: {second}");
    let mut tx = store.begin().await.expect("tx");
    let pending = tx.pending_reviews(10).await.expect("pending");
    assert_eq!(pending.len() as u64, queued);
    drop(tx);

    // Unlinking separates the source again.
    let (status, unlinked) = send(&app, "POST", &format!("/Patient/{b}/$unlink"), None).await;
    assert_eq!(status, StatusCode::OK, "{unlinked}");
    assert_eq!(
        unlinked["parameter"][0]["valueReference"]["reference"],
        golden
    );
    // Each link writes two versions (golden and source), as does the unlink.
    assert_eq!(audited_by(&store, "reviewer-1").await.len(), 6);
    // A source already linked cannot be linked again.
    let (status, _) = send(&app, "POST", &format!("/Patient/{a}/$link"), None).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn only_signed_in_reviewers_use_the_registry() {
    let store = store().await;
    let a = source(&store, "KAM013", "Nakato", "0772000001").await;
    let (status, _) = send(
        &app(&store, None),
        "POST",
        &format!("/Patient/{a}/$link"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let clerk = subject("clerk-1", "KAM013", &["Patient.crus"]);
    let (status, _) = send(
        &app(&store, Some(&clerk)),
        "POST",
        &format!("/Patient/{a}/$link"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// `$match` for `family` with `nin`, as `subject`: the status and the candidates' names.
async fn match_as(
    store: &SurrealStore,
    subject: &Subject,
    pdp: Pdp,
    query: Value,
) -> (StatusCode, Vec<String>) {
    let router = FhirServer::new(store.clone(), BASE, Arc::new(Unrestricted))
        .with_access_control(pdp, Arc::new(NoFacilityScope))
        .with_client_registry(config(), rules())
        .router()
        .layer(Extension(subject.clone()));
    let (status, body) = send(
        &router,
        "POST",
        "/Patient/$match",
        Some(json!({ "resourceType": "Parameters", "parameter": [
            { "name": "resource", "resource": query },
            { "name": "count", "valueInteger": 5 },
        ] })),
    )
    .await;
    let names = body["entry"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|e| {
                    e["resource"]["name"][0]["family"]
                        .as_str()
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default();
    (status, names)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn match_returns_ranked_candidates_the_subject_may_see() {
    const ACT: &str = "http://terminology.hl7.org/CodeSystem/v3-ActCode";
    let store = store().await;
    let admin = FhirServer::new(store.clone(), BASE, Arc::new(Unrestricted)).router();
    let organization = organization(&store, "KAM013").await;
    for (family, nin, labelled) in [
        ("Nakato", "CF12345678ABCD", false),
        ("Nakato", "CF12345678ABCE", true),
        ("Achen", "CM98765432WXYZ", false),
    ] {
        let mut patient = json!({
            "resourceType": "Patient",
            "meta": { "tag": [{ "system": "urn:health:registry:trust", "code": "staff-entered" }] },
            "identifier": [{ "system": NIN, "value": nin }],
            "name": [{ "family": family, "given": ["Grace"] }],
            "gender": "female",
            "birthDate": "1990-04-02",
            "managingOrganization": { "reference": format!("Organization/{organization}") },
        });
        if labelled {
            patient["meta"]["security"] = json!([{ "system": ACT, "code": "HIV" }]);
        }
        let (status, body) = send(&admin, "POST", "/Patient", Some(patient)).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    let query = json!({
        "resourceType": "Patient",
        "identifier": [{ "system": NIN, "value": "CF12345678ABCD" }],
        "name": [{ "family": "Nakato", "given": ["Grace"] }],
        "gender": "female",
        "birthDate": "1990-04-02",
    });
    let pdp = || {
        Pdp::new(vec![health_access_policy::LabelRule {
            label: health_access_policy::Label::new(ACT, "HIV"),
            program: "hiv".into(),
        }])
    };
    let clerk = subject("clerk-1", "KAM013", &["Patient.rs"]);
    let (status, names) = match_as(&store, &clerk, pdp(), query.clone()).await;
    assert_eq!(status, StatusCode::OK);
    // The exact NIN ranks first; the labelled namesake is not shown to a clerk without the
    // program, and the unrelated patient is no candidate.
    assert_eq!(names, vec!["Nakato".to_owned()]);
    let counsellor = Subject {
        programs: BTreeSet::from(["hiv".to_owned()]),
        ..clerk.clone()
    };
    let (_, names) = match_as(&store, &counsellor, pdp(), query.clone()).await;
    assert_eq!(names, vec!["Nakato".to_owned(), "Nakato".to_owned()]);
    // Matching is a search: without Patient search permission it is refused.
    let cashier = subject("cashier-1", "KAM013", &["Invoice.crus"]);
    let (status, _) = match_as(&store, &cashier, pdp(), query).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // And it needs a signed-in user.
    let anonymous = FhirServer::new(store.clone(), BASE, Arc::new(Unrestricted))
        .with_access_control(pdp(), Arc::new(NoFacilityScope))
        .with_client_registry(config(), rules())
        .router();
    let (status, _) = send(&anonymous, "POST", "/Patient/$match", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
