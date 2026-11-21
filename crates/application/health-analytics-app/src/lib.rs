//! Indicators evaluated (analytics.md; studio.md § Indicators; ADR 0068; #548): the same
//! evaluation runs on every Facility Edge Node and at central, here over a patient's records
//! however they were read (a package's test patients in the Studio, a facility's records on the
//! edge).
//!
//! - [`measure`] reads a FHIR R5 `Measure` as an indicator's definition: its `scoring`, its
//!   `basis` (the type of record its criteria read), its first group's populations, and two
//!   extensions, [`PERIOD_DATE`] (an `Expression`, required) and [`COUNTS`] (a code, people when
//!   absent).
//! - [`evaluate`] evaluates a count over one period: for each record of the basis, the period
//!   date places it in the period or not, and the `initial-population` criteria decide whether
//!   it counts. Each expression is evaluated with the record as its root and `%resource`,
//!   `%patient` as the patient, and `%periodStart` and `%periodEnd` as the period's first and
//!   last days (`YYYY-MM-DD`).
//! - [`test_patient`] reads a package's test patient: a `tests/` file of `type`
//!   [`TEST_PATIENT_TYPE`] holding a Patient and their records.

use std::collections::BTreeMap;

use health_analytics::{
    Count, CountError, Expression, Indicator, MeasureDefinition, PatientOutcome, Period,
    PopulationDefinition, PopulationKind, RecordOutcome, Scoring, count, local_date,
};
use health_fhir_validation::{FhirPathEngine, FhirPathItem};
use serde_json::{Value, json};

/// The indicator's period date: an `Expression` (`valueExpression`) giving the declared date
/// that places a record's care in a Report's period.
pub const PERIOD_DATE: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-period-date";
/// What the indicator counts (`valueCode`): `people`, `enrollments`, `visits` or
/// `observations`.
pub const COUNTS: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-counts";
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
        })
        .collect();
    MeasureDefinition {
        url: text(&resource["url"]),
        version: text(&resource["version"]),
        title: text(&resource["title"]).or_else(|| text(&resource["name"])),
        scoring: code(&resource["scoring"]),
        basis: text(&resource["basis"]),
        counts: extension(resource, COUNTS).and_then(|e| text(&e["valueCode"])),
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

/// Why an indicator was not evaluated.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EvaluateError {
    /// Evaluating an indicator of this scoring is not built yet.
    #[error("evaluating a {} indicator is not supported yet", .0.code())]
    Scoring(Scoring),
    /// Counting this is not built yet.
    #[error(transparent)]
    Count(#[from] CountError),
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

/// `indicator` evaluated over `period` from each patient's records: the count and, for each
/// patient, every record of the indicator's basis with its part in it.
pub async fn evaluate(
    engine: &dyn FhirPathEngine,
    indicator: &Indicator,
    period: &Period,
    patients: &[PatientRecords],
) -> Result<Count, EvaluateError> {
    if indicator.scoring != Scoring::Cohort {
        return Err(EvaluateError::Scoring(indicator.scoring));
    }
    // An Indicator has the populations its scoring needs.
    let criteria = indicator
        .criteria(PopulationKind::InitialPopulation)
        .unwrap_or("false");
    let expressions = [indicator.period_date.clone(), criteria.to_owned()];
    let mut outcomes = Vec::with_capacity(patients.len());
    for patient in patients {
        let variables = BTreeMap::from([
            ("patient".to_owned(), patient.patient.clone()),
            ("periodStart".to_owned(), json!(period.start.to_string())),
            ("periodEnd".to_owned(), json!(period.end.to_string())),
        ]);
        let mut records = Vec::new();
        let candidates = std::iter::once(&patient.patient).chain(&patient.records);
        for record in candidates.filter(|r| r["resourceType"] == indicator.basis.as_str()) {
            let mut results = engine
                .evaluate_with(record, &expressions, &variables)
                .await
                .into_iter()
                .map(|r| r.map_err(|failure| failure.0));
            let date = results.next().unwrap_or(Ok(Vec::new()));
            let met = results.next().unwrap_or(Ok(Vec::new()));
            records.push(RecordOutcome {
                reference: reference(record),
                date: period_date(date),
                meets: meets(met),
            });
        }
        outcomes.push(PatientOutcome {
            patient: patient.reference(),
            records,
        });
    }
    Ok(count(indicator.counts, period, &outcomes)?)
}
