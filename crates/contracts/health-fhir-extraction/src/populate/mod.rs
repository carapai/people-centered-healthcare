//! Population, or prefill (questionnaire-extraction.md § Population; #94): a new
//! QuestionnaireResponse with what the node already knows answered, for the user to check.
//!
//! - **Expression-based** (SDC): the Questionnaire's `launchContext`s (the patient, the
//!   encounter, the user) are bound by name, `%patient`, with each `variable` in turn, the
//!   Questionnaire's and then each item's own. An item's `initialExpression` gives its answers;
//!   its `candidateExpression` gives answers to offer, returned beside the response.
//! - **Observation-based** (SDC): an item with an `observationLinkPeriod` and a `code` takes the
//!   value of the newest Observation of the patient with one of its codes within that period,
//!   when it has no initial expression or that gives nothing.
//!
//! Population only reads: the launch contexts and Observations are what the caller may see (the
//! node's policy decision point chose them), and nothing is written. Unlike extraction it depends
//! on the node's data, so it is never re-derived elsewhere.

use std::collections::BTreeMap;

use health_fhir_validation::{FhirPathEngine, FhirPathItem};
use health_time::{SignedDuration, Timestamp};
use serde_json::{Map, Value, json};

/// SDC's `launchContext` extension.
pub const LAUNCH_CONTEXT: &str =
    "http://hl7.org/fhir/uv/sdc/StructureDefinition/sdc-questionnaire-launchContext";
/// FHIR's `variable` extension.
pub const VARIABLE: &str = "http://hl7.org/fhir/StructureDefinition/variable";
/// SDC's `initialExpression` extension.
pub const INITIAL_EXPRESSION: &str =
    "http://hl7.org/fhir/uv/sdc/StructureDefinition/sdc-questionnaire-initialExpression";
/// SDC's `candidateExpression` extension.
pub const CANDIDATE_EXPRESSION: &str =
    "http://hl7.org/fhir/uv/sdc/StructureDefinition/sdc-questionnaire-candidateExpression";
/// SDC's `observationLinkPeriod` extension.
pub const OBSERVATION_LINK_PERIOD: &str =
    "http://hl7.org/fhir/uv/sdc/StructureDefinition/sdc-questionnaire-observationLinkPeriod";

/// What a Questionnaire is populated with.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Launch {
    /// The launch contexts by name (`patient`, `encounter`, `user`), each a resource.
    pub contexts: BTreeMap<String, Value>,
    /// The patient's Observations the caller may see, for observation-based population.
    pub observations: Vec<Value>,
    /// When it is populated, for each `observationLinkPeriod`.
    pub now: Option<Timestamp>,
}

/// A populated response, and the candidate answers for items that offer them.
#[derive(Debug, Clone, PartialEq)]
pub struct Populated {
    /// The new response, `in-progress`.
    pub response: Value,
    /// Answers to offer, by `linkId`, from each `candidateExpression`.
    pub candidates: BTreeMap<String, Vec<Value>>,
}

/// Why a Questionnaire could not be populated.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PopulateError {
    /// A launch context it requires was not given.
    #[error("the launch context {0} is required")]
    MissingContext(String),
    /// An expression could not be evaluated.
    #[error("{expression}: {reason}")]
    Expression {
        /// The expression.
        expression: String,
        /// Why not.
        reason: String,
    },
}

/// The extensions on `element` with `url`.
fn extensions<'a>(element: &'a Value, url: &'a str) -> impl Iterator<Item = &'a Value> {
    element["extension"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(move |e| e["url"] == url)
}

/// An expression extension's FHIRPath (`valueExpression.expression`), and its `name`.
fn expression(extension: &Value) -> Option<(Option<&str>, &str)> {
    let value = &extension["valueExpression"];
    let language = value["language"].as_str().unwrap_or("text/fhirpath");
    (language == "text/fhirpath")
        .then(|| {
            value["expression"]
                .as_str()
                .map(|e| (value["name"].as_str(), e))
        })
        .flatten()
}

/// A result as the JSON a variable is bound to.
fn as_json(item: &FhirPathItem) -> Value {
    match item {
        FhirPathItem::Boolean(b) => json!(b),
        FhirPathItem::Integer(i) => json!(i),
        FhirPathItem::Decimal(d) => {
            serde_json::from_str(d).unwrap_or_else(|_| Value::String(d.clone()))
        }
        FhirPathItem::String(s) | FhirPathItem::Temporal(s) => json!(s),
        FhirPathItem::Quantity { value, unit } => {
            let mut quantity =
                json!({ "value": serde_json::from_str::<Value>(value).unwrap_or(Value::Null) });
            if let Some(unit) = unit {
                quantity["unit"] = json!(unit);
            }
            quantity
        }
        FhirPathItem::Element(value) => value.clone(),
    }
}

/// `item`'s answer for a result, in its type; `None` when the result does not fit it.
fn answer(kind: &str, item: &FhirPathItem) -> Option<Value> {
    let decimal = |text: &str| serde_json::from_str::<Value>(text).ok();
    Some(match (kind, item) {
        ("boolean", FhirPathItem::Boolean(b)) => json!({ "valueBoolean": b }),
        ("integer", FhirPathItem::Integer(i)) => json!({ "valueInteger": i }),
        ("decimal", FhirPathItem::Decimal(d)) => json!({ "valueDecimal": decimal(d)? }),
        ("decimal", FhirPathItem::Integer(i)) => json!({ "valueDecimal": i }),
        ("decimal", FhirPathItem::Quantity { value, .. }) => {
            json!({ "valueDecimal": decimal(value)? })
        }
        ("integer", FhirPathItem::Quantity { value, .. }) => {
            json!({ "valueInteger": value.parse::<i64>().ok()? })
        }
        ("string" | "text", FhirPathItem::String(s)) => json!({ "valueString": s }),
        ("date", FhirPathItem::Temporal(t)) => json!({ "valueDate": t.get(..10)? }),
        ("dateTime", FhirPathItem::Temporal(t)) => json!({ "valueDateTime": t }),
        ("coding", FhirPathItem::Element(coding)) if coding.get("code").is_some() => {
            json!({ "valueCoding": coding })
        }
        ("coding", FhirPathItem::Element(concept)) if concept["coding"].is_array() => {
            json!({ "valueCoding": concept["coding"][0] })
        }
        ("quantity", FhirPathItem::Quantity { .. } | FhirPathItem::Element(_)) => {
            json!({ "valueQuantity": as_json(item) })
        }
        _ => return None,
    })
}

/// An Observation's value as `kind`'s answer.
fn observed(kind: &str, observation: &Value) -> Option<Value> {
    let value = |key: &str| observation.get(key).cloned();
    Some(match kind {
        "decimal" => json!({ "valueDecimal": value("valueQuantity")?["value"].clone() }),
        "integer" => json!({ "valueInteger": value("valueQuantity")?["value"].as_i64().or_else(|| value("valueInteger")?.as_i64())? }),
        "quantity" => json!({ "valueQuantity": value("valueQuantity")? }),
        "boolean" => json!({ "valueBoolean": value("valueBoolean")? }),
        "string" | "text" => json!({ "valueString": value("valueString")? }),
        "coding" => json!({ "valueCoding": value("valueCodeableConcept")?["coding"][0].clone() }),
        "dateTime" => json!({ "valueDateTime": value("valueDateTime")? }),
        _ => return None,
    })
    .filter(|answer| answer.as_object().is_some_and(|o| o.values().all(|v| !v.is_null())))
}

/// Whether `observation` has one of `codes` (each `{system, code}`).
fn coded(observation: &Value, codes: &[Value]) -> bool {
    observation["code"]["coding"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|c| {
            codes
                .iter()
                .any(|code| code["system"] == c["system"] && code["code"] == c["code"])
        })
}

/// When an Observation was made: its `effectiveDateTime`, else the start of its period, else
/// when it was issued.
fn effective(observation: &Value) -> Option<Timestamp> {
    [
        &observation["effectiveDateTime"],
        &observation["effectivePeriod"]["start"],
        &observation["effectiveInstant"],
        &observation["issued"],
    ]
    .into_iter()
    .find_map(|v| v.as_str())
    .and_then(|text| text.parse().ok())
}

/// The newest of `observations` with one of `codes`, no older than `period` before `now`.
fn newest<'a>(
    observations: &'a [Value],
    codes: &[Value],
    period: Option<SignedDuration>,
    now: Option<Timestamp>,
) -> Option<&'a Value> {
    let since = match (period, now) {
        (Some(period), Some(now)) => now.checked_sub(period).ok(),
        _ => None,
    };
    observations
        .iter()
        .filter(|o| !matches!(o["status"].as_str(), Some("entered-in-error" | "cancelled")))
        .filter(|o| coded(o, codes))
        .filter_map(|o| effective(o).map(|at| (at, o)))
        .filter(|(at, _)| since.is_none_or(|since| *at >= since))
        .max_by_key(|(at, _)| *at)
        .map(|(_, o)| o)
}

/// A `Duration`'s length (`{value, code}` in UCUM's `s`, `min`, `h`, `d`, `wk`, `mo`, `a`).
fn duration(value: &Value) -> Option<SignedDuration> {
    let amount = value["value"].as_f64()?;
    let seconds = match value["code"].as_str().or(value["unit"].as_str())? {
        "s" => 1.0,
        "min" => 60.0,
        "h" => 3_600.0,
        "d" => 86_400.0,
        "wk" => 604_800.0,
        "mo" => 2_629_746.0,
        "a" => 31_556_952.0,
        _ => return None,
    };
    #[allow(clippy::cast_possible_truncation)]
    Some(SignedDuration::from_secs((amount * seconds) as i64))
}

struct Run<'a> {
    engine: &'a dyn FhirPathEngine,
    response: Value,
    launch: &'a Launch,
    candidates: BTreeMap<String, Vec<Value>>,
}

impl Run<'_> {
    async fn evaluate(
        &self,
        expression: &str,
        variables: &BTreeMap<String, Value>,
    ) -> Result<Vec<FhirPathItem>, PopulateError> {
        let expressions = [expression.to_owned()];
        let mut results = self
            .engine
            .evaluate_with(&self.response, &expressions, variables)
            .await;
        results
            .pop()
            .unwrap_or_else(|| Ok(Vec::new()))
            .map_err(|failure| PopulateError::Expression {
                expression: expression.to_owned(),
                reason: failure.0,
            })
    }

    /// Binds `element`'s variables, in order, onto `variables`.
    async fn bind(
        &self,
        element: &Value,
        variables: &mut BTreeMap<String, Value>,
    ) -> Result<(), PopulateError> {
        for extension in extensions(element, VARIABLE) {
            let Some((Some(name), expression)) = expression(extension) else {
                continue;
            };
            let found = self.evaluate(expression, variables).await?;
            let value = match found.as_slice() {
                [one] => as_json(one),
                many => Value::Array(many.iter().map(as_json).collect()),
            };
            variables.insert(name.to_owned(), value);
        }
        Ok(())
    }

    /// The response items for `items`, populated.
    fn items<'b>(
        &'b mut self,
        items: &'b [Value],
        variables: &'b BTreeMap<String, Value>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<Value>, PopulateError>> + Send + 'b>,
    >
    where
        Self: Send,
    {
        Box::pin(async move {
            let mut out = Vec::new();
            for item in items {
                let kind = item["type"].as_str().unwrap_or_default();
                if kind == "display" {
                    continue;
                }
                let Some(link) = item["linkId"].as_str() else {
                    continue;
                };
                let mut scoped = variables.clone();
                self.bind(item, &mut scoped).await?;
                let mut response = Map::new();
                response.insert("linkId".into(), json!(link));
                if let Some(text) = item.get("text") {
                    response.insert("text".into(), text.clone());
                }
                let mut answers = Vec::new();
                if let Some((_, initial)) =
                    extensions(item, INITIAL_EXPRESSION).find_map(expression)
                {
                    let found = self.evaluate(initial, &scoped).await?;
                    answers.extend(found.iter().filter_map(|f| answer(kind, f)));
                }
                if answers.is_empty()
                    && let Some(period) = extensions(item, OBSERVATION_LINK_PERIOD).next()
                    && let Some(codes) = item["code"].as_array()
                    && let Some(observation) = newest(
                        &self.launch.observations,
                        codes,
                        duration(&period["valueDuration"]),
                        self.launch.now,
                    )
                {
                    answers.extend(observed(kind, observation));
                }
                if item["repeats"] != true {
                    answers.truncate(1);
                }
                if let Some((_, candidate)) =
                    extensions(item, CANDIDATE_EXPRESSION).find_map(expression)
                {
                    let found = self.evaluate(candidate, &scoped).await?;
                    let offered: Vec<Value> =
                        found.iter().filter_map(|f| answer(kind, f)).collect();
                    if !offered.is_empty() {
                        self.candidates.insert(link.to_owned(), offered);
                    }
                }
                if !answers.is_empty() {
                    response.insert("answer".into(), Value::Array(answers));
                }
                if let Some(children) = item["item"].as_array() {
                    let children = self.items(children, &scoped).await?;
                    if !children.is_empty() {
                        response.insert("item".into(), Value::Array(children));
                    }
                }
                out.push(Value::Object(response));
            }
            Ok(out)
        })
    }
}

/// Populates `questionnaire` from `launch`: a new `in-progress` response for its subject.
pub async fn populate(
    engine: &dyn FhirPathEngine,
    questionnaire: &Value,
    launch: &Launch,
) -> Result<Populated, PopulateError> {
    let mut variables = BTreeMap::new();
    for context in extensions(questionnaire, LAUNCH_CONTEXT) {
        let name = context["extension"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|e| e["url"] == "name")
            .and_then(|e| e["valueCoding"]["code"].as_str().or(e["valueId"].as_str()));
        let Some(name) = name else {
            continue;
        };
        match launch.contexts.get(name) {
            Some(resource) => {
                variables.insert(name.to_owned(), resource.clone());
            }
            None if name == "patient" => {
                return Err(PopulateError::MissingContext(name.to_owned()));
            }
            None => {}
        }
    }
    let canonical = match (
        questionnaire["url"].as_str(),
        questionnaire["version"].as_str(),
    ) {
        (Some(url), Some(version)) => Some(format!("{url}|{version}")),
        (Some(url), None) => Some(url.to_owned()),
        _ => None,
    };
    let mut response = json!({ "resourceType": "QuestionnaireResponse", "status": "in-progress" });
    if let Some(canonical) = canonical {
        response["questionnaire"] = json!(canonical);
    }
    if let Some(id) = launch
        .contexts
        .get("patient")
        .and_then(|p| p["id"].as_str())
    {
        response["subject"] = json!({ "reference": format!("Patient/{id}") });
    }
    if let Some(id) = launch
        .contexts
        .get("encounter")
        .and_then(|e| e["id"].as_str())
    {
        response["encounter"] = json!({ "reference": format!("Encounter/{id}") });
    }
    let mut run = Run {
        engine,
        response,
        launch,
        candidates: BTreeMap::new(),
    };
    let root = questionnaire.clone();
    run.bind(&root, &mut variables).await?;
    let items = root["item"].as_array().cloned().unwrap_or_default();
    let populated = run.items(&items, &variables).await?;
    let mut response = run.response.clone();
    if !populated.is_empty() {
        response["item"] = Value::Array(populated);
    }
    Ok(Populated {
        response,
        candidates: run.candidates,
    })
}
