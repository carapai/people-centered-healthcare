//! The longitudinal timeline over HTTP (#115): `GET /Patient/{id}/$timeline` draws on the
//! patient's records and, through the golden record, the same person's other records, the newest
//! first. The policy decision point decides every entry at read time; each entry names its
//! authoritative resource and version, and the timeline writes nothing, so rebuilding it gives the
//! same timeline.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use health_access_policy::{FACILITY_TAG_SYSTEM, Label, LabelRule, Pdp, Permissions, Subject};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::{FacilityTag, FhirServer};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const ACT: &str = "http://terminology.hl7.org/CodeSystem/v3-ActCode";
const HERE: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000a1";
const THERE: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000a2";
const GOLDEN: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000a9";

/// The patient each timeline type is searched by.
const PATIENT_PATHS: [(&str, &str); 9] = [
    ("Encounter", "subject"),
    ("EpisodeOfCare", "patient"),
    ("Observation", "subject"),
    ("ServiceRequest", "subject"),
    ("MedicationRequest", "subject"),
    ("DiagnosticReport", "subject"),
    ("Appointment", "subject"),
    ("Composition", "subject"),
    ("Task", "for"),
];

fn registry() -> Registry {
    let mut definitions = vec![
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/Resource-tag", "code": "_tag", "base": ["Resource"], "type": "token", "expression": "Resource.meta.tag" }),
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/Resource-security", "code": "_security", "base": ["Resource"], "type": "token", "expression": "Resource.meta.security" }),
    ];
    for (base, path) in PATIENT_PATHS {
        definitions.push(json!({
            "resourceType": "SearchParameter", "url": format!("http://hl7.org/fhir/SearchParameter/{base}-patient"),
            "code": "patient", "base": [base], "type": "reference", "expression": format!("{base}.{path}"), "target": ["Patient"]
        }));
    }
    let catalog = Catalog::from_definitions(definitions.iter());
    let mut registry = Registry::new(jiff::tz::Offset::UTC);
    for (base, _) in PATIENT_PATHS {
        for code in ["_tag", "_security", "patient"] {
            registry
                .support(&catalog, base, code, ParamState::Active)
                .expect(code);
        }
    }
    registry
}

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
    let response = router
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

fn subject(permissions: Permissions, programs: &[&str]) -> Subject {
    Subject {
        account: "clinician-1".into(),
        facility: "F1".into(),
        permissions,
        programs: programs.iter().map(|p| (*p).to_owned()).collect(),
        purpose: Some("TREAT".into()),
        break_glass: None,
    }
}

fn tagged(mut resource: Value, labels: &[&str]) -> Value {
    resource["meta"] = json!({ "tag": [{ "system": FACILITY_TAG_SYSTEM, "code": "F1" }] });
    if !labels.is_empty() {
        resource["meta"]["security"] = Value::Array(
            labels
                .iter()
                .map(|l| json!({ "system": ACT, "code": l }))
                .collect(),
        );
    }
    resource
}

async fn create(admin: &Router, resource: Value) -> String {
    let kind = resource["resourceType"].as_str().expect("type").to_owned();
    let (status, created) = send(
        admin,
        None,
        Method::POST,
        &format!("/{kind}"),
        Some(resource),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    format!("{kind}/{}", created["id"].as_str().expect("id"))
}

/// The server under access control, and an unrestricted one to write and read with.
async fn routers() -> (Router, Router) {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "timeline".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry());
    let admin = FhirServer::new(store.clone(), "http://test/fhir", Arc::new(Unrestricted)).router();
    let patient =
        |id: &str, links: Value| json!({ "resourceType": "Patient", "id": id, "link": links });
    // The links name each other, so the three are written in one transaction.
    let entries: Vec<Value> = [
        patient(GOLDEN, json!([{ "other": { "reference": format!("Patient/{HERE}") }, "type": "seealso" }, { "other": { "reference": format!("Patient/{THERE}") }, "type": "seealso" }])),
        patient(HERE, json!([{ "other": { "reference": format!("Patient/{GOLDEN}") }, "type": "refer" }])),
        patient(THERE, json!([{ "other": { "reference": format!("Patient/{GOLDEN}") }, "type": "refer" }])),
    ]
    .into_iter()
    .map(|p| json!({ "resource": p, "request": { "method": "PUT", "url": format!("Patient/{}", p["id"].as_str().expect("id")) } }))
    .collect();
    let (status, body) = send(
        &admin,
        None,
        Method::POST,
        "/",
        Some(json!({ "resourceType": "Bundle", "type": "transaction", "entry": entries })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let here = format!("Patient/{HERE}");
    let there = format!("Patient/{THERE}");
    let episode = create(&admin, tagged(json!({ "resourceType": "EpisodeOfCare", "status": "active",
        "type": [{ "text": "Diabetes care" }], "patient": { "reference": here }, "period": { "start": "2026-10-01T08:00:00Z" } }), &[])).await;
    for resource in [
        tagged(
            json!({ "resourceType": "Encounter", "status": "completed", "type": [{ "text": "Diabetes clinic" }],
            "subject": { "reference": here }, "episodeOfCare": [{ "reference": episode }],
            "actualPeriod": { "start": "2026-11-02T09:00:00Z" } }),
            &[],
        ),
        tagged(
            json!({ "resourceType": "Observation", "status": "final", "code": { "text": "HbA1c" },
            "subject": { "reference": here }, "effectiveDateTime": "2026-11-02T09:30:00Z" }),
            &[],
        ),
        tagged(
            json!({ "resourceType": "Observation", "status": "final", "code": { "text": "HIV viral load" },
            "subject": { "reference": here }, "effectiveDateTime": "2026-11-03T09:30:00Z" }),
            &["HIV"],
        ),
        // At another facility, under the same person's other record.
        tagged(
            json!({ "resourceType": "Encounter", "status": "completed", "type": [{ "text": "Outpatient visit" }],
            "subject": { "reference": there }, "actualPeriod": { "start": "2026-10-20T09:00:00Z" } }),
            &[],
        ),
        // Not yet verified, and a draft note: neither is on the timeline.
        tagged(
            json!({ "resourceType": "DiagnosticReport", "status": "preliminary", "code": { "text": "Full blood count" },
            "subject": { "reference": here }, "effectiveDateTime": "2026-11-04T09:30:00Z" }),
            &[],
        ),
        tagged(
            json!({ "resourceType": "Composition", "status": "preliminary", "title": "Progress note", "type": { "text": "Progress note" },
            "subject": [{ "reference": here }], "date": "2026-11-04T10:00:00Z", "author": [{ "display": "clinician-1" }] }),
            &[],
        ),
    ] {
        create(&admin, resource).await;
    }
    let pdp = Pdp::new(vec![LabelRule {
        label: Label::new(ACT, "HIV"),
        program: "hiv".into(),
    }]);
    let guarded = FhirServer::new(store, "http://test/fhir", Arc::new(Unrestricted))
        .with_access_control(pdp, Arc::new(FacilityTag))
        .router();
    (guarded, admin)
}

async fn timeline(router: &Router, who: &Subject) -> Value {
    let (status, body) = send(
        router,
        Some(who),
        Method::GET,
        &format!("/Patient/{HERE}/$timeline"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

fn titles(timeline: &Value) -> Vec<String> {
    timeline["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["title"].as_str().map(str::to_owned))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_timeline_draws_on_linked_records_the_reader_may_see() {
    let (router, _) = routers().await;
    let clinician = subject(Permissions::from(["*.rs"]), &[]);
    let shown = timeline(&router, &clinician).await;
    assert_eq!(
        titles(&shown),
        [
            "HbA1c",
            "Diabetes clinic",
            "Outpatient visit",
            "Diabetes care"
        ],
        "newest first; the linked record's visit is in; the HIV-labelled result, the unverified report and the draft are not"
    );
    assert_eq!(shown["unseen"], json!([]));
    assert_eq!(shown["entries"][1]["kind"], "visit");
    assert!(
        shown["entries"][1]["episode"]
            .as_str()
            .is_some_and(|e| e.starts_with("EpisodeOfCare/")),
        "{shown}"
    );

    // In the HIV program, the labelled result is on it.
    let counsellor = subject(Permissions::from(["*.rs"]), &["hiv"]);
    assert_eq!(
        titles(&timeline(&router, &counsellor).await)[0],
        "HIV viral load"
    );

    // Only Patient and Encounter searches: the rest is unseen, not empty.
    let narrow = subject(Permissions::from(["Patient.rs", "Encounter.rs"]), &[]);
    let shown = timeline(&router, &narrow).await;
    assert_eq!(titles(&shown), ["Diabetes clinic", "Outpatient visit"]);
    assert_eq!(shown["unseen"].as_array().map(Vec::len), Some(8));

    // A patient the reader may not read is not found, as a read.
    let cashier = subject(Permissions::from(["Invoice.rs"]), &[]);
    let (status, _) = send(
        &router,
        Some(&cashier),
        Method::GET,
        &format!("/Patient/{HERE}/$timeline"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn entries_name_their_owners_and_a_rebuild_is_identical() {
    let (router, admin) = routers().await;
    let clinician = subject(Permissions::from(["*.rs"]), &[]);
    let first = timeline(&router, &clinician).await;
    // Each entry is the authoritative resource's current version, at its facility.
    for entry in first["entries"].as_array().into_iter().flatten() {
        let reference = entry["reference"].as_str().expect("reference");
        let (status, resource) =
            send(&admin, None, Method::GET, &format!("/{reference}"), None).await;
        assert_eq!(status, StatusCode::OK, "{reference}");
        assert_eq!(
            entry["version"], resource["meta"]["versionId"],
            "{reference}"
        );
        assert_eq!(entry["facility"], "F1", "{reference}");
    }
    // Reading writes nothing: every resource keeps its version, and a rebuild is the same.
    assert_eq!(timeline(&router, &clinician).await, first);

    // A result entered in error leaves the timeline.
    let hba1c = first["entries"][0]["reference"]
        .as_str()
        .expect("reference")
        .to_owned();
    let (_, mut resource) = send(&admin, None, Method::GET, &format!("/{hba1c}"), None).await;
    let version = resource["meta"]["versionId"]
        .as_str()
        .expect("version")
        .to_owned();
    resource["status"] = json!("entered-in-error");
    let response = admin
        .clone()
        .oneshot(
            Request::put(format!("/{hba1c}"))
                .header("content-type", "application/fhir+json")
                .header("if-match", format!("W/\"{version}\""))
                .body(Body::from(resource.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!titles(&timeline(&router, &clinician).await).contains(&"HbA1c".to_owned()));
}
