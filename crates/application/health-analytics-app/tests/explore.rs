//! The data explorer (#599; ADR 0075): figures for places × standard periods from stored
//! summaries, at central over what the facilities synced and on an edge over its own, with the
//! facilities that have not synced listed as missing and a median shown at facility level only.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use health_analytics::{
    Aggregate, Calculation, Counts, Indicator, IndicatorStore as _, Period, PeriodType,
    StoredSummary, Summary, Values,
};
use health_analytics_app::explore::{
    ExplorePlace, Figure, OwnSummaries, cells_json, explore, indicator_json,
};
use health_analytics_app::report::measure_report;
use health_persistence::{Store as _, Transaction as _};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_sync_protocol::IndicatorSummary;
use health_time::SystemClock;
use rust_decimal::Decimal;

async fn store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "explore".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

fn indicator(name: &str, calculation: Calculation) -> Indicator {
    Indicator {
        url: format!("https://x/Measure/{name}"),
        version: "1.0.0".into(),
        title: name.into(),
        scoring: calculation.scoring(),
        calculation,
        basis: "Encounter".into(),
        counts: Counts::People,
        cumulative: false,
        period_date: "Encounter.actualPeriod.start".into(),
        populations: BTreeMap::new(),
        references: BTreeSet::new(),
        disaggregations: Vec::new(),
        thresholds: Vec::new(),
        value: None,
    }
}

fn visits() -> Indicator {
    indicator("anc1", Calculation::Count)
}

fn weeks() -> Indicator {
    indicator("gestational-age", Calculation::Value(Aggregate::Median))
}

fn month(id: &str) -> Period {
    Period::parse(PeriodType::Monthly, id).expect("month")
}

fn summary(indicator: &Indicator, count: u64) -> Summary {
    let median = matches!(indicator.calculation, Calculation::Value(_));
    Summary {
        calculation: indicator.calculation,
        value: Some(Decimal::from(if median { 14 } else { count })),
        count,
        denominator: None,
        values: median.then_some(Values {
            count,
            sum: None,
            minimum: None,
            maximum: None,
        }),
        disaggregations: Vec::new(),
    }
}

fn stored(indicator: &Indicator, period: &Period, facility: &str, count: u64) -> StoredSummary {
    let report = measure_report(
        &indicator.canonical(),
        period,
        facility,
        &summary(indicator, count),
        "2026-04-01T06:00:00Z",
    );
    StoredSummary {
        measure: indicator.canonical(),
        period: period.clone(),
        facility: facility.into(),
        report: report.to_string(),
    }
}

/// Central receives `facility`'s summaries and records its sync, as the sync endpoint does.
async fn synced(central: &SurrealStore, facility: &str, summaries: &[StoredSummary]) {
    let mut tx = central.begin().await.expect("begin");
    for s in summaries {
        let sent = IndicatorSummary {
            measure: s.measure.clone(),
            period_type: "monthly".into(),
            period: s.period.id.clone(),
            facility: s.facility.clone(),
            hash: health_surrealdb::figures_hash(&s.report),
            report: Some(serde_json::from_str(&s.report).expect("json")),
        };
        tx.receive_indicator_summary("node", &sent)
            .await
            .expect("receive");
    }
    tx.indicator_summaries_synced(facility, "node")
        .await
        .expect("synced");
    tx.commit().await.expect("commit");
}

fn place(code: &str, facilities: &[&str]) -> ExplorePlace {
    ExplorePlace {
        code: code.into(),
        facilities: facilities.iter().map(|f| (*f).to_owned()).collect(),
    }
}

#[tokio::test]
async fn a_district_s_months_count_its_reported_facilities_and_list_the_missing_ones() {
    let central = store().await;
    let march = month("2026-03");
    // KAM013 counted 4 first visits in March and KAM020 6; KAM031 never synced.
    synced(
        &central,
        "KAM013",
        &[stored(&visits(), &march, "KAM013", 4)],
    )
    .await;
    synced(
        &central,
        "KAM020",
        &[stored(&visits(), &march, "KAM020", 6)],
    )
    .await;
    let district = place("district-kampala", &["KAM013", "KAM020", "KAM031"]);

    let mut tx = central.begin().await.expect("begin");
    let cells = explore(
        &mut tx,
        &[visits(), weeks()],
        std::slice::from_ref(&march),
        &[district, place("KAM013", &["KAM013"])],
    )
    .await
    .expect("explored");
    drop(tx);
    assert_eq!(cells.len(), 4);
    let Figure::Rolled(rolled) = &cells[0].figure else {
        panic!("a count rolls up: {:?}", cells[0]);
    };
    // 4 + 6, with KAM031 missing rather than counted as zero.
    assert_eq!(
        rolled.summary.as_ref().and_then(|s| s.value),
        Some(Decimal::from(10))
    );
    assert_eq!(rolled.completeness(), (2, 3));
    assert_eq!(rolled.missing[0].facility, "KAM031");
    let names = BTreeMap::from([("KAM031".to_owned(), "Kitebi HC III".to_owned())]);
    let json = cells_json(&cells, &names);
    assert_eq!(json[0]["value"], "10");
    assert_eq!(json[0]["reported"], 2);
    assert_eq!(json[0]["expected"], 3);
    assert_eq!(json[0]["missing"][0]["name"], "Kitebi HC III");
    assert_eq!(json[0]["missing"][0]["lastSync"], serde_json::Value::Null);
    assert_eq!(json[0]["computedAt"], "2026-04-01T06:00:00Z");
    // A median over the district has no figure, and says why.
    assert_eq!(cells[2].figure, Figure::FacilityOnly(Aggregate::Median));
    assert_eq!(json[2]["facilityOnly"], "median");
    assert_eq!(json[2]["value"], serde_json::Value::Null);
    assert_eq!(indicator_json(&weeks())["rollsUp"], false);
    assert_eq!(indicator_json(&visits())["calculation"], "count");
}

#[tokio::test]
async fn a_facility_s_median_shows_at_facility_level() {
    let central = store().await;
    let march = month("2026-03");
    synced(&central, "KAM013", &[stored(&weeks(), &march, "KAM013", 3)]).await;
    let mut tx = central.begin().await.expect("begin");
    let cells = explore(
        &mut tx,
        &[weeks()],
        &[march],
        &[place("KAM013", &["KAM013"])],
    )
    .await
    .expect("explored");
    assert_eq!(cells_json(&cells, &BTreeMap::new())[0]["value"], "14");
}

#[tokio::test]
async fn an_edge_explores_its_own_months_offline() {
    let edge = store().await;
    let (march, april) = (month("2026-03"), month("2026-04"));
    let mut tx = edge.begin().await.expect("begin");
    tx.put_summary(&stored(&visits(), &march, "KAM013", 5))
        .await
        .expect("put");
    // Another facility's summary on the same node is not this facility's figure.
    tx.put_summary(&stored(&visits(), &march, "KAM020", 9))
        .await
        .expect("put");
    tx.commit().await.expect("commit");

    let now: jiff::Timestamp = "2026-05-02T08:00:00Z".parse().expect("now");
    let mut tx = edge.begin().await.expect("begin");
    let mut own = OwnSummaries::new(&mut tx, "KAM013", now);
    let cells = explore(
        &mut own,
        &[visits()],
        &[march, april],
        &[place("KAM013", &["KAM013"])],
    )
    .await
    .expect("explored");
    let json = cells_json(&cells, &BTreeMap::new());
    assert_eq!(json[0]["value"], "5");
    assert_eq!(json[0]["computedAt"], "2026-04-01T06:00:00Z");
    // Nothing counted in April: the facility computed it, so it is zero, not missing.
    assert_eq!(json[1]["value"], "0");
    assert_eq!(
        (json[1]["reported"].clone(), json[1]["expected"].clone()),
        (1.into(), 1.into())
    );
}

#[test]
fn a_question_names_offered_indicators_and_whole_standard_periods() {
    use health_analytics_app::explore::read_question;
    use serde_json::json;
    let offered = [visits(), weeks()];
    let question = read_question(
        &json!({
            "indicators": [visits().canonical()],
            "periodType": "monthly",
            "from": "2026-01",
            "to": "2026-03",
            "places": ["district-kampala", "district-kampala"],
        }),
        &offered,
    )
    .expect("question");
    assert_eq!(question.indicators, [visits()]);
    assert_eq!(question.periods.len(), 3);
    assert_eq!(question.places, ["district-kampala"]);
    // No arbitrary date range, and no period type dashboards do not offer.
    for (period_type, from, to) in [
        ("monthly", "2026-01-05", "2026-03-20"),
        ("annual", "2025", "2026"),
        ("days", "2026-01-01", "2026-01-31"),
    ] {
        let refused = read_question(
            &json!({
                "indicators": [visits().canonical()],
                "periodType": period_type, "from": from, "to": to,
                "places": ["KAM013"],
            }),
            &offered,
        );
        assert!(refused.is_err(), "{period_type} {from}–{to}");
    }
    let unknown = read_question(
        &json!({
            "indicators": ["https://x/Measure/other|1.0.0"],
            "periodType": "monthly", "from": "2026-01", "to": "2026-01",
            "places": ["KAM013"],
        }),
        &offered,
    );
    assert!(unknown.is_err());
}

/// #600: a Measure's threshold bands (a legend set) reach the explorer, lowest first, each bound
/// exact decimal text, for every Visualization to colour figures by.
#[test]
fn an_indicator_s_threshold_bands_reach_the_explorer() {
    let band = |from: Option<&str>, to: Option<&str>, tone: &str| {
        let mut parts = Vec::new();
        if let Some(from) = from {
            parts.push(serde_json::json!({ "url": "from", "valueDecimal": serde_json::Number::from_string_unchecked(from.into()) }));
        }
        if let Some(to) = to {
            parts.push(serde_json::json!({ "url": "to", "valueDecimal": serde_json::Number::from_string_unchecked(to.into()) }));
        }
        parts.push(serde_json::json!({ "url": "tone", "valueCode": tone }));
        serde_json::json!({ "url": health_analytics_app::THRESHOLD, "extension": parts })
    };
    let resource = serde_json::json!({
        "resourceType": "Measure",
        "url": "https://x/Measure/malaria-positivity",
        "version": "1.0.0",
        "title": "Malaria test positivity",
        "scoring": { "coding": [{ "code": "proportion" }] },
        "basis": "Observation",
        "extension": [
            { "url": health_analytics_app::PERIOD_DATE, "valueExpression": { "language": "text/fhirpath", "expression": "Observation.effective" } },
            band(Some("40.0"), None, "bad"),
            band(None, Some("20"), "good"),
            band(Some("20"), Some("40"), "warning"),
        ],
        "group": [{ "population": [
            { "code": { "coding": [{ "code": "initial-population" }] }, "criteria": { "language": "text/fhirpath", "expression": "true" } },
            { "code": { "coding": [{ "code": "denominator" }] }, "criteria": { "language": "text/fhirpath", "expression": "true" } },
            { "code": { "coding": [{ "code": "numerator" }] }, "criteria": { "language": "text/fhirpath", "expression": "true" } },
        ] }],
    });
    let indicator = health_analytics_app::measure(&resource)
        .indicator()
        .expect("indicator");
    assert_eq!(
        health_analytics::tone_of(&indicator.thresholds, Decimal::new(45, 0)),
        Some(health_analytics::Tone::Bad),
        "a positivity of 45% is bad"
    );
    assert_eq!(
        indicator_json(&indicator)["thresholds"],
        serde_json::json!([
            { "from": null, "to": "20", "tone": "good" },
            { "from": "20", "to": "40", "tone": "warning" },
            { "from": "40", "to": null, "tone": "bad" },
        ])
    );
    assert_eq!(
        indicator_json(&visits())["thresholds"],
        serde_json::json!([])
    );
}
