//! Clinical notes over HTTP (#107): a draft is edited and signed, a signed note is amended
//! (never edited), each signature and amendment has its Provenance, and the patient's notes are
//! listed at their own facility only.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::{Extension, Router};
use health_access_policy::{Permissions, Subject};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::{DOCUMENT_PERMISSION, FhirServer};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const PATIENT: &str = "0190f1a2-7c3d-7e4f-8a5b-0000000000aa";
const NOTE: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000901";
const AMENDMENT: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000902";

fn registry() -> Registry {
    let definitions = [
        json!({ "resourceType": "SearchParameter", "url": "http://hl7.org/fhir/SearchParameter/Composition-patient", "code": "patient",
        "base": ["Composition"], "type": "reference", "expression": "Composition.subject", "target": ["Patient"] }),
    ];
    let catalog = Catalog::from_definitions(definitions.iter());
    let mut registry = Registry::new(jiff::tz::Offset::UTC);
    registry
        .support(&catalog, "Composition", "patient", ParamState::Active)
        .expect("patient");
    registry
}

async fn server() -> FhirServer {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "notes-http".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let server = FhirServer::new(
        SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry()),
        "http://test/fhir",
        Arc::new(Unrestricted),
    );
    let (status, _) = call(
        &server.clone().router(),
        Method::PUT,
        &format!("/Patient/{PATIENT}"),
        Some(json!({ "resourceType": "Patient", "id": PATIENT })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    server
}

fn notes(server: &FhirServer, account: &str, facility: &str, permissions: &[&str]) -> Router {
    server.notes_router().layer(Extension(Subject {
        account: account.into(),
        facility: facility.into(),
        permissions: Permissions(permissions.iter().map(|p| (*p).to_owned()).collect()),
        programs: BTreeSet::new(),
        purpose: Some("TREAT".into()),
        break_glass: None,
    }))
}

async fn call(
    app: &Router,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
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

fn soap(plan: &str) -> Value {
    json!([
        { "title": "Subjective", "text": "Fever for three days" },
        { "title": "Objective", "text": "T 38.6" },
        { "title": "Assessment", "text": "Malaria" },
        { "title": "Plan", "text": plan }
    ])
}

#[tokio::test]
async fn a_note_is_drafted_signed_and_amended_never_edited() {
    let server = server().await;
    let app = notes(&server, "clinician-1", "KAM013", &[DOCUMENT_PERMISSION]);
    let draft = json!({ "id": NOTE, "kind": "progress", "patient": format!("Patient/{PATIENT}"), "sections": soap("RDT") });
    let (status, drafted) = call(&app, Method::POST, "/notes", Some(draft.clone())).await;
    assert_eq!(
        (status, drafted["status"].as_str()),
        (StatusCode::CREATED, Some("preliminary")),
        "{drafted}"
    );
    assert_eq!(
        call(&app, Method::POST, "/notes", Some(draft)).await.0,
        StatusCode::CONFLICT,
        "drafted once"
    );

    let (status, edited) = call(
        &app,
        Method::POST,
        &format!("/notes/{NOTE}/edit"),
        Some(json!({ "sections": soap("RDT, then treat") })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    let (status, signed) = call(&app, Method::POST, &format!("/notes/{NOTE}/sign"), None).await;
    assert_eq!(
        (status, signed["status"].as_str()),
        (StatusCode::OK, Some("final")),
        "{signed}"
    );
    assert_eq!(signed["signed"]["by"], "clinician-1");

    // Signed: never edited, signed again or cancelled.
    assert_eq!(
        call(
            &app,
            Method::POST,
            &format!("/notes/{NOTE}/edit"),
            Some(json!({ "sections": soap("Admit") }))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(&app, Method::POST, &format!("/notes/{NOTE}/sign"), None)
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(&app, Method::POST, &format!("/notes/{NOTE}/cancel"), None)
            .await
            .0,
        StatusCode::CONFLICT
    );

    // Amended by another clinician.
    let colleague = notes(&server, "clinician-2", "KAM013", &[DOCUMENT_PERMISSION]);
    let amendment = json!({ "id": AMENDMENT, "relation": "replaces", "sections": [{ "title": "Plan", "text": "Admit for IV artesunate" }] });
    let (status, amended) = call(
        &colleague,
        Method::POST,
        &format!("/notes/{NOTE}/amend"),
        Some(amendment),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{amended}");
    assert_eq!(
        (amended["status"].as_str(), amended["author"].as_str()),
        (Some("amended"), Some("clinician-2"))
    );
    assert_eq!(
        amended["relatesTo"],
        json!({ "relation": "replaces", "note": NOTE })
    );

    let (status, listed) = call(
        &app,
        Method::GET,
        &format!("/patients/{PATIENT}/notes"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let listed = listed.as_array().cloned().unwrap_or_default();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0]["id"], AMENDMENT, "latest first");
    assert_eq!(
        listed[1]["sections"][3]["text"], "RDT, then treat",
        "the original is unchanged"
    );

    // One Provenance for the signature, one for the amendment, naming the original's version.
    let (_, provenances) = call(&server.clone().router(), Method::GET, "/Provenance", None).await;
    let entries = provenances["entry"].as_array().cloned().unwrap_or_default();
    assert_eq!(entries.len(), 2, "{provenances}");
    let amendment_record = entries
        .iter()
        .map(|e| &e["resource"])
        .find(|r| r["entity"].is_array())
        .expect("the amendment's");
    assert!(
        amendment_record["entity"][0]["what"]["reference"]
            .as_str()
            .unwrap_or_default()
            .starts_with(&format!("Composition/{NOTE}/_history/"))
    );
}

#[tokio::test]
async fn permissions_facilities_and_empty_notes_are_checked() {
    let server = server().await;
    let app = notes(&server, "clinician-1", "KAM013", &[DOCUMENT_PERMISSION]);
    let draft = json!({ "id": NOTE, "kind": "discharge", "patient": format!("Patient/{PATIENT}"), "sections": [{ "title": "Course", "text": "Uneventful" }] });
    assert_eq!(
        call(
            &notes(&server, "cashier-1", "KAM013", &["payments.cash"]),
            Method::POST,
            "/notes",
            Some(draft.clone())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let empty = json!({ "id": AMENDMENT, "kind": "letter", "patient": format!("Patient/{PATIENT}"), "sections": [{ "title": "Note", "text": " " }] });
    assert_eq!(
        call(&app, Method::POST, "/notes", Some(empty)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        call(&app, Method::POST, "/notes", Some(draft)).await.0,
        StatusCode::CREATED
    );
    let elsewhere = notes(&server, "clinician-9", "MBR001", &[DOCUMENT_PERMISSION]);
    assert_eq!(
        call(
            &elsewhere,
            Method::POST,
            &format!("/notes/{NOTE}/sign"),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &elsewhere,
            Method::GET,
            &format!("/patients/{PATIENT}/notes"),
            None
        )
        .await
        .1,
        json!([])
    );
    let amendment = json!({ "id": AMENDMENT, "relation": "appends", "sections": [{ "title": "Note", "text": "x" }] });
    assert_eq!(
        call(
            &app,
            Method::POST,
            &format!("/notes/{NOTE}/amend"),
            Some(amendment)
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "only a signed note"
    );
    let (status, cancelled) =
        call(&app, Method::POST, &format!("/notes/{NOTE}/cancel"), None).await;
    assert_eq!(
        (status, cancelled["status"].as_str()),
        (StatusCode::OK, Some("cancelled"))
    );
}
