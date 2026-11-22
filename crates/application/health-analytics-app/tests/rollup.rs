//! Roll-up at central (#555): each facility's synced summary read back and rolled up over a
//! district from summed parts, a corrected summary replacing the old one in the figure, and a
//! facility that has not synced shown as missing.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use health_analytics::{
    Calculation, Counts, Indicator, Period, PeriodType, StoredSummary, Summary,
};
use health_analytics_app::report::measure_report;
use health_analytics_app::roll_up;
use health_persistence::{Store as _, Transaction as _};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_sync_protocol::IndicatorSummary;
use health_time::SystemClock;
use rust_decimal::Decimal;

async fn store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "central".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

fn indicator() -> Indicator {
    Indicator {
        url: "https://x/Measure/anc4".into(),
        version: "1.0.0".into(),
        title: "ANC 4th visit coverage".into(),
        scoring: Calculation::Percentage.scoring(),
        calculation: Calculation::Percentage,
        basis: "Encounter".into(),
        counts: Counts::People,
        cumulative: false,
        period_date: "Encounter.actualPeriod.start".into(),
        populations: BTreeMap::new(),
        references: BTreeSet::new(),
        disaggregations: Vec::new(),
        value: None,
    }
}

fn march() -> Period {
    Period::parse(PeriodType::Monthly, "2026-03").expect("march")
}

/// `facility`'s summary of `count` of `denominator`, as its edge sends it.
fn sent(facility: &str, count: u64, denominator: u64) -> IndicatorSummary {
    let summary = Summary {
        calculation: Calculation::Percentage,
        value: None,
        count,
        denominator: Some(denominator),
        values: None,
        disaggregations: Vec::new(),
    };
    let report = measure_report(
        &indicator().canonical(),
        &march(),
        facility,
        &summary,
        "2026-04-01T06:00:00Z",
    );
    let stored = StoredSummary {
        measure: indicator().canonical(),
        period: march(),
        facility: facility.into(),
        report: report.to_string(),
    };
    IndicatorSummary {
        measure: stored.measure,
        period_type: "monthly".into(),
        period: stored.period.id,
        facility: stored.facility,
        hash: health_surrealdb::figures_hash(&stored.report),
        report: Some(report),
    }
}

async fn receive(central: &SurrealStore, summary: &IndicatorSummary) {
    let mut tx = central.begin().await.expect("begin");
    tx.receive_indicator_summary("node", summary)
        .await
        .expect("receive");
    tx.indicator_summaries_synced(&summary.facility, "node")
        .await
        .expect("synced");
    tx.commit().await.expect("commit");
}

#[tokio::test]
async fn a_district_percentage_is_its_facilities_summed_numerators_over_summed_denominators() {
    let central = store().await;
    receive(&central, &sent("A", 1, 2)).await;
    receive(&central, &sent("B", 9, 10)).await;
    let district: Vec<String> = ["A", "B", "C"].map(String::from).to_vec();

    let mut tx = central.begin().await.expect("begin");
    let rolled = roll_up(&mut tx, &indicator(), &march(), &district)
        .await
        .expect("rolled up");
    drop(tx);
    let summary = rolled.summary.as_ref().expect("a figure");
    // 10 of 12, not the 70% that averaging 50% and 90% gives.
    assert_eq!((summary.count, summary.denominator), (10, Some(12)));
    assert_eq!(summary.value, Some(Decimal::new(8333, 2)));
    assert_eq!(rolled.completeness(), (2, 3));
    assert_eq!(rolled.missing[0].facility, "C");
    assert_eq!(rolled.missing[0].last_sync, None);

    // B corrects its March: the new summary replaces the old in the figure.
    receive(&central, &sent("B", 3, 10)).await;
    let mut tx = central.begin().await.expect("begin");
    let rolled = roll_up(&mut tx, &indicator(), &march(), &district)
        .await
        .expect("rolled up");
    let summary = rolled.summary.expect("a figure");
    assert_eq!((summary.count, summary.denominator), (4, Some(12)));
    assert_eq!(summary.value, Some(Decimal::new(3333, 2)));
}
