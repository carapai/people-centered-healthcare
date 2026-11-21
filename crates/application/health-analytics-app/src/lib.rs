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
//!   `%previousPeriods.start` holds the first days of the [`PREVIOUS_PERIODS`] periods of the
//!   same type before it, the nearest first (`YYYY-MM-DD`), so *in the previous 3 periods* is
//!   `d >= %previousPeriods.start[2].toDate() and d < %periodStart.toDate()` (#550).
//!   `%indicators` holds the other indicators the patient is in, by side
//!   (`numerator`, `denominator`), each `url|version`: a criterion reads it as
//!   `'<url>|<version>' in %indicators.numerator`. Those indicators come from a [`Library`] and
//!   are evaluated over the same period and patients first; a circle is refused (#550).
//! - [`age_band_set`] reads a `Library` of type [`AGE_BAND_SET_TYPE`] as an Age Band Set (#551):
//!   its `url`, `version` and `title`, and its inline content ([`AGE_BAND_SET_CONTENT`]),
//!   `{"unit": "years", "ageAt": "period-end", "bands": [{"from": 10, "to": 14}, {"from": 25}]}`.
//!   A Measure names the sets it disaggregates by on its first group's stratifiers, each with
//!   [`AGE_BAND_SET`] (`valueCanonical`, `url|version`).
//! - [`test_patient`] reads a package's test patient: a `tests/` file of `type`
//!   [`TEST_PATIENT_TYPE`] holding a Patient and their records.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
pub use health_analytics::{AGE_BAND_SET_CONTENT, AGE_BAND_SET_TYPE};
use health_analytics::{
    AgeBandSetDefinition, Aggregate, BandDefinition, BetweenDefinition, Calculation, Evaluation,
    Expression, Indicator, MEMBERSHIP_VARIABLE, MeasureDefinition, Observed, PREVIOUS_PERIODS,
    PatientOutcome, Period, PopulationDefinition, RecordOutcome, Side, ValueSource, calculate,
    local_date,
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
/// On a Measure's stratifier: the Age Band Set it disaggregates by (`valueCanonical`,
/// `url|version`).
pub const AGE_BAND_SET: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-age-band-set";
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
        age_band_sets: resource["group"][0]["stratifier"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| extension(s, AGE_BAND_SET))
            .map(|e| text(&e["valueCanonical"]).unwrap_or_default())
            .collect(),
    }
}

/// Whether `resource` is an Age Band Set: a `Library` of type [`AGE_BAND_SET_TYPE`].
pub fn is_age_band_set(resource: &Value) -> bool {
    resource["resourceType"] == "Library"
        && resource["type"]["coding"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|c| c["code"] == AGE_BAND_SET_TYPE)
}

/// The content of an Age Band Set `Library`: its first attachment, [`AGE_BAND_SET_CONTENT`]
/// inline as base64 JSON.
pub fn age_band_set_content(resource: &Value) -> Result<Value, String> {
    let content = &resource["content"][0];
    if content.is_null() {
        return Err("is missing: the Library has no content".into());
    }
    if content["contentType"] != AGE_BAND_SET_CONTENT {
        return Err(format!("is not {AGE_BAND_SET_CONTENT}"));
    }
    let data = content["data"]
        .as_str()
        .ok_or("is not inline: the attachment has no data")?;
    let bytes = STANDARD
        .decode(data)
        .map_err(|e| format!("is not base64: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("is not JSON: {e}"))
}

/// The Age Band Set a `Library` of type [`AGE_BAND_SET_TYPE`] defines, as written.
pub fn age_band_set(resource: &Value) -> AgeBandSetDefinition {
    let mut definition = AgeBandSetDefinition {
        url: text(&resource["url"]),
        version: text(&resource["version"]),
        title: text(&resource["title"]).or_else(|| text(&resource["name"])),
        ..AgeBandSetDefinition::default()
    };
    match age_band_set_content(resource) {
        Ok(content) => {
            definition.unit = text(&content["unit"]);
            definition.age_at = text(&content["ageAt"]);
            definition.bands = content["bands"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|band| BandDefinition {
                    from: band["from"].as_i64(),
                    to: band["to"].as_i64(),
                })
                .collect();
        }
        Err(why) => definition.unreadable = Some(why),
    }
    definition
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

/// The indicators another indicator's criteria may refer to, by `url|version` (#550).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Library {
    indicators: BTreeMap<String, Indicator>,
}

impl Library {
    /// A library of `indicators`; a later one replaces an earlier one with the same
    /// `url|version`.
    pub fn new(indicators: impl IntoIterator<Item = Indicator>) -> Self {
        Self {
            indicators: indicators
                .into_iter()
                .map(|indicator| (indicator.canonical(), indicator))
                .collect(),
        }
    }

    /// The indicator `measure` (`url|version`) names.
    pub fn get(&self, measure: &str) -> Option<&Indicator> {
        self.indicators.get(measure)
    }
}

/// For each patient, the other indicators' sides they are in: `%indicators`.
type Memberships = Vec<BTreeMap<Side, Vec<String>>>;

/// Each patient's membership in the indicators `indicator` refers to, each evaluated over the
/// same period and patients; `within` is the chain of indicators being evaluated, to refuse a
/// circle.
fn memberships<'a>(
    engine: &'a dyn FhirPathEngine,
    library: &'a Library,
    indicator: &'a Indicator,
    period: &'a Period,
    patients: &'a [PatientRecords],
    within: Vec<String>,
) -> Pin<Box<dyn Future<Output = Result<Memberships, String>> + Send + 'a>> {
    Box::pin(async move {
        let mut out: Memberships = vec![BTreeMap::new(); patients.len()];
        let mut evaluated: BTreeMap<&str, Evaluation> = BTreeMap::new();
        for reference in &indicator.references {
            let measure = reference.measure.as_str();
            let Some(other) = library.get(measure) else {
                return Err(format!(
                    "{} refers to the indicator {measure}, which is not available",
                    indicator.canonical()
                ));
            };
            if within.iter().any(|c| c == measure) {
                let mut circle = within.clone();
                circle.push(measure.to_owned());
                return Err(format!(
                    "the indicators refer to each other in a circle: {}",
                    circle.join(" → ")
                ));
            }
            if reference.side == Side::Denominator && !other.calculation.has_denominator() {
                return Err(format!(
                    "{} refers to the denominator of {measure}, which has none",
                    indicator.canonical()
                ));
            }
            if !evaluated.contains_key(measure) {
                let mut chain = within.clone();
                chain.push(measure.to_owned());
                let evaluation =
                    evaluate_within(engine, library, other, period, patients, chain).await?;
                evaluated.insert(measure, evaluation);
            }
            for (member, sides) in evaluated[measure].members.iter().zip(&mut out) {
                let is_in = match reference.side {
                    Side::Numerator => member.count > 0,
                    Side::Denominator => member.denominator.unwrap_or_default() > 0,
                };
                let side = sides.entry(reference.side).or_default();
                if is_in {
                    side.push(measure.to_owned());
                }
            }
        }
        Ok(out)
    })
}

/// `indicator` evaluated over `period` from each patient's records: the figure with what
/// roll-up needs and, for each patient, every record of the indicator's basis with its part in
/// it. The indicators its criteria refer to are taken from `library` and evaluated over the
/// same period and patients first; one that is missing, or a circle, is an error.
pub async fn evaluate(
    engine: &dyn FhirPathEngine,
    library: &Library,
    indicator: &Indicator,
    period: &Period,
    patients: &[PatientRecords],
) -> Result<Evaluation, String> {
    evaluate_within(
        engine,
        library,
        indicator,
        period,
        patients,
        vec![indicator.canonical()],
    )
    .await
}

async fn evaluate_within(
    engine: &dyn FhirPathEngine,
    library: &Library,
    indicator: &Indicator,
    period: &Period,
    patients: &[PatientRecords],
    within: Vec<String>,
) -> Result<Evaluation, String> {
    let memberships = memberships(engine, library, indicator, period, patients, within).await?;
    let previous: Vec<String> = period
        .previous_starts(PREVIOUS_PERIODS)
        .iter()
        .map(ToString::to_string)
        .collect();
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
    for (patient, membership) in patients.iter().zip(&memberships) {
        let sides = |side: Side| membership.get(&side).cloned().unwrap_or_default();
        let variables = BTreeMap::from([
            ("patient".to_owned(), patient.patient.clone()),
            ("records".to_owned(), patient.bundle()),
            ("periodStart".to_owned(), json!(period.start.to_string())),
            ("periodEnd".to_owned(), json!(period.end.to_string())),
            ("previousPeriods".to_owned(), json!({ "start": previous })),
            (
                MEMBERSHIP_VARIABLE.to_owned(),
                json!({
                    "numerator": sides(Side::Numerator),
                    "denominator": sides(Side::Denominator),
                }),
            ),
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
    Ok(calculate(indicator, period, &outcomes))
}
