//! Indicators evaluated (analytics.md; studio.md § Indicators; ADR 0068; #548): the same
//! evaluation runs on every Facility Edge Node and at central, here over a patient's records
//! however they were read (a package's test patients in the Studio, a facility's records on the
//! edge).
//!
//! - [`measure`] reads a FHIR R5 `Measure` as an indicator's definition: its `scoring`, its
//!   `basis` (the type of record its criteria read), its first group's populations, and two
//!   extensions, [`PERIOD_DATE`] (an `Expression`, required) and [`COUNTS`] (a code, people when
//!   absent).
//!   A rate carries its factor in [`RATE_PER`] (`valueInteger`), a cumulative indicator says so
//!   in [`CUMULATIVE`] (`valueBoolean`), and a continuous-variable indicator's
//!   `measure-observation` has its `aggregateMethod` (`sum`, `average`, `median`, `minimum`,
//!   `maximum`, or `distinct-count`), its value as `criteria` or as [`VALUE_BETWEEN`] (the time
//!   between two dates), and [`VALUE_PICK`] (`first` or `last`, first when absent) (#549).
//! - [`evaluate`] evaluates an indicator over one period: for each record of the basis, the
//!   period date places it in the period or not, each population's criteria decide whether it
//!   counts, and the value is taken. Each expression is evaluated with the record as its root,
//!   `%patient` as the patient, `%records` as a `collection` Bundle of the patient's other
//!   resources (so a visit's criteria can read its observations:
//!   `%records.entry.resource.ofType(Observation)`; a variable holds one item, so a Bundle carries
//!   the many), and `%periodStart` and `%periodEnd` as the period's
//!   first and last days (`YYYY-MM-DD`). A record's enrollment is its `episodeOfCare` or its
//!   `workflow-episodeOfCare` extension.
//! - [`test_patient`] reads a package's test patient: a `tests/` file of `type`
//!   [`TEST_PATIENT_TYPE`] holding a Patient and their records.

use std::collections::BTreeMap;

use health_analytics::{
    Aggregate, BetweenDefinition, Calculation, Evaluation, Expression, Indicator,
    MeasureDefinition, Observed, PatientOutcome, Period, PopulationDefinition, RecordOutcome,
    ValueSource, calculate, local_date,
};
use health_fhir_validation::{FhirPathEngine, FhirPathItem};
use rust_decimal::Decimal;
use serde_json::{Value, json};

/// The indicator's period date: an `Expression` (`valueExpression`) giving the declared date
/// that places a record's care in a Report's period.
pub const PERIOD_DATE: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-period-date";
/// What the indicator counts (`valueCode`): `people`, `enrollments`, `visits` or
/// `observations`.
pub const COUNTS: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-counts";
/// A rate's factor (`valueInteger`): 100, 1000, 10000 or 100000.
pub const RATE_PER: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-rate-per";
/// Whether the indicator counts from the first record up to the period's end (`valueBoolean`).
pub const CUMULATIVE: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-cumulative";
/// On the `measure-observation`: which value of a counted thing is taken (`valueCode`, `first`
/// or `last`).
pub const VALUE_PICK: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-value-pick";
/// On the `measure-observation`: its value as the time between two dates, with the
/// sub-extensions `from` and `to` (`valueExpression`) and `unit` (`valueCode`: `days`, `weeks`,
/// `months` or `years`).
pub const VALUE_BETWEEN: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-value-between";
/// The R5 core extension naming a resource's episode of care.
pub const WORKFLOW_EPISODE_OF_CARE: &str =
    "http://hl7.org/fhir/StructureDefinition/workflow-episodeOfCare";
/// The `type` of a test patient in a package's `tests/`.
pub const TEST_PATIENT_TYPE: &str = "test-patient";

fn text(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

fn code(concept: &Value) -> Option<String> {
    concept["coding"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|c| text(&c["code"]))
}

fn expression(value: &Value) -> Option<Expression> {
    value.is_object().then(|| Expression {
        language: text(&value["language"]),
        expression: text(&value["expression"]),
    })
}

fn extension<'a>(resource: &'a Value, url: &str) -> Option<&'a Value> {
    resource["extension"]
        .as_array()?
        .iter()
        .find(|e| e["url"] == url)
}

/// The definition a FHIR R5 `Measure` gives.
pub fn measure(resource: &Value) -> MeasureDefinition {
    let groups = resource["group"].as_array().map_or(0, Vec::len);
    let populations = resource["group"][0]["population"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| PopulationDefinition {
            code: code(&p["code"]),
            criteria: expression(&p["criteria"]),
            aggregate: code(&p["aggregateMethod"]),
            pick: extension(p, VALUE_PICK).and_then(|e| text(&e["valueCode"])),
            between: extension(p, VALUE_BETWEEN).map(|e| BetweenDefinition {
                from: extension(e, "from").and_then(|f| expression(&f["valueExpression"])),
                to: extension(e, "to").and_then(|t| expression(&t["valueExpression"])),
                unit: extension(e, "unit").and_then(|u| text(&u["valueCode"])),
            }),
        })
        .collect();
    MeasureDefinition {
        url: text(&resource["url"]),
        version: text(&resource["version"]),
        title: text(&resource["title"]).or_else(|| text(&resource["name"])),
        scoring: code(&resource["scoring"]),
        basis: text(&resource["basis"]),
        counts: extension(resource, COUNTS).and_then(|e| text(&e["valueCode"])),
        rate_per: extension(resource, RATE_PER).and_then(|e| e["valueInteger"].as_i64()),
        cumulative: extension(resource, CUMULATIVE).and_then(|e| e["valueBoolean"].as_bool()),
        period_date: extension(resource, PERIOD_DATE)
            .map(|e| expression(&e["valueExpression"]).unwrap_or_default()),
        populations,
        groups,
    }
}

/// One patient and their records.
#[derive(Debug, Clone, PartialEq)]
pub struct PatientRecords {
    /// The Patient.
    pub patient: Value,
    /// Their other resources.
    pub records: Vec<Value>,
}

impl PatientRecords {
    /// The patient as a reference (`Patient/<id>`).
    pub fn reference(&self) -> String {
        reference(&self.patient)
    }

    /// Their other resources as a `collection` Bundle: `%records`.
    pub fn bundle(&self) -> Value {
        let entry: Vec<Value> = self
            .records
            .iter()
            .map(|resource| json!({ "resource": resource }))
            .collect();
        json!({ "resourceType": "Bundle", "type": "collection", "entry": entry })
    }

    /// What a person reads for the patient: their first name, given names then family.
    pub fn name(&self) -> Option<String> {
        let name = &self.patient["name"][0];
        if let Some(text) = name["text"].as_str() {
            return Some(text.to_owned());
        }
        let mut parts: Vec<&str> = name["given"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        parts.extend(name["family"].as_str());
        (!parts.is_empty()).then(|| parts.join(" "))
    }
}

fn reference(resource: &Value) -> String {
    format!(
        "{}/{}",
        resource["resourceType"].as_str().unwrap_or("?"),
        resource["id"].as_str().unwrap_or("?")
    )
}

/// A package's test patient.
#[derive(Debug, Clone, PartialEq)]
pub struct TestPatient {
    /// What it shows.
    pub name: String,
    /// The patient and their records.
    pub records: PatientRecords,
}

/// Whether a `tests/` file is a test patient.
pub fn is_test_patient(file: &Value) -> bool {
    file["type"] == TEST_PATIENT_TYPE
}

/// The test patient a `tests/` file of type [`TEST_PATIENT_TYPE`] holds, or why it holds none:
/// it needs a name and exactly one Patient among its resources, each of which has a
/// `resourceType` and an `id`.
pub fn test_patient(file: &Value) -> Result<TestPatient, String> {
    let name = text(&file["name"]).ok_or("the test patient has no name")?;
    let resources = file["resources"]
        .as_array()
        .ok_or("the test patient has no resources")?;
    if let Some(bad) = resources
        .iter()
        .find(|r| r["resourceType"].as_str().is_none() || r["id"].as_str().is_none())
    {
        return Err(format!(
            "every resource of the test patient needs a resourceType and an id, and {bad} has not"
        ));
    }
    let (patients, records): (Vec<&Value>, Vec<&Value>) = resources
        .iter()
        .partition(|r| r["resourceType"] == "Patient");
    let [patient] = patients.as_slice() else {
        return Err(format!(
            "the test patient needs one Patient, and it has {}",
            patients.len()
        ));
    };
    Ok(TestPatient {
        name,
        records: PatientRecords {
            patient: (*patient).clone(),
            records: records.into_iter().cloned().collect(),
        },
    })
}

/// The period date a FHIRPath result gives: the first item, read as a local date.
fn period_date(
    result: Result<Vec<FhirPathItem>, String>,
) -> Result<Option<jiff::civil::Date>, String> {
    match result?.first() {
        None => Ok(None),
        Some(FhirPathItem::Temporal(written) | FhirPathItem::String(written)) => {
            Ok(local_date(written))
        }
        Some(other) => Err(format!("it gave {other:?}, not a date")),
    }
}

fn meets(result: Result<Vec<FhirPathItem>, String>) -> Result<bool, String> {
    match result?.as_slice() {
        [] | [FhirPathItem::Boolean(false)] => Ok(false),
        [FhirPathItem::Boolean(true)] => Ok(true),
        other => Err(format!("the criteria gave {other:?}, not true or false")),
    }
}

fn decimal(text: &str) -> Result<Decimal, String> {
    text.parse()
        .map_err(|_| format!("the value {text} is not a number"))
}

/// The value a FHIRPath result gives: a number, or, for a distinct count, a code.
fn observed(
    result: Result<Vec<FhirPathItem>, String>,
    distinct: bool,
) -> Result<Option<Observed>, String> {
    let items = result?;
    let item = match items.as_slice() {
        [] => return Ok(None),
        [item] => item,
        more => {
            return Err(format!(
                "the value gave {} items, not one (add .first() or narrow it)",
                more.len()
            ));
        }
    };
    let number = match item {
        FhirPathItem::Integer(n) => Some(Decimal::from(*n)),
        FhirPathItem::Decimal(text) | FhirPathItem::Quantity { value: text, .. } => {
            Some(decimal(text)?)
        }
        _ => None,
    };
    match (number, item, distinct) {
        (Some(number), _, _) => Ok(Some(Observed::Number(number))),
        (None, FhirPathItem::String(code) | FhirPathItem::Temporal(code), true) => {
            Ok(Some(Observed::Code(code.clone())))
        }
        (None, FhirPathItem::Boolean(b), true) => Ok(Some(Observed::Code(b.to_string()))),
        (None, other, _) => Err(format!(
            "the value gave {other:?}, not {}",
            if distinct { "a code" } else { "a number" }
        )),
    }
}

/// The enrollment a record belongs to: its `episodeOfCare`, or its `workflow-episodeOfCare`.
fn enrollment(record: &Value) -> Option<String> {
    text(&record["episodeOfCare"][0]["reference"]).or_else(|| {
        extension(record, WORKFLOW_EPISODE_OF_CARE)
            .and_then(|e| text(&e["valueReference"]["reference"]))
    })
}

/// `indicator` evaluated over `period` from each patient's records: the figure with what
/// roll-up needs and, for each patient, every record of the indicator's basis with its part in
/// it.
pub async fn evaluate(
    engine: &dyn FhirPathEngine,
    indicator: &Indicator,
    period: &Period,
    patients: &[PatientRecords],
) -> Evaluation {
    let kinds: Vec<_> = indicator.populations.keys().copied().collect();
    let mut expressions = vec![indicator.period_date.clone()];
    expressions.extend(indicator.populations.values().cloned());
    let first_value = expressions.len();
    match indicator.value.as_ref().map(|v| &v.source) {
        Some(ValueSource::Expression(value)) => expressions.push(value.clone()),
        Some(ValueSource::Between { from, to, .. }) => {
            expressions.extend([from.clone(), to.clone()]);
        }
        None => {}
    }
    let distinct = indicator.calculation == Calculation::Value(Aggregate::DistinctCount);
    let mut outcomes = Vec::with_capacity(patients.len());
    for patient in patients {
        let variables = BTreeMap::from([
            ("patient".to_owned(), patient.patient.clone()),
            ("records".to_owned(), patient.bundle()),
            ("periodStart".to_owned(), json!(period.start.to_string())),
            ("periodEnd".to_owned(), json!(period.end.to_string())),
        ]);
        let mut records = Vec::new();
        let candidates = std::iter::once(&patient.patient).chain(&patient.records);
        for record in candidates.filter(|r| r["resourceType"] == indicator.basis.as_str()) {
            let mut results: Vec<Result<Vec<FhirPathItem>, String>> = engine
                .evaluate_with(record, &expressions, &variables)
                .await
                .into_iter()
                .map(|r| r.map_err(|failure| failure.0))
                .collect();
            results.resize(expressions.len(), Ok(Vec::new()));
            let mut value_results = results.split_off(first_value);
            let date = period_date(results.remove(0));
            let populations = kinds
                .iter()
                .copied()
                .zip(results.into_iter().map(meets))
                .collect();
            let value = match indicator.value.as_ref().map(|v| &v.source) {
                None => Ok(None),
                Some(ValueSource::Expression(_)) => observed(value_results.remove(0), distinct),
                Some(ValueSource::Between { unit, .. }) => {
                    let to = period_date(value_results.remove(1));
                    let from = period_date(value_results.remove(0));
                    match (from, to) {
                        (Ok(Some(from)), Ok(Some(to))) => unit
                            .between(from, to)
                            .map(|n| Some(Observed::Number(Decimal::from(n)))),
                        (Err(why), _) => Err(format!("the value's first date: {why}")),
                        (_, Err(why)) => Err(format!("the value's second date: {why}")),
                        _ => Ok(None),
                    }
                }
            };
            records.push(RecordOutcome {
                reference: reference(record),
                enrollment: enrollment(record),
                date,
                populations,
                value,
            });
        }
        outcomes.push(PatientOutcome {
            patient: patient.reference(),
            records,
        });
    }
    calculate(indicator, period, &outcomes)
}
