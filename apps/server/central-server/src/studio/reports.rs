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
//! level asks nothing of the server. A cell crossing several Disaggregations (#867) is counted
//! here: each row's figures carry every crossed cell the section lays out, so the crossed cells
//! across one Disaggregation add up to the band of the others.
//!
//! **A line list's rows** (#867) are read over the same test patients, at facility level, as
//! `health-analytics-app`'s `line_list` reads them. The program is its PlanDefinition (a
//! ProgramPlanDefinition), taken from the draft, else a published version, else another
//! package's open draft, and its episode type is what makes a test patient's EpisodeOfCare an
//! enrollment in it; the test patients of the package holding it are read too. *Only those
//! counted by* evaluates the indicator over the period first.

use std::collections::BTreeMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use health_access_policy::Subject;
use health_analytics::{
    AgeBandSet, Axis, Evaluation, Indicator, LineListSection, NOT_RECORDED, Period, PeriodType,
    Reason, ReportDefinition, Section, TableSection, pinned,
};
use health_analytics_app::line_list::{self, CellValue, ProgramType};
use health_analytics_app::{
    Library, PatientRecords, TestPatient, age_band_set, evaluate, is_age_band_set, is_test_patient,
    measure, test_patient,
};
use health_publication::{Files, ReportSources};
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
    /// The figure at each cell crossing several of the section's Disaggregations (#867): its
    /// row bands crossed, its column bands crossed, and both crossed, for those the indicator
    /// is broken down by.
    pub(crate) crossed: Vec<StudioReportCell>,
}

/// One band of one of a section's Disaggregations, as a crossed cell is at it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportAt {
    /// The Disaggregation's name: the Age Band Set's `url|version`, or the code.
    pub(crate) name: String,
    /// The band's code.
    pub(crate) code: String,
}

/// An indicator's figure at one band of each of several Disaggregations: what was counted in
/// all of them, a percentage's numerator and denominator both within the cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportCell {
    /// The bands it is at, in the section's order.
    pub(crate) at: Vec<StudioReportAt>,
    /// The figure, to two decimal places; none when the denominator is zero or no value was
    /// taken.
    #[schema(value_type = Option<f64>)]
    pub(crate) value: Option<serde_json::Number>,
    /// How many are counted: the numerator of a percentage, rate or ratio.
    pub(crate) count: u64,
    /// The denominator of a percentage, rate or ratio.
    pub(crate) denominator: Option<u64>,
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

/// The program a line list names, as found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportProgram {
    /// Its title, else its name.
    pub(crate) title: String,
    /// What a person reads for its enrollments' type.
    pub(crate) episode_type: String,
    /// The package version it was taken from (`name@version`).
    pub(crate) package: String,
    /// Whether that version is published (or is the draft itself); otherwise it is another
    /// package's open draft.
    pub(crate) published: bool,
}

/// One value of a line list cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportListValue {
    /// What a person reads.
    pub(crate) value: String,
    /// When it was recorded (`YYYY-MM-DD`), for a concept's value.
    pub(crate) date: Option<String>,
}

/// One row of a line list's preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportListRow {
    /// The test patient it is of, as they show.
    pub(crate) patient: String,
    /// The enrollment (`EpisodeOfCare/…`) or visit (`Encounter/…`) it is.
    pub(crate) record: String,
    /// Each column's values, in the section's order: none when it has no value, several for a
    /// concept's *all*.
    pub(crate) cells: Vec<Vec<StudioReportListValue>>,
}

/// A line list's preview: its rows over the test patients, at facility level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportList {
    /// The program, if it was found.
    pub(crate) program: Option<StudioReportProgram>,
    /// Its rows, in order.
    pub(crate) rows: Vec<StudioReportListRow>,
    /// Why it could not be read, if it could not.
    pub(crate) error: Option<String>,
}

/// One section's preview: a table's Disaggregations and rows, or a line list's rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioReportSection {
    /// The section's Disaggregations, in order, with their bands.
    pub(crate) disaggregations: Vec<StudioReportBands>,
    /// Its rows, in order.
    pub(crate) rows: Vec<StudioReportRow>,
    /// A line list's rows.
    pub(crate) list: Option<StudioReportList>,
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
pub(super) struct Found {
    /// The indicator.
    pub(super) indicator: Indicator,
    /// The package version it was taken from.
    pub(super) package: String,
    /// Whether that version is published (or is the draft itself).
    pub(super) published: bool,
}

/// What a package version holds that the preview uses.
#[derive(Default)]
struct Held {
    indicators: Vec<Indicator>,
    sets: Vec<AgeBandSet>,
    /// Its PlanDefinitions.
    plans: Vec<Value>,
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
        } else if file["resourceType"] == "PlanDefinition" && !path.contains('/') {
            out.plans.push(file);
        } else if path.starts_with("tests/") && path.ends_with(".json") && is_test_patient(&file) {
            match test_patient(&file) {
                Ok(patient) => out.patients.push(patient),
                Err(why) => out.errors.push(format!("{path}: {why}")),
            }
        }
    }
    out
}

/// The indicators and Age Band Sets a preview computes with, and the test patients it may read.
pub(super) struct Gathered {
    /// Each package version, the draft first, then published versions, then other open
    /// drafts, with whether it is published and what it holds.
    holders: Vec<(String, bool, Held)>,
    /// Each indicator by `url|version`, from the first holder holding it.
    pub(super) found: BTreeMap<String, Found>,
    /// Every published Age Band Set, and those the holders add.
    pub(super) sets: Vec<AgeBandSet>,
}

/// What draft `id` can compute with, from `sources` and the published Age Band Sets `sets`:
/// an indicator is taken from the draft, else a published version, else another package's
/// open draft.
pub(super) fn gather(id: &str, sources: &ReportSources, mut sets: Vec<AgeBandSet>) -> Gathered {
    let mut holders = vec![(id.to_owned(), false, held(&sources.draft))];
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
                published: *published || package == id,
            });
        }
        for set in &holding.sets {
            if !sets.iter().any(|s| s.canonical() == set.canonical()) {
                sets.push(set.clone());
            }
        }
    }
    Gathered {
        holders,
        found,
        sets,
    }
}

impl Gathered {
    /// The test patients to read for indicators `used`: the draft's, and those of each package
    /// version holding one of them; with the versions read and the patients that cannot be.
    pub(super) fn test_patients(
        &self,
        id: &str,
        used: &[&str],
    ) -> (Vec<String>, Vec<TestPatient>, Vec<String>) {
        self.test_patients_with(id, used, &[])
    }

    /// [`Self::test_patients`], and those of the package versions `also` (each holding a line
    /// list's program).
    fn test_patients_with(
        &self,
        id: &str,
        used: &[&str],
        also: &[&str],
    ) -> (Vec<String>, Vec<TestPatient>, Vec<String>) {
        let mut read = vec![id.to_owned()];
        for package in also {
            if !read.iter().any(|r| r == package) {
                read.push((*package).to_owned());
            }
        }
        for reference in used {
            if let Some(f) = self.found.get(*reference)
                && !read.contains(&f.package)
            {
                read.push(f.package.clone());
            }
        }
        let mut patients: Vec<TestPatient> = Vec::new();
        let mut errors = Vec::new();
        for (package, _, holding) in &self.holders {
            if read.contains(package) {
                patients.extend(holding.patients.iter().cloned());
                errors.extend(holding.errors.iter().map(|e| format!("{package}: {e}")));
            }
        }
        (read, patients, errors)
    }

    /// The program `canonical` names (`url`, or `url|version`): its PlanDefinition, from the
    /// first holder holding it, with the package version and whether it is published.
    fn program(&self, id: &str, canonical: &str) -> Option<(&Value, &str, bool)> {
        let (url, version) = match canonical.split_once('|') {
            Some((url, version)) => (url, Some(version)),
            None => (canonical, None),
        };
        self.holders
            .iter()
            .find_map(|(package, published, holding)| {
                holding
                    .plans
                    .iter()
                    .find(|p| p["url"] == url && version.is_none_or(|v| p["version"] == v))
                    .map(|plan| (plan, package.as_str(), *published || package == id))
            })
    }
}

/// A line list's program, as the preview shows it and as its enrollments are matched.
fn list_program(
    plan: &Value,
    package: &str,
    published: bool,
) -> Result<(StudioReportProgram, ProgramType), String> {
    let program = health_programs_app::program(plan).map_err(|e| e.to_string())?;
    let shown = StudioReportProgram {
        title: plan["title"]
            .as_str()
            .or_else(|| plan["name"].as_str())
            .unwrap_or(program.url.as_str())
            .to_owned(),
        episode_type: program.episode_type.text.clone(),
        package: package.to_owned(),
        published,
    };
    let kind = ProgramType {
        url: program.url,
        system: program.episode_type.system,
        code: program.episode_type.code,
        text: program.episode_type.text,
    };
    Ok((shown, kind))
}

fn list_value(value: CellValue) -> StudioReportListValue {
    StudioReportListValue {
        value: value.value,
        date: value.date.map(|d| d.to_string()),
    }
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
        crossed: Vec::new(),
    }
}

/// Which of a table section's Disaggregations its cells cross, by name: those on rows, those
/// on columns, and all of them, each when it is two or more.
fn crossings(table: &TableSection) -> Vec<Vec<&str>> {
    let names = |axis: Option<Axis>| -> Vec<&str> {
        table
            .disaggregations
            .iter()
            .filter(|d| axis.is_none_or(|a| d.axis == a))
            .filter_map(|d| d.age_band_set.as_deref().or(d.code.as_deref()))
            .collect()
    };
    let mut out: Vec<Vec<&str>> = Vec::new();
    for names in [
        names(Some(Axis::Rows)),
        names(Some(Axis::Columns)),
        names(None),
    ] {
        if names.len() > 1 && !out.contains(&names) {
            out.push(names);
        }
    }
    out
}

/// The indicator's figure at every cell crossing each of `crossings`, over the bands the
/// section lays out; a crossing with a Disaggregation the indicator is not broken down by is
/// left out (the designer says so).
fn crossed(
    indicator: &Indicator,
    evaluation: &Evaluation,
    crossings: &[Vec<&str>],
    bands: &[StudioReportBands],
) -> Vec<StudioReportCell> {
    let mut cells = Vec::new();
    for names in crossings {
        let Some(sides) = names
            .iter()
            .map(|name| {
                let d = indicator
                    .disaggregations
                    .iter()
                    .position(|d| d.name == *name)?;
                let codes: Vec<&str> = bands
                    .iter()
                    .find(|b| b.name == *name)?
                    .bands
                    .iter()
                    .map(|b| b.code.as_str())
                    .collect();
                Some((*name, d, codes))
            })
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let combos = sides
            .iter()
            .fold(vec![Vec::new()], |combos, (_, d, codes)| {
                combos
                    .iter()
                    .flat_map(|combo: &Vec<(usize, &str)>| {
                        codes.iter().map(move |code| {
                            let mut next = combo.clone();
                            next.push((*d, *code));
                            next
                        })
                    })
                    .collect()
            });
        for combo in combos {
            let summary = evaluation.crossed(indicator, &combo);
            cells.push(StudioReportCell {
                at: sides
                    .iter()
                    .zip(&combo)
                    .map(|((name, _, _), (_, code))| StudioReportAt {
                        name: (*name).to_owned(),
                        code: (*code).to_owned(),
                    })
                    .collect(),
                value: summary.value.map(exact),
                count: summary.count,
                denominator: summary.denominator,
            });
        }
    }
    cells
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
    description = "The Report designer's preview of a draft Report (unsaved edits included) for a period of its type: Validation's problems, each with the section it is in, and each table row's figures (the whole, each band of the indicator's disaggregations, and each cell crossing several of the section's Disaggregations, for the period and, with % change, the period before) with who was counted, computed over test patients as *Try* computes an indicator. Indicators are taken from the draft, else a published package version, else another package's open draft; the test patients are the Reporting package's and those of each package version holding one of its indicators or a line list's program. Each line list's rows (one per enrollment in its program, matched by the program's episode type, or per visit) are read over the same test patients at facility level, with each column's value.",
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
    let sets: Vec<AgeBandSet> = match studio.publications.age_band_sets().await {
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

    let gathered = gather(&id, &sources, sets);
    let sets = gathered.sets.clone();
    let found = &gathered.found;
    // The test patients: the draft's, and those of each package holding a used indicator.
    let used: Vec<&str> = report
        .sections
        .iter()
        .flat_map(|section| match section {
            Section::Table(table) => table.rows.iter().map(|r| r.indicator.as_str()).collect(),
            Section::LineList(list) => list.counted_by.as_deref().into_iter().collect::<Vec<_>>(),
        })
        .collect();
    // And those of each package holding a line list's program.
    let programs: Vec<&str> = report
        .sections
        .iter()
        .filter_map(|section| match section {
            Section::LineList(list) => gathered.program(&id, &list.program).map(|(_, p, _)| p),
            Section::Table(_) => None,
        })
        .collect();
    let (read, patients, errors) = gathered.test_patients_with(&id, &used, &programs);
    let records: Vec<_> = patients.iter().map(|p| p.records.clone()).collect();
    let library =
        Library::new(found.values().map(|f| f.indicator.clone())).with_age_band_sets(sets.clone());
    let previous = period.previous();

    let mut sections = Vec::with_capacity(report.sections.len());
    for section in &report.sections {
        let table = match section {
            Section::Table(table) => table,
            Section::LineList(list) => {
                let list =
                    list_preview(&studio, &id, &gathered, &library, list, &period, &patients).await;
                sections.push(StudioReportSection {
                    disaggregations: Vec::new(),
                    rows: Vec::new(),
                    list: Some(list),
                });
                continue;
            }
        };
        let mut rows = Vec::with_capacity(table.rows.len());
        // Each row's evaluations, now and before, to cross its bands once they are known.
        let mut evaluations: Vec<Option<(&Indicator, Evaluation, Option<Evaluation>)>> =
            Vec::with_capacity(table.rows.len());
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
                evaluations.push(None);
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
            let before = before.and_then(Result::ok);
            match now {
                Ok(now) => {
                    rows.push(StudioReportRow {
                        row: row.id.clone(),
                        indicator: Some(indicator),
                        current: Some(figures(&f.indicator, &now, &sets)),
                        previous: before.as_ref().map(|b| figures(&f.indicator, b, &sets)),
                        counted: counted(&now, &patients),
                        error: None,
                    });
                    evaluations.push(Some((&f.indicator, now, before)));
                }
                Err(why) => {
                    rows.push(StudioReportRow {
                        row: row.id.clone(),
                        indicator: Some(indicator),
                        current: None,
                        previous: None,
                        counted: Vec::new(),
                        error: Some(why),
                    });
                    evaluations.push(None);
                }
            }
        }
        let disaggregations: Vec<StudioReportBands> = table
            .disaggregations
            .iter()
            .filter_map(|d| {
                let name = d.age_band_set.as_deref().or(d.code.as_deref())?;
                Some(section_bands(name, d.age_band_set.is_some(), &sets, &rows))
            })
            .collect();
        let crossings = crossings(table);
        if !crossings.is_empty() {
            for (row, evaluated) in rows.iter_mut().zip(&evaluations) {
                let Some((indicator, now, before)) = evaluated else {
                    continue;
                };
                if let Some(current) = &mut row.current {
                    current.crossed = crossed(indicator, now, &crossings, &disaggregations);
                }
                if let (Some(previous), Some(before)) = (&mut row.previous, before) {
                    previous.crossed = crossed(indicator, before, &crossings, &disaggregations);
                }
            }
        }
        sections.push(StudioReportSection {
            disaggregations,
            rows,
            list: None,
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

/// A line list's rows over the test patients, for `period`.
async fn list_preview(
    studio: &Studio,
    id: &str,
    gathered: &Gathered,
    library: &Library,
    list: &LineListSection,
    period: &Period,
    patients: &[TestPatient],
) -> StudioReportList {
    let records: Vec<PatientRecords> = patients.iter().map(|p| p.records.clone()).collect();
    let failed = |program: Option<StudioReportProgram>, why: String| StudioReportList {
        program,
        rows: Vec::new(),
        error: Some(why),
    };
    let Some((plan, package, published)) = gathered.program(id, &list.program) else {
        return failed(None, format!("no program is {}", list.program));
    };
    let (shown, kind) = match list_program(plan, package, published) {
        Ok(found) => found,
        Err(why) => {
            return failed(
                None,
                format!("{} is not a usable program: {why}", list.program),
            );
        }
    };
    let counted = match &list.counted_by {
        None => None,
        Some(reference) => {
            let Some(f) = gathered.found.get(reference) else {
                return failed(Some(shown), format!("no indicator is {reference}"));
            };
            match evaluate(
                studio.engine.as_ref(),
                library,
                &f.indicator,
                period,
                &records,
            )
            .await
            {
                Ok(evaluation) => Some(line_list::counted(&evaluation)),
                Err(why) => return failed(Some(shown), why),
            }
        }
    };
    let rows = line_list::rows(list, &kind, &records, period, counted.as_ref())
        .into_iter()
        .map(|row| StudioReportListRow {
            patient: patients
                .iter()
                .find(|p| p.records.reference() == row.patient)
                .map_or_else(
                    || row.patient.clone(),
                    |p| p.records.name().unwrap_or_else(|| p.name.clone()),
                ),
            record: row.record,
            cells: row
                .cells
                .into_iter()
                .map(|cell| cell.into_iter().map(list_value).collect())
                .collect(),
        })
        .collect();
    StudioReportList {
        program: Some(shown),
        rows,
        error: None,
    }
}
