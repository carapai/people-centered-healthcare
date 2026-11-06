//! The Care Journey over HTTP (#115): `GET /Patient/{id}/$journey` arranges the person's care,
//! across their linked records, by program, with what waits on a Task. The policy decision point
//! decides every record at read time: a program the reader may not see is not on the journey, and
//! its steps fall under care outside any program. Each step names its authoritative resource and
//! version, and the journey writes nothing, so rebuilding it gives the same journey.

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

/// The patient each journey type is searched by.
const PATIENT_PATHS: [(&str, &str); 7] = [
    ("EpisodeOfCare", "patient"),
    ("Encounter", "subject"),
    ("Appointment", "subject"),
    ("ServiceRequest", "subject"),
    ("DiagnosticReport", "subject"),
    ("MedicationRequest", "subject"),
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
        namespace: "journey".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry());
    let admin = FhirServer::new(store.clone(), "http://test/fhir", Arc::new(Unrestricted)).router();
    let patient =
        |id: &str, links: Value| json!({ "resourceType": "Patient", "id": id, "link": links });
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
    let diabetes = create(&admin, tagged(json!({ "resourceType": "EpisodeOfCare", "status": "active",
        "type": [{ "text": "Diabetes care" }], "patient": { "reference": here }, "period": { "start": "2026-10-01T08:00:00Z" } }), &[])).await;
    let hiv = create(&admin, tagged(json!({ "resourceType": "EpisodeOfCare", "status": "active",
        "type": [{ "text": "HIV care" }], "patient": { "reference": here }, "period": { "start": "2026-09-01T08:00:00Z" } }), &["HIV"])).await;
    create(&admin, tagged(json!({ "resourceType": "Encounter", "status": "completed", "type": [{ "text": "Diabetes clinic" }],
        "subject": { "reference": here }, "episodeOfCare": [{ "reference": diabetes }],
        "actualPeriod": { "start": "2026-10-02T09:00:00Z" } }), &[])).await;
    let referral = create(&admin, tagged(json!({ "resourceType": "ServiceRequest", "status": "active", "intent": "order",
        "code": { "concept": { "text": "Eye clinic" } }, "category": [{ "coding": [{ "system": "http://snomed.info/sct", "code": "3457005" }] }],
        "subject": { "reference": here }, "authoredOn": "2026-10-02T09:40:00Z",
        "extension": [{ "url": "http://hl7.org/fhir/StructureDefinition/workflow-episodeOfCare", "valueReference": { "reference": diabetes } }] }), &[])).await;
    create(
        &admin,
        tagged(
            json!({ "resourceType": "Task", "status": "in-progress", "intent": "order",
        "focus": { "reference": referral }, "for": { "reference": here } }),
            &[],
        ),
    )
    .await;
    // A visit in the HIV program: the clinician sees the visit, not the program.
    create(&admin, tagged(json!({ "resourceType": "Encounter", "status": "completed", "type": [{ "text": "Outpatient visit" }],
        "subject": { "reference": here }, "episodeOfCare": [{ "reference": hiv }],
        "actualPeriod": { "start": "2026-09-05T09:00:00Z" } }), &[])).await;
    // At another facility, under the same person's other record.
    create(&admin, tagged(json!({ "resourceType": "Appointment", "status": "booked", "description": "Eye review",
        "subject": { "reference": there }, "participant": [{ "actor": { "reference": there }, "status": "accepted" }],
        "start": "2026-12-01T09:00:00Z", "end": "2026-12-01T09:30:00Z" }), &[])).await;
    let pdp = Pdp::new(vec![LabelRule {
        label: Label::new(ACT, "HIV"),
        program: "hiv".into(),
    }]);
    let guarded = FhirServer::new(store, "http://test/fhir", Arc::new(Unrestricted))
        .with_access_control(pdp, Arc::new(FacilityTag))
        .router();
    (guarded, admin)
}

async fn journey(router: &Router, who: &Subject) -> Value {
    let (status, body) = send(
        router,
        Some(who),
        Method::GET,
        &format!("/Patient/{HERE}/$journey"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

/// Each stretch's program title (or `-`) and its steps' titles.
fn shape(journey: &Value) -> Vec<(String, Vec<String>)> {
    journey["stretches"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| {
            (
                s["program"]["title"].as_str().unwrap_or("-").to_owned(),
                s["steps"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|st| st["title"].as_str().map(str::to_owned))
                    .collect(),
            )
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_journey_arranges_linked_care_the_reader_may_see_by_program() {
    let (router, _) = routers().await;
    let clinician = subject(Permissions::from(["*.rs"]), &[]);
    let shown = journey(&router, &clinician).await;
    let own = |s: &[&str]| s.iter().map(|t| (*t).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        shape(&shown),
        [
            (
                "Diabetes care".to_owned(),
                own(&["Diabetes clinic", "Eye clinic"])
            ),
            ("-".to_owned(), own(&["Outpatient visit", "Eye review"])),
        ],
        "the HIV program is withheld, so its visit is outside any program; the linked record's appointment is in"
    );
    assert_eq!(shown["waiting"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        shown["waiting"][0]["title"], "Eye clinic",
        "its Task is open"
    );
    assert_eq!(shown["waiting"][0]["stage"], "referral");
    assert_eq!(shown["unseen"], json!([]));

    // In the HIV program, the program and its visit are on the journey.
    let counsellor = subject(Permissions::from(["*.rs"]), &["hiv"]);
    let titles: Vec<String> = shape(&journey(&router, &counsellor).await)
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    assert_eq!(titles, ["HIV care", "Diabetes care", "-"]);

    // Without Task searches, nothing is known to wait, and Task is unseen.
    let narrow = subject(
        Permissions::from([
            "Patient.rs",
            "EpisodeOfCare.rs",
            "Encounter.rs",
            "ServiceRequest.rs",
        ]),
        &[],
    );
    let shown = journey(&router, &narrow).await;
    assert_eq!(shown["waiting"], json!([]));
    assert_eq!(
        shown["unseen"],
        json!([
            "Appointment",
            "DiagnosticReport",
            "MedicationRequest",
            "Task"
        ])
    );

    // A patient the reader may not read is not found, as a read.
    let cashier = subject(Permissions::from(["Invoice.rs"]), &[]);
    let (status, _) = send(
        &router,
        Some(&cashier),
        Method::GET,
        &format!("/Patient/{HERE}/$journey"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn steps_name_their_owners_and_a_rebuild_is_identical() {
    let (router, admin) = routers().await;
    let clinician = subject(Permissions::from(["*.rs"]), &[]);
    let first = journey(&router, &clinician).await;
    let steps: Vec<Value> = first["stretches"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|s| s["steps"].as_array().cloned().unwrap_or_default())
        .collect();
    assert_eq!(steps.len(), 4);
    for step in &steps {
        let reference = step["reference"].as_str().expect("reference");
        let (status, resource) =
            send(&admin, None, Method::GET, &format!("/{reference}"), None).await;
        assert_eq!(status, StatusCode::OK, "{reference}");
        assert_eq!(
            step["version"], resource["meta"]["versionId"],
            "{reference}"
        );
        assert_eq!(step["facility"], "F1", "{reference}");
    }
    assert_eq!(
        journey(&router, &clinician).await,
        first,
        "reading writes nothing, and a rebuild is the same"
    );
}
