//! Indicators computed on a node from its change log (#554; ADR 0068), on the in-memory engine,
//! with the ANC reference package's indicators and MoH's Age Band Sets.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use health_analytics::{IndicatorStore as _, Period, PeriodType};
use health_analytics_app::Indicators;
use health_analytics_app::report::report_summary;
use health_identifiers::{OpId, ResourceId};
use health_node_api::indicators::{INTERVAL, catch_up, load};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store as _, Transaction as _, WriteRequest,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{FixedClock, Timestamp};
use serde_json::{Value, json};

const FIRST_VISITS: &str = "https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0";
const FACILITY: &str = "KAM013";

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

fn package(name: &str) -> health_packages::Files {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/reference-packages")
        .join(name);
    let mut files = health_packages::Files::new();
    for entry in std::fs::read_dir(&dir).expect("package") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|e| e == "json") {
            files.insert(
                path.file_name().expect("name").to_string_lossy().into(),
                std::fs::read(&path).expect("read"),
            );
        }
    }
    files
}

fn indicators() -> Indicators {
    let (anc, core) = (package("anc"), package("core"));
    let (indicators, problems) = load([("ug.moh.anc", &anc), ("ug.moh.core", &core)], FACILITY);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(indicators.indicators().len(), 5);
    indicators
}

fn at(day: &str) -> jiff::Zoned {
    format!("{day}T10:00:00+03:00[Africa/Kampala]")
        .parse()
        .expect("zoned")
}

fn uuid(name: &str) -> String {
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, name.as_bytes()).to_string()
}

/// Writes `resource` as its next version (or its tombstone), through the unit of work.
async fn write(store: &SurrealStore, resource: &Value) {
    let kind: ResourceType = resource["resourceType"]
        .as_str()
        .expect("type")
        .parse()
        .expect("type");
    let id: ResourceId = resource["id"].as_str().expect("id").parse().expect("id");
    let mut tx = store.begin().await.expect("begin");
    let held = tx.read(&kind, id).await.expect("read");
    tx.write(WriteRequest {
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
}

async fn delete(store: &SurrealStore, resource: &Value) {
    let kind: ResourceType = resource["resourceType"]
        .as_str()
        .expect("type")
        .parse()
        .expect("type");
    let id: ResourceId = resource["id"].as_str().expect("id").parse().expect("id");
    let mut tx = store.begin().await.expect("begin");
    tx.delete(&kind, id, OpId::new(), None)
        .await
        .expect("delete");
    tx.commit().await.expect("commit");
}

fn woman(name: &str) -> Value {
    json!({
        "resourceType": "Patient",
        "id": uuid(name),
        "gender": "female",
        "birthDate": "2000-04-04",
    })
}

fn first_visit(name: &str, patient: &str, start: &str) -> Value {
    json!({
        "resourceType": "Encounter",
        "id": uuid(name),
        "status": "completed",
        "class": [{ "coding": [{
            "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode",
            "code": "AMB",
        }] }],
        "type": [{ "coding": [{
            "system": "https://health.go.ug/fhir/CodeSystem/anc-visit-type",
            "code": "first",
        }] }],
        "subject": { "reference": format!("Patient/{}", uuid(patient)) },
        "actualPeriod": { "start": start },
    })
}

/// The count of `measure`'s summary for `period`, if there is one.
async fn count(store: &SurrealStore, measure: &str, kind: PeriodType, period: &str) -> Option<u64> {
    let mut tx = store.begin().await.expect("begin");
    let summaries = tx.summaries(kind, period).await.expect("summaries");
    let summary = summaries.into_iter().find(|s| s.measure == measure)?;
    let report: Value = serde_json::from_str(&summary.report).expect("report");
    assert_eq!(report["type"], "summary");
    assert_eq!(report["reporter"]["identifier"]["value"], FACILITY);
    Some(
        report_summary(&report, health_analytics::Calculation::Count)
            .expect("summary")
            .count,
    )
}

#[tokio::test]
async fn recording_a_first_visit_updates_its_month_at_the_next_run() {
    let store = store().await;
    let indicators = indicators();
    let now = at("2026-03-30");
    assert!(
        catch_up(&indicators, &store, &now, false)
            .await
            .expect("first run")
            .recomputed
    );
    assert_eq!(
        count(&store, FIRST_VISITS, PeriodType::Monthly, "2026-03").await,
        None
    );

    write(&store, &woman("auma")).await;
    write(
        &store,
        &first_visit("auma-1", "auma", "2026-03-30T09:00:00+03:00"),
    )
    .await;
    // The node runs every INTERVAL, with no network: the next run counts her.
    assert!(INTERVAL.as_secs() <= 5);
    let caught = catch_up(&indicators, &store, &now, false)
        .await
        .expect("run");
    assert!(!caught.recomputed);
    assert_eq!(caught.changes, 2);
    assert_eq!(caught.refreshed.patients, 1);
    assert_eq!(
        count(&store, FIRST_VISITS, PeriodType::Monthly, "2026-03").await,
        Some(1)
    );
    assert_eq!(
        count(&store, FIRST_VISITS, PeriodType::Weekly, "2026W14").await,
        Some(1)
    );
    assert_eq!(
        count(&store, FIRST_VISITS, PeriodType::Quarterly, "2026Q1").await,
        Some(1)
    );
    assert_eq!(
        count(&store, FIRST_VISITS, PeriodType::FinancialJuly, "2025July").await,
        Some(1)
    );

    // A run with nothing new changes nothing.
    let idle = catch_up(&indicators, &store, &now, false)
        .await
        .expect("idle");
    assert_eq!((idle.changes, idle.refreshed.patients), (0, 0));
}

#[tokio::test]
async fn a_visit_entered_in_april_for_a_march_date_changes_march() {
    let store = store().await;
    let indicators = indicators();
    let april = at("2026-04-03");
    catch_up(&indicators, &store, &april, false)
        .await
        .expect("first run");
    write(&store, &woman("nakato")).await;
    write(
        &store,
        &first_visit("nakato-1", "nakato", "2026-03-30T15:00:00+03:00"),
    )
    .await;
    catch_up(&indicators, &store, &april, false)
        .await
        .expect("run");
    assert_eq!(
        count(&store, FIRST_VISITS, PeriodType::Monthly, "2026-03").await,
        Some(1)
    );
    assert_eq!(
        count(&store, FIRST_VISITS, PeriodType::Monthly, "2026-04").await,
        None
    );

    // Corrected to 1 April: March has no first visit left, and April has one.
    write(
        &store,
        &first_visit("nakato-1", "nakato", "2026-04-01T08:00:00+03:00"),
    )
    .await;
    catch_up(&indicators, &store, &april, false)
        .await
        .expect("run");
    assert_eq!(
        count(&store, FIRST_VISITS, PeriodType::Monthly, "2026-03").await,
        None
    );
    assert_eq!(
        count(&store, FIRST_VISITS, PeriodType::Monthly, "2026-04").await,
        Some(1)
    );

    // Deleted: April has none either.
    delete(&store, &first_visit("nakato-1", "nakato", "")).await;
    catch_up(&indicators, &store, &april, false)
        .await
        .expect("run");
    assert_eq!(
        count(&store, FIRST_VISITS, PeriodType::Monthly, "2026-04").await,
        None
    );
}

#[tokio::test]
async fn a_first_visit_another_facility_recorded_is_not_counted_here() {
    let store = store().await;
    let indicators = indicators();
    let now = at("2026-03-30");
    catch_up(&indicators, &store, &now, false)
        .await
        .expect("first run");
    write(&store, &woman("opio")).await;
    let mut elsewhere = first_visit("opio-1", "opio", "2026-03-10");
    elsewhere["meta"] = json!({ "tag": [{ "system": "urn:health:facility", "code": "KAM099" }] });
    write(&store, &elsewhere).await;
    catch_up(&indicators, &store, &now, false)
        .await
        .expect("run");
    assert_eq!(
        count(&store, FIRST_VISITS, PeriodType::Monthly, "2026-03").await,
        None
    );
}

/// Every test patient of the ANC reference package, with ids made UUIDs as a node needs.
fn test_patients() -> Vec<Vec<Value>> {
    fn rename(value: &mut Value) {
        match value {
            Value::Object(map) => {
                for (key, inner) in map.iter_mut() {
                    match (key.as_str(), inner.as_str()) {
                        ("id", Some(id)) => *inner = Value::String(uuid(id)),
                        ("reference", Some(reference)) => {
                            if let Some((kind, id)) = reference.split_once('/') {
                                *inner = Value::String(format!("{kind}/{}", uuid(id)));
                            }
                        }
                        _ => rename(inner),
                    }
                }
            }
            Value::Array(items) => items.iter_mut().for_each(rename),
            _ => {}
        }
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/reference-packages/anc/tests");
    let mut patients = Vec::new();
    for entry in std::fs::read_dir(dir).expect("tests") {
        let file: Value =
            serde_json::from_slice(&std::fs::read(entry.expect("entry").path()).expect("read"))
                .expect("json");
        if file["type"] != "test-patient" {
            continue;
        }
        let mut resources = file["resources"].as_array().expect("resources").clone();
        resources.iter_mut().for_each(rename);
        // The Patient first, so their records find them.
        resources.sort_by_key(|r| r["resourceType"] != "Patient");
        patients.push(resources);
    }
    patients.sort_by_key(|p| p[0]["id"].as_str().map(str::to_owned));
    patients
}

/// Every summary kept, by period type, period and indicator, without when it was computed.
async fn every_summary(store: &SurrealStore) -> BTreeMap<(String, String, String), Value> {
    let mut found = BTreeMap::new();
    let from: jiff::civil::Date = "2025-01-01".parse().expect("date");
    for kind in health_node_api::indicators::STANDARD_PERIODS {
        let mut period = Period::of(kind, from);
        while period.start.year() < 2028 {
            let mut tx = store.begin().await.expect("begin");
            for summary in tx.summaries(kind, &period.id).await.expect("summaries") {
                let mut report: Value = serde_json::from_str(&summary.report).expect("report");
                report["date"] = Value::Null;
                found.insert(
                    (format!("{kind:?}"), period.id.clone(), summary.measure),
                    report,
                );
            }
            period = period.next().expect("next");
        }
    }
    found
}

#[tokio::test]
async fn recomputing_from_scratch_gives_what_the_incremental_path_gives() {
    let store = store().await;
    let indicators = indicators();
    let now = at("2026-07-01");
    catch_up(&indicators, &store, &now, false)
        .await
        .expect("first run");
    let patients = test_patients();
    assert_eq!(patients.len(), 11);
    for patient in &patients {
        for resource in patient {
            write(&store, resource).await;
        }
        catch_up(&indicators, &store, &now, false)
            .await
            .expect("incremental");
    }
    let incremental = every_summary(&store).await;
    assert!(
        incremental
            .keys()
            .any(|(_, period, measure)| period == "2026-03" && measure == FIRST_VISITS),
        "{:?}",
        incremental.keys().collect::<Vec<_>>()
    );
    let recomputed = catch_up(&indicators, &store, &now, true)
        .await
        .expect("recompute");
    assert!(recomputed.recomputed);
    assert!(recomputed.refreshed.problems.is_empty(), "{recomputed:?}");
    assert_eq!(every_summary(&store).await, incremental);

    // Corrections and deletions too: move a visit, delete another.
    let encounters: Vec<&Value> = patients
        .iter()
        .flatten()
        .filter(|r| r["resourceType"] == "Encounter")
        .collect();
    let mut moved = encounters[0].clone();
    moved["actualPeriod"]["start"] = json!("2026-05-20T09:00:00+03:00");
    write(&store, &moved).await;
    delete(&store, encounters[1]).await;
    catch_up(&indicators, &store, &now, false)
        .await
        .expect("incremental");
    let incremental = every_summary(&store).await;
    catch_up(&indicators, &store, &now, true)
        .await
        .expect("recompute");
    assert_eq!(every_summary(&store).await, incremental);
}
