//! Check-in and the queues it fills (#102) over SurrealDB 3.3.5 (in-memory): a checked-in patient
//! joins the triage queue, once however often the event is replayed; moving and ending the
//! Encounter move and close the queue entries; one open Encounter per patient per service point.

use std::sync::Arc;

use health_encounters::{CheckIn, Class, EncounterStatus};
use health_encounters_app::{Encounters, EncountersError};
use health_identifiers::ResourceId;
use health_patient_flow::{QueueStatus, Step};
use health_patient_flow_app::Flow;
use health_persistence::{ResourceType, Store, Transaction};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use serde_json::Value;

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

async fn store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "encounters".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

fn request(id: &str, patient: &str, class: Class, when: &str) -> CheckIn {
    CheckIn {
        id: id.into(),
        patient: patient.into(),
        facility: "KAM013".into(),
        service_point: "triage".into(),
        class,
        by: "reception-1".into(),
        at: at(when),
    }
}

const E1: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000101";
const E2: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000102";
const E3: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000103";

async fn fhir(store: &SurrealStore, id: &str) -> Value {
    let kind: ResourceType = "Encounter".parse().expect("type");
    let id = ResourceId::from_uuid(id.parse().expect("uuid")).expect("id");
    let mut tx = store.begin().await.expect("begin");
    let head = tx.read(&kind, id).await.expect("read").expect("projected");
    let issues = health_fhir_validation::validate_structure(head.json.as_str());
    assert!(issues.is_empty(), "a valid R5 Encounter: {issues:?}");
    serde_json::from_str(head.json.as_str()).expect("json")
}

#[tokio::test]
async fn a_checked_in_patient_appears_in_the_triage_queue_once() {
    let store = store().await;
    let encounters = Encounters::new(store.clone());
    let flow = Flow::new(store.clone());

    let (encounter, event) = encounters
        .check_in(&request(
            E1,
            "Patient/p-1",
            Class::Ambulatory,
            "2026-11-05T08:00:00Z",
        ))
        .await
        .expect("checked in");
    assert_eq!(encounter.status, EncounterStatus::InProgress);
    let projected = fhir(&store, E1).await;
    assert_eq!(projected["status"], "in-progress");
    assert_eq!(projected["subject"]["reference"], "Patient/p-1");
    assert_eq!(projected["meta"]["tag"][0]["code"], "KAM013");
    assert_eq!(projected["location"][0]["location"]["display"], "triage");

    for _ in 0..3 {
        flow.follow(&event).await.expect("followed");
    }
    let triage = flow.queue("triage").await.expect("queue");
    assert_eq!(triage.len(), 1, "a replayed event queues no one twice");
    assert_eq!(triage[0].patient, "Patient/p-1");
    assert_eq!(triage[0].status, QueueStatus::Waiting);

    assert_eq!(
        encounters
            .check_in(&request(
                E2,
                "Patient/p-1",
                Class::Ambulatory,
                "2026-11-05T08:05:00Z"
            ))
            .await,
        Err(EncountersError::AlreadyOpen("triage".into())),
        "one open encounter per patient at a service point"
    );
}

#[tokio::test]
async fn moving_and_ending_the_encounter_move_and_close_the_queue_entries() {
    let store = store().await;
    let encounters = Encounters::new(store.clone());
    let flow = Flow::new(store.clone());
    let (_, started) = encounters
        .check_in(&request(
            E1,
            "Patient/p-1",
            Class::Ambulatory,
            "2026-11-05T08:00:00Z",
        ))
        .await
        .expect("checked in");
    flow.follow(&started).await.expect("followed");
    let waiting = flow.queue("triage").await.expect("queue")[0].clone();
    flow.step(
        &waiting.id,
        &Step::Call {
            by: "nurse-1".into(),
        },
        at("2026-11-05T08:10:00Z"),
    )
    .await
    .expect("called");

    let (moved, event) = encounters
        .move_to(E1, "consultation", at("2026-11-05T08:20:00Z"))
        .await
        .expect("moved");
    assert_eq!(moved.service_point, "consultation");
    flow.follow(&event).await.expect("followed");
    flow.follow(&event).await.expect("replayed");
    assert!(flow.queue("triage").await.expect("queue").is_empty());
    assert_eq!(flow.queue("consultation").await.expect("queue").len(), 1);

    let (done, event) = encounters
        .transition(E1, EncounterStatus::Completed, at("2026-11-05T09:00:00Z"))
        .await
        .expect("completed");
    flow.follow(&event).await.expect("followed");
    assert!(flow.queue("consultation").await.expect("queue").is_empty());
    assert_eq!(fhir(&store, E1).await["status"], "completed");
    assert!(done.ended.is_some());

    encounters
        .check_in(&request(
            E2,
            "Patient/p-1",
            Class::Ambulatory,
            "2026-11-05T10:00:00Z",
        ))
        .await
        .expect("the guard was released when the encounter ended");
}

#[tokio::test]
async fn an_emergency_is_seen_first() {
    let store = store().await;
    let encounters = Encounters::new(store.clone());
    let flow = Flow::new(store.clone());
    for (id, patient, class, when) in [
        (E1, "Patient/p-1", Class::Ambulatory, "2026-11-05T08:00:00Z"),
        (E2, "Patient/p-2", Class::Ambulatory, "2026-11-05T08:05:00Z"),
        (E3, "Patient/p-3", Class::Emergency, "2026-11-05T08:30:00Z"),
    ] {
        let (_, event) = encounters
            .check_in(&request(id, patient, class, when))
            .await
            .expect("checked in");
        flow.follow(&event).await.expect("followed");
    }
    let order: Vec<String> = flow
        .queue("triage")
        .await
        .expect("queue")
        .into_iter()
        .map(|e| e.patient)
        .collect();
    assert_eq!(order, ["Patient/p-3", "Patient/p-1", "Patient/p-2"]);
}
