//! A node's indicators kept up to date patient by patient (#554), on the in-memory engine.

use std::collections::BTreeSet;
use std::sync::Arc;

use health_analytics::{Calculation, IndicatorStore as _, PeriodType};
use health_analytics_app::report::{report_period, report_summary};
use health_analytics_app::{Indicators, Library, measure};
use health_fhir_validation::OctofhirEngine;
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store as _, Transaction as _, WriteRequest,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{FixedClock, Timestamp};
use serde_json::{Value, json};

const PATIENT: &str = "0190f0a0-0000-7000-8000-000000000001";
const VISIT: &str = "0190f0a0-0000-7000-8000-000000000002";

async fn store() -> SurrealStore {
    let settings = Settings {
        target: Target::Memory,
        namespace: "test".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let at: Timestamp = "2026-04-03T08:00:00Z".parse().expect("timestamp");
    SurrealStore::new(connection, Arc::new(FixedClock::new(at)))
}

async fn write(store: &SurrealStore, resource: Value) -> u64 {
    let kind: ResourceType = resource["resourceType"]
        .as_str()
        .expect("type")
        .parse()
        .expect("type");
    let id: ResourceId = resource["id"].as_str().expect("id").parse().expect("id");
    let mut tx = store.begin().await.expect("begin");
    let held = tx.read(&kind, id).await.expect("read");
    let written = tx
        .write(WriteRequest {
            resource_type: kind,
            id,
            json: RawJson::from_validated(resource.to_string()),
            op_id: OpId::new(),
            precondition: match held {
                Some(held) => Precondition::IfMatch(held.version.version_id),
                None => Precondition::MustNotExist,
            },
        })
        .await
        .expect("write");
    tx.commit().await.expect("commit");
    written.version_seq
}

fn visit(start: &str) -> Value {
    json!({
        "resourceType": "Encounter",
        "id": VISIT,
        "status": "completed",
        "subject": { "reference": format!("Patient/{PATIENT}") },
        "actualPeriod": { "start": start },
    })
}

fn indicators() -> Indicators {
    let resource = json!({
        "resourceType": "Measure",
        "url": "https://x/Measure/visits",
        "version": "1.0.0",
        "title": "Visits",
        "extension": [{
            "url": health_analytics_app::PERIOD_DATE,
            "valueExpression": { "language": "text/fhirpath", "expression": "Encounter.actualPeriod.start" },
        }],
        "scoring": { "coding": [{ "code": "cohort" }] },
        "basis": "Encounter",
        "group": [{ "population": [{
            "code": { "coding": [{ "code": "initial-population" }] },
            "criteria": { "language": "text/fhirpath", "expression": "%patient.gender = 'female'" },
        }] }],
    });
    let indicator = measure(&resource).indicator().expect("indicator");
    Indicators::new(
        Arc::new(OctofhirEngine::new()),
        Library::new([indicator.clone()]),
        vec![indicator],
        vec![PeriodType::Monthly],
        "KAM013",
    )
}

async fn counts(store: &SurrealStore, period: &str) -> Vec<u64> {
    let mut tx = store.begin().await.expect("begin");
    tx.summaries(PeriodType::Monthly, period)
        .await
        .expect("summaries")
        .iter()
        .map(|s| {
            let report: Value = serde_json::from_str(&s.report).expect("report");
            assert_eq!(report_period(&report).expect("period").id, period);
            report_summary(&report, Calculation::Count)
                .expect("summary")
                .count
        })
        .collect()
}

#[tokio::test]
async fn a_changed_patient_moves_their_care_between_periods() {
    let store = store().await;
    let indicators = indicators();
    let now: jiff::Zoned = "2026-04-03T10:00:00+03:00[Africa/Kampala]"
        .parse()
        .expect("now");
    write(
        &store,
        json!({ "resourceType": "Patient", "id": PATIENT, "gender": "female" }),
    )
    .await;
    let first = write(&store, visit("2026-03-30")).await;

    let changed =
        Indicators::patients_changed(&store, &[("Encounter".into(), VISIT.into(), first)])
            .await
            .expect("changed");
    assert_eq!(changed, BTreeSet::from([PATIENT.to_owned()]));
    let done = indicators
        .refresh(&store, &changed, &now)
        .await
        .expect("refresh");
    assert_eq!((done.patients, done.summaries), (1, 1));
    assert_eq!(counts(&store, "2026-03").await, [1]);

    write(&store, visit("2026-04-01")).await;
    indicators
        .refresh(&store, &changed, &now)
        .await
        .expect("refresh");
    assert!(counts(&store, "2026-03").await.is_empty());
    assert_eq!(counts(&store, "2026-04").await, [1]);

    let again = indicators.recompute(&store, &now).await.expect("recompute");
    assert_eq!(again.patients, 1);
    assert!(counts(&store, "2026-03").await.is_empty());
    assert_eq!(counts(&store, "2026-04").await, [1]);
}
