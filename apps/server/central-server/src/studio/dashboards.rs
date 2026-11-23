//! The Dashboard designer's preview (studio.md § Dashboards; analytics.md § Dashboards; ADR 0075;
//! #879), at `POST /packages/{id}/dashboards/preview`. It takes a draft standard Dashboard as the
//! designer holds it, unsaved edits included, its file and the day its "last *n*" periods run up
//! to, and answers:
//!
//! - **Validation's problems**, each with the Visualization it is in, as publication words them
//!   (`health_publication::dashboard_problems`): its indicators are looked up in every published
//!   package version and the draft itself, so an unknown or unpinned indicator is refused here
//!   as publication would refuse it.
//! - **Each Visualization's figures** as the explorer answers them (indicators × periods ×
//!   places, each figure exact decimal text, split by its disaggregation's bands), over the
//!   package's sample data: the test patients (`tests/` files of type `test-patient`) of the
//!   Reporting package and of each package version holding one of its indicators, evaluated as
//!   *Try* evaluates an indicator. The sample is one place, [`SAMPLE`], whatever the
//!   Visualization follows; a single number also has the period before its last.
//! - **Each indicator**, as the explorer lists it, so the designer draws the figures with the
//!   explorer's own charts, tables and single numbers.
//!
//! An indicator is taken from the draft, else a published version, else another package's open
//! draft, so a Dashboard can be previewed while its indicators are still drafts; Validation
//! still asks for them to be published.

use std::collections::BTreeMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use health_access_policy::Subject;
use health_analytics::{AgeBandSet, Period, PeriodType, StandardDashboard, Summary};
use health_analytics_app::{Library, evaluate, explore::indicator_json};
use health_publication::dashboard_problems;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use super::reports::gather;
use super::{Studio, StudioProblem, author, failure, problem};

/// The place the sample data stands for in each answer.
pub(crate) const SAMPLE: &str = "sample";

/// The most figures one preview computes: each indicator over each period of each
/// Visualization, counted once however many Visualizations ask for it.
const MOST_FIGURES: usize = 2000;

/// The preview of a draft standard Dashboard.
#[derive(Debug, Clone, PartialEq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDashboardPreviewInput {
    /// The draft Dashboard, as the designer holds it (`dashboard/1`).
    #[schema(value_type = Object)]
    pub(crate) dashboard: Value,
    /// Its file in the package (`configuration/dashboards/<id>.json`).
    pub(crate) path: String,
    /// The day the "last *n*" periods run up to (`2026-04-30`).
    pub(crate) as_of: String,
}

/// One of Validation's problems with the Dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDashboardProblem {
    /// The Visualization it is in (its index), or none for the Dashboard itself.
    pub(crate) item: Option<u32>,
    /// What is wrong.
    pub(crate) message: String,
}

/// A disaggregation an indicator offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDashboardDisaggregation {
    /// Its name: an Age Band Set's `url|version`, or the code.
    pub(crate) name: String,
    /// `age` or `code`.
    pub(crate) by: String,
}

/// One of an indicator's threshold bands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDashboardThreshold {
    /// Its lower bound (inclusive), exact decimal text, none below every figure.
    pub(crate) from: Option<String>,
    /// Its upper bound (exclusive), none above every figure.
    pub(crate) to: Option<String>,
    /// `good`, `warning` or `bad`.
    pub(crate) tone: String,
}

/// An indicator, as the explorer lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDashboardIndicator {
    /// `url|version`.
    pub(crate) measure: String,
    /// Its title.
    pub(crate) title: String,
    /// What it calculates: `count`, `percentage`, `rate`, `ratio`, or a value's aggregate.
    pub(crate) calculation: String,
    /// A rate's factor.
    #[schema(value_type = Option<f64>)]
    pub(crate) per: Option<serde_json::Number>,
    /// False for a median or a distinct count.
    pub(crate) rolls_up: bool,
    /// Its threshold bands, lowest first.
    pub(crate) thresholds: Vec<StudioDashboardThreshold>,
    /// What it is broken down by.
    pub(crate) disaggregations: Vec<StudioDashboardDisaggregation>,
    /// The package version it was taken from (`name@version`).
    pub(crate) package: String,
    /// Whether that version is published (or is this draft); otherwise it is an open draft.
    pub(crate) published: bool,
}

/// A period of an answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDashboardPeriod {
    /// Its id (`2026-03`).
    pub(crate) id: String,
    /// Its first day.
    pub(crate) start: String,
    /// Its last day.
    pub(crate) end: String,
}

/// One band's figure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDashboardBand {
    /// Its code.
    pub(crate) code: String,
    /// Exact decimal text; none when the band's denominator is zero.
    pub(crate) value: Option<String>,
}

/// One indicator's figure for one period, at the sample place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDashboardCell {
    /// `url|version`.
    pub(crate) indicator: String,
    /// The period's id.
    pub(crate) period: String,
    /// Always `sample`: the test patients stand for one place.
    pub(crate) place: String,
    /// Exact decimal text; none when the denominator is zero or no value was taken.
    pub(crate) value: Option<String>,
    /// Split by the Visualization's disaggregation: each band's figure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bands: Option<Vec<StudioDashboardBand>>,
}

/// Figures as the explorer answers them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDashboardAnswer {
    /// The periods, first to last.
    pub(crate) periods: Vec<StudioDashboardPeriod>,
    /// Indicator by indicator, then period.
    pub(crate) cells: Vec<StudioDashboardCell>,
}

/// One Visualization's preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDashboardItem {
    /// The item's id.
    pub(crate) id: String,
    /// The first period shown, when its periods can be read.
    pub(crate) from: Option<String>,
    /// The last period shown.
    pub(crate) to: Option<String>,
    /// Its figures.
    pub(crate) answer: Option<StudioDashboardAnswer>,
    /// A single number's period before its last.
    pub(crate) before: Option<StudioDashboardAnswer>,
    /// Why it could not be computed, if it could not.
    pub(crate) error: Option<String>,
}

/// The preview of a draft standard Dashboard.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDashboardPreview {
    /// The day the periods run up to.
    pub(crate) as_of: String,
    /// Validation's problems, as they stand.
    pub(crate) problems: Vec<StudioDashboardProblem>,
    /// Each indicator a Visualization shows that was found, as the explorer lists it.
    pub(crate) indicators: Vec<StudioDashboardIndicator>,
    /// Each Visualization, in order.
    pub(crate) items: Vec<StudioDashboardItem>,
    /// The package versions whose test patients were read.
    pub(crate) sources: Vec<String>,
    /// How many test patients were read.
    pub(crate) patients: u64,
    /// Test patients that could not be read.
    pub(crate) errors: Vec<String>,
}

fn unprocessable(detail: impl Into<String>) -> Response {
    problem(StatusCode::UNPROCESSABLE_ENTITY, detail, None)
}

fn text(summary: &Summary) -> Option<String> {
    summary.value.map(|v| v.normalize().to_string())
}

fn period_of(period: &Period) -> StudioDashboardPeriod {
    StudioDashboardPeriod {
        id: period.id.clone(),
        start: period.start.to_string(),
        end: period.end.to_string(),
    }
}

/// Each indicator's summary over each period, computed once.
type Computed = BTreeMap<(String, String), Result<Summary, String>>;

fn answer(
    indicators: &[String],
    periods: &[Period],
    by: Option<&str>,
    computed: &Computed,
) -> Result<StudioDashboardAnswer, String> {
    let mut cells = Vec::new();
    for indicator in indicators {
        for period in periods {
            let summary = computed
                .get(&(indicator.clone(), period.id.clone()))
                .ok_or_else(|| format!("no indicator is {indicator}"))?
                .as_ref()
                .map_err(Clone::clone)?;
            cells.push(StudioDashboardCell {
                indicator: indicator.clone(),
                period: period.id.clone(),
                place: SAMPLE.to_owned(),
                value: text(summary),
                bands: by.map(|by| {
                    summary
                        .disaggregations
                        .iter()
                        .filter(|d| d.name == by)
                        .flat_map(|d| &d.bands)
                        .map(|b| StudioDashboardBand {
                            code: b.code.clone(),
                            value: text(&b.summary),
                        })
                        .collect()
                }),
            });
        }
    }
    Ok(StudioDashboardAnswer {
        periods: periods.iter().map(period_of).collect(),
        cells,
    })
}

/// Documents and serves the Dashboard designer's preview.
#[utoipa::path(post, path = "/api/studio/packages/{id}/dashboards/preview", tag = "studio", operation_id = "previewStudioDashboard",
    description = "The Dashboard designer's preview of a draft standard Dashboard (unsaved edits included) as of a day: Validation's problems as publication words them, each with the Visualization it is in, and each Visualization's figures as the explorer answers them (indicators × its periods, split by its disaggregation), computed over the package's test patients as *Try* computes an indicator, the sample standing for one place. Indicators are taken from the draft, else a published package version, else another package's open draft.",
    params(("id" = String, Path, description = "`name@version`")),
    request_body = StudioDashboardPreviewInput,
    responses(
        (status = 200, description = "The preview", body = StudioDashboardPreview),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version", body = StudioProblem),
        (status = 422, description = "The posted Dashboard cannot be read, or the day is not a date", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn preview(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(input): Json<StudioDashboardPreviewInput>,
) -> Response {
    if let Err(response) = author(subject) {
        return *response;
    }
    let dashboard: StandardDashboard = match serde_json::from_value(input.dashboard) {
        Ok(dashboard) => dashboard,
        Err(why) => return unprocessable(format!("the Dashboard cannot be read: {why}")),
    };
    let Ok(as_of) = input.as_of.parse::<Date>() else {
        return unprocessable(format!("{} is not a date (2026-04-30)", input.as_of));
    };
    let sources = match studio.publications.report_sources(&id).await {
        Ok(sources) => sources,
        Err(error) => return failure(&error),
    };
    let sets: Vec<AgeBandSet> = match studio.publications.age_band_sets().await {
        Ok(published) => published.into_iter().map(|p| p.set).collect(),
        Err(error) => return failure(&error),
    };
    let problems = dashboard_problems(&input.path, &dashboard, &sources.catalogue)
        .into_iter()
        .map(|p| StudioDashboardProblem {
            item: p.item.and_then(|n| u32::try_from(n).ok()),
            message: p.message,
        })
        .collect();
    let gathered = gather(&id, &sources, sets);
    let found = &gathered.found;
    let used: Vec<&str> = dashboard
        .items
        .iter()
        .flat_map(|item| item.visualization.question.indicators.iter())
        .map(String::as_str)
        .collect();
    let (read, patients, errors) = gathered.test_patients(&id, &used);
    let records: Vec<_> = patients.iter().map(|p| p.records.clone()).collect();
    let library = Library::new(found.values().map(|f| f.indicator.clone()))
        .with_age_band_sets(gathered.sets.clone());

    // Each item's periods, and the period before a single number's last.
    let mut planned = Vec::with_capacity(dashboard.items.len());
    for item in &dashboard.items {
        let question = &item.visualization.question;
        let range = question.range(as_of).and_then(|(from, to)| {
            Period::range(question.period_type, &from, &to)
                .ok()
                .map(|periods| (from, to, periods))
        });
        let before = match &range {
            Some((from, to, periods)) if item.visualization.kind == "single" && from == to => {
                periods.last().and_then(Period::previous)
            }
            _ => None,
        };
        planned.push((range, before));
    }
    let mut wanted: Vec<(String, Period)> = Vec::new();
    for (item, (range, before)) in dashboard.items.iter().zip(&planned) {
        let periods = range.iter().flat_map(|(_, _, p)| p).chain(before);
        for period in periods {
            for indicator in &item.visualization.question.indicators {
                if found.contains_key(indicator)
                    && !wanted
                        .iter()
                        .any(|(i, p)| i == indicator && p.id == period.id && p.kind == period.kind)
                {
                    wanted.push((indicator.clone(), period.clone()));
                }
            }
        }
    }
    let too_many = wanted.len() > MOST_FIGURES;
    let mut computed: BTreeMap<PeriodType, Computed> = BTreeMap::new();
    if !too_many {
        for (indicator, period) in &wanted {
            let Some(f) = found.get(indicator) else {
                continue;
            };
            let figure = evaluate(
                studio.engine.as_ref(),
                &library,
                &f.indicator,
                period,
                &records,
            )
            .await
            .map(|evaluation| evaluation.summary);
            computed
                .entry(period.kind)
                .or_default()
                .insert((indicator.clone(), period.id.clone()), figure);
        }
    }
    let empty = Computed::new();
    let items = dashboard
        .items
        .iter()
        .zip(planned)
        .map(|(item, (range, before))| {
            let question = &item.visualization.question;
            let none = |error: String| StudioDashboardItem {
                id: item.id.clone(),
                from: None,
                to: None,
                answer: None,
                before: None,
                error: Some(error),
            };
            let Some((from, to, periods)) = range else {
                return none("its periods cannot be read".to_owned());
            };
            if too_many {
                return none(format!(
                    "the Dashboard asks for more than {MOST_FIGURES} figures: preview fewer periods"
                ));
            }
            let computed = computed.get(&question.period_type).unwrap_or(&empty);
            let by = question.disaggregation.as_deref();
            let shown = answer(&question.indicators, &periods, by, computed);
            let before = before.and_then(|b| answer(&question.indicators, &[b], by, computed).ok());
            match shown {
                Ok(shown) => StudioDashboardItem {
                    id: item.id.clone(),
                    from: Some(from),
                    to: Some(to),
                    answer: Some(shown),
                    before,
                    error: None,
                },
                Err(why) => StudioDashboardItem {
                    from: Some(from),
                    to: Some(to),
                    ..none(why)
                },
            }
        })
        .collect();
    let mut listed: Vec<&str> = Vec::new();
    let indicators = used
        .iter()
        .filter(|m| {
            let first = !listed.contains(m);
            listed.push(m);
            first
        })
        .filter_map(|m| found.get(*m))
        .filter_map(|f| {
            let mut listed = indicator_json(&f.indicator);
            listed["package"] = Value::String(f.package.clone());
            listed["published"] = Value::Bool(f.published);
            serde_json::from_value(listed).ok()
        })
        .collect();
    Json(StudioDashboardPreview {
        as_of: as_of.to_string(),
        problems,
        indicators,
        items,
        sources: read,
        patients: u64::try_from(patients.len()).unwrap_or(u64::MAX),
        errors,
    })
    .into_response()
}
