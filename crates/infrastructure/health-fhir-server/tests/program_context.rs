//! Program context on completion (#116; questionnaire-extraction.md § Program context): a response
//! to a program's form records the patient's open episode with `workflow-episodeOfCare`, and so
//! does every resource extracted from it. With several open episodes the clinician names one;
//! with none, nothing is recorded.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{Permissions, Subject};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::{FhirServer, PROGRAMS_ENROL_PERMISSION};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const PATIENT: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000aa";
const EPISODE_EXTENSION: &str = "http://hl7.org/fhir/StructureDefinition/workflow-episodeOfCare";
const FORM: &str = "https://health.go.ug/fhir/Questionnaire/anc-visit";

fn registry() -> Registry {
    let definition = |base: &str, code: &str, kind: &str| {
        json!({ "resourceType": "SearchParameter", "url": format!("http://hl7.org/fhir/SearchParameter/{base}-{code}"),
                "code": code, "base": [base], "type": kind, "expression": format!("{base}.{code}") })
    };
    let supported = [
        ("Questionnaire", "url", "uri"),
        ("Questionnaire", "version", "token"),
        ("Library", "url", "uri"),
        ("Library", "version", "token"),
        ("Library", "type", "token"),
        ("PlanDefinition", "type", "token"),
        ("EpisodeOfCare", "patient", "reference"),
    ];
    let definitions: Vec<Value> = supported
        .iter()
        .map(|(b, c, k)| definition(b, c, k))
        .collect();
    let catalog = Catalog::from_definitions(definitions.iter());
    let mut registry = Registry::new(jiff::tz::Offset::UTC);
    for (base, code, _) in supported {
        registry
            .support(&catalog, base, code, ParamState::Active)
            .expect("supported");
    }
    registry
}

async fn call(app: &Router, method: Method, path: &str, body: &Value) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/fhir+json");
    if let Ok(existing) = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).expect("request"))
        .await
        && existing.status() == StatusCode::OK
        && let Some(etag) = existing.headers().get("etag").cloned()
    {
        request = request.header("if-match", etag);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).expect("request"))
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

fn program(id: &str, name: &str) -> Value {
    json!({
        "resourceType": "PlanDefinition", "id": id, "status": "active", "version": "1.0.0",
        "url": format!("https://health.go.ug/fhir/PlanDefinition/{name}"),
        "type": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/plan-definition-type", "code": "clinical-protocol" }] },
        "extension": [{ "url": "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/program-episode-type", "valueCodeableConcept": { "text": name } }],
        "relatedArtifact": [{ "type": "composed-of", "resource": format!("{FORM}|1.4.0") }]
    })
}

async fn setup() -> (FhirServer, Router) {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "program-context".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry());
    let server = FhirServer::new(store, "http://test/fhir", Arc::new(Unrestricted))
        .with_questionnaire_checks()
        .with_extraction();
    let fhir = server.clone().router();
    let mut mapping: Value = serde_json::from_str(include_str!(
        "../../../contracts/health-fhir-extraction/tests/fixtures/anc-mapping.json"
    ))
    .expect("json");
    mapping["id"] = json!("0190f1a2-7c3d-7e4f-8a5b-0000000000e1");
    for (path, body) in [
        (
            format!("/Patient/{PATIENT}"),
            json!({ "resourceType": "Patient", "id": PATIENT }),
        ),
        (
            "/Encounter/0190f1a2-7c3d-7e4f-8a5b-000000000201".to_owned(),
            json!({ "resourceType": "Encounter", "id": "0190f1a2-7c3d-7e4f-8a5b-000000000201", "status": "in-progress" }),
        ),
        (
            "/Questionnaire/0190f1a2-7c3d-7e4f-8a5b-0000000000b1".to_owned(),
            json!({ "resourceType": "Questionnaire", "id": "0190f1a2-7c3d-7e4f-8a5b-0000000000b1", "url": FORM, "version": "1.4.0", "status": "active",
                    "item": [
                        { "linkId": "vitals", "type": "group", "item": [{ "linkId": "weight", "type": "decimal" }, { "linkId": "weight-note", "type": "string", "repeats": true }] },
                        { "linkId": "medications", "type": "group", "item": [{ "linkId": "medication", "type": "group", "repeats": true,
                          "item": [{ "linkId": "drug", "type": "coding" }, { "linkId": "instructions", "type": "string" }] }] },
                        { "linkId": "danger-signs", "type": "boolean" } ] }),
        ),
        (
            "/Library/0190f1a2-7c3d-7e4f-8a5b-0000000000e1".to_owned(),
            mapping,
        ),
        (
            "/PlanDefinition/0190f1a2-7c3d-7e4f-8a5b-0000000000f1".to_owned(),
            program("0190f1a2-7c3d-7e4f-8a5b-0000000000f1", "anc-program"),
        ),
    ] {
        let (status, created) = call(&fhir, Method::PUT, &path, &body).await;
        assert_eq!(status, StatusCode::CREATED, "{path}: {created}");
    }
    (server, fhir)
}

fn programs(server: &FhirServer) -> Router {
    server.programs_router().layer(Extension(Subject {
        account: "midwife-1".into(),
        facility: "KAM013".into(),
        permissions: Permissions::from([PROGRAMS_ENROL_PERMISSION]),
        programs: BTreeSet::new(),
        purpose: Some("TREAT".into()),
        break_glass: None,
    }))
}

async fn enrol(server: &FhirServer, n: u32, program: &str) -> String {
    let body = json!({ "id": format!("0190f1a2-7c3d-7e4f-8a5b-{n:012}"), "patient": format!("Patient/{PATIENT}"),
                       "program": format!("https://health.go.ug/fhir/PlanDefinition/{program}") });
    let (status, episode) = call(&programs(server), Method::POST, "/enrolments", &body).await;
    assert_eq!(status, StatusCode::CREATED, "{episode}");
    format!("EpisodeOfCare/{}", episode["id"].as_str().expect("id"))
}

fn response(id: &str) -> Value {
    let mut response: Value = serde_json::from_str(include_str!(
        "../../../contracts/health-fhir-extraction/tests/fixtures/anc-response.json"
    ))
    .expect("json");
    response["id"] = json!(id);
    response
}

fn named_episode(resource: &Value) -> Option<String> {
    resource["extension"]
        .as_array()?
        .iter()
        .find(|e| e["url"] == EPISODE_EXTENSION)?["valueReference"]["reference"]
        .as_str()
        .map(str::to_owned)
}

#[tokio::test]
async fn a_programs_form_records_the_patients_episode_and_so_do_its_extracted_records() {
    let (server, fhir) = setup().await;
    // Before any enrolment: nothing is recorded.
    let id = "0190f1a2-7c3d-7e4f-8a5b-0000000000c1";
    let (status, stored) = call(
        &fhir,
        Method::PUT,
        &format!("/QuestionnaireResponse/{id}"),
        &response(id),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{stored}");
    assert_eq!(named_episode(&stored), None);

    let anc = enrol(&server, 701, "anc-program").await;
    let id = "0190f1a2-7c3d-7e4f-8a5b-0000000000c2";
    let (status, stored) = call(
        &fhir,
        Method::PUT,
        &format!("/QuestionnaireResponse/{id}"),
        &response(id),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{stored}");
    assert_eq!(named_episode(&stored).as_deref(), Some(anc.as_str()));
    let (_, observations) = call(&fhir, Method::GET, "/Observation", &Value::Null).await;
    let named: Vec<Option<String>> = observations["entry"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| named_episode(&e["resource"]))
        .collect();
    assert!(
        named.contains(&Some(anc.clone())),
        "the extracted Observation names the episode: {named:?}"
    );
}

#[tokio::test]
async fn with_several_open_episodes_the_clinician_names_one() {
    let (server, fhir) = setup().await;
    let (status, _) = call(
        &fhir,
        Method::PUT,
        "/PlanDefinition/0190f1a2-7c3d-7e4f-8a5b-0000000000f2",
        &program("0190f1a2-7c3d-7e4f-8a5b-0000000000f2", "pmtct-program"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    enrol(&server, 801, "anc-program").await;
    let pmtct = enrol(&server, 802, "pmtct-program").await;

    let id = "0190f1a2-7c3d-7e4f-8a5b-0000000000c3";
    let (status, outcome) = call(
        &fhir,
        Method::PUT,
        &format!("/QuestionnaireResponse/{id}"),
        &response(id),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{outcome}");
    assert!(
        outcome
            .to_string()
            .contains("name one with workflow-episodeOfCare")
    );

    let mut chosen = response(id);
    chosen["extension"] =
        json!([{ "url": EPISODE_EXTENSION, "valueReference": { "reference": pmtct } }]);
    let (status, stored) = call(
        &fhir,
        Method::PUT,
        &format!("/QuestionnaireResponse/{id}"),
        &chosen,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{stored}");
    assert_eq!(named_episode(&stored).as_deref(), Some(pmtct.as_str()));
}
