//! The indicator editor's *Try* (studio.md § Indicators; ADR 0068; #548), at
//! `POST /packages/{id}/indicators/try`. It takes the draft `Measure` as the Studio holds it,
//! unsaved edits included, and a period type and period as a Report would choose them, and
//! evaluates it with `health-analytics-app` on central's FHIRPath engine (the evaluation every
//! Facility Edge Node runs) over the package's test patients (`tests/` files of type
//! `test-patient`). The answer is the value with what roll-up needs (the count or numerator,
//! the denominator, the values' count, sum, minimum and maximum) and every test patient, counted
//! or not, with each record's part: why they are counted or not, its value, and whether that
//! value was the one taken (#549).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use health_access_policy::Subject;
use health_analytics::{Calculation, Observed, Period, PeriodType, Reason};
use health_analytics_app::{PatientRecords, evaluate, is_test_patient, measure, test_patient};
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
    /// Every test patient, counted or not, in file order.
    pub(crate) patients: Vec<StudioIndicatorPatient>,
    /// Test patients that could not be read, and records that could not be evaluated.
    pub(crate) errors: Vec<String>,
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
fn exact(value: Decimal) -> serde_json::Number {
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
    description = "*Try* on a draft indicator: the server evaluates the posted (unsaved) Measure over the package version's test patients (`tests/` files of type `test-patient`) for a period type and period as a Report would choose them, on the FHIRPath engine every node runs. Each record of the Measure's basis is placed in the period by its period date (up to its end, cumulative), and the criteria decide whether it counts; people and enrollments count once each, visits and observations each one. A percentage, rate or ratio's numerator is within its denominator; a sum, average, median, minimum, maximum or distinct count takes each counted thing's first or last value. It answers the figure with what roll-up needs and every test patient with each record's part, so the editor shows who is counted and why.",
    params(("id" = String, Path, description = "`name@version`")),
    request_body = StudioIndicatorTryInput,
    responses(
        (status = 200, description = "The value and who it counts", body = StudioIndicatorTry),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version", body = StudioProblem),
        (status = 422, description = "The Measure is not a usable indicator, or the period is not one of its type", body = StudioProblem),
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
    let mut errors = Vec::new();
    let mut tested = Vec::new();
    for (path, bytes) in &files {
        if !path.starts_with("tests/") || !path.ends_with(".json") {
            continue;
        }
        let Ok(file) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        if !is_test_patient(&file) {
            continue;
        }
        match test_patient(&file) {
            Ok(patient) => tested.push((path.clone(), patient)),
            Err(problem) => errors.push(format!("{path}: {problem}")),
        }
    }
    let records: Vec<PatientRecords> = tested.iter().map(|(_, t)| t.records.clone()).collect();
    let count = evaluate(studio.engine.as_ref(), &indicator, &period, &records).await;
    errors.extend(
        count
            .errors()
            .map(|(record, why)| format!("{record}: {why}")),
    );
    let patients = tested
        .iter()
        .zip(&count.members)
        .map(|((path, tested), member)| StudioIndicatorPatient {
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
                .map(|record| {
                    let (reason, error) = shown(&record.reason);
                    StudioIndicatorRecord {
                        reference: record.reference.clone(),
                        date: record.date.map(|d| d.to_string()),
                        reason,
                        error,
                        value: shown_value(&record.value),
                        picked: record.picked,
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
        values: summary.values.as_ref().map(|v| StudioIndicatorValues {
            count: v.count,
            sum: v.sum.map(exact),
            minimum: v.minimum.map(exact),
            maximum: v.maximum.map(exact),
        }),
        patients,
        errors,
    })
    .into_response()
}
