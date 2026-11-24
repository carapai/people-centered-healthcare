//! The data explorer (#599; ADR 0075): figures for places × standard periods from stored
//! summaries, at central over what the facilities synced and on an edge over its own, with the
//! facilities that have not synced listed as missing and a median shown at facility level only.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use health_analytics::{
    Aggregate, Band, Calculation, Counts, Disaggregated, Disaggregation, DisaggregationSource,
    Indicator, IndicatorStore as _, NOT_RECORDED, Period, PeriodType, PopulationEstimates,
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
        estimate: None,
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
        &PopulationEstimates::default(),
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
    let json = cells_json(&cells, &names, None);
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
        &PopulationEstimates::default(),
    )
    .await
    .expect("explored");
    assert_eq!(cells_json(&cells, &BTreeMap::new(), None)[0]["value"], "14");
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
        &PopulationEstimates::default(),
    )
    .await
    .expect("explored");
    let json = cells_json(&cells, &BTreeMap::new(), None);
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

const AGE: &str = "age";

/// ANC 1st visits by age band.
fn visits_by_age() -> Indicator {
    let mut indicator = visits();
    indicator.disaggregations = vec![Disaggregation {
        name: AGE.into(),
        source: DisaggregationSource::AgeBands("https://x/AgeBandSet/anc|1.0.0".into()),
    }];
    indicator
}

/// Positivity by age band.
fn positivity_by_age() -> Indicator {
    let mut indicator = indicator("positivity", Calculation::Percentage);
    indicator.disaggregations = visits_by_age().disaggregations;
    indicator
}

/// A summary with its parts per age band, each `(code, count, denominator)`.
fn by_age(indicator: &Indicator, bands: &[(&str, u64, Option<u64>)]) -> Summary {
    let part = |count: u64, denominator: Option<u64>| {
        let mut part = Summary::roll_up(
            indicator.calculation,
            &[Summary {
                calculation: indicator.calculation,
                value: None,
                count,
                denominator,
                values: None,
                disaggregations: Vec::new(),
            }],
        )
        .expect("a part");
        part.disaggregations.clear();
        part
    };
    let mut whole = part(
        bands.iter().map(|b| b.1).sum(),
        denominator_sum(bands.iter().map(|b| b.2)),
    );
    whole.disaggregations = vec![Disaggregated {
        name: AGE.into(),
        bands: bands
            .iter()
            .map(|(code, count, denominator)| Band {
                code: (*code).into(),
                summary: part(*count, *denominator),
            })
            .collect(),
    }];
    whole
}

fn denominator_sum(parts: impl Iterator<Item = Option<u64>>) -> Option<u64> {
    parts.sum()
}

fn stored_summary(
    indicator: &Indicator,
    period: &Period,
    facility: &str,
    summary: &Summary,
) -> StoredSummary {
    StoredSummary {
        measure: indicator.canonical(),
        period: period.clone(),
        facility: facility.into(),
        report: measure_report(
            &indicator.canonical(),
            period,
            facility,
            summary,
            "2026-04-01T06:00:00Z",
        )
        .to_string(),
    }
}

/// #601: ANC 1st visits by age band show one series per band, each summed over the district.
#[tokio::test]
async fn anc_first_visits_by_age_band_give_one_figure_per_band() {
    let central = store().await;
    let march = month("2026-03");
    let anc = visits_by_age();
    synced(
        &central,
        "KAM013",
        &[stored_summary(
            &anc,
            &march,
            "KAM013",
            &by_age(&anc, &[("15-19", 1, None), ("20-24", 3, None)]),
        )],
    )
    .await;
    synced(
        &central,
        "KAM020",
        &[stored_summary(
            &anc,
            &march,
            "KAM020",
            &by_age(
                &anc,
                &[
                    ("15-19", 2, None),
                    ("20-24", 1, None),
                    (NOT_RECORDED, 1, None),
                ],
            ),
        )],
    )
    .await;
    let mut tx = central.begin().await.expect("begin");
    let cells = explore(
        &mut tx,
        &[anc],
        &[march],
        &[place("district-kampala", &["KAM013", "KAM020"])],
        &PopulationEstimates::default(),
    )
    .await
    .expect("explored");
    drop(tx);
    let json = cells_json(&cells, &BTreeMap::new(), Some(AGE));
    assert_eq!(json[0]["value"], "8");
    assert_eq!(
        json[0]["bands"],
        serde_json::json!([
            { "code": "15-19", "value": "3" },
            { "code": "20-24", "value": "4" },
            { "code": NOT_RECORDED, "value": "1" },
        ])
    );
    // Not split, no bands.
    assert!(
        cells_json(&cells, &BTreeMap::new(), None)[0]
            .get("bands")
            .is_none()
    );
}

/// #601: a band's percentage over a district is its summed numerators over its summed
/// denominators, never the average of the facilities' percentages.
#[tokio::test]
async fn a_band_s_percentage_rolls_up_from_summed_parts_never_averaged() {
    let central = store().await;
    let march = month("2026-03");
    let positivity = positivity_by_age();
    // In 15-19, 1 of 2 (50%) at KAM013 and 9 of 10 (90%) at KAM020: 10 of 12, not 70%.
    synced(
        &central,
        "KAM013",
        &[stored_summary(
            &positivity,
            &march,
            "KAM013",
            &by_age(&positivity, &[("15-19", 1, Some(2))]),
        )],
    )
    .await;
    synced(
        &central,
        "KAM020",
        &[stored_summary(
            &positivity,
            &march,
            "KAM020",
            &by_age(&positivity, &[("15-19", 9, Some(10))]),
        )],
    )
    .await;
    let mut tx = central.begin().await.expect("begin");
    let cells = explore(
        &mut tx,
        &[positivity],
        &[march],
        &[place("district-kampala", &["KAM013", "KAM020"])],
        &PopulationEstimates::default(),
    )
    .await
    .expect("explored");
    drop(tx);
    let json = cells_json(&cells, &BTreeMap::new(), Some(AGE));
    assert_eq!(json[0]["bands"][0]["value"], "83.33");
}

#[test]
fn a_question_splits_by_a_disaggregation_every_indicator_has() {
    use health_analytics_app::explore::read_question;
    use serde_json::json;
    let offered = [visits_by_age(), weeks()];
    let ask = |indicators: Vec<String>, by: serde_json::Value| {
        read_question(
            &json!({
                "indicators": indicators,
                "periodType": "monthly", "from": "2026-01", "to": "2026-03",
                "places": ["district-kampala"],
                "disaggregation": by,
            }),
            &offered,
        )
    };
    let question = ask(vec![visits_by_age().canonical()], json!(AGE)).expect("question");
    assert_eq!(question.disaggregation.as_deref(), Some(AGE));
    assert_eq!(
        ask(vec![visits_by_age().canonical()], json!(null))
            .expect("question")
            .disaggregation,
        None
    );
    // The median is not broken down by age, and a disaggregation is named by text.
    assert!(
        ask(
            vec![visits_by_age().canonical(), weeks().canonical()],
            json!(AGE)
        )
        .is_err()
    );
    assert!(ask(vec![visits_by_age().canonical()], json!("sex")).is_err());
    assert!(ask(vec![visits_by_age().canonical()], json!(3)).is_err());
    assert_eq!(
        indicator_json(&visits_by_age())["disaggregations"],
        json!([{ "name": AGE, "by": "age" }])
    );
}

/// ANC 1st visit coverage (#553): a place's first visits over its expected pregnancies for the
/// period, pro rata, from the year's Population package; a facility below the figures has *no
/// estimate*, never a guessed share.
#[tokio::test]
async fn coverage_is_each_place_s_count_over_its_estimate_for_the_period() {
    use health_analytics::{EstimateTypeDefinition, EstimatesDefinition, Figure as Given};
    let central = store().await;
    let march = month("2026-03");
    let coverage = Indicator {
        estimate: Some("expected-pregnancies".into()),
        ..indicator("anc1-coverage", Calculation::Percentage)
    };
    let first_visits = |count| Summary {
        value: None,
        denominator: None,
        ..summary(&coverage, count)
    };
    for (facility, count) in [("KAM013", 420), ("KAM020", 330)] {
        let report = measure_report(
            &coverage.canonical(),
            &march,
            facility,
            &first_visits(count),
            "2026-04-01T06:00:00Z",
        );
        synced(
            &central,
            facility,
            &[StoredSummary {
                measure: coverage.canonical(),
                period: march.clone(),
                facility: facility.into(),
                report: report.to_string(),
            }],
        )
        .await;
    }
    let parents = BTreeMap::from([
        ("KAM013", "district-kampala"),
        ("KAM020", "district-kampala"),
    ]);
    let estimates = PopulationEstimates::new(
        [EstimatesDefinition {
            year: 2026,
            types: vec![
                EstimateTypeDefinition {
                    code: "total-population".into(),
                    title: "Total population".into(),
                    derived_from: None,
                    percent: None,
                },
                EstimateTypeDefinition {
                    code: "expected-pregnancies".into(),
                    title: "Expected pregnancies".into(),
                    derived_from: Some("total-population".into()),
                    percent: Some("5".into()),
                },
            ],
            figures: vec![Given {
                place: "district-kampala".into(),
                kind: "total-population".into(),
                value: 1_797_722,
            }],
        }],
        |code| parents.get(code).map(|p| (*p).to_owned()),
    );
    let mut tx = central.begin().await.expect("begin");
    let cells = explore(
        &mut tx,
        std::slice::from_ref(&coverage),
        std::slice::from_ref(&march),
        &[
            place("district-kampala", &["KAM013", "KAM020"]),
            place("KAM013", &["KAM013"]),
        ],
        &estimates,
    )
    .await
    .expect("explored");
    drop(tx);
    let json = cells_json(&cells, &BTreeMap::new(), None);
    // 750 over 89,886.1 / 12 = 7,490.51 expected pregnancies in March: 10.01%.
    assert_eq!(json[0]["value"], "10.01", "{json}");
    assert_eq!(json[0]["count"], 750);
    assert_eq!(json[0]["estimate"]["type"], "expected-pregnancies");
    assert_eq!(json[0]["estimate"]["title"], "Expected pregnancies");
    assert_eq!(json[0]["estimate"]["value"], "7490.51");
    // The facility has no estimate of its own: no figure.
    assert_eq!(json[1]["value"], serde_json::Value::Null);
    assert_eq!(json[1]["count"], 420);
    assert_eq!(json[1]["estimate"]["value"], serde_json::Value::Null);
    assert_eq!(
        indicator_json(&coverage)["denominatorEstimate"],
        "expected-pregnancies"
    );
    assert_eq!(
        indicator_json(&visits())["denominatorEstimate"],
        serde_json::Value::Null
    );
}
