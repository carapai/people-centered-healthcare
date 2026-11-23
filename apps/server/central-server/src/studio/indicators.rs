//! The indicator editor's *Try* (studio.md § Indicators; ADR 0068; #548), at
//! `POST /packages/{id}/indicators/try`. It takes the draft `Measure` as the Studio holds it,
//! unsaved edits included, and a period type and period as a Report would choose them, and
//! evaluates it with `health-analytics-app` on central's FHIRPath engine (the evaluation every
//! Facility Edge Node runs) over the package's test patients (`tests/` files of type
//! `test-patient`). The answer is the value with what roll-up needs (the count or numerator,
//! the denominator, the values' count, sum, minimum and maximum) and every test patient, counted
//! or not, with each record's part: why they are counted or not, its value, and whether that
//! value was the one taken (#549). Criteria may read `%periodStart`, `%periodEnd` and
//! `%previousPeriods`, and refer to another indicator of the package by `url|version`
//! (`%indicators`), which is evaluated over the same period and test patients first (#550).
//! The figure is also given per band of each of the indicator's disaggregations (#552): an Age
//! Band Set pinned to `url|version`, looked up among every published version and the draft's own
//! sets, or a coded value, each with a *not recorded* band; each record says its band. The editor
//! may also post each condition of the numerator's and denominator's trees, and each record then
//! says whether each held (#864).

use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use health_access_policy::Subject;
use health_analytics::{
    AgeBandSet, Calculation, DisaggregationSource, Indicator, NOT_RECORDED, Observed, Period,
    PeriodType, Reason, Summary, pinned,
};
use health_analytics_app::{
    Library, PatientRecords, age_band_set, evaluate, explain, is_age_band_set, is_test_patient,
    measure, test_patient,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use super::{Studio, StudioProblem, author, failure, problem};

/// A kind of reporting period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum StudioPeriodType {
    /// An ISO week, Monday to Sunday: `2026W10`.
    Weekly,
    /// A calendar month: `2026-03`.
    Monthly,
    /// A calendar quarter: `2026Q1`.
    Quarterly,
    /// A half year: `2026S1`.
    SemiAnnual,
    /// A calendar year: `2026`.
    Annual,
    /// A financial year from July: `2025July` is July 2025 to June 2026.
    FinancialJuly,
}

impl From<PeriodType> for StudioPeriodType {
    fn from(value: PeriodType) -> Self {
        match value {
            PeriodType::Weekly => Self::Weekly,
            PeriodType::Monthly => Self::Monthly,
            PeriodType::Quarterly => Self::Quarterly,
            PeriodType::SemiAnnual => Self::SemiAnnual,
            PeriodType::Annual => Self::Annual,
            PeriodType::FinancialJuly => Self::FinancialJuly,
        }
    }
}

impl From<StudioPeriodType> for PeriodType {
    fn from(value: StudioPeriodType) -> Self {
        match value {
            StudioPeriodType::Weekly => Self::Weekly,
            StudioPeriodType::Monthly => Self::Monthly,
            StudioPeriodType::Quarterly => Self::Quarterly,
            StudioPeriodType::SemiAnnual => Self::SemiAnnual,
            StudioPeriodType::Annual => Self::Annual,
            StudioPeriodType::FinancialJuly => Self::FinancialJuly,
        }
    }
}

/// *Try* on a draft indicator.
#[derive(Debug, Clone, PartialEq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioIndicatorTryInput {
    /// The draft Measure, as the Studio holds it.
    #[schema(value_type = Object)]
    pub(crate) measure: Value,
    /// The period type a Report would choose.
    pub(crate) period_type: StudioPeriodType,
    /// The period, written as its type writes it (`2026-03`, `2026W10`, `2026Q1`, …).
    pub(crate) period: String,
    /// Conditions of the criteria to explain (#864): FHIRPath, each one condition of the
    /// numerator's or denominator's tree as the editor writes it. Each record answers whether
    /// each held, in this order.
    #[serde(default)]
    pub(crate) conditions: Vec<String>,
}

/// Whether one of the asked conditions held on a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StudioHeld {
    /// It gave true.
    Held,
    /// It gave false or nothing.
    NotHeld,
    /// It could not be evaluated, or gave something other than true or false.
    Error,
}

/// The period evaluated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioPeriod {
    /// Its type.
    pub(crate) period_type: StudioPeriodType,
    /// Its id.
    pub(crate) id: String,
    /// Its first day (`%periodStart`).
    pub(crate) start: String,
    /// Its last day (`%periodEnd`).
    pub(crate) end: String,
}

/// Why a record counts or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StudioRecordReason {
    /// In the period and in the numerator (and the denominator, if there is one): it counts.
    Meets,
    /// In the period and the denominator, but not the numerator.
    DenominatorOnly,
    /// In the period, but does not meet the criteria (of the denominator, if there is one).
    DoesNotMeet,
    /// Counting enrollments, it belongs to none.
    NoEnrollment,
    /// Its period date is outside the period.
    OutsidePeriod,
    /// It has no period date.
    NoPeriodDate,
    /// The period date or the criteria could not be evaluated on it.
    Error,
}

/// One record's part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioIndicatorRecord {
    /// The record (`Encounter/…`).
    pub(crate) reference: String,
    /// Its period date, if it has one.
    pub(crate) date: Option<String>,
    /// Why it counts or not.
    pub(crate) reason: StudioRecordReason,
    /// Why it could not be evaluated, if it could not.
    pub(crate) error: Option<String>,
    /// Its value, if the indicator takes one and it gave one: a number, or a code for a
    /// distinct count.
    pub(crate) value: Option<String>,
    /// Whether its value is the one taken for its counted thing (its person's or enrollment's
    /// first or last).
    pub(crate) picked: bool,
    /// Its band in each of the indicator's disaggregations, in order: a band's code, or
    /// `not-recorded` (#552).
    pub(crate) bands: Vec<String>,
    /// Whether each asked condition held on it, in the order asked (#864).
    pub(crate) conditions: Vec<StudioHeld>,
}

/// One test patient, counted or not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioIndicatorPatient {
    /// The test patient's file.
    pub(crate) path: String,
    /// What the test patient shows.
    pub(crate) name: String,
    /// The patient (`Patient/…`).
    pub(crate) patient: String,
    /// The patient's name, if they have one.
    pub(crate) display: Option<String>,
    /// Whether they are counted (in the numerator, if there is a denominator).
    pub(crate) counted: bool,
    /// How many of their counted things are counted: once counting people, once per counted
    /// enrollment, visit or observation.
    pub(crate) count: u64,
    /// How many of their counted things are in the denominator, if there is one.
    pub(crate) denominator: Option<u64>,
    /// Each of their records of the indicator's basis, with its part.
    pub(crate) records: Vec<StudioIndicatorRecord>,
}

/// What *Try* found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioIndicatorTry {
    /// The period evaluated.
    pub(crate) period: StudioPeriod,
    /// What the indicator calculates: `count`, `percentage`, `rate`, `ratio`, `sum`,
    /// `average`, `median`, `minimum`, `maximum` or `distinct-count`.
    pub(crate) calculation: String,
    /// A rate's factor (per 1,000, …).
    pub(crate) rate_per: Option<u32>,
    /// The figure, to two decimal places; none when the denominator is zero or no value was
    /// taken.
    #[schema(value_type = Option<f64>)]
    pub(crate) value: Option<serde_json::Number>,
    /// How many are counted: the numerator of a percentage, rate or ratio.
    pub(crate) count: u64,
    /// The denominator of a percentage, rate or ratio.
    pub(crate) denominator: Option<u64>,
    /// The values taken, for a sum, average, median, minimum, maximum or distinct count.
    pub(crate) values: Option<StudioIndicatorValues>,
    /// The figure per band of each of the indicator's disaggregations, in order (#552).
    pub(crate) disaggregations: Vec<StudioDisaggregation>,
    /// Every test patient, counted or not, in file order.
    pub(crate) patients: Vec<StudioIndicatorPatient>,
    /// Test patients that could not be read, and records that could not be evaluated.
    pub(crate) errors: Vec<String>,
}

/// The figure per band of one disaggregation: the bands add up to the whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDisaggregation {
    /// Its name: the stratifier's code, or the Age Band Set's `url|version`.
    pub(crate) name: String,
    /// What a person reads: the Age Band Set's title, or the name.
    pub(crate) title: String,
    /// Its bands: an Age Band Set's in its order, or the codes found, then `not-recorded`.
    pub(crate) bands: Vec<StudioBand>,
}

/// One band's figure, its numerator and denominator both within the band.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioBand {
    /// Its code: an age band's (`15-19`, `25+`), a coded value, or `not-recorded`.
    pub(crate) code: String,
    /// What a person reads: `15–19 years`, the code, or *Not recorded*.
    pub(crate) label: String,
    /// The figure within the band, to two decimal places.
    #[schema(value_type = Option<f64>)]
    pub(crate) value: Option<serde_json::Number>,
    /// How many are counted in the band: its numerator.
    pub(crate) count: u64,
    /// The band's denominator, for a percentage, rate or ratio.
    pub(crate) denominator: Option<u64>,
    /// The values taken in the band.
    pub(crate) values: Option<StudioIndicatorValues>,
}

/// The values a continuous-variable indicator took, as roll-up needs them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioIndicatorValues {
    /// How many.
    pub(crate) count: u64,
    /// Their sum, if they are numbers.
    #[schema(value_type = Option<f64>)]
    pub(crate) sum: Option<serde_json::Number>,
    /// The smallest, if they are numbers.
    #[schema(value_type = Option<f64>)]
    pub(crate) minimum: Option<serde_json::Number>,
    /// The largest, if they are numbers.
    #[schema(value_type = Option<f64>)]
    pub(crate) maximum: Option<serde_json::Number>,
}

/// A decimal as an exact JSON number.
pub(crate) fn exact(value: Decimal) -> serde_json::Number {
    value
        .normalize()
        .to_string()
        .parse()
        .unwrap_or_else(|_| serde_json::Number::from(0))
}

fn shown_value(value: &Result<Option<Observed>, String>) -> Option<String> {
    match value {
        Ok(Some(Observed::Number(number))) => Some(number.normalize().to_string()),
        Ok(Some(Observed::Code(code))) => Some(code.clone()),
        _ => None,
    }
}

fn values(summary: &Summary) -> Option<StudioIndicatorValues> {
    summary.values.as_ref().map(|v| StudioIndicatorValues {
        count: v.count,
        sum: v.sum.map(exact),
        minimum: v.minimum.map(exact),
        maximum: v.maximum.map(exact),
    })
}

/// The disaggregations of `summary`, titled and labelled from the indicator and its sets.
pub(crate) fn disaggregations(
    indicator: &Indicator,
    summary: &Summary,
    sets: &[AgeBandSet],
) -> Vec<StudioDisaggregation> {
    indicator
        .disaggregations
        .iter()
        .zip(&summary.disaggregations)
        .map(|(disaggregation, by)| {
            let set = match &disaggregation.source {
                DisaggregationSource::AgeBands(reference) => pinned(reference, sets).ok(),
                DisaggregationSource::Coded(_) => None,
            };
            StudioDisaggregation {
                name: by.name.clone(),
                title: set.map_or_else(|| by.name.clone(), |s| s.title.clone()),
                bands: by
                    .bands
                    .iter()
                    .map(|band| StudioBand {
                        code: band.code.clone(),
                        label: if band.code == NOT_RECORDED {
                            "Not recorded".to_owned()
                        } else {
                            set.and_then(|s| {
                                s.bands
                                    .iter()
                                    .find(|b| b.code() == band.code)
                                    .map(|b| b.label(s.unit))
                            })
                            .unwrap_or_else(|| band.code.clone())
                        },
                        value: band.summary.value.map(exact),
                        count: band.summary.count,
                        denominator: band.summary.denominator,
                        values: values(&band.summary),
                    })
                    .collect(),
            }
        })
        .collect()
}

fn unprocessable(detail: impl Into<String>) -> Response {
    problem(StatusCode::UNPROCESSABLE_ENTITY, detail, None)
}

fn shown(reason: &Reason) -> (StudioRecordReason, Option<String>) {
    match reason {
        Reason::Meets => (StudioRecordReason::Meets, None),
        Reason::DenominatorOnly => (StudioRecordReason::DenominatorOnly, None),
        Reason::DoesNotMeet => (StudioRecordReason::DoesNotMeet, None),
        Reason::NoEnrollment => (StudioRecordReason::NoEnrollment, None),
        Reason::OutsidePeriod => (StudioRecordReason::OutsidePeriod, None),
        Reason::NoPeriodDate => (StudioRecordReason::NoPeriodDate, None),
        Reason::Error(why) => (StudioRecordReason::Error, Some(why.clone())),
    }
}

/// Documents and serves *Try*.
#[utoipa::path(post, path = "/api/studio/packages/{id}/indicators/try", tag = "studio", operation_id = "tryStudioIndicator",
    description = "*Try* on a draft indicator: the server evaluates the posted (unsaved) Measure over the package version's test patients (`tests/` files of type `test-patient`) for a period type and period as a Report would choose them, on the FHIRPath engine every node runs. Each record of the Measure's basis is placed in the period by its period date (up to its end, cumulative), and the criteria decide whether it counts; people and enrollments count once each, visits and observations each one. A percentage, rate or ratio's numerator is within its denominator; a sum, average, median, minimum, maximum or distinct count takes each counted thing's first or last value. Criteria may compare dates with `%periodStart` and `%periodEnd` (`YYYY-MM-DD`) and `%previousPeriods.start` (the starts of the periods before, the nearest first), and may require a patient to be in another indicator of the package's numerator or denominator (`'<url>|<version>' in %indicators.numerator`), which is evaluated first. It answers the figure with what roll-up needs and every test patient with each record's part, so the editor shows who is counted and why; given `conditions` (each one condition of the criteria, as FHIRPath), each record also says whether each held.",
    params(("id" = String, Path, description = "`name@version`")),
    request_body = StudioIndicatorTryInput,
    responses(
        (status = 200, description = "The value and who it counts", body = StudioIndicatorTry),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version", body = StudioProblem),
        (status = 422, description = "The Measure is not a usable indicator, the period is not one of its type, or it refers to an indicator the package does not hold or in a circle", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn try_indicator(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(input): Json<StudioIndicatorTryInput>,
) -> Response {
    if let Err(response) = author(subject) {
        return *response;
    }
    let files = match studio.publications.draft_files(&id).await {
        Ok((files, _)) => files,
        Err(error) => return failure(&error),
    };
    let indicator = match measure(&input.measure).indicator() {
        Ok(indicator) => indicator,
        Err(problems) => {
            return unprocessable(format!(
                "the Measure is not a usable indicator: {}",
                problems.join("; ")
            ));
        }
    };
    let period = match Period::parse(input.period_type.into(), &input.period) {
        Ok(period) => period,
        Err(error) => return unprocessable(error.to_string()),
    };
    let mut sets: Vec<AgeBandSet> = match studio.publications.age_band_sets().await {
        Ok(published) => published.into_iter().map(|p| p.set).collect(),
        Err(error) => return failure(&error),
    };
    let mut errors = Vec::new();
    let mut tested = Vec::new();
    let mut indicators = Vec::new();
    for (path, bytes) in &files {
        let Ok(file) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        if is_age_band_set(&file) {
            // The draft's own sets, as it holds them, stand in for a published version.
            if let Ok(set) = age_band_set(&file).age_band_set() {
                sets.retain(|s| s.canonical() != set.canonical());
                sets.push(set);
            }
            continue;
        }
        if file["resourceType"] == "Measure" {
            indicators.extend(measure(&file).indicator().ok());
            continue;
        }
        if !path.starts_with("tests/") || !path.ends_with(".json") {
            continue;
        }
        if !is_test_patient(&file) {
            continue;
        }
        match test_patient(&file) {
            Ok(patient) => tested.push((path.clone(), patient)),
            Err(problem) => errors.push(format!("{path}: {problem}")),
        }
    }
    let records: Vec<PatientRecords> = tested.iter().map(|(_, t)| t.records.clone()).collect();
    // The posted Measure stands in for its saved file.
    indicators.push(indicator.clone());
    let library = Library::new(indicators).with_age_band_sets(sets.clone());
    let count = match evaluate(
        studio.engine.as_ref(),
        &library,
        &indicator,
        &period,
        &records,
    )
    .await
    {
        Ok(count) => count,
        Err(why) => return unprocessable(why),
    };
    let held = match explain(
        studio.engine.as_ref(),
        &library,
        &indicator,
        &period,
        &records,
        &input.conditions,
    )
    .await
    {
        Ok(held) => held,
        Err(why) => return unprocessable(why),
    };
    errors.extend(
        count
            .errors()
            .map(|(record, why)| format!("{record}: {why}")),
    );
    let patients = tested
        .iter()
        .zip(&count.members)
        .zip(&held)
        .map(|(((path, tested), member), held)| StudioIndicatorPatient {
            path: path.clone(),
            name: tested.name.clone(),
            patient: member.patient.clone(),
            display: tested.records.name(),
            counted: member.count > 0,
            count: member.count,
            denominator: member.denominator,
            records: member
                .records
                .iter()
                .zip(held)
                .map(|(record, held)| {
                    let (reason, error) = shown(&record.reason);
                    StudioIndicatorRecord {
                        reference: record.reference.clone(),
                        date: record.date.map(|d| d.to_string()),
                        reason,
                        error,
                        value: shown_value(&record.value),
                        picked: record.picked,
                        bands: record
                            .strata
                            .iter()
                            .map(|s| match s {
                                Ok(Some(code)) => code.clone(),
                                _ => NOT_RECORDED.to_owned(),
                            })
                            .collect(),
                        conditions: held
                            .iter()
                            .map(|h| match h {
                                Ok(true) => StudioHeld::Held,
                                Ok(false) => StudioHeld::NotHeld,
                                Err(_) => StudioHeld::Error,
                            })
                            .collect(),
                    }
                })
                .collect(),
        })
        .collect();
    let summary = &count.summary;
    Json(StudioIndicatorTry {
        period: StudioPeriod {
            period_type: input.period_type,
            id: period.id.clone(),
            start: period.start.to_string(),
            end: period.end.to_string(),
        },
        calculation: summary.calculation.code().to_owned(),
        rate_per: match summary.calculation {
            Calculation::Rate { per } => Some(per),
            _ => None,
        },
        value: summary.value.map(exact),
        count: summary.count,
        denominator: summary.denominator,
        values: values(summary),
        disaggregations: disaggregations(&indicator, summary, &sets),
        patients,
        errors,
    })
    .into_response()
}

/// Where a Report lays out an indicator: one row of a table section (#864).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioIndicatorMapping {
    /// The package version the Report is in, `name@version`.
    pub(crate) package_id: String,
    /// Whether that version is published; otherwise it is the package's open draft.
    pub(crate) published: bool,
    /// The Report's id: its file is `configuration/reports/<id>.json`.
    pub(crate) report: String,
    /// The Report's title (e.g. *HMIS 105 · 2 Antenatal care*).
    pub(crate) title: String,
    /// The section's title.
    pub(crate) section: String,
    /// The row's label (e.g. *ANC 1st visit for women*).
    pub(crate) label: String,
    /// The version of the indicator the row pins, `url|version`.
    pub(crate) indicator: String,
    /// Whether the section's cells are sent to DHIS2.
    pub(crate) sent_to_dhis2: bool,
}

/// Documents and serves an indicator's HMIS mapping.
#[utoipa::path(get, path = "/api/studio/indicator-mappings", tag = "studio", operation_id = "listStudioIndicatorMappings",
    description = "An indicator's HMIS mapping, read-only (studio.md § Indicators): every row of a Report table section that lays out the indicator `url`, whichever version it pins, in each package's newest published version and its open draft, by package version, then Report, section and row. Mappings are edited in the Reporting package's Report designer.",
    params(("url" = String, Query, description = "The indicator's canonical URL, without a version")),
    responses(
        (status = 200, description = "Where it is laid out", body = [StudioIndicatorMapping]),
        (status = 400, description = "No `url`", body = StudioProblem),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn mappings(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    RawQuery(query): RawQuery,
) -> Response {
    if let Err(response) = author(subject) {
        return *response;
    }
    let url = form_urlencoded::parse(query.as_deref().unwrap_or("").as_bytes())
        .find(|(name, _)| name == "url")
        .map(|(_, value)| value.into_owned())
        .filter(|url| !url.trim().is_empty());
    let Some(url) = url else {
        return problem(StatusCode::BAD_REQUEST, "name the indicator's url", None);
    };
    match studio.publications.indicator_mappings(&url).await {
        Ok(found) => Json(
            found
                .into_iter()
                .map(|m| StudioIndicatorMapping {
                    package_id: m.package,
                    published: m.published,
                    report: m.report,
                    title: m.title,
                    section: m.section,
                    label: m.label,
                    indicator: m.indicator,
                    sent_to_dhis2: m.sent_to_dhis2,
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => failure(&error),
    }
}
