//! Every write and every access to a sensitive record yields an AuditEvent (#66), written by the
//! audit module in the same transaction as the access.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use health_audit::{Agent, Auditor};
use health_fhir_search::{Registry, Unrestricted};
use health_fhir_server::FhirServer;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const ACT: &str = "http://terminology.hl7.org/CodeSystem/v3-ActCode";

async fn store() -> SurrealStore {
    let settings = Settings {
        target: Target::Memory,
        namespace: "audit".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
        .with_registry(Registry::new(jiff::tz::Offset::UTC))
}

fn agent(break_glass: bool) -> Agent {
    Agent {
        who: Some("Practitioner/p9".into()),
        display: "Dr Nakato".into(),
        client: Some("health-client".into()),
        network: Some("10.0.0.7".into()),
        purpose: vec!["TREAT".into()],
        break_glass,
    }
}

/// The FHIR routes as the authentication layer would serve them: the agent is a request
/// extension.
fn app(store: &SurrealStore, agent: Agent) -> Router {
    let server =
        FhirServer::new(store.clone(), "http://test/fhir", Arc::new(Unrestricted)).with_audit(
            Auditor::new("test node E1", vec![(ACT.into(), "HIV".into())]),
        );
    server.router().layer(Extension(agent))
}

async fn send(app: &Router, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if body.is_some() {
        request = request.header("content-type", "application/fhir+json");
    }
    let body = body.map_or(Body::empty(), |b| Body::from(b.to_string()));
    let response = app
        .clone()
        .oneshot(request.body(body).expect("request"))
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

/// Every AuditEvent so far, read through an unaudited server so reading them adds none.
async fn events(store: &SurrealStore) -> Vec<Value> {
    let reader =
        FhirServer::new(store.clone(), "http://test/fhir", Arc::new(Unrestricted)).router();
    let (status, bundle) = send(&reader, "GET", "/AuditEvent?_count=1000", None).await;
    assert_eq!(status, StatusCode::OK, "{bundle}");
    let mut events: Vec<Value> = bundle["entry"]
        .as_array()
        .map(|entries| entries.iter().map(|e| e["resource"].clone()).collect())
        .unwrap_or_default();
    events.sort_by(|a, b| a["recorded"].as_str().cmp(&b["recorded"].as_str()));
    events
}

fn summary(event: &Value) -> (String, String, Vec<String>) {
    let code = event["code"]["coding"][0]["code"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let action = event["action"].as_str().unwrap_or_default().to_owned();
    let what = event["entity"]
        .as_array()
        .map(|e| {
            e.iter()
                .filter_map(|x| x["what"]["reference"].as_str())
                .map(|r| r.split('/').take(2).collect::<Vec<_>>().join("/"))
                .collect()
        })
        .unwrap_or_default();
    (code, action, what)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_write_yields_an_audit_event_in_its_transaction() {
    let store = store().await;
    let app = app(&store, agent(false));
    let (status, patient) = send(
        &app,
        "POST",
        "/Patient",
        Some(json!({ "resourceType": "Patient" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = patient["id"].as_str().expect("id").to_owned();
    let version = patient["meta"]["versionId"]
        .as_str()
        .expect("version")
        .to_owned();

    let created = events(&store).await;
    assert_eq!(created.len(), 1);
    let event = &created[0];
    assert_eq!(
        summary(event),
        ("create".into(), "C".into(), vec![format!("Patient/{id}")])
    );
    assert_eq!(
        event["entity"][0]["what"]["reference"],
        format!("Patient/{id}/_history/{version}")
    );
    assert_eq!(event["patient"]["reference"], format!("Patient/{id}"));
    assert_eq!(event["agent"][0]["who"]["reference"], "Practitioner/p9");
    assert_eq!(event["agent"][0]["networkString"], "10.0.0.7");
    assert_eq!(event["authorization"][0]["coding"][0]["code"], "TREAT");
    assert_eq!(event["source"]["observer"]["display"], "test node E1");

    // Update, conditional create, a transaction Bundle and a delete: one event per write.
    let mut updated = patient.clone();
    updated["active"] = json!(true);
    let request = Request::put(format!("/Patient/{id}"))
        .header("content-type", "application/fhir+json")
        .header("if-match", format!("W/\"{version}\""))
        .body(Body::from(updated.to_string()))
        .expect("request");
    assert_eq!(
        app.clone()
            .oneshot(request)
            .await
            .expect("response")
            .status(),
        StatusCode::OK
    );
    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            { "fullUrl": "urn:uuid:7f2f6a3e-1e4b-4f57-9a3c-0b0b6f0e7a01", "resource": { "resourceType": "Patient" }, "request": { "method": "POST", "url": "Patient" } },
            { "resource": { "resourceType": "Observation", "status": "final", "code": { "text": "x" }, "subject": { "reference": "urn:uuid:7f2f6a3e-1e4b-4f57-9a3c-0b0b6f0e7a01" } }, "request": { "method": "POST", "url": "Observation" } },
        ],
    });
    let (status, body) = send(&app, "POST", "/", Some(bundle)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _) = send(&app, "DELETE", &format!("/Patient/{id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let actions: Vec<String> = events(&store)
        .await
        .iter()
        .map(|e| e["action"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(actions, ["C", "U", "C", "C", "D"]);

    // A write that fails rolls back its event with it: nothing is recorded as done that was not.
    let dangling = json!({ "resourceType": "Observation", "status": "final", "code": { "text": "x" }, "subject": { "reference": "Patient/01a19000-0000-7000-8000-000000000000" } });
    let (status, _) = send(&app, "POST", "/Observation", Some(dangling)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(events(&store).await.len(), 5);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_access_to_a_sensitive_record_yields_an_audit_event() {
    let store = store().await;
    let app = app(&store, agent(false));
    let (_, patient) = send(
        &app,
        "POST",
        "/Patient",
        Some(json!({ "resourceType": "Patient" })),
    )
    .await;
    let subject = format!("Patient/{}", patient["id"].as_str().expect("id"));
    let observation = |labels: Value| {
        let mut o = json!({ "resourceType": "Observation", "status": "final", "code": { "text": "x" }, "subject": { "reference": subject } });
        if labels.as_array().is_some_and(|l| !l.is_empty()) {
            o["meta"] = json!({ "security": labels });
        }
        o
    };
    let (_, plain) = send(&app, "POST", "/Observation", Some(observation(json!([])))).await;
    let (_, hiv) = send(
        &app,
        "POST",
        "/Observation",
        Some(observation(json!([{ "system": ACT, "code": "HIV" }]))),
    )
    .await;
    let writes = events(&store).await.len();
    assert_eq!(writes, 3);
    let (plain, hiv) = (
        plain["id"].as_str().expect("id").to_owned(),
        hiv["id"].as_str().expect("id").to_owned(),
    );
    let hiv_version = events(&store).await[2]["entity"][0]["what"]["reference"]
        .as_str()
        .expect("what")
        .rsplit('/')
        .next()
        .expect("v")
        .to_owned();

    // Reading an ordinary record is not audited; reading a sensitive one is, however it is read.
    assert_eq!(
        send(&app, "GET", &format!("/Observation/{plain}"), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(events(&store).await.len(), writes);
    for (path, code) in [
        (format!("/Observation/{hiv}"), "read"),
        (
            format!("/Observation/{hiv}/_history/{hiv_version}"),
            "vread",
        ),
        ("/Observation".to_owned(), "search-type"),
        (format!("/Observation/{hiv}/_history"), "history"),
    ] {
        let (status, body) = send(&app, "GET", &path, None).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        let all = events(&store).await;
        let last = all.last().expect("an event");
        let (got, action, what) = summary(last);
        assert_eq!(got, code, "{path}");
        assert_eq!(action, if code == "search-type" { "E" } else { "R" });
        // Only the sensitive record is named, with the label that made it so.
        assert_eq!(what, vec![format!("Observation/{hiv}")], "{path}");
        assert_eq!(
            last["entity"][0]["securityLabel"][0]["coding"][0]["code"],
            "HIV"
        );
    }
    let after_reads = events(&store).await.len();
    assert_eq!(after_reads, writes + 4);

    // A Bundle GET of the sensitive record is audited too.
    let batch = json!({ "resourceType": "Bundle", "type": "batch", "entry": [{ "request": { "method": "GET", "url": format!("Observation/{hiv}") } }] });
    let (status, _) = send(&app, "POST", "/", Some(batch)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(events(&store).await.len(), after_reads + 1);

    // Under break-glass every read is audited, naming every record, with BTG.
    let glass = app_with_glass(&store);
    assert_eq!(
        send(&glass, "GET", &format!("/Observation/{plain}"), None)
            .await
            .0,
        StatusCode::OK
    );
    let all = events(&store).await;
    let last = all.last().expect("event");
    assert_eq!(summary(last).2, vec![format!("Observation/{plain}")]);
    assert_eq!(last["authorization"][1]["coding"][0]["code"], "BTG");
    assert_eq!(last["meta"]["security"][0]["code"], "BTG");
}

fn app_with_glass(store: &SurrealStore) -> Router {
    app(store, agent(true))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_an_agent_the_access_is_recorded_as_unauthenticated() {
    let store = store().await;
    let server = FhirServer::new(store.clone(), "http://test/fhir", Arc::new(Unrestricted))
        .with_audit(Auditor::new("test node E1", Vec::new()))
        .router();
    let (status, _) = send(
        &server,
        "POST",
        "/Patient",
        Some(json!({ "resourceType": "Patient" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let all = events(&store).await;
    assert_eq!(
        all[0]["agent"][0]["who"]["display"],
        "unauthenticated request"
    );
    assert!(all[0]["agent"][0]["who"].get("reference").is_none());
}
