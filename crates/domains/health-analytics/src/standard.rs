//! Standard Dashboards (analytics.md § Dashboards; ADR 0075; #603): Dashboards that ship in a
//! Reporting package, versioned, reviewed and published like Reports, and shown to every user,
//! each seeing their own area.
//!
//! - **A standard Dashboard** ([`StandardDashboard`]) has an id, a title and its Visualizations
//!   in order ([`StandardItem`]). It is written as JSON (`configuration/dashboards/<id>.json`,
//!   schema `dashboard/1`) and read as written. Changing one is a new package version; a user
//!   holding `analytics.author` may *Save as* their own saved Dashboard instead.
//! - **A standard Visualization** ([`StandardVisualization`]) is the explorer's setup, with two
//!   differences, since one package serves every user for as long as it is active: its places
//!   always follow the viewer ([`FOLLOW`]: their own place, or each area under it), and its
//!   periods may be the last *n* of their type ([`StandardQuestion::last`]) rather than a fixed
//!   range.
//! - **Validation** ([`StandardDashboard::problems`]) checks a Dashboard against the indicators
//!   it can pin ([`Catalogue`]): every indicator is pinned to `url|version` and published (or in
//!   the package itself), and a disaggregation is one every indicator offers.

use std::collections::BTreeSet;

use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use crate::age_bands::is_pinned;
use crate::period::{MOST_PERIODS, Period, PeriodType, STANDARD_PERIODS};
use crate::report::Catalogue;

/// Where standard Dashboards live in a package: `configuration/dashboards/<id>.json`.
pub const DASHBOARDS_FOLDER: &str = "configuration/dashboards/";

/// A Visualization whose place is the viewer's own.
pub const MY_PLACE: &str = "myPlace";

/// A Visualization whose places are each area directly under the viewer's place.
pub const EACH_AREA_UNDER: &str = "eachAreaUnder";

/// Whose places a Visualization may follow.
pub const FOLLOW: [&str; 2] = [MY_PLACE, EACH_AREA_UNDER];

/// The kinds of Visualization the explorer draws (a map, #604: boundaries and facility points
/// shaded by threshold band).
pub const TYPES: [&str; 5] = ["column", "line", "table", "single", "map"];

/// How a table lays out its figures.
pub const LAYOUTS: [&str; 2] = ["placesByIndicators", "indicatorsByPeriods"];

/// What a chart's series are.
pub const SERIES: [&str; 3] = ["indicator", "place", "band"];

/// The most Visualizations a Dashboard holds.
pub const MOST_ITEMS: usize = 40;

/// The most indicators one Visualization shows, as the explorer asks.
pub const MOST_INDICATORS: usize = 12;

/// What a standard Visualization asks the explorer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandardQuestion {
    /// The indicators, each `url|version`.
    pub indicators: Vec<String>,
    /// The period type: one of [`STANDARD_PERIODS`].
    pub period_type: PeriodType,
    /// The last *n* periods of the type, up to the one holding today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<usize>,
    /// Or a fixed range's first period…
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// …and its last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// What each figure is split by: a disaggregation every indicator offers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disaggregation: Option<String>,
}

impl StandardQuestion {
    /// The first and last period it shows on `today`, or none when it names no usable range.
    pub fn range(&self, today: Date) -> Option<(String, String)> {
        match (self.last, &self.from, &self.to) {
            (Some(n), None, None) if (1..=MOST_PERIODS).contains(&n) => {
                let last = Period::of(self.period_type, today);
                let mut first = last.clone();
                for _ in 1..n {
                    first = first.previous()?;
                }
                Some((first.id, last.id))
            }
            (None, Some(from), Some(to)) => Period::range(self.period_type, from, to)
                .ok()
                .map(|_| (from.clone(), to.clone())),
            _ => None,
        }
    }
}

/// One Visualization of a standard Dashboard, as the explorer draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandardVisualization {
    /// One of [`TYPES`].
    #[serde(rename = "type")]
    pub kind: String,
    /// What it asks.
    pub question: StandardQuestion,
    /// One of [`LAYOUTS`].
    pub layout: String,
    /// One of [`SERIES`].
    pub series: String,
    /// Whose places it shows: one of [`FOLLOW`].
    pub follow: String,
}

/// One item of a standard Dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandardItem {
    /// Its id in the Dashboard.
    pub id: String,
    /// Its title.
    pub title: String,
    /// What it shows.
    pub visualization: StandardVisualization,
}

/// A Dashboard in a Reporting package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandardDashboard {
    /// Its id: its file name in [`DASHBOARDS_FOLDER`].
    pub id: String,
    /// Its title.
    pub title: String,
    /// What it is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Its Visualizations, in order.
    #[serde(default)]
    pub items: Vec<StandardItem>,
}

fn blank(text: &str) -> bool {
    text.trim().is_empty()
}

fn one_of(problems: &mut Vec<String>, name: &str, what: &str, value: &str, allowed: &[&str]) {
    if !allowed.contains(&value) {
        problems.push(format!(
            "{name}'s {what} {value} is not one of {}",
            allowed.join(", ")
        ));
    }
}

impl StandardDashboard {
    /// Every problem with it, given the indicators it can pin.
    pub fn problems(&self, catalogue: &Catalogue) -> Vec<String> {
        let mut problems = Vec::new();
        if blank(&self.id) {
            problems.push("the Dashboard has no id".to_owned());
        }
        if blank(&self.title) {
            problems.push("the Dashboard has no title".to_owned());
        }
        if self.items.is_empty() {
            problems.push("the Dashboard has no Visualizations".to_owned());
        }
        if self.items.len() > MOST_ITEMS {
            problems.push(format!(
                "the Dashboard holds {} Visualizations, more than {MOST_ITEMS}",
                self.items.len()
            ));
        }
        let mut ids = BTreeSet::new();
        for (n, item) in self.items.iter().enumerate() {
            let name = if blank(&item.title) {
                format!("Visualization {}", n + 1)
            } else {
                format!("Visualization {} ({})", n + 1, item.title)
            };
            if blank(&item.id) {
                problems.push(format!("{name} has no id"));
            } else if !ids.insert(item.id.as_str()) {
                problems.push(format!(
                    "{name} has the id {}, which another Visualization has",
                    item.id
                ));
            }
            if blank(&item.title) {
                problems.push(format!("{name} has no title"));
            }
            problems.extend(visualization_problems(
                &item.visualization,
                &name,
                catalogue,
            ));
        }
        problems
    }
}

fn visualization_problems(
    shown: &StandardVisualization,
    name: &str,
    catalogue: &Catalogue,
) -> Vec<String> {
    let mut problems = Vec::new();
    one_of(&mut problems, name, "type", &shown.kind, &TYPES);
    one_of(&mut problems, name, "layout", &shown.layout, &LAYOUTS);
    one_of(&mut problems, name, "series", &shown.series, &SERIES);
    if !FOLLOW.contains(&shown.follow.as_str()) {
        problems.push(format!(
            "{name} follows {}: a standard Dashboard shows each viewer their own area, so follow {MY_PLACE} or {EACH_AREA_UNDER}",
            shown.follow
        ));
    }
    let question = &shown.question;
    if question.indicators.is_empty() {
        problems.push(format!("{name} shows no indicator"));
    }
    if question.indicators.len() > MOST_INDICATORS {
        problems.push(format!(
            "{name} shows more than {MOST_INDICATORS} indicators"
        ));
    }
    for indicator in &question.indicators {
        if !is_pinned(indicator) {
            problems.push(format!(
                "{name}: the indicator {indicator} is not pinned to a version (url|version)"
            ));
        } else if !catalogue.indicators.contains_key(indicator) {
            problems.push(format!("{name}: no published indicator is {indicator}"));
        } else if let Some(by) = &question.disaggregation
            && !catalogue
                .disaggregations
                .get(indicator)
                .is_some_and(|names| names.contains(by))
        {
            problems.push(format!(
                "{name}: the indicator {indicator} is not disaggregated by {by}"
            ));
        }
    }
    if shown.series == "band" && question.disaggregation.is_none() {
        problems.push(format!("{name} has series by band but no disaggregation"));
    }
    if !STANDARD_PERIODS.contains(&question.period_type) {
        problems.push(format!(
            "{name}'s period type is not one dashboards offer: pick weekly, monthly, quarterly or financialJuly"
        ));
    } else {
        match (question.last, &question.from, &question.to) {
            (Some(n), None, None) if !(1..=MOST_PERIODS).contains(&n) => problems.push(format!(
                "{name} shows the last {n} periods: show 1 to {MOST_PERIODS}"
            )),
            (Some(_), None, None) => {}
            (None, Some(from), Some(to)) => {
                if let Err(why) = Period::range(question.period_type, from, to) {
                    problems.push(format!("{name}'s periods: {why}"));
                }
            }
            _ => problems.push(format!(
                "{name} needs its periods: either `last` or both `from` and `to`"
            )),
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::indicator::Calculation;

    const FIRST: &str = "https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0";
    const BANDS: &str = "https://health.go.ug/fhir/Library/age-bands-anc|1.0.0";

    fn catalogue() -> Catalogue {
        Catalogue {
            indicators: [(FIRST.to_owned(), Calculation::Count)].into(),
            disaggregations: [(FIRST.to_owned(), [BANDS.to_owned()].into())].into(),
            ..Catalogue::default()
        }
    }

    fn dashboard(edit: impl FnOnce(&mut Value)) -> StandardDashboard {
        let mut value = json!({
            "id": "anc-overview",
            "title": "ANC overview",
            "items": [{
                "id": "first-visits",
                "title": "ANC 1st visits by area",
                "visualization": {
                    "type": "column",
                    "question": { "indicators": [FIRST], "periodType": "monthly", "last": 6 },
                    "layout": "placesByIndicators",
                    "series": "place",
                    "follow": "eachAreaUnder"
                }
            }]
        });
        edit(&mut value);
        serde_json::from_value(value).expect("a standard Dashboard")
    }

    #[test]
    fn a_dashboard_over_published_indicators_has_no_problems() {
        assert_eq!(
            dashboard(|_| {}).problems(&catalogue()),
            Vec::<String>::new()
        );
        let banded = dashboard(|d| {
            d["items"][0]["visualization"]["question"]["disaggregation"] = json!(BANDS);
            d["items"][0]["visualization"]["series"] = json!("band");
        });
        assert_eq!(banded.problems(&catalogue()), Vec::<String>::new());
    }

    #[test]
    fn a_visualization_referencing_an_unknown_indicator_fails() {
        let unknown = dashboard(|d| {
            d["items"][0]["visualization"]["question"]["indicators"] = json!([
                "https://health.go.ug/fhir/Measure/anc-unknown|1.0.0",
                "https://x/Measure/m"
            ]);
        });
        assert_eq!(
            unknown.problems(&catalogue()),
            [
                "Visualization 1 (ANC 1st visits by area): no published indicator is https://health.go.ug/fhir/Measure/anc-unknown|1.0.0",
                "Visualization 1 (ANC 1st visits by area): the indicator https://x/Measure/m is not pinned to a version (url|version)",
            ]
        );
    }

    #[test]
    fn its_places_follow_the_viewer_and_its_setup_is_one_the_explorer_draws() {
        let named = dashboard(|d| {
            let shown = &mut d["items"][0]["visualization"];
            shown["follow"] = json!("KAM013");
            shown["type"] = json!("pie");
            shown["series"] = json!("band");
            shown["question"]["last"] = json!(0);
        });
        assert_eq!(
            named.problems(&catalogue()),
            [
                "Visualization 1 (ANC 1st visits by area)'s type pie is not one of column, line, table, single, map",
                "Visualization 1 (ANC 1st visits by area) follows KAM013: a standard Dashboard shows each viewer their own area, so follow myPlace or eachAreaUnder",
                "Visualization 1 (ANC 1st visits by area) has series by band but no disaggregation",
                "Visualization 1 (ANC 1st visits by area) shows the last 0 periods: show 1 to 106",
            ]
        );
        let undisaggregated = dashboard(|d| {
            d["items"][0]["visualization"]["question"]["disaggregation"] = json!("hiv-status");
        });
        assert_eq!(
            undisaggregated.problems(&catalogue()),
            [format!(
                "Visualization 1 (ANC 1st visits by area): the indicator {FIRST} is not disaggregated by hiv-status"
            )]
        );
    }

    #[test]
    fn a_dashboard_needs_a_title_items_and_ids_once_each() {
        let empty = dashboard(|d| {
            d["title"] = json!(" ");
            d["items"] = json!([]);
        });
        assert_eq!(
            empty.problems(&catalogue()),
            [
                "the Dashboard has no title",
                "the Dashboard has no Visualizations"
            ]
        );
        let twice = dashboard(|d| {
            let item = d["items"][0].clone();
            d["items"].as_array_mut().expect("items").push(item);
            d["items"][1]["visualization"]["question"] = json!({ "indicators": [FIRST], "periodType": "monthly", "from": "2026-03", "to": "2026-01" });
        });
        assert_eq!(
            twice.problems(&catalogue()),
            [
                "Visualization 2 (ANC 1st visits by area) has the id first-visits, which another Visualization has",
                "Visualization 2 (ANC 1st visits by area)'s periods: 2026-03 is after 2026-01",
            ]
        );
    }

    #[test]
    fn the_last_n_periods_end_with_the_one_holding_today() {
        let today = Date::constant(2026, 11, 23);
        let question = |last: Value, kind: &str| -> StandardQuestion {
            serde_json::from_value(
                json!({ "indicators": [FIRST], "periodType": kind, "last": last }),
            )
            .expect("question")
        };
        assert_eq!(
            question(json!(6), "monthly").range(today),
            Some(("2026-06".to_owned(), "2026-11".to_owned()))
        );
        assert_eq!(
            question(json!(4), "quarterly").range(today),
            Some(("2026Q1".to_owned(), "2026Q4".to_owned()))
        );
        assert_eq!(question(json!(0), "monthly").range(today), None);
        let fixed: StandardQuestion = serde_json::from_value(
            json!({ "indicators": [FIRST], "periodType": "monthly", "from": "2026-01", "to": "2026-03" }),
        )
        .expect("question");
        assert_eq!(
            fixed.range(today),
            Some(("2026-01".to_owned(), "2026-03".to_owned()))
        );
    }
}
