//! No external payload that fails validation is lost, and a replay after a fix re-validates and
//! writes (ADR 0051). The sink is the real FhirServer over SurrealDB (memory engine).

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use health_audit::{Agent, Auditor};
use health_fhir_search::{Registry, Unrestricted};
use health_fhir_server::FhirServer;
use health_fhir_validation::OctofhirEngine;
use health_integration::{Inbound, InboundError, Integration, Received, Replayed, Sink, SinkError};
use health_surrealdb::{
    Connection, InboundKind, MIGRATIONS, Settings, SurrealStore, Target, migrate,
};
use health_time::{FixedClock, SignedDuration, SystemClock, Timestamp};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

/// Writes through the node's FHIR API, in process.
struct FhirSink(Router);

impl Sink for FhirSink {
    async fn write(&self, resource: &Value) -> Result<(), SinkError> {
        let path = format!("/{}", resource["resourceType"].as_str().unwrap_or_default());
        let request = Request::post(path)
            .header("content-type", "application/fhir+json")
            .body(Body::from(resource.to_string()))
            .map_err(|e| SinkError::Unavailable(e.to_string()))?;
        let response = self
            .0
            .clone()
            .oneshot(request)
            .await
            .map_err(|e| SinkError::Unavailable(e.to_string()))?;
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let body = response
            .into_body()
            .collect()
            .await
            .map_err(|e| SinkError::Unavailable(e.to_string()))?
            .to_bytes();
        let outcome: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let issues = outcome["issue"]
            .as_array()
            .map(|i| {
                i.iter()
                    .filter_map(|x| x["diagnostics"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        if status.is_client_error() {
            Err(SinkError::Refused(issues))
        } else {
            Err(SinkError::Unavailable(status.to_string()))
        }
    }
}

struct Node {
    store: SurrealStore,
    fhir: Router,
    inbound: Inbound<FhirSink>,
    clock: Arc<FixedClock>,
}

async fn node() -> Node {
    let settings = Settings {
        target: Target::Memory,
        namespace: "inbound".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock))
        .with_registry(Registry::new(jiff::tz::Offset::UTC));
    let fhir = FhirServer::new(store.clone(), "http://test/fhir", Arc::new(Unrestricted)).router();
    let clock = Arc::new(FixedClock::new(
        Timestamp::from_second(1_800_000_000).expect("time"),
    ));
    let inbound = Inbound::new(
        store.clone(),
        Auditor::new("test node", Vec::new()),
        Arc::new(OctofhirEngine::new()),
        FhirSink(fhir.clone()),
        clock.clone(),
    );
    Node {
        store,
        fhir,
        inbound,
        clock,
    }
}

async fn get(router: &Router, path: &str) -> Value {
    let response = router
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).expect("request"))
        .await
        .expect("response");
    serde_json::from_slice(
        &response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes(),
    )
    .expect("json")
}

async fn count(router: &Router, resource_type: &str) -> usize {
    get(router, &format!("/{resource_type}?_count=1000")).await["entry"]
        .as_array()
        .map_or(0, Vec::len)
}

fn reviewer() -> Agent {
    Agent {
        who: Some("Practitioner/reviewer".into()),
        display: "Integration reviewer".into(),
        ..Agent::unauthenticated()
    }
}

fn ids(entries: &[health_surrealdb::InboundEntry]) -> BTreeSet<String> {
    entries.iter().map(|e| e.id.clone()).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_external_payload_that_fails_validation_is_lost() {
    let node = node().await;
    let lab = Integration::new("lab-analyser-1");

    // Valid: written.
    let valid = json!({ "resourceType": "Patient", "name": [{ "family": "Okello" }] }).to_string();
    assert_eq!(
        node.inbound.receive(&lab, &valid).await,
        Ok(Received::Accepted)
    );
    assert_eq!(count(&node.fhir, "Patient").await, 1);

    // Tier 1 failures: refused to the caller, kept verbatim in the dead letter.
    let not_json = "{\"resourceType\": \"Patient\", ";
    let unknown_element = json!({ "resourceType": "Patient", "nickname": "Oki" }).to_string();
    // Tier 2 failure (pat-1: a contact needs details or an organization), and a write the store
    // refuses (an unresolved reference): quarantined.
    let invariant =
        json!({ "resourceType": "Patient", "contact": [{ "gender": "female" }] }).to_string();
    let dangling = json!({ "resourceType": "Observation", "status": "final", "code": { "text": "Hb" }, "subject": { "reference": "Patient/01a19000-0000-7000-8000-000000000001" } }).to_string();

    let mut dead = BTreeSet::new();
    for payload in [not_json, unknown_element.as_str()] {
        let Ok(Received::DeadLettered { id, outcome }) = node.inbound.receive(&lab, payload).await
        else {
            panic!("{payload} should be dead-lettered");
        };
        assert_eq!(outcome["resourceType"], "OperationOutcome");
        assert_eq!(outcome["issue"][0]["code"], "structure");
        dead.insert(id);
    }
    let mut quarantined = BTreeSet::new();
    for payload in [invariant.as_str(), dangling.as_str()] {
        let Ok(Received::Quarantined { id, .. }) = node.inbound.receive(&lab, payload).await else {
            panic!("{payload} should be quarantined");
        };
        quarantined.insert(id);
    }
    // None of the failures reached the record.
    assert_eq!(count(&node.fhir, "Patient").await, 1);
    assert_eq!(count(&node.fhir, "Observation").await, 0);

    // Every one is kept, for review, exactly as it arrived.
    let letters = node
        .inbound
        .pending(InboundKind::DeadLetter, &reviewer(), 100)
        .await
        .expect("dead letters");
    assert_eq!(ids(&letters), dead);
    assert!(letters.iter().any(|e| e.payload == not_json));
    assert!(
        letters
            .iter()
            .all(|e| e.integration == "lab-analyser-1" && !e.issues.is_empty())
    );
    let queue = node
        .inbound
        .pending(InboundKind::Quarantine, &reviewer(), 100)
        .await
        .expect("quarantine");
    assert_eq!(ids(&queue), quarantined);
    let invariant_entry = queue
        .iter()
        .find(|e| e.payload == invariant)
        .expect("the invariant failure");
    assert!(
        invariant_entry.issues.iter().any(|i| i.contains("pat-1")),
        "{:?}",
        invariant_entry.issues
    );
    assert_eq!(
        invariant_entry.retain_until.as_second() - invariant_entry.received_at.as_second(),
        90 * 24 * 3600
    );

    // Every receipt of a failure and every listing is audited.
    let events = count(&node.fhir, "AuditEvent").await;
    assert_eq!(events, 4 + 2);
    let audit = get(&node.fhir, "/AuditEvent?_count=1000").await;
    let named: BTreeSet<String> = audit["entry"]
        .as_array()
        .expect("events")
        .iter()
        .flat_map(|e| {
            e["resource"]["entity"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|e| e["what"]["reference"].as_str().map(str::to_owned))
        .collect();
    for id in dead.iter().chain(&quarantined) {
        assert!(
            named.contains(&format!("urn:health:inbound:{id}")),
            "{id} audited"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replay_after_a_fix_revalidates_and_writes() {
    let node = node().await;
    let feed = Integration::new("dhis2");
    let patient_id = "01a19000-0000-7000-8000-000000000002";
    let dangling = json!({ "resourceType": "Observation", "status": "final", "code": { "text": "Hb" }, "subject": { "reference": format!("Patient/{patient_id}") } }).to_string();
    let Ok(Received::Quarantined { id: waiting, .. }) =
        node.inbound.receive(&feed, &dangling).await
    else {
        panic!("quarantined");
    };
    // Replaying before the fix: still failing, still pending.
    assert!(matches!(
        node.inbound.replay(&waiting, None, &reviewer()).await,
        Ok(Replayed::StillFailing(_))
    ));
    assert_eq!(
        node.inbound
            .pending(InboundKind::Quarantine, &reviewer(), 10)
            .await
            .expect("pending")
            .len(),
        1
    );

    // The fix: the patient arrives. The unchanged payload now validates and is written.
    let request = Request::put(format!("/Patient/{patient_id}"))
        .header("content-type", "application/fhir+json")
        .body(Body::from(
            json!({ "resourceType": "Patient", "id": patient_id }).to_string(),
        ))
        .expect("request");
    assert!(
        node.fhir
            .clone()
            .oneshot(request)
            .await
            .expect("response")
            .status()
            .is_success()
    );
    assert_eq!(
        node.inbound.replay(&waiting, None, &reviewer()).await,
        Ok(Replayed::Written)
    );
    assert_eq!(count(&node.fhir, "Observation").await, 1);
    assert!(
        node.inbound
            .pending(InboundKind::Quarantine, &reviewer(), 10)
            .await
            .expect("pending")
            .is_empty()
    );
    // A resolved entry cannot be replayed twice.
    assert_eq!(
        node.inbound.replay(&waiting, None, &reviewer()).await,
        Err(InboundError::NotPending(waiting.clone()))
    );

    // A dead letter replayed with the reviewer's correction is validated from tier 1 again.
    let Ok(Received::DeadLettered { id: letter, .. }) = node
        .inbound
        .receive(
            &feed,
            "{\"resourceType\":\"Patient\",\"birthDate\":\"1990-02-30x\"}",
        )
        .await
    else {
        panic!("dead-lettered");
    };
    assert!(matches!(
        node.inbound
            .replay(
                &letter,
                Some("{\"resourceType\":\"Patient\",\"nick\":1}"),
                &reviewer()
            )
            .await,
        Ok(Replayed::StillFailing(_))
    ));
    assert_eq!(
        node.inbound
            .replay(
                &letter,
                Some("{\"resourceType\":\"Patient\",\"birthDate\":\"1990-02-28\"}"),
                &reviewer()
            )
            .await,
        Ok(Replayed::Written)
    );
    assert_eq!(count(&node.fhir, "Patient").await, 2);

    // Discarding resolves an entry without writing it.
    let Ok(Received::DeadLettered { id: junk, .. }) =
        node.inbound.receive(&feed, "not even json").await
    else {
        panic!("dead-lettered");
    };
    node.inbound
        .discard(&junk, "test message from the vendor", &reviewer())
        .await
        .expect("discard");
    assert!(
        node.inbound
            .pending(InboundKind::DeadLetter, &reviewer(), 10)
            .await
            .expect("pending")
            .is_empty()
    );
    assert_eq!(
        node.inbound.discard(&junk, "again", &reviewer()).await,
        Err(InboundError::NotPending(junk))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retention_is_enforced_per_integration() {
    let node = node().await;
    let short = Integration {
        id: "short".into(),
        retention: SignedDuration::from_hours(1),
    };
    let long = Integration::new("long");
    let Ok(Received::DeadLettered { id: early, .. }) = node.inbound.receive(&short, "x").await
    else {
        panic!("kept")
    };
    let Ok(Received::DeadLettered { id: kept, .. }) = node.inbound.receive(&long, "y").await else {
        panic!("kept")
    };
    assert!(node.inbound.purge().await.expect("purge").is_empty());
    node.clock.advance(SignedDuration::from_hours(2));
    assert_eq!(
        node.inbound.purge().await.expect("purge"),
        vec![early.clone()]
    );
    let left = node
        .inbound
        .pending(InboundKind::DeadLetter, &reviewer(), 10)
        .await
        .expect("pending");
    assert_eq!(ids(&left), BTreeSet::from([kept]));
    // The purge itself is audited.
    let audit = get(&node.fhir, "/AuditEvent?_count=1000").await;
    let deletes = audit["entry"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|e| e["resource"]["action"] == "D")
        .count();
    assert_eq!(deletes, 1);
    let _ = &node.store;
}
