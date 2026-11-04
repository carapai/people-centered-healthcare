//! The terminology operations over HTTP (#90): `$expand`, `$validate-code`, `$lookup`,
//! `$subsumes` and `$translate`, served from the node's own terminology with no external server,
//! by GET and by a Parameters POST.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use health_fhir_search::Unrestricted;
use health_fhir_server::{ActiveTerminology, FhirServer};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_terminology::{
    CodeSystem, Concept, ConceptMap, Content, Filter, FilterOp, Group, Mapping, Relationship, Rule,
    Target as MapTarget, Terminology, ValueSet,
};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const SIGNS: &str = "urn:signs";
const DANGER: &str = "urn:danger";
const ICD: &str = "http://hl7.org/fhir/sid/icd-10";

fn concept(code: &str, display: &str, children: Vec<Concept>) -> Concept {
    Concept {
        code: code.into(),
        display: Some(display.into()),
        children,
        ..Concept::default()
    }
}

fn terminology() -> Terminology {
    let mut terminology = Terminology::new();
    terminology.add_code_system(CodeSystem {
        url: SIGNS.into(),
        version: Some("1.0.0".into()),
        case_sensitive: true,
        content: Content::Complete,
        concepts: vec![concept(
            "danger",
            "Danger signs",
            vec![
                concept("bleeding", "Vaginal bleeding", vec![]),
                concept("fits", "Convulsions", vec![]),
            ],
        )],
    });
    let rule = |filters: Vec<Filter>| Rule {
        system: Some(SIGNS.into()),
        filters,
        ..Rule::default()
    };
    let value_set = |url: &str, filters| ValueSet {
        url: url.into(),
        version: Some("1.0.0".into()),
        include: vec![rule(filters)],
        exclude: vec![],
        inactive: None,
        expansion: None,
    };
    terminology.add_value_set(value_set(
        DANGER,
        vec![Filter {
            property: "concept".into(),
            op: FilterOp::DescendentOf,
            value: "danger".into(),
        }],
    ));
    terminology.add_value_set(value_set(
        "urn:regex",
        vec![Filter {
            property: "code".into(),
            op: FilterOp::Regex,
            value: "^f".into(),
        }],
    ));
    terminology.add_concept_map(ConceptMap {
        url: "urn:map".into(),
        version: Some("1.0.0".into()),
        groups: vec![Group {
            source: SIGNS.into(),
            target: ICD.into(),
            mappings: vec![Mapping {
                code: "fits".into(),
                targets: vec![MapTarget {
                    code: Some("O15".into()),
                    display: Some("Eclampsia".into()),
                    relationship: Relationship::SourceIsBroaderThanTarget,
                }],
            }],
        }],
    });
    terminology
}

async fn store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "terminology-http".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

async fn serve(active: Option<ActiveTerminology>) -> Router {
    let server = FhirServer::new(store().await, "http://test/fhir", Arc::new(Unrestricted));
    match active {
        Some(active) => server.with_terminology(active),
        None => server,
    }
    .router()
}

async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
    send(app, Request::get(uri).body(Body::empty()).expect("request")).await
}

async fn post(app: &Router, uri: &str, parameter: Value) -> (StatusCode, Value) {
    let body = json!({ "resourceType": "Parameters", "parameter": parameter });
    send(
        app,
        Request::post(uri)
            .header("content-type", "application/fhir+json")
            .body(Body::from(body.to_string()))
            .expect("request"),
    )
    .await
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.expect("response");
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

fn parameter<'a>(parameters: &'a Value, name: &str) -> Option<&'a Value> {
    parameters["parameter"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == name)
}

#[tokio::test]
async fn expand_by_get_with_filter_and_paging() {
    let app = serve(Some(ActiveTerminology::new(terminology()))).await;
    let (status, body) = get(&app, &format!("/ValueSet/$expand?url={DANGER}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["resourceType"], "ValueSet");
    assert_eq!(body["version"], "1.0.0");
    assert_eq!(body["expansion"]["total"], 2);
    let codes: Vec<&str> = body["expansion"]["contains"]
        .as_array()
        .expect("contains")
        .iter()
        .filter_map(|c| c["code"].as_str())
        .collect();
    assert_eq!(codes, ["bleeding", "fits"]);

    let (_, page) = get(
        &app,
        &format!("/ValueSet/$expand?url={DANGER}&filter=conv&count=1"),
    )
    .await;
    assert_eq!(page["expansion"]["total"], 1);
    assert_eq!(page["expansion"]["contains"][0]["display"], "Convulsions");
}

#[tokio::test]
async fn validate_code_by_post_with_a_coding() {
    let app = serve(Some(ActiveTerminology::new(terminology()))).await;
    let coding = |display: &str| {
        json!([
            { "name": "url", "valueUri": DANGER },
            { "name": "coding", "valueCoding": { "system": SIGNS, "code": "fits", "display": display } }
        ])
    };
    let (status, valid) = post(&app, "/ValueSet/$validate-code", coding("Convulsions")).await;
    assert_eq!(status, StatusCode::OK, "{valid}");
    assert_eq!(
        parameter(&valid, "result").expect("result")["valueBoolean"],
        true
    );

    let (_, wrong) = post(&app, "/ValueSet/$validate-code", coding("Seizure")).await;
    assert_eq!(
        parameter(&wrong, "result").expect("result")["valueBoolean"],
        false
    );
    assert_eq!(
        parameter(&wrong, "display").expect("display")["valueString"],
        "Convulsions"
    );

    let (_, outside) = get(
        &app,
        &format!("/ValueSet/$validate-code?url={DANGER}&system={SIGNS}&code=danger"),
    )
    .await;
    assert_eq!(
        parameter(&outside, "result").expect("result")["valueBoolean"],
        false
    );

    let (_, in_system) = get(
        &app,
        &format!("/CodeSystem/$validate-code?url={SIGNS}&code=danger"),
    )
    .await;
    assert_eq!(
        parameter(&in_system, "result").expect("result")["valueBoolean"],
        true
    );
}

#[tokio::test]
async fn lookup_subsumes_and_translate() {
    let app = serve(Some(ActiveTerminology::new(terminology()))).await;
    let (status, found) = get(
        &app,
        &format!("/CodeSystem/$lookup?system={SIGNS}&code=fits"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{found}");
    assert_eq!(
        parameter(&found, "display").expect("display")["valueString"],
        "Convulsions"
    );
    assert_eq!(
        parameter(&found, "version").expect("version")["valueString"],
        "1.0.0"
    );
    let parent = found["parameter"]
        .as_array()
        .expect("parameters")
        .iter()
        .find(|p| p["name"] == "property" && p["part"][0]["valueCode"] == "parent")
        .expect("a parent");
    assert_eq!(parent["part"][1]["valueCode"], "danger");

    let (_, outcome) = get(
        &app,
        &format!("/CodeSystem/$subsumes?system={SIGNS}&codeA=danger&codeB=fits"),
    )
    .await;
    assert_eq!(
        parameter(&outcome, "outcome").expect("outcome")["valueCode"],
        "subsumes"
    );

    let (_, translated) = post(
        &app,
        "/ConceptMap/$translate",
        json!([
            { "name": "url", "valueUri": "urn:map" },
            { "name": "sourceCoding", "valueCoding": { "system": SIGNS, "code": "fits" } }
        ]),
    )
    .await;
    assert_eq!(
        parameter(&translated, "result").expect("result")["valueBoolean"],
        true
    );
    let matched = parameter(&translated, "match").expect("a match");
    assert_eq!(
        matched["part"][0]["valueCode"],
        "source-is-broader-than-target"
    );
    assert_eq!(matched["part"][2]["valueCoding"]["code"], "O15");
}

#[tokio::test]
async fn what_cannot_be_answered_offline_is_an_error_not_an_empty_answer() {
    let app = serve(Some(ActiveTerminology::new(terminology()))).await;
    let (status, outcome) = get(&app, "/ValueSet/$expand?url=urn:unknown").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{outcome}");
    assert_eq!(outcome["resourceType"], "OperationOutcome");
    let (status, outcome) = get(&app, "/ValueSet/$expand?url=urn:regex").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{outcome}");
    assert_eq!(outcome["issue"][0]["code"], "not-supported");
    let (status, _) = get(&app, "/ValueSet/$expand").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = get(
        &app,
        &format!("/CodeSystem/$lookup?system={SIGNS}&code=nausea"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&app, &format!("/ValueSet/$expand?url={DANGER}&count=many")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let without = serve(None).await;
    let (status, _) = get(&without, &format!("/ValueSet/$expand?url={DANGER}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn replacing_the_terminology_takes_effect_at_once() {
    let active = ActiveTerminology::new(Terminology::new());
    let app = serve(Some(active.clone())).await;
    let (status, _) = get(&app, &format!("/ValueSet/$expand?url={DANGER}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    active.replace(terminology());
    let (status, _) = get(&app, &format!("/ValueSet/$expand?url={DANGER}")).await;
    assert_eq!(status, StatusCode::OK);
}
