//! The Report designer's preview (studio.md § Reports; ADR 0070, ADR 0072; #559), at
//! `POST /packages/{id}/reports/preview`. It takes the draft Report as the designer holds it,
//! unsaved edits included, and a period of the Report's type, and answers:
//!
//! - **Validation's problems**, each with the section it is in, as `health-publication`'s
//!   `reports` would refuse the Report: its indicators and Age Band Sets are looked up in every
//!   published package version and the draft itself.
//! - **Each table row's figures** for the period and, for a section with % change, the period
//!   before: the whole and each band of the indicator's disaggregations, with who was counted
//!   in which bands. Here the Studio reads test patients, not stored Indicator Reports: each
//!   indicator is evaluated as *Try* evaluates it (`health-analytics-app`, the evaluation every
//!   Facility Edge Node runs), over the test patients (`tests/` files of type `test-patient`)
//!   of the Reporting package and of each package version that holds one of its indicators. An
//!   indicator is taken from the draft, else a published version, else another package's open
//!   draft, so a Report can be previewed while its indicators are still drafts; Validation
//!   still asks for them to be published.
//! - **Each table section's bands**, in order, to lay out its rows and columns.
//!
//! The designer lays the cells out (rows, columns, totals, % change) and greys what a level
//! cannot show (medians and distinct counts above facility level, line lists), so choosing a
//! level asks nothing of the server. Rows of a line list are not previewed yet.

use std::collections::BTreeMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use health_access_policy::Subject;
use health_analytics::{
    AgeBandSet, Evaluation, Indicator, NOT_RECORDED, Period, PeriodType, Reason, ReportDefinition,
    Section, pinned,
};
use health_analytics_app::{
    Library, TestPatient, age_band_set, evaluate, is_age_band_set, is_test_patient, measure,
    test_patient,
};
use health_publication::Files;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use super::indicators::{
    StudioDisaggregation, StudioPeriod, StudioPeriodType, disaggregations, exact,
};
use super::{Studio, StudioProblem, author, failure, problem};

/// The preview of a draft Report.
#[derive(Debug, Clone, PartialEq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportPreviewInput {
    /// The draft Report, as the designer holds it (`report/1`).
    #[schema(value_type = Object)]
    pub(crate) report: Value,
    /// The period, of the Report's period type, written as its type writes it (`2026-03`).
    pub(crate) period: String,
}

/// One of Validation's problems with the Report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportProblem {
    /// The section it is in (its index), or none for the Report's settings.
    pub(crate) section: Option<u32>,
    /// What is wrong.
    pub(crate) message: String,
}

/// A band a section lays out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportBand {
    /// Its code: an age band's, a coded value, or `not-recorded`.
    pub(crate) code: String,
    /// What a person reads.
    pub(crate) label: String,
}

/// One of a section's Disaggregations, with its bands in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportBands {
    /// Its name, as an indicator's disaggregation is named: the Age Band Set's `url|version`,
    /// or the code.
    pub(crate) name: String,
    /// What a person reads: the Age Band Set's title, or the code.
    pub(crate) title: String,
    /// Its bands: an Age Band Set's in its order, or the codes the rows found, then
    /// `not-recorded`.
    pub(crate) bands: Vec<StudioReportBand>,
}

/// The indicator a row shows, as found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportIndicator {
    /// Its title.
    pub(crate) title: String,
    /// What it calculates: `count`, `percentage`, `rate`, `ratio`, `sum`, `average`,
    /// `median`, `minimum`, `maximum` or `distinct-count`.
    pub(crate) calculation: String,
    /// The package version it was taken from (`name@version`).
    pub(crate) package: String,
    /// Whether that version is published; otherwise it is an open draft.
    pub(crate) published: bool,
}

/// An indicator's figure over one period: the whole, and each band of its disaggregations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportFigures {
    /// The figure, to two decimal places; none when the denominator is zero or no value was
    /// taken.
    #[schema(value_type = Option<f64>)]
    pub(crate) value: Option<serde_json::Number>,
    /// How many are counted: the numerator of a percentage, rate or ratio.
    pub(crate) count: u64,
    /// The denominator of a percentage, rate or ratio.
    pub(crate) denominator: Option<u64>,
    /// The figure per band of each of the indicator's disaggregations.
    pub(crate) disaggregations: Vec<StudioDisaggregation>,
}

/// A test patient a row counted, and in which bands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportCounted {
    /// What the test patient shows.
    pub(crate) name: String,
    /// For each counted record, its band in each of the indicator's disaggregations, in order.
    pub(crate) bands: Vec<Vec<String>>,
}

/// One indicator row's preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportRow {
    /// The row's id.
    pub(crate) row: String,
    /// The indicator, if it was found.
    pub(crate) indicator: Option<StudioReportIndicator>,
    /// Its figures for the period.
    pub(crate) current: Option<StudioReportFigures>,
    /// Its figures for the period before, when the section shows % change.
    pub(crate) previous: Option<StudioReportFigures>,
    /// Who it counted in the period.
    pub(crate) counted: Vec<StudioReportCounted>,
    /// Why it could not be computed, if it could not.
    pub(crate) error: Option<String>,
}

/// One section's preview; a line list's is empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportSection {
    /// The section's Disaggregations, in order, with their bands.
    pub(crate) disaggregations: Vec<StudioReportBands>,
    /// Its rows, in order.
    pub(crate) rows: Vec<StudioReportRow>,
}

/// The preview of a draft Report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportPreview {
    /// The period computed.
    pub(crate) period: StudioPeriod,
    /// The period before, for % change.
    pub(crate) previous: Option<StudioPeriod>,
    /// Validation's problems, as they stand.
    pub(crate) problems: Vec<StudioReportProblem>,
    /// Each section, in order.
    pub(crate) sections: Vec<StudioReportSection>,
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

fn studio_period(kind: PeriodType, period: &Period) -> StudioPeriod {
    StudioPeriod {
        period_type: StudioPeriodType::from(kind),
        id: period.id.clone(),
        start: period.start.to_string(),
        end: period.end.to_string(),
    }
}

/// An indicator found, and where.
struct Found {
    indicator: Indicator,
    package: String,
    published: bool,
}

/// What a package version holds that the preview uses.
#[derive(Default)]
struct Held {
    indicators: Vec<Indicator>,
    sets: Vec<AgeBandSet>,
    patients: Vec<TestPatient>,
    errors: Vec<String>,
}

fn held(files: &Files) -> Held {
    let mut out = Held::default();
    for (path, bytes) in files {
        let Ok(file) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        if is_age_band_set(&file) {
            out.sets.extend(age_band_set(&file).age_band_set().ok());
        } else if file["resourceType"] == "Measure" && !path.contains('/') {
            out.indicators.extend(measure(&file).indicator().ok());
        } else if path.starts_with("tests/") && path.ends_with(".json") && is_test_patient(&file) {
            match test_patient(&file) {
                Ok(patient) => out.patients.push(patient),
                Err(why) => out.errors.push(format!("{path}: {why}")),
            }
        }
    }
    out
}

fn figures(
    indicator: &Indicator,
    evaluation: &Evaluation,
    sets: &[AgeBandSet],
) -> StudioReportFigures {
    let summary = &evaluation.summary;
    StudioReportFigures {
        value: summary.value.map(exact),
        count: summary.count,
        denominator: summary.denominator,
        disaggregations: disaggregations(indicator, summary, sets),
    }
}

fn counted(evaluation: &Evaluation, patients: &[TestPatient]) -> Vec<StudioReportCounted> {
    patients
        .iter()
        .zip(&evaluation.members)
        .filter(|(_, member)| member.count > 0)
        .map(|(patient, member)| StudioReportCounted {
            name: patient
                .records
                .name()
                .unwrap_or_else(|| patient.name.clone()),
            bands: member
                .records
                .iter()
                .filter(|record| record.reason == Reason::Meets)
                .map(|record| {
                    record
                        .strata
                        .iter()
                        .map(|s| match s {
                            Ok(Some(code)) => code.clone(),
                            _ => NOT_RECORDED.to_owned(),
                        })
                        .collect()
                })
                .collect(),
        })
        .collect()
}

/// The bands of a section's Disaggregation named `name`: an Age Band Set's from the set, a
/// coded value's from what the rows found.
fn section_bands(
    name: &str,
    is_set: bool,
    sets: &[AgeBandSet],
    rows: &[StudioReportRow],
) -> StudioReportBands {
    let not_recorded = || StudioReportBand {
        code: NOT_RECORDED.to_owned(),
        label: "Not recorded".to_owned(),
    };
    if is_set && let Ok(set) = pinned(name, sets) {
        let mut bands: Vec<StudioReportBand> = set
            .bands
            .iter()
            .map(|band| StudioReportBand {
                code: band.code(),
                label: band.label(set.unit),
            })
            .collect();
        bands.push(not_recorded());
        return StudioReportBands {
            name: name.to_owned(),
            title: set.title.clone(),
            bands,
        };
    }
    let mut bands: Vec<StudioReportBand> = Vec::new();
    for row in rows {
        for figures in row.current.iter().chain(&row.previous) {
            for by in figures.disaggregations.iter().filter(|d| d.name == name) {
                for band in &by.bands {
                    if band.code != NOT_RECORDED && !bands.iter().any(|b| b.code == band.code) {
                        bands.push(StudioReportBand {
                            code: band.code.clone(),
                            label: band.label.clone(),
                        });
                    }
                }
            }
        }
    }
    bands.push(not_recorded());
    StudioReportBands {
        name: name.to_owned(),
        title: name.to_owned(),
        bands,
    }
}

/// Documents and serves the Report designer's preview.
#[utoipa::path(post, path = "/api/studio/packages/{id}/reports/preview", tag = "studio", operation_id = "previewStudioReport",
    description = "The Report designer's preview of a draft Report (unsaved edits included) for a period of its type: Validation's problems, each with the section it is in, and each table row's figures (the whole and each band of the indicator's disaggregations, for the period and, with % change, the period before) with who was counted, computed over test patients as *Try* computes an indicator. Indicators are taken from the draft, else a published package version, else another package's open draft; the test patients are the Reporting package's and those of each package version holding one of its indicators. Line list rows are not previewed.",
    params(("id" = String, Path, description = "`name@version`")),
    request_body = StudioReportPreviewInput,
    responses(
        (status = 200, description = "The preview", body = StudioReportPreview),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version", body = StudioProblem),
        (status = 422, description = "The posted Report cannot be read, or the period is not one of its type", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn preview(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(input): Json<StudioReportPreviewInput>,
) -> Response {
    if let Err(response) = author(subject) {
        return *response;
    }
    let report: ReportDefinition = match serde_json::from_value(input.report) {
        Ok(report) => report,
        Err(why) => return unprocessable(format!("the Report cannot be read: {why}")),
    };
    let period = match Period::parse(report.period_type, &input.period) {
        Ok(period) => period,
        Err(error) => return unprocessable(error.to_string()),
    };
    let sources = match studio.publications.report_sources(&id).await {
        Ok(sources) => sources,
        Err(error) => return failure(&error),
    };
    let mut sets: Vec<AgeBandSet> = match studio.publications.age_band_sets().await {
        Ok(published) => published.into_iter().map(|p| p.set).collect(),
        Err(error) => return failure(&error),
    };
    let problems = report
        .located_problems(&sources.catalogue)
        .into_iter()
        .map(|p| StudioReportProblem {
            section: p.section.and_then(|n| u32::try_from(n).ok()),
            message: p.message,
        })
        .collect();

    // The draft first, then published versions, then other open drafts: the first holding an
    // indicator is where it is taken from.
    let mut holders = vec![(id.clone(), false, held(&sources.draft))];
    holders.extend(
        sources
            .packages
            .iter()
            .map(|p| (p.id.clone(), p.published, held(&p.files))),
    );
    let mut found: BTreeMap<String, Found> = BTreeMap::new();
    for (package, published, holding) in &holders {
        for indicator in &holding.indicators {
            found.entry(indicator.canonical()).or_insert_with(|| Found {
                indicator: indicator.clone(),
                package: package.clone(),
                published: *published || *package == id,
            });
        }
        for set in &holding.sets {
            if !sets.iter().any(|s| s.canonical() == set.canonical()) {
                sets.push(set.clone());
            }
        }
    }
    // The test patients: the draft's, and those of each package holding a used indicator.
    let used: Vec<&str> = report
        .sections
        .iter()
        .flat_map(|section| match section {
            Section::Table(table) => table.rows.iter().map(|r| r.indicator.as_str()).collect(),
            Section::LineList(list) => list.counted_by.as_deref().into_iter().collect::<Vec<_>>(),
        })
        .collect();
    let mut read = vec![id.clone()];
    for reference in &used {
        if let Some(f) = found.get(*reference)
            && !read.contains(&f.package)
        {
            read.push(f.package.clone());
        }
    }
    let mut patients: Vec<TestPatient> = Vec::new();
    let mut errors = Vec::new();
    for (package, _, holding) in &holders {
        if read.contains(package) {
            patients.extend(holding.patients.iter().cloned());
            errors.extend(holding.errors.iter().map(|e| format!("{package}: {e}")));
        }
    }
    let records: Vec<_> = patients.iter().map(|p| p.records.clone()).collect();
    let library =
        Library::new(found.values().map(|f| f.indicator.clone())).with_age_band_sets(sets.clone());
    let previous = period.previous();

    let mut sections = Vec::with_capacity(report.sections.len());
    for section in &report.sections {
        let Section::Table(table) = section else {
            sections.push(StudioReportSection {
                disaggregations: Vec::new(),
                rows: Vec::new(),
            });
            continue;
        };
        let mut rows = Vec::with_capacity(table.rows.len());
        for row in &table.rows {
            let Some(f) = found.get(&row.indicator) else {
                rows.push(StudioReportRow {
                    row: row.id.clone(),
                    indicator: None,
                    current: None,
                    previous: None,
                    counted: Vec::new(),
                    error: Some(format!("no indicator is {}", row.indicator)),
                });
                continue;
            };
            let indicator = StudioReportIndicator {
                title: f.indicator.title.clone(),
                calculation: f.indicator.calculation.code().to_owned(),
                package: f.package.clone(),
                published: f.published,
            };
            let now = evaluate(
                studio.engine.as_ref(),
                &library,
                &f.indicator,
                &period,
                &records,
            )
            .await;
            let before = match (&previous, table.change) {
                (Some(previous), true) => Some(
                    evaluate(
                        studio.engine.as_ref(),
                        &library,
                        &f.indicator,
                        previous,
                        &records,
                    )
                    .await,
                ),
                _ => None,
            };
            rows.push(match now {
                Ok(now) => StudioReportRow {
                    row: row.id.clone(),
                    indicator: Some(indicator),
                    current: Some(figures(&f.indicator, &now, &sets)),
                    previous: before
                        .and_then(Result::ok)
                        .map(|b| figures(&f.indicator, &b, &sets)),
                    counted: counted(&now, &patients),
                    error: None,
                },
                Err(why) => StudioReportRow {
                    row: row.id.clone(),
                    indicator: Some(indicator),
                    current: None,
                    previous: None,
                    counted: Vec::new(),
                    error: Some(why),
                },
            });
        }
        let disaggregations = table
            .disaggregations
            .iter()
            .filter_map(|d| {
                let name = d.age_band_set.as_deref().or(d.code.as_deref())?;
                Some(section_bands(name, d.age_band_set.is_some(), &sets, &rows))
            })
            .collect();
        sections.push(StudioReportSection {
            disaggregations,
            rows,
        });
    }
    Json(StudioReportPreview {
        period: studio_period(report.period_type, &period),
        previous: previous
            .as_ref()
            .map(|p| studio_period(report.period_type, p)),
        problems,
        sections,
        sources: read,
        patients: u64::try_from(patients.len()).unwrap_or(u64::MAX),
        errors,
    })
    .into_response()
}
