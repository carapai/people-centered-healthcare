use std::collections::{BTreeMap, BTreeSet};

use health_analytics::{
    Band, Calculation, Counts, Disaggregated, Disaggregation, DisaggregationSource,
    FacilitySummary, IndicatorRow, PeriodType, Summary,
};
use health_persistence::StoreError;
use rust_decimal::Decimal;

use super::*;
use crate::report::measure_report;

const HIV: &str = "hiv-status";

/// Summaries kept in memory, each facility synced at `synced`.
struct Kept {
    summaries: Vec<(String, Period, FacilitySummary)>,
    synced: BTreeMap<String, Timestamp>,
}

impl FacilitySummaries for Kept {
    async fn facility_summaries(
        &mut self,
        measure: &str,
        period: &Period,
    ) -> Result<Vec<FacilitySummary>, StoreError> {
        Ok(self
            .summaries
            .iter()
            .filter(|(m, p, _)| m == measure && p == period)
            .map(|(_, _, s)| s.clone())
            .collect())
    }

    async fn summaries_synced(&mut self) -> Result<BTreeMap<String, Timestamp>, StoreError> {
        Ok(self.synced.clone())
    }
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
        disaggregations: vec![Disaggregation {
            name: HIV.into(),
            source: DisaggregationSource::Coded("%record.code".into()),
        }],
        thresholds: Vec::new(),
        estimate: None,
        value: None,
    }
}

fn tested() -> Indicator {
    indicator("tested", Calculation::Percentage)
}

fn part(count: u64, denominator: u64) -> Summary {
    Summary {
        calculation: Calculation::Percentage,
        value: Some(Decimal::from(count * 100) / Decimal::from(denominator)),
        count,
        denominator: Some(denominator),
        values: None,
        disaggregations: Vec::new(),
    }
}

/// A percentage summary: its bands' parts, and the whole summed from them.
fn percentage(bands: &[(&str, u64, u64)]) -> Summary {
    let count = bands.iter().map(|b| b.1).sum();
    let denominator = bands.iter().map(|b| b.2).sum();
    Summary {
        disaggregations: vec![Disaggregated {
            name: HIV.into(),
            bands: bands
                .iter()
                .map(|(code, n, d)| Band {
                    code: (*code).into(),
                    summary: part(*n, *d),
                })
                .collect(),
        }],
        ..part(count, denominator)
    }
}

fn kept(facility: &str, period: &Period, summary: &Summary) -> (String, Period, FacilitySummary) {
    let measure = tested().canonical();
    let report = measure_report(&measure, period, facility, summary, "2026-04-01T06:00:00Z");
    (
        measure,
        period.clone(),
        FacilitySummary {
            facility: facility.into(),
            report: report.to_string(),
            computed_at: "2026-04-01T06:00:00Z".parse().expect("time"),
        },
    )
}

fn report() -> ReportDefinition {
    ReportDefinition {
        id: "tested".into(),
        title: "Tested".into(),
        period_type: PeriodType::Monthly,
        levels: vec![Level::Facility, Level::District],
        sections: vec![Section::Table(TableSection {
            id: "testing".into(),
            title: "Testing".into(),
            disaggregations: vec![SectionDisaggregation {
                age_band_set: None,
                code: Some(HIV.into()),
                expression: Some("%record.code".into()),
                axis: health_analytics::Axis::Columns,
            }],
            rows: vec![IndicatorRow {
                id: "tested".into(),
                indicator: tested().canonical(),
                label: "Tested (%)".into(),
            }],
            column_totals: false,
            row_totals: true,
            change: true,
            send_to_dhis2: false,
        })],
    }
}

/// #557: a district's row and column totals of a percentage are computed from the facilities'
/// summed parts, never by averaging their percentages; each band likewise, and the period before
/// for % change. A facility that never synced is missing, never zero.
#[tokio::test]
async fn a_district_s_totals_of_a_percentage_come_from_summed_parts() {
    let march = Period::parse(PeriodType::Monthly, "2026-03").expect("march");
    let february = march.previous().expect("february");
    // A: 1 of 2 positive (50%), and 9 of 10 negative; B: 30 of 40 positive, 0 of 8 negative.
    let a = percentage(&[("positive", 1, 2), ("negative", 9, 10)]);
    let b = percentage(&[("positive", 30, 40), ("negative", 0, 8)]);
    let synced = "2026-04-02T00:00:00Z".parse().expect("time");
    let mut store = Kept {
        summaries: vec![
            kept("A", &march, &a),
            kept("B", &march, &b),
            kept("A", &february, &percentage(&[("positive", 2, 4)])),
        ],
        synced: [("A".into(), synced), ("B".into(), synced)].into(),
    };
    let place = RunPlace {
        place: ExplorePlace {
            code: "district-kampala".into(),
            facilities: vec!["A".into(), "B".into(), "C".into()],
        },
        name: "Kampala".into(),
        level: Level::District,
    };
    let names = [("C".to_owned(), "Kitebi HC III".to_owned())].into();
    let with = RunWith {
        indicators: &[tested()],
        sets: &[],
        estimates: &PopulationEstimates::default(),
        names: &names,
    };
    let run = run_report(&mut store, &report(), &place, &march, &with, "central")
        .await
        .expect("run");

    let row = &run["sections"][0]["rows"][0];
    // The Total column: 40 of 60 tested, 66.67%; averaging 75% and 46.39% would give 60.69%.
    assert_eq!(row["current"]["value"], "66.67", "{run}");
    assert_eq!(row["current"]["count"], 40);
    assert_eq!(row["current"]["denominator"], 60);
    let bands = &row["current"]["disaggregations"][0];
    assert_eq!(bands["name"], HIV);
    // Positive: 31 of 42, not the average of 50% and 75%.
    assert_eq!(bands["bands"][0]["code"], "positive");
    assert_eq!(bands["bands"][0]["value"], "73.81");
    assert_eq!(bands["bands"][1]["value"], "50");
    // February, for % change: A's alone (B had nothing then and had synced since).
    assert_eq!(row["previous"]["value"], "50");
    // The section's bands: those found, then *not recorded*.
    let codes: Vec<&Value> = run["sections"][0]["disaggregations"][0]["bands"]
        .as_array()
        .expect("bands")
        .iter()
        .map(|b| &b["code"])
        .collect();
    assert_eq!(codes, ["positive", "negative", "not-recorded"]);
    // C never synced: missing, named, and not counted as zero.
    assert_eq!(run["completeness"]["reported"], 2);
    assert_eq!(run["completeness"]["expected"], 3);
    assert_eq!(run["completeness"]["missing"][0]["name"], "Kitebi HC III");
    assert_eq!(run["period"]["id"], "2026-03");
    assert_eq!(run["previous"]["id"], "2026-02");
    assert_eq!(run["place"]["level"], "district");
}

/// A median above facility level has no figure and says why; a row whose indicator is not
/// offered says so; a line list has no rows here.
#[tokio::test]
async fn what_a_report_cannot_show_says_why() {
    use health_analytics::{Aggregate, LineListSection};
    let march = Period::parse(PeriodType::Monthly, "2026-03").expect("march");
    let median = indicator("weeks", Calculation::Value(Aggregate::Median));
    let mut definition = report();
    let Section::Table(table) = &mut definition.sections[0] else {
        unreachable!()
    };
    table.rows.push(IndicatorRow {
        id: "weeks".into(),
        indicator: median.canonical(),
        label: "Weeks".into(),
    });
    table.rows.push(IndicatorRow {
        id: "gone".into(),
        indicator: "https://x/Measure/gone|1.0.0".into(),
        label: "Gone".into(),
    });
    let list: LineListSection = serde_json::from_value(json!({
        "id": "clients", "title": "Clients", "rowPer": "enrollment",
        "program": "https://x/PlanDefinition/anc", "columns": [],
    }))
    .expect("list");
    definition.sections.push(Section::LineList(list));
    let mut store = Kept {
        summaries: Vec::new(),
        synced: BTreeMap::new(),
    };
    let place = RunPlace {
        place: ExplorePlace {
            code: "district-kampala".into(),
            facilities: vec!["A".into(), "B".into()],
        },
        name: "Kampala".into(),
        level: Level::District,
    };
    let with = RunWith {
        indicators: &[tested(), median],
        sets: &[],
        estimates: &PopulationEstimates::default(),
        names: &BTreeMap::new(),
    };
    let run = run_report(&mut store, &definition, &place, &march, &with, "central")
        .await
        .expect("run");
    let rows = &run["sections"][0]["rows"];
    // Nobody synced: no figure rather than zero.
    assert_eq!(rows[0]["current"], Value::Null);
    assert_eq!(rows[1]["facilityOnly"], "median");
    assert_eq!(rows[1]["indicator"]["calculation"], "median");
    assert_eq!(
        rows[2]["error"],
        "no indicator is https://x/Measure/gone|1.0.0"
    );
    assert_eq!(run["sections"][1]["kind"], "lineList");
    assert_eq!(run["completeness"]["expected"], 2);
    assert_eq!(run["completeness"]["reported"], 0);
}
