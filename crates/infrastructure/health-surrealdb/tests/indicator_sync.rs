//! Indicator summaries sync to central (#555): the edge sends each summary whose figures central
//! does not hold, once, and each one it has forgotten since; central keeps each facility's,
//! a corrected one replacing the old, and when the facility's summaries last synced.

use std::sync::Arc;

use health_analytics::StoredSummary;
use health_analytics::{FacilitySummaries as _, IndicatorStore as _, Period, PeriodType};
use health_persistence::Store as _;
use health_persistence::Transaction as _;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use serde_json::json;

const MEASURE: &str = "https://x/Measure/anc1|1.0.0";

async fn store(namespace: &str) -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: namespace.into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

fn march() -> Period {
    Period::parse(PeriodType::Monthly, "2026-03").expect("march")
}

fn summary(count: u64, date: &str) -> StoredSummary {
    StoredSummary {
        measure: MEASURE.into(),
        period: march(),
        facility: "KAM013".into(),
        report: json!({
            "resourceType": "MeasureReport",
            "type": "summary",
            "measure": MEASURE,
            "date": date,
            "reporter": { "identifier": { "system": "urn:health:facility", "value": "KAM013" } },
            "period": { "start": "2026-03-01", "end": "2026-03-31" },
            "group": [{ "population": [{ "count": count }] }]
        })
        .to_string(),
    }
}

async fn put(edge: &SurrealStore, summary: &StoredSummary) {
    let mut tx = edge.begin().await.expect("begin");
    tx.put_summary(summary).await.expect("put");
    tx.commit().await.expect("commit");
}

/// What the edge would send, taken in at central as the sync server does, and acknowledged.
async fn sync(edge: &SurrealStore, central: &SurrealStore) -> usize {
    let unsent = edge.unsent_indicator_summaries(100).await.expect("unsent");
    let mut tx = central.begin().await.expect("begin");
    for summary in &unsent {
        tx.receive_indicator_summary("node-1", summary)
            .await
            .expect("receive");
    }
    tx.indicator_summaries_synced("KAM013", "node-1")
        .await
        .expect("synced");
    tx.commit().await.expect("commit");
    edge.indicator_summaries_sent(&unsent).await.expect("sent");
    unsent.len()
}

async fn at_central(central: &SurrealStore) -> Vec<u64> {
    let mut tx = central.begin().await.expect("begin");
    tx.facility_summaries(MEASURE, &march())
        .await
        .expect("summaries")
        .iter()
        .map(|s| {
            let report: serde_json::Value = serde_json::from_str(&s.report).expect("json");
            report["group"][0]["population"][0]["count"]
                .as_u64()
                .expect("count")
        })
        .collect()
}

#[tokio::test]
async fn a_corrected_summary_replaces_the_old_one_at_central_and_a_forgotten_one_goes() {
    let (edge, central) = (store("edge").await, store("central").await);
    put(&edge, &summary(3, "2026-04-01T06:00:00Z")).await;
    assert_eq!(sync(&edge, &central).await, 1);
    assert_eq!(at_central(&central).await, [3]);
    assert_eq!(sync(&edge, &central).await, 0, "sent once");

    // Computed again with the same figures (the daily recompute): nothing to send.
    let mut tx = edge.begin().await.expect("begin");
    tx.clear_indicators().await.expect("clear");
    tx.put_summary(&summary(3, "2026-04-02T06:00:00Z"))
        .await
        .expect("put");
    tx.commit().await.expect("commit");
    assert_eq!(sync(&edge, &central).await, 0, "same figures");

    // A late entry corrects March: the new figures replace the old.
    put(&edge, &summary(4, "2026-04-03T06:00:00Z")).await;
    assert_eq!(sync(&edge, &central).await, 1);
    assert_eq!(at_central(&central).await, [4]);
    let mut tx = central.begin().await.expect("begin");
    let kept = tx
        .facility_summaries(MEASURE, &march())
        .await
        .expect("kept");
    assert_eq!(
        kept[0].computed_at,
        "2026-04-03T06:00:00Z".parse().expect("time")
    );
    assert!(
        tx.summaries_synced()
            .await
            .expect("synced")
            .contains_key("KAM013")
    );
    drop(tx);

    // The record is deleted and March has no care: central forgets it too.
    let mut tx = edge.begin().await.expect("begin");
    tx.remove_summary(MEASURE, &march(), "KAM013")
        .await
        .expect("remove");
    tx.commit().await.expect("commit");
    assert_eq!(sync(&edge, &central).await, 1);
    assert!(at_central(&central).await.is_empty());
    assert_eq!(sync(&edge, &central).await, 0);
}

#[tokio::test]
async fn a_summary_changed_while_it_was_being_sent_is_sent_again() {
    let (edge, central) = (store("edge").await, store("central").await);
    put(&edge, &summary(3, "2026-04-01T06:00:00Z")).await;
    let unsent = edge.unsent_indicator_summaries(100).await.expect("unsent");
    // Recomputed with new figures before central's answer arrives.
    put(&edge, &summary(5, "2026-04-01T06:01:00Z")).await;
    edge.indicator_summaries_sent(&unsent).await.expect("sent");
    assert_eq!(sync(&edge, &central).await, 1);
    assert_eq!(at_central(&central).await, [5]);

    // Forgotten, and computed again with the same figures before the removal was answered.
    let mut tx = edge.begin().await.expect("begin");
    tx.remove_summary(MEASURE, &march(), "KAM013")
        .await
        .expect("remove");
    tx.commit().await.expect("commit");
    let removal = edge.unsent_indicator_summaries(100).await.expect("unsent");
    assert_eq!(removal.len(), 1);
    assert!(removal[0].report.is_none());
    let mut tx = central.begin().await.expect("begin");
    tx.receive_indicator_summary("node-1", &removal[0])
        .await
        .expect("receive");
    tx.commit().await.expect("commit");
    put(&edge, &summary(5, "2026-04-01T06:02:00Z")).await;
    edge.indicator_summaries_sent(&removal).await.expect("sent");
    assert_eq!(sync(&edge, &central).await, 1, "central has it again");
    assert_eq!(at_central(&central).await, [5]);
}
