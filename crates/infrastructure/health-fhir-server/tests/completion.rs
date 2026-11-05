//! Completing a QuestionnaireResponse over HTTP (#93): a submitted response writes what its
//! Extraction Mapping produces, with one Provenance, in its own transaction. An amendment versions
//! managed targets and leaves seed targets for review, an unanswered managed target is retracted,
//! and a refused extraction keeps nothing.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::FhirServer;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const RESPONSE: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000c1";
const MAPPING: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000e1";

fn registry() -> Registry {
    let definition = |base: &str, code: &str, kind: &str| {
        json!({
            "resourceType": "SearchParameter",
            "url": format!("http://hl7.org/fhir/SearchParameter/{base}-{code}"),
            "code": code, "base": [base], "type": kind,
            "expression": format!("{base}.{code}"),
        })
    };
    let supported = [
        ("Questionnaire", "url", "uri"),
        ("Questionnaire", "version", "token"),
        ("Library", "url", "uri"),
        ("Library", "version", "token"),
        ("Library", "type", "token"),
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

async fn app() -> Router {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "completion-http".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry());
    let app = FhirServer::new(store, "http://test/fhir", Arc::new(Unrestricted))
        .with_questionnaire_checks()
        .with_extraction()
        .router();
    for (path, body) in [
        ("/Patient/0190f1a2-7c3d-7e4f-8a5b-0000000000aa", json!({ "resourceType": "Patient", "id": "0190f1a2-7c3d-7e4f-8a5b-0000000000aa" }).to_string()),
        (
            "/Encounter/0190f1a2-7c3d-7e4f-8a5b-000000000201",
            json!({ "resourceType": "Encounter", "id": "0190f1a2-7c3d-7e4f-8a5b-000000000201", "status": "in-progress" }).to_string(),
        ),
        ("/Questionnaire/0190f1a2-7c3d-7e4f-8a5b-0000000000b1", questionnaire()),
        (&format!("/Library/{MAPPING}"), mapping(None)),
    ] {
        let (status, body) = send(&app, "PUT", path, &body).await;
        assert_eq!(status, StatusCode::CREATED, "{path}: {body}");
    }
    app
}

async fn send(app: &Router, method: &str, path: &str, body: &str) -> (StatusCode, Value) {
    let current = if method == "PUT" {
        get(app, path).await.1["meta"]["versionId"]
            .as_str()
            .map(str::to_owned)
    } else {
        None
    };
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/fhir+json");
    if let Some(version) = current {
        request = request.header("if-match", format!("W/\"{version}\""));
    }
    let response = app
        .clone()
        .oneshot(
            request
                .body(if body.is_empty() {
                    Body::empty()
                } else {
                    Body::from(body.to_owned())
                })
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

async fn get(app: &Router, path: &str) -> (StatusCode, Value) {
    Box::pin(send(app, "GET", path, "")).await
}

fn questionnaire() -> String {
    json!({
        "resourceType": "Questionnaire", "id": "0190f1a2-7c3d-7e4f-8a5b-0000000000b1",
        "url": "https://health.go.ug/fhir/Questionnaire/anc-visit", "version": "1.4.0", "status": "active",
        "item": [
            { "linkId": "vitals", "type": "group", "item": [
                { "linkId": "weight", "type": "decimal" },
                { "linkId": "weight-note", "type": "string", "repeats": true }
            ] },
            { "linkId": "medications", "type": "group", "item": [
                { "linkId": "medication", "type": "group", "repeats": true, "item": [
                    { "linkId": "drug", "type": "coding" },
                    { "linkId": "instructions", "type": "string" }
                ] }
            ] },
            { "linkId": "danger-signs", "type": "boolean" }
        ]
    })
    .to_string()
}

/// The fixture mapping, with an id, and optionally the medication template given to `use_case`.
fn mapping(use_case: Option<&str>) -> String {
    let mut library: Value = serde_json::from_str(include_str!(
        "../../../contracts/health-fhir-extraction/tests/fixtures/anc-mapping.json"
    ))
    .expect("json");
    library["id"] = json!(MAPPING);
    if let Some(use_case) = use_case {
        library["contained"][1]["extension"][0]["extension"][1]["valueString"] = json!(use_case);
    }
    library.to_string()
}

fn response() -> Value {
    serde_json::from_str(include_str!(
        "../../../contracts/health-fhir-extraction/tests/fixtures/anc-response.json"
    ))
    .expect("json")
}

async fn submit(app: &Router, response: &Value) -> (StatusCode, Value) {
    send(
        app,
        "PUT",
        &format!("/QuestionnaireResponse/{RESPONSE}"),
        &response.to_string(),
    )
    .await
}

async fn provenance(app: &Router) -> Value {
    let (status, bundle) = get(app, "/Provenance").await;
    assert_eq!(status, StatusCode::OK, "{bundle}");
    let entries = bundle["entry"].as_array().cloned().unwrap_or_default();
    assert_eq!(entries.len(), 1, "one Provenance per response");
    entries[0]["resource"].clone()
}

fn targets(provenance: &Value) -> Vec<String> {
    provenance["target"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|t| t["reference"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// The resource a Provenance target names, at that version.
async fn at(app: &Router, target: &str) -> Value {
    let (status, resource) = get(app, &format!("/{target}")).await;
    assert_eq!(status, StatusCode::OK, "{target}: {resource}");
    resource
}

#[tokio::test]
async fn a_completed_response_writes_what_it_produces_with_its_provenance() {
    let app = app().await;
    let (status, stored) = submit(&app, &response()).await;
    assert_eq!(status, StatusCode::CREATED, "{stored}");
    assert_eq!(
        stored["extension"][0],
        json!({
            "url": "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/extraction-mapping",
            "valueCanonical": "https://health.go.ug/fhir/Library/anc-visit-mapping|1.0.0"
        }),
        "the response records its mapping"
    );
    let record = provenance(&app).await;
    let first = targets(&record);
    assert_eq!(first.len(), 3, "{first:?}");
    let weight = at(&app, &first[0]).await;
    assert_eq!(weight["resourceType"], "Observation");
    assert_eq!(
        weight["valueQuantity"]["value"].to_string(),
        "61.50",
        "the decimal keeps its scale"
    );
    assert_eq!(
        weight["derivedFrom"][0]["reference"],
        format!("QuestionnaireResponse/{RESPONSE}")
    );
    assert!(
        first[1].starts_with("MedicationRequest/") && first[2].starts_with("MedicationRequest/")
    );
    assert_eq!(
        record["entity"][0]["what"]["reference"],
        format!(
            "QuestionnaireResponse/{RESPONSE}/_history/{}",
            stored["meta"]["versionId"].as_str().expect("version")
        )
    );

    // Amended: the weight gets a new version, the changed drug is left for review.
    let mut amended = stored.clone();
    amended["status"] = json!("amended");
    amended["item"][0]["item"][0]["answer"][0] =
        serde_json::from_str(r#"{ "valueDecimal": 62.00 }"#).expect("json");
    amended["item"][1]["item"][0]["item"][0]["answer"][0]["valueCoding"]["code"] =
        json!("322236009");
    let (status, stored) = send(
        &app,
        "PUT",
        &format!("/QuestionnaireResponse/{RESPONSE}"),
        &amended.to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{stored}");
    let second = targets(&provenance(&app).await);
    assert_ne!(second[0], first[0], "a new version of the weight");
    assert_eq!(
        at(&app, &second[0]).await["valueQuantity"]["value"].to_string(),
        "62.00"
    );
    assert_eq!(
        second[1], first[1],
        "the seed MedicationRequest is not rewritten"
    );
    assert_eq!(
        at(&app, &second[1]).await["medication"]["concept"]["coding"][0]["code"],
        "387517004"
    );

    // The weight unanswered: the managed Observation is retracted, never deleted.
    let mut emptied = stored.clone();
    emptied["item"][0]["item"]
        .as_array_mut()
        .expect("vitals")
        .remove(0);
    let (status, body) = send(
        &app,
        "PUT",
        &format!("/QuestionnaireResponse/{RESPONSE}"),
        &emptied.to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let third = targets(&provenance(&app).await);
    let retracted = third
        .iter()
        .find(|t| t.starts_with("Observation/"))
        .expect("still named");
    assert_eq!(at(&app, retracted).await["status"], "entered-in-error");
}

#[tokio::test]
async fn a_refused_extraction_keeps_nothing() {
    let app = app().await;
    // A mapping whose medication template names a use case that does not take MedicationRequests.
    let (status, body) = send(
        &app,
        "PUT",
        &format!("/Library/{MAPPING}"),
        &mapping(Some("clinical-data.record-observation")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, outcome) = submit(&app, &response()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{outcome}");
    assert!(outcome.to_string().contains("does not accept"), "{outcome}");
    assert_eq!(
        get(&app, &format!("/QuestionnaireResponse/{RESPONSE}"))
            .await
            .0,
        StatusCode::NOT_FOUND,
        "the response is not kept"
    );
    assert_eq!(
        get(&app, "/Observation").await.1["entry"],
        Value::Null,
        "nor anything extracted"
    );
}

#[tokio::test]
async fn drafts_and_unmapped_questionnaires_produce_nothing() {
    let app = app().await;
    let mut draft = response();
    draft["status"] = json!("in-progress");
    assert_eq!(submit(&app, &draft).await.0, StatusCode::CREATED);
    assert_eq!(get(&app, "/Observation").await.1["entry"], Value::Null);

    // A Questionnaire version outside the mapping's range: no extraction.
    let mut other = questionnaire().replace("1.4.0", "2.0.0");
    other = other.replace(
        "0190f1a2-7c3d-7e4f-8a5b-0000000000b1",
        "0190f1a2-7c3d-7e4f-8a5b-0000000000b2",
    );
    assert_eq!(
        send(
            &app,
            "PUT",
            "/Questionnaire/0190f1a2-7c3d-7e4f-8a5b-0000000000b2",
            &other
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let mut unmapped = response();
    unmapped["id"] = json!("0190f1a2-7c3d-7e4f-8a5b-0000000000c3");
    unmapped["questionnaire"] = json!("https://health.go.ug/fhir/Questionnaire/anc-visit|2.0.0");
    let (status, stored) = send(
        &app,
        "PUT",
        "/QuestionnaireResponse/0190f1a2-7c3d-7e4f-8a5b-0000000000c3",
        &unmapped.to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{stored}");
    assert!(stored.get("extension").is_none());
    assert_eq!(get(&app, "/Observation").await.1["entry"], Value::Null);
}

#[tokio::test]
async fn the_dry_run_shows_what_would_be_written_and_keeps_nothing() {
    let app = app().await;
    let (status, bundle) = send(
        &app,
        "POST",
        "/QuestionnaireResponse/$extract",
        &response().to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{bundle}");
    assert_eq!(bundle["type"], "transaction");
    let entries = bundle["entry"].as_array().cloned().unwrap_or_default();
    assert_eq!(entries.len(), 3);
    assert_eq!(
        entries[0]["resource"]["valueQuantity"]["value"].to_string(),
        "61.50"
    );
    assert_eq!(entries[0]["request"]["method"], "PUT");
    assert_eq!(
        get(&app, "/Observation").await.1["entry"],
        Value::Null,
        "nothing is kept"
    );
    assert_eq!(
        get(&app, &format!("/QuestionnaireResponse/{RESPONSE}"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    // As Parameters, and without an id: the preview gives it one.
    let mut anonymous = response();
    anonymous.as_object_mut().expect("object").remove("id");
    let parameters = json!({ "resourceType": "Parameters", "parameter": [{ "name": "questionnaire-response", "resource": anonymous }] });
    let (status, bundle) = send(
        &app,
        "POST",
        "/QuestionnaireResponse/$extract",
        &parameters.to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{bundle}");
    assert_eq!(bundle["entry"].as_array().map(Vec::len), Some(3));

    // No mapping for that Questionnaire version: nothing would be written, a Bundle without entries.
    let mut unmapped = response();
    unmapped["questionnaire"] = json!("https://health.go.ug/fhir/Questionnaire/anc-visit|2.0.0");
    let (status, bundle) = send(
        &app,
        "POST",
        "/QuestionnaireResponse/$extract",
        &unmapped.to_string(),
    )
    .await;
    assert_eq!(
        (status, bundle["entry"].as_array().map(Vec::len)),
        (StatusCode::OK, None)
    );

    // Something that is not a response.
    let (status, _) = send(
        &app,
        "POST",
        "/QuestionnaireResponse/$extract",
        &json!({ "resourceType": "Patient" }).to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
