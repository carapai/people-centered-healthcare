//! Indicators at validation (analytics.md; studio.md § Indicators; ADR 0068; #548): every
//! `Measure` in a package is an indicator, and must be a usable one: a period date that places
//! care in a Report's period, a scoring with the populations it needs, a basis, and FHIRPath
//! criteria. Every test patient in `tests/` (`type` `test-patient`) must hold one Patient and
//! their records, as *Try* evaluates indicators over them. Indicators that refer to each other in
//! a circle (#550) are refused: each would need the other's membership first. At simulation,
//! each saved *Try* (`indicator-test`, #537) is evaluated again and must give what it kept.

use std::collections::{BTreeMap, BTreeSet};

use health_analytics::{Period, PeriodType, circles};
use health_analytics_app::{Library, evaluate, is_test_patient, measure, test_patient};
use health_fhir_validation::FhirPathEngine;
use health_packages::Packed;
use serde_json::Value;

/// Every problem with the package's indicators and test patients, each prefixed with its file.
pub(crate) fn problems(packed: &Packed) -> Vec<String> {
    let mut problems = Vec::new();
    let mut refers = BTreeMap::new();
    let mut paths = BTreeMap::new();
    for artefact in &packed.contents.artefacts {
        if artefact.resource_type != "Measure" {
            continue;
        }
        let Some(value) = packed
            .files
            .get(&artefact.path)
            .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok())
        else {
            continue;
        };
        let definition = measure(&value);
        if let Ok(indicator) = definition.indicator() {
            refers.insert(
                indicator.canonical(),
                indicator
                    .references
                    .iter()
                    .map(|r| r.measure.clone())
                    .collect::<BTreeSet<_>>(),
            );
            paths.insert(indicator.canonical(), artefact.path.clone());
        }
        problems.extend(
            definition
                .problems()
                .into_iter()
                .map(|problem| format!("{}: {problem}", artefact.path)),
        );
    }
    for circle in circles(&refers) {
        // One refers to itself is already a problem of its own.
        if circle.len() < 2 {
            continue;
        }
        let files: Vec<&str> = circle.iter().map(|c| paths[c].as_str()).collect();
        problems.push(format!(
            "{}: the indicators {} refer to each other in a circle",
            files.join(", "),
            circle.join(", ")
        ));
    }
    for (path, bytes) in &packed.files {
        if !path.starts_with("tests/") || !path.ends_with(".json") {
            continue;
        }
        let Ok(file) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        if is_test_patient(&file)
            && let Err(problem) = test_patient(&file)
        {
            problems.push(format!("{path}: {problem}"));
        }
    }
    problems
}

/// The `type` of an indicator's saved *Try* in `tests/` (#537): the figure it gave for a period
/// over the package's test patients.
pub(crate) const INDICATOR_TEST_TYPE: &str = "indicator-test";

/// An indicator's saved *Try*: what *Save as test* writes.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndicatorTest {
    /// What it is called.
    name: String,
    /// The indicator, `url|version`.
    measure: String,
    /// The period type and period it was tried for.
    period_type: PeriodType,
    period: String,
    /// What it gave.
    expect: Expected,
}

/// The figure a test keeps: the count (the numerator), the denominator if there is one, and
/// the value as its exact decimal text.
#[derive(Debug, Clone, serde::Deserialize)]
struct Expected {
    count: u64,
    #[serde(default)]
    denominator: Option<u64>,
    #[serde(default)]
    value: Option<String>,
}

/// Runs a package's indicator tests (`indicator-test`, #537) at simulation: each indicator is
/// evaluated again for its test's period over the package's test patients, on the FHIRPath
/// engine every node runs, and a count, denominator or value other than the one kept fails it.
/// Disaggregations are not compared, so a pinned Age Band Set another package holds is not
/// needed.
pub(crate) async fn run_tests(engine: &dyn FhirPathEngine, package: &Packed) -> Vec<String> {
    let mut tests = Vec::new();
    let mut patients = Vec::new();
    let mut indicators = Vec::new();
    for (path, bytes) in &package.files {
        let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        if value["resourceType"] == "Measure" && !path.contains('/') {
            if let Ok(mut indicator) = measure(&value).indicator() {
                indicator.disaggregations.clear();
                indicators.push(indicator);
            }
            continue;
        }
        if !path.starts_with("tests/") || !path.ends_with(".json") {
            continue;
        }
        if is_test_patient(&value) {
            patients.extend(test_patient(&value).ok().map(|t| t.records));
        } else if value["type"] == INDICATOR_TEST_TYPE {
            tests.push((path.clone(), value));
        }
    }
    if tests.is_empty() {
        return Vec::new();
    }
    let library = Library::new(indicators.clone());
    let mut failed = Vec::new();
    for (path, value) in tests {
        let test = match serde_json::from_value::<IndicatorTest>(value) {
            Ok(test) => test,
            Err(error) => {
                failed.push(format!("{path}: not an indicator test: {error}"));
                continue;
            }
        };
        let Some(indicator) = indicators.iter().find(|i| i.canonical() == test.measure) else {
            failed.push(format!(
                "{path}: the package holds no indicator {}",
                test.measure
            ));
            continue;
        };
        let period = match Period::parse(test.period_type, &test.period) {
            Ok(period) => period,
            Err(error) => {
                failed.push(format!("{path}: {error}"));
                continue;
            }
        };
        let summary = match evaluate(engine, &library, indicator, &period, &patients).await {
            Ok(evaluation) => evaluation.summary,
            Err(error) => {
                failed.push(format!("{path}: {}: {error}", test.name));
                continue;
            }
        };
        let value = summary.value.map(|v| v.normalize().to_string());
        let expected = &test.expect;
        if summary.count != expected.count
            || summary.denominator != expected.denominator
            || value != expected.value
        {
            let shown = |count: u64, denominator: Option<u64>, value: Option<&String>| {
                format!(
                    "{} (count {count}{})",
                    value.map_or("no value", String::as_str),
                    denominator.map_or_else(String::new, |d| format!(", denominator {d}"))
                )
            };
            failed.push(format!(
                "{path}: {}: {} for {} expected {}, got {}",
                test.name,
                test.measure,
                test.period,
                shown(
                    expected.count,
                    expected.denominator,
                    expected.value.as_ref()
                ),
                shown(summary.count, summary.denominator, value.as_ref()),
            ));
        }
    }
    failed
}
