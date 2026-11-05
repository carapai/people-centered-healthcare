//! The patient summary over HTTP (#104): `GET /Patient/{id}/$summary` draws on the patient's
//! records and, through the golden record, the same person's other records. Sensitive labels and
//! permissions are the policy decision point's: a withheld record never appears, and a section the
//! clinician may not search is `null`.

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

/// The patient each section's type is searched by.
const PATIENT_PATHS: [(&str, &str); 6] = [
    ("AllergyIntolerance", "patient"),
    ("Condition", "subject"),
    ("MedicationRequest", "subject"),
    ("Observation", "subject"),
    ("Encounter", "subject"),
    ("EpisodeOfCare", "patient"),
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

async fn router() -> Router {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "summary".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry());
    let admin = FhirServer::new(store.clone(), "http://test/fhir", Arc::new(Unrestricted)).router();
    let patient =
        |id: &str, links: Value| json!({ "resourceType": "Patient", "id": id, "link": links });
    let condition = |text: &str, who: &str| {
        json!({ "resourceType": "Condition", "clinicalStatus": { "coding": [{ "code": "active" }] },
                "code": { "text": text }, "subject": { "reference": format!("Patient/{who}") }, "onsetDateTime": "2025-01-01" })
    };
    let weight: Value = serde_json::from_str(&format!(
        r#"{{ "resourceType": "Observation", "status": "final",
              "category": [{{ "coding": [{{ "code": "vital-signs" }}] }}], "code": {{ "text": "Body weight" }},
              "subject": {{ "reference": "Patient/{HERE}" }}, "effectiveDateTime": "2026-11-05",
              "valueQuantity": {{ "value": 61.50, "unit": "kg" }} }}"#
    ))
    .expect("json");
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
    for resource in [
        tagged(condition("Hypertension", HERE), &[]),
        tagged(condition("HIV infection", HERE), &["HIV"]),
        tagged(condition("Asthma", THERE), &[]),
        tagged(weight, &[]),
        tagged(
            json!({ "resourceType": "AllergyIntolerance", "criticality": "high", "code": { "text": "Penicillin" },
                    "patient": { "reference": format!("Patient/{HERE}") } }),
            &[],
        ),
    ] {
        let kind = resource["resourceType"].as_str().expect("type").to_owned();
        let (status, created) = send(
            &admin,
            None,
            Method::POST,
            &format!("/{kind}"),
            Some(resource),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
    }
    let pdp = Pdp::new(vec![LabelRule {
        label: Label::new(ACT, "HIV"),
        program: "hiv".into(),
    }]);
    FhirServer::new(store, "http://test/fhir", Arc::new(Unrestricted))
        .with_access_control(pdp, Arc::new(FacilityTag))
        .router()
}

fn texts(summary: &Value, section: &str, field: &str) -> Vec<String> {
    summary[section]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| i[field]["text"].as_str().map(str::to_owned))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_summary_draws_on_the_linked_records_the_clinician_may_see() {
    let router = router().await;
    let clinician = subject(Permissions::from(["*.rs"]), &[]);
    let (status, summary) = send(
        &router,
        Some(&clinician),
        Method::GET,
        &format!("/Patient/{HERE}/$summary"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{summary}");
    let mut conditions = texts(&summary, "conditions", "code");
    conditions.sort();
    assert_eq!(
        conditions,
        ["Asthma", "Hypertension"],
        "the HIV-labelled one is withheld; the linked record's is in"
    );
    assert_eq!(texts(&summary, "allergies", "substance"), ["Penicillin"]);
    assert_eq!(summary["vitals"][0]["value"], "61.50");
    assert_eq!(summary["medications"], json!([]));

    // In the HIV program, the labelled condition is seen.
    let counsellor = subject(Permissions::from(["*.rs"]), &["hiv"]);
    let (_, summary) = send(
        &router,
        Some(&counsellor),
        Method::GET,
        &format!("/Patient/{HERE}/$summary"),
        None,
    )
    .await;
    assert_eq!(texts(&summary, "conditions", "code").len(), 3);

    // Only Patient and Condition searches: the other sections are not seen at all.
    let narrow = subject(Permissions::from(["Patient.rs", "Condition.rs"]), &[]);
    let (status, summary) = send(
        &router,
        Some(&narrow),
        Method::GET,
        &format!("/Patient/{HERE}/$summary"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{summary}");
    assert_eq!(summary["allergies"], Value::Null);
    assert_eq!(texts(&summary, "conditions", "code").len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_patient_the_clinician_may_not_read_is_not_found() {
    let router = router().await;
    let cashier = subject(Permissions::from(["Invoice.rs"]), &[]);
    assert_eq!(
        send(
            &router,
            Some(&cashier),
            Method::GET,
            &format!("/Patient/{HERE}/$summary"),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let clinician = subject(Permissions::from(["*.rs"]), &[]);
    let unknown = "0190f1a2-7c3d-7e4f-8a5b-0000000000ff";
    assert_eq!(
        send(
            &router,
            Some(&clinician),
            Method::GET,
            &format!("/Patient/{unknown}/$summary"),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            &router,
            None,
            Method::GET,
            &format!("/Patient/{HERE}/$summary"),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}
