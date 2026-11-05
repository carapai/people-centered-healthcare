//! Submitted QuestionnaireResponses re-validated against their Questionnaire on the write path
//! (#92; ADRs 0038, 0052): the server's calculated value is stored, an incomplete or unknown
//! submission is refused, and drafts may be incomplete.

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

const QUESTIONNAIRE: &str = "0190f1a2-7c3d-7e4f-8a5b-6c7d8e9f0a1b";

fn registry() -> Registry {
    let definition = |code: &str, kind: &str| {
        json!({
            "resourceType": "SearchParameter",
            "url": format!("http://hl7.org/fhir/SearchParameter/Questionnaire-{code}"),
            "code": code, "base": ["Questionnaire"], "type": kind,
            "expression": format!("Questionnaire.{code}"),
        })
    };
    let definitions = [definition("url", "uri"), definition("version", "token")];
    let catalog = Catalog::from_definitions(definitions.iter());
    let mut registry = Registry::new(jiff::tz::Offset::UTC);
    for code in ["url", "version"] {
        registry
            .support(&catalog, "Questionnaire", code, ParamState::Active)
            .expect("supported");
    }
    registry
}

async fn app() -> Router {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "questionnaires-http".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry());
    FhirServer::new(store, "http://test/fhir", Arc::new(Unrestricted))
        .with_questionnaire_checks()
        .router()
}

async fn put(app: &Router, path: &str, body: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::put(path)
                .header("content-type", "application/fhir+json")
                .body(Body::from(body.to_owned()))
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

fn questionnaire() -> String {
    json!({
        "resourceType": "Questionnaire", "id": QUESTIONNAIRE,
        "url": "https://health.go.ug/fhir/Questionnaire/anc-visit", "version": "2.0.0",
        "status": "active",
        "item": [
            { "linkId": "age", "type": "integer", "required": true },
            { "linkId": "start", "type": "decimal" },
            { "linkId": "gain", "type": "decimal" },
            { "linkId": "now", "type": "decimal", "readOnly": true, "extension": [{
                "url": "http://hl7.org/fhir/uv/sdc/StructureDefinition/sdc-questionnaire-calculatedExpression",
                "valueExpression": { "language": "text/fhirpath",
                  "expression": "%resource.item.where(linkId = 'start').answer.value + %resource.item.where(linkId = 'gain').answer.value" }
            }] }
        ]
    })
    .to_string()
}

fn response(id: &str, status: &str, questionnaire: &str, age: bool) -> String {
    let age = if age {
        r#"{ "linkId": "age", "answer": [{ "valueInteger": 25 }] },"#
    } else {
        ""
    };
    format!(
        r#"{{ "resourceType": "QuestionnaireResponse", "id": "{id}", "status": "{status}",
             "questionnaire": "{questionnaire}", "item": [ {age}
          {{ "linkId": "start", "answer": [{{ "valueDecimal": 61.50 }}] }},
          {{ "linkId": "gain", "answer": [{{ "valueDecimal": 0.10 }}] }},
          {{ "linkId": "now", "answer": [{{ "valueDecimal": 1 }}] }}
        ] }}"#
    )
}

const ANC: &str = "https://health.go.ug/fhir/Questionnaire/anc-visit|2.0.0";

#[tokio::test]
async fn a_submission_is_stored_as_the_server_revalidated_it() {
    let app = app().await;
    let (status, _) = put(
        &app,
        &format!("/Questionnaire/{QUESTIONNAIRE}"),
        &questionnaire(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let id = "0190f1a2-7c3d-7e4f-8a5b-6c7d8e9f0a2c";
    let (status, stored) = put(
        &app,
        &format!("/QuestionnaireResponse/{id}"),
        &response(id, "completed", ANC, true),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{stored}");
    let now = stored["item"]
        .as_array()
        .expect("items")
        .iter()
        .find(|i| i["linkId"] == "now")
        .expect("now");
    assert_eq!(
        now["answer"][0]["valueDecimal"].to_string(),
        "61.60",
        "the server's calculated value wins"
    );
}

#[tokio::test]
async fn incomplete_or_unknown_submissions_are_refused_and_drafts_are_not() {
    let app = app().await;
    put(
        &app,
        &format!("/Questionnaire/{QUESTIONNAIRE}"),
        &questionnaire(),
    )
    .await;

    let id = "0190f1a2-7c3d-7e4f-8a5b-6c7d8e9f0a3d";
    let path = format!("/QuestionnaireResponse/{id}");
    let (status, outcome) = put(&app, &path, &response(id, "completed", ANC, false)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{outcome}");
    assert_eq!(outcome["issue"][0]["code"], "required");

    let (status, _) = put(&app, &path, &response(id, "in-progress", ANC, false)).await;
    assert_eq!(status, StatusCode::CREATED, "a draft may be incomplete");

    let other = "0190f1a2-7c3d-7e4f-8a5b-6c7d8e9f0a4e";
    let (status, outcome) = put(
        &app,
        &format!("/QuestionnaireResponse/{other}"),
        &response(other, "completed", "urn:unknown|1", true),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{outcome}");
}

#[tokio::test]
async fn a_transaction_does_not_bypass_the_check() {
    let app = app().await;
    put(
        &app,
        &format!("/Questionnaire/{QUESTIONNAIRE}"),
        &questionnaire(),
    )
    .await;
    let id = "0190f1a2-7c3d-7e4f-8a5b-6c7d8e9f0a5f";
    let entry: Value = serde_json::from_str(&response(id, "completed", ANC, false)).expect("json");
    let bundle = json!({
        "resourceType": "Bundle", "type": "transaction",
        "entry": [{ "resource": entry, "request": { "method": "PUT", "url": format!("QuestionnaireResponse/{id}") } }]
    });
    let response = app
        .clone()
        .oneshot(
            Request::post("/")
                .header("content-type", "application/fhir+json")
                .body(Body::from(bundle.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
