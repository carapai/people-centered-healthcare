//! Restricted records never appear in search results: the PDP's filter, applied by the real
//! FhirServer over SurrealDB (memory engine), inside the query.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use health_access_policy::{BTG, BreakGlassPolicy, Label, LabelRule, Pdp, Permissions, Subject};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::FhirServer;
use health_identity_app::SubjectSearchPolicy;
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
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/Resource-security", "code": "_security", "base": ["Resource"], "type": "token", "expression": "Resource.meta.security" }),
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/clinical-patient", "code": "patient", "base": ["Observation"], "type": "reference", "expression": "Observation.subject.where(resolve() is Patient)", "target": ["Patient"] }),
    ];
    let catalog = Catalog::from_definitions(definitions.iter());
    let mut registry = Registry::new(jiff::tz::Offset::UTC);
    for code in ["_id", "_security", "patient"] {
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

async fn send(
    router: &Router,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if body.is_some() {
        request = request.header("content-type", "application/fhir+json");
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
        programs: programs
            .iter()
            .map(|p| (*p).to_owned())
            .collect::<BTreeSet<_>>(),
        purpose: Some("TREAT".into()),
        break_glass: None,
    }
}

async fn ids(router: &Router, query: &str) -> BTreeSet<String> {
    let (status, bundle) = send(router, Method::GET, &format!("/Observation{query}"), None).await;
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
async fn restricted_records_never_appear_in_search_results() {
    let store = store().await;
    let admin = FhirServer::new(store.clone(), BASE, Arc::new(Unrestricted)).router();
    let (_, p1) = send(
        &admin,
        Method::POST,
        "/Patient",
        Some(json!({ "resourceType": "Patient" })),
    )
    .await;
    let (_, p2) = send(
        &admin,
        Method::POST,
        "/Patient",
        Some(json!({ "resourceType": "Patient" })),
    )
    .await;
    let (p1, p2) = (
        p1["id"].as_str().expect("id").to_owned(),
        p2["id"].as_str().expect("id").to_owned(),
    );
    for (name, patient, labels) in [
        ("plain", &p1, json!([])),
        ("hiv", &p1, json!([{ "system": ACT, "code": "HIV" }])),
        ("psy", &p1, json!([{ "system": ACT, "code": "PSY" }])),
        (
            "normal-conf",
            &p1,
            json!([{ "system": "http://terminology.hl7.org/CodeSystem/v3-Confidentiality", "code": "N" }]),
        ),
        (
            "hiv-other-patient",
            &p2,
            json!([{ "system": ACT, "code": "HIV" }]),
        ),
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
        let (status, body) = send(&admin, Method::POST, "/Observation", Some(observation)).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    let now = Timestamp::now();
    let as_subject = |subject: Subject, patient: Option<String>| {
        FhirServer::new(
            store.clone(),
            BASE,
            Arc::new(SubjectSearchPolicy::new(pdp(), subject, patient, now)),
        )
        .router()
    };
    let set = |names: &[&str]| {
        names
            .iter()
            .map(|n| (*n).to_owned())
            .collect::<BTreeSet<_>>()
    };

    // A clinician without program assignments sees no labelled record, in any search.
    let clinician = subject(Permissions::from(["*.rs", "break-glass"]), &[]);
    let router = as_subject(clinician.clone(), None);
    assert_eq!(ids(&router, "").await, set(&["plain", "normal-conf"]));
    assert_eq!(
        ids(&router, &format!("?patient={p1}")).await,
        set(&["plain", "normal-conf"])
    );
    // Asking for the label explicitly does not get round the filter.
    assert_eq!(
        ids(&router, &format!("?_security={ACT}|HIV")).await,
        set(&[])
    );

    // An HIV counsellor sees HIV records too, but not mental-health ones.
    let counsellor = as_subject(
        subject(Permissions::from(["Observation.rs"]), &["hiv"]),
        None,
    );
    assert_eq!(
        ids(&counsellor, "").await,
        set(&["plain", "normal-conf", "hiv", "hiv-other-patient"])
    );

    // `check` lets the caller answer 403 before searching.
    let policy = SubjectSearchPolicy::new(
        pdp(),
        subject(Permissions::from(["Invoice.rs"]), &[]),
        None,
        now,
    );
    assert_eq!(policy.check("Invoice"), Ok(()));
    assert_eq!(
        policy.check("Observation"),
        Err(health_access_policy::Deny::NoPermission)
    );

    // No search permission: nothing at all.
    let cashier = as_subject(subject(Permissions::from(["Invoice.rs"]), &[]), None);
    assert_eq!(ids(&cashier, "").await, set(&[]));

    // Break-glass for patient 1 opens that patient's records, and only in a search confined to
    // that patient.
    let mut glass = clinician;
    glass.break_glass = Some(
        BreakGlassPolicy::default()
            .grant(&glass, &p1, BTG, "emergency", now)
            .expect("grant")
            .0,
    );
    let confined = as_subject(glass.clone(), Some(p1.clone()));
    assert_eq!(
        ids(&confined, &format!("?patient={p1}")).await,
        set(&["plain", "hiv", "psy", "normal-conf"])
    );
    let unconfined = as_subject(glass, None);
    assert_eq!(ids(&unconfined, "").await, set(&["plain", "normal-conf"]));
}
