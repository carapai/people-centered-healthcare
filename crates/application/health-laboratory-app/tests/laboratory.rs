//! The laboratory workflow over SurrealDB 3.3.5 in memory (#109): a test from order to review,
//! with its FHIR Specimen, Observations and DiagnosticReport at each step valid R5, results
//! released only once a second person verifies them, critical results flagged, and corrections,
//! rejections and cancellations.

use std::sync::Arc;

use health_fhir_validation::validate_structure;
use health_identifiers::ResourceId;
use health_laboratory::{
    Coded, Done, Flag, LabEvent, LabRefused, Order, Range, ResultEntry, ResultValue, Specimen,
    Stage,
};
use health_laboratory_app::{LabError, Laboratory, report_id, result_id};
use health_persistence::{ResourceType, Store, Transaction};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use serde_json::Value;

const TEST: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000a01";
const SPECIMEN: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000b01";

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn done(by: &str, when: &str) -> Done {
    Done {
        by: by.into(),
        at: at(when),
    }
}

fn coded(text: &str) -> Coded {
    Coded {
        text: text.into(),
        system: None,
        code: None,
    }
}

async fn store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "laboratory".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

fn ordered() -> Order {
    Order {
        id: TEST.into(),
        patient: "Patient/p-1".into(),
        encounter: Some("Encounter/e-1".into()),
        facility: "KAM013".into(),
        test: coded("Full blood count"),
        ordered: done("clinician-1", "2026-11-05T10:00:00Z"),
    }
}

fn specimen() -> Specimen {
    Specimen {
        id: SPECIMEN.into(),
        kind: coded("Venous blood"),
        collected: done("phlebotomist-1", "2026-11-05T10:15:00Z"),
    }
}

fn hb(value: &str) -> ResultEntry {
    ResultEntry {
        code: coded("Haemoglobin"),
        value: ResultValue::Quantity {
            value: value.into(),
            unit: "g/dL".into(),
        },
        reference: Some(Range {
            low: Some("11.5".into()),
            high: Some("16.5".into()),
        }),
        critical: Some(Range {
            low: Some("7.0".into()),
            high: None,
        }),
        flag: None,
    }
}

fn rdt() -> ResultEntry {
    ResultEntry {
        code: coded("Malaria RDT"),
        value: ResultValue::Coded {
            concept: coded("Positive"),
        },
        reference: None,
        critical: None,
        flag: Some(Flag::Abnormal),
    }
}

async fn read(store: &SurrealStore, kind: &str, id: ResourceId) -> Option<Value> {
    let kind: ResourceType = kind.parse().expect("type");
    let mut tx = store.begin().await.expect("begin");
    tx.read(&kind, id)
        .await
        .expect("read")
        .map(|h| serde_json::from_str(h.json.as_str()).expect("json"))
}

fn valid(resource: &Value) {
    let issues = validate_structure(&resource.to_string());
    assert!(issues.is_empty(), "{resource}: {issues:?}");
}

#[tokio::test]
async fn a_test_runs_from_order_to_review_with_its_fhir_at_each_step() {
    let store = store().await;
    let lab = Laboratory::new(store.clone());
    lab.order(ordered()).await.expect("ordered");
    assert_eq!(
        lab.order(ordered()).await,
        Err(LabError::AlreadyOrdered(TEST.into()))
    );
    assert!(
        read(&store, "DiagnosticReport", report_id(TEST))
            .await
            .is_none(),
        "no report before collection"
    );

    lab.collect(TEST, specimen()).await.expect("collected");
    let collected = read(&store, "Specimen", SPECIMEN.parse().expect("id"))
        .await
        .expect("specimen");
    valid(&collected);
    assert_eq!(
        collected["request"][0]["reference"],
        format!("ServiceRequest/{TEST}")
    );
    assert_eq!(
        read(&store, "DiagnosticReport", report_id(TEST))
            .await
            .expect("report")["status"],
        "registered"
    );

    let (_, event) = lab
        .result(
            TEST,
            vec![hb("13.20"), rdt()],
            done("analyst-1", "2026-11-05T11:00:00Z"),
        )
        .await
        .expect("resulted");
    assert_eq!(event.name(), "laboratory.resulted.v1");
    let first = read(&store, "Observation", result_id(TEST, 0))
        .await
        .expect("haemoglobin");
    valid(&first);
    assert_eq!(
        first["status"], "preliminary",
        "not released before verification"
    );
    assert_eq!(
        first["valueQuantity"]["value"].to_string(),
        "13.20",
        "the decimal keeps its scale"
    );
    assert_eq!(first["interpretation"][0]["coding"][0]["code"], "N");
    assert_eq!(
        first["referenceRange"][0]["low"]["value"].to_string(),
        "11.5"
    );
    let second = read(&store, "Observation", result_id(TEST, 1))
        .await
        .expect("rdt");
    valid(&second);
    assert_eq!(
        (
            second["valueCodeableConcept"]["text"].as_str(),
            second["interpretation"][0]["coding"][0]["code"].as_str()
        ),
        (Some("Positive"), Some("A"))
    );
    let preliminary = read(&store, "DiagnosticReport", report_id(TEST))
        .await
        .expect("report");
    valid(&preliminary);
    assert_eq!(
        (
            preliminary["status"].as_str(),
            preliminary["result"].as_array().map(Vec::len)
        ),
        (Some("preliminary"), Some(2))
    );

    // A correction that drops the RDT: its Observation is entered in error, never deleted.
    lab.result(
        TEST,
        vec![hb("12.90")],
        done("analyst-1", "2026-11-05T11:05:00Z"),
    )
    .await
    .expect("corrected");
    assert_eq!(
        read(&store, "Observation", result_id(TEST, 1))
            .await
            .expect("kept")["status"],
        "entered-in-error"
    );

    assert_eq!(
        lab.verify(TEST, done("analyst-1", "2026-11-05T11:10:00Z"))
            .await,
        Err(LabError::Refused(LabRefused::SameVerifier))
    );
    let (verified, event) = lab
        .verify(TEST, done("analyst-2", "2026-11-05T11:10:00Z"))
        .await
        .expect("verified");
    assert!(verified.released());
    assert!(matches!(
        event,
        LabEvent::Verified {
            critical: false,
            ..
        }
    ));
    let released = read(&store, "DiagnosticReport", report_id(TEST))
        .await
        .expect("report");
    valid(&released);
    assert_eq!(
        (
            released["status"].as_str(),
            released["resultsInterpreter"][0]["display"].as_str()
        ),
        (Some("final"), Some("analyst-2"))
    );
    assert_eq!(
        read(&store, "Observation", result_id(TEST, 0))
            .await
            .expect("hb")["status"],
        "final"
    );

    let (reviewed, event) = lab
        .review(TEST, done("clinician-1", "2026-11-05T12:00:00Z"))
        .await
        .expect("reviewed");
    assert_eq!(
        (reviewed.stage, event.name()),
        (Stage::Reviewed, "laboratory.reviewed.v1")
    );
    assert_eq!(lab.test(TEST).await.expect("test"), reviewed);
}

#[tokio::test]
async fn a_critical_result_and_a_rejected_specimen() {
    let store = store().await;
    let lab = Laboratory::new(store.clone());
    lab.order(ordered()).await.expect("ordered");
    lab.collect(TEST, specimen()).await.expect("collected");
    lab.result(
        TEST,
        vec![hb("6.4")],
        done("analyst-1", "2026-11-05T11:00:00Z"),
    )
    .await
    .expect("resulted");
    assert_eq!(
        read(&store, "Observation", result_id(TEST, 0))
            .await
            .expect("hb")["interpretation"][0]["coding"][0]["code"],
        "LL"
    );
    let (_, event) = lab
        .verify(TEST, done("analyst-2", "2026-11-05T11:10:00Z"))
        .await
        .expect("verified");
    assert!(
        matches!(event, LabEvent::Verified { critical: true, .. }),
        "routed urgently to the clinician"
    );

    let other = "0190f1a2-7c3d-7e4f-8a5b-000000000a02";
    lab.order(Order {
        id: other.into(),
        ..ordered()
    })
    .await
    .expect("ordered");
    let sample = Specimen {
        id: "0190f1a2-7c3d-7e4f-8a5b-000000000b02".into(),
        ..specimen()
    };
    lab.collect(other, sample.clone()).await.expect("collected");
    let (rejected, _) = lab
        .reject(other, "haemolysed", at("2026-11-05T10:30:00Z"))
        .await
        .expect("rejected");
    assert_eq!(rejected.stage, Stage::Rejected);
    let specimen = read(&store, "Specimen", sample.id.parse().expect("id"))
        .await
        .expect("specimen");
    assert_eq!(
        (
            specimen["status"].as_str(),
            specimen["condition"][0]["text"].as_str()
        ),
        (Some("unsatisfactory"), Some("haemolysed"))
    );
    assert_eq!(
        read(&store, "DiagnosticReport", report_id(other))
            .await
            .expect("report")["status"],
        "cancelled"
    );

    let third = "0190f1a2-7c3d-7e4f-8a5b-000000000a03";
    lab.order(Order {
        id: third.into(),
        ..ordered()
    })
    .await
    .expect("ordered");
    assert_eq!(
        lab.cancel(third, "ordered in error", at("2026-11-05T10:05:00Z"))
            .await
            .expect("cancelled")
            .0
            .stage,
        Stage::Cancelled
    );
    let unknown = "0190f1a2-7c3d-7e4f-8a5b-000000000a09";
    assert_eq!(
        lab.test(unknown).await,
        Err(LabError::NotFound(unknown.into()))
    );
    assert!(matches!(
        lab.review(third, done("c", "2026-11-05T12:00:00Z")).await,
        Err(LabError::Refused(_))
    ));
}
