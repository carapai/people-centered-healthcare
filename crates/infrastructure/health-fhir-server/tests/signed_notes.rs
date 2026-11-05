//! Signed notes over the FHIR API (#107; synchronization.md): a draft Composition is edited and
//! signed; once signed, an in-place edit is rejected (409) whether by PUT or in a transaction, it
//! is never deleted, and it may only be marked entered in error with its content unchanged.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use health_fhir_search::Unrestricted;
use health_fhir_server::FhirServer;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const NOTE: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000901";

async fn app() -> Router {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "signed-notes".into(),
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
    .router()
}

async fn send(
    app: &Router,
    method: Method,
    path: &str,
    body: Option<&Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method.clone())
        .uri(path)
        .header("content-type", "application/fhir+json");
    if method != Method::POST {
        let current = app
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).expect("request"))
            .await
            .expect("response");
        if current.status() == StatusCode::OK
            && let Some(etag) = current.headers().get("etag").cloned()
        {
            request = request.header("if-match", etag);
        }
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

fn note(status: &str, plan: &str) -> Value {
    json!({
        "resourceType": "Composition", "id": NOTE, "status": status,
        "type": { "coding": [{ "system": "http://loinc.org", "code": "11506-3", "display": "Progress note" }] },
        "date": "2026-11-05T10:00:00Z", "author": [{ "display": "clinician-1" }], "title": "Progress note",
        "section": [{ "title": "Plan", "text": { "status": "generated", "div": format!("<div xmlns=\"http://www.w3.org/1999/xhtml\">{plan}</div>") } }]
    })
}

fn path() -> String {
    format!("/Composition/{NOTE}")
}

#[tokio::test]
async fn a_signed_note_is_never_edited_in_place() {
    let app = app().await;
    assert_eq!(
        send(
            &app,
            Method::PUT,
            &path(),
            Some(&note("preliminary", "Review in a week"))
        )
        .await
        .0,
        StatusCode::CREATED
    );
    assert_eq!(
        send(
            &app,
            Method::PUT,
            &path(),
            Some(&note("preliminary", "Review in two weeks"))
        )
        .await
        .0,
        StatusCode::OK,
        "a draft is edited"
    );
    assert_eq!(
        send(
            &app,
            Method::PUT,
            &path(),
            Some(&note("final", "Review in two weeks"))
        )
        .await
        .0,
        StatusCode::OK,
        "and signed"
    );

    let (status, outcome) = send(
        &app,
        Method::PUT,
        &path(),
        Some(&note("final", "Review tomorrow")),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{outcome}");
    assert_eq!(outcome["issue"][0]["code"], "business-rule");
    assert!(
        outcome["issue"][0]["diagnostics"]
            .as_str()
            .unwrap_or_default()
            .contains("amend it")
    );

    // Nor in a transaction.
    let bundle = json!({ "resourceType": "Bundle", "type": "transaction", "entry": [{
        "resource": note("final", "Review tomorrow"), "request": { "method": "PUT", "url": format!("Composition/{NOTE}") } }] });
    let (status, outcome) = send(&app, Method::POST, "/", Some(&bundle)).await;
    assert_ne!(status, StatusCode::OK, "{outcome}");

    // Never deleted.
    assert_eq!(
        send(&app, Method::DELETE, &path(), None).await.0,
        StatusCode::CONFLICT
    );

    // Only marked entered in error, as it was.
    assert_eq!(
        send(
            &app,
            Method::PUT,
            &path(),
            Some(&note("entered-in-error", "Review tomorrow"))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (status, withdrawn) = send(
        &app,
        Method::PUT,
        &path(),
        Some(&note("entered-in-error", "Review in two weeks")),
    )
    .await;
    assert_eq!(
        (status, withdrawn["status"].as_str()),
        (StatusCode::OK, Some("entered-in-error")),
        "{withdrawn}"
    );
    let (_, stored) = send(&app, Method::GET, &path(), None).await;
    assert_eq!(
        stored["section"][0]["text"]["div"],
        note("final", "Review in two weeks")["section"][0]["text"]["div"]
    );
}

#[tokio::test]
async fn a_draft_may_still_be_deleted() {
    let app = app().await;
    assert_eq!(
        send(
            &app,
            Method::PUT,
            &path(),
            Some(&note("preliminary", "Draft"))
        )
        .await
        .0,
        StatusCode::CREATED
    );
    assert_eq!(
        send(&app, Method::DELETE, &path(), None).await.0,
        StatusCode::NO_CONTENT
    );
}
