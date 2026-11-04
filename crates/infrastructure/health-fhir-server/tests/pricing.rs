//! Price Books and rules over HTTP (#120): drafted by finance staff, approved by someone else,
//! projected as ChargeItemDefinitions, and charges quoted with FHIRPath applicability evaluated
//! over each line's context.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{Permissions, Subject};
use health_fhir_search::Unrestricted;
use health_fhir_server::{
    FhirServer, PRICING_APPROVE_PERMISSION, PRICING_PERMISSION, PRICING_QUOTE_PERMISSION,
    PricingConfig,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn server() -> FhirServer {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "pricing-http".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    FhirServer::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        "http://test/fhir",
        Arc::new(Unrestricted),
    )
    .with_pricing(PricingConfig {
        currency: "UGX".into(),
    })
}

fn as_(server: &FhirServer, account: &str, permissions: &[&str]) -> Router {
    server.pricing_router().layer(Extension(Subject {
        account: account.into(),
        facility: "KAM013".into(),
        permissions: Permissions(permissions.iter().map(|p| (*p).to_owned()).collect()),
        programs: BTreeSet::new(),
        purpose: None,
        break_glass: None,
    }))
}

async fn send(app: &Router, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let response = app
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

fn book() -> Value {
    json!({
        "id": "kam",
        "scope": { "level": "facility", "id": "KAM013" },
        "currency": "UGX",
        "effective": { "from": "2026-01-01T00:00:00Z" },
        "entries": [{ "item": "consult", "unit": "10000" }]
    })
}

fn rule(applicability: &str) -> Value {
    json!({
        "id": "after-hours",
        "stage": "surcharge",
        "priority": 1,
        "applicability": applicability,
        "adjustment": { "kind": "percent", "percent": "20" },
        "effective": { "from": "2026-01-01T00:00:00Z" }
    })
}

fn quote(context: Value) -> Value {
    json!({
        "request": {
            "service_time": "2026-11-04T20:00:00Z",
            "currency": "UGX",
            "minor_units": 0,
            "scopes": [{ "level": "facility", "id": "KAM013" }],
            "lines": [{ "id": "L1", "item": "consult", "quantity": "1" }]
        },
        "context": context
    })
}

fn after_hours() -> Value {
    json!({ "L1": { "resourceType": "Parameters", "parameter": [
        { "name": "afterHours", "valueBoolean": true }
    ] } })
}

#[tokio::test]
async fn finance_drafts_another_approves_and_charges_are_quoted() {
    let server = server().await;
    let author = as_(&server, "author-1", &[PRICING_PERMISSION]);
    let approver = as_(
        &server,
        "finance-1",
        &[PRICING_PERMISSION, PRICING_APPROVE_PERMISSION],
    );
    let both = as_(
        &server,
        "author-1",
        &[PRICING_PERMISSION, PRICING_APPROVE_PERMISSION],
    );
    let cashier = as_(&server, "cashier-1", &[PRICING_QUOTE_PERMISSION]);

    let (status, drafted) = send(&author, "POST", "/books", Some(book())).await;
    assert_eq!(status, StatusCode::OK, "{drafted}");
    assert_eq!(drafted["version"], 1);
    let (status, _) = send(&author, "POST", "/books/kam/1/approve", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "drafting is not approving");
    let (status, refused) = send(&both, "POST", "/books/kam/1/approve", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{refused}");
    let (status, approved) = send(&approver, "POST", "/books/kam/1/approve", None).await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(approved["approval"]["status"], "approved");

    let expression = "Parameters.parameter.where(name = 'afterHours').valueBoolean";
    send(&author, "POST", "/rules", Some(rule(expression))).await;
    let (status, _) = send(&approver, "POST", "/rules/after-hours/1/approve", None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, priced) = send(&cashier, "POST", "/quote", Some(quote(after_hours()))).await;
    assert_eq!(status, StatusCode::OK, "{priced}");
    assert_eq!(priced["lines"][0]["gross"], "12000");
    assert_eq!(
        priced["rules"],
        json!([{ "id": "after-hours", "version": 1 }])
    );
    let (_, daytime) = send(&cashier, "POST", "/quote", Some(quote(json!({})))).await;
    assert_eq!(
        daytime["lines"][0]["gross"], "10000",
        "no context: the rule does not apply"
    );

    let (status, _) = send(&cashier, "GET", "/books", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (_, books) = send(&author, "GET", "/books", None).await;
    assert_eq!(books.as_array().map(Vec::len), Some(1));

    let (status, _) = send(
        &approver,
        "POST",
        "/books/kam/1/retire",
        Some(json!({ "to": "2020-01-01T00:00:00Z" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the past is not rewritten"
    );
}

#[tokio::test]
async fn an_applicability_that_is_not_boolean_refuses_the_quote() {
    let server = server().await;
    let author = as_(&server, "author-1", &[PRICING_PERMISSION]);
    let approver = as_(&server, "finance-1", &[PRICING_APPROVE_PERMISSION]);
    send(&author, "POST", "/books", Some(book())).await;
    send(&approver, "POST", "/books/kam/1/approve", None).await;
    send(
        &author,
        "POST",
        "/rules",
        Some(rule("Parameters.parameter.name")),
    )
    .await;
    send(&approver, "POST", "/rules/after-hours/1/approve", None).await;
    let quoter = as_(&server, "cashier-1", &[PRICING_QUOTE_PERMISSION]);
    let (status, refused) = send(&quoter, "POST", "/quote", Some(quote(after_hours()))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");
}
