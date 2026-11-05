//! The server's re-validation of a QuestionnaireResponse against its Questionnaire (ADRs 0038,
//! 0052; #92). The browser's live behaviour is a preview; this is authoritative, with the same
//! FHIRPath semantics the conformance suite pins.
//!
//! [`check_response`] refuses a response whose:
//! - items name a `linkId` the Questionnaire does not have;
//! - answers are of a type the item does not take, or a coding outside its `answerOption`s;
//! - non-repeating item has more than one answer;
//! - disabled item (by `enableWhen` or `enableWhenExpression`, over the submitted response) is
//!   answered;
//! - required, enabled item is unanswered, once the response is `completed`;
//! - `questionnaire-constraint` of severity `error` fails (warnings are returned, not refused).
//!
//! It then recomputes every `calculatedExpression` and puts the server's value in the response:
//! the server's value wins (ADR 0052). An expression that fails to evaluate refuses the response,
//! so nothing unchecked is stored.

use std::collections::BTreeMap;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde_json::{Value, json};

use crate::fhirpath::{FhirPathEngine, FhirPathItem};
use crate::structure::{Issue, IssueCode, Severity};

const SDC: &str = "http://hl7.org/fhir/uv/sdc/StructureDefinition";
const CONSTRAINT: &str = "http://hl7.org/fhir/StructureDefinition/questionnaire-constraint";

fn enable_when_expression() -> String {
    format!("{SDC}/sdc-questionnaire-enableWhenExpression")
}

fn calculated_expression() -> String {
    format!("{SDC}/sdc-questionnaire-calculatedExpression")
}

fn issue(severity: Severity, code: IssueCode, location: &str, message: impl Into<String>) -> Issue {
    Issue {
        severity,
        code,
        location: location.to_owned(),
        message: message.into(),
    }
}

/// The answer key an item type takes.
fn answer_key(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "boolean" => "valueBoolean",
        "decimal" => "valueDecimal",
        "integer" => "valueInteger",
        "date" => "valueDate",
        "dateTime" => "valueDateTime",
        "time" => "valueTime",
        "string" | "text" => "valueString",
        "url" => "valueUri",
        "coding" => "valueCoding",
        "attachment" => "valueAttachment",
        "reference" => "valueReference",
        "quantity" => "valueQuantity",
        _ => return None,
    })
}

fn extension_expression(item: &Value, url: &str) -> Option<String> {
    item["extension"]
        .as_array()?
        .iter()
        .find(|e| e["url"] == url)
        .and_then(|e| e["valueExpression"]["expression"].as_str())
        .map(str::to_owned)
}

fn items_of(definition: &Value) -> impl Iterator<Item = &Value> {
    definition["item"].as_array().into_iter().flatten()
}

/// Every Questionnaire item by linkId.
fn definitions(questionnaire: &Value) -> BTreeMap<String, Value> {
    let mut found = BTreeMap::new();
    let mut pending: Vec<&Value> = items_of(questionnaire).collect();
    while let Some(item) = pending.pop() {
        if let Some(link) = item["linkId"].as_str() {
            found.insert(link.to_owned(), item.clone());
        }
        pending.extend(items_of(item));
    }
    found
}

/// Every response item with its location, depth first.
fn responses(response: &Value) -> Vec<(String, Value)> {
    fn walk(items: &Value, at: &str, into: &mut Vec<(String, Value)>) {
        for (index, item) in items.as_array().into_iter().flatten().enumerate() {
            let here = format!("{at}.item[{index}]");
            into.push((here.clone(), item.clone()));
            walk(&item["item"], &here, into);
        }
    }
    let mut found = Vec::new();
    walk(&response["item"], "QuestionnaireResponse", &mut found);
    found
}

fn value_of(answer: &Value) -> Option<(&str, &Value)> {
    answer
        .as_object()?
        .iter()
        .find(|(key, _)| key.starts_with("value"))
        .map(|(key, value)| (key.as_str(), value))
}

fn decimal(value: &Value) -> Option<Decimal> {
    match value {
        Value::Number(number) => Decimal::from_str(&number.to_string()).ok(),
        _ => None,
    }
}

fn compare(left: &Value, operator: &str, right: &Value) -> bool {
    use std::cmp::Ordering;
    let ordering = if let (Some(l), Some(r)) = (decimal(left), decimal(right)) {
        Some(l.cmp(&r))
    } else if let (Value::String(l), Value::String(r)) = (left, right) {
        Some(l.cmp(r))
    } else if left.get("code").is_some() || right.get("code").is_some() {
        let same = left["code"] == right["code"]
            && (right.get("system").is_none() || left["system"] == right["system"]);
        return match operator {
            "=" => same,
            "!=" => !same,
            _ => false,
        };
    } else {
        None
    };
    match (operator, ordering) {
        ("=", _) => left == right || ordering == Some(Ordering::Equal),
        ("!=", _) => !(left == right || ordering == Some(Ordering::Equal)),
        (">", Some(o)) => o == Ordering::Greater,
        ("<", Some(o)) => o == Ordering::Less,
        (">=", Some(o)) => o != Ordering::Less,
        ("<=", Some(o)) => o != Ordering::Greater,
        _ => false,
    }
}

/// Whether `item` is enabled over `response`.
async fn enabled(
    engine: &dyn FhirPathEngine,
    item: &Value,
    response: &Value,
    answers: &BTreeMap<String, Vec<Value>>,
) -> Result<bool, String> {
    if let Some(expression) = extension_expression(item, &enable_when_expression()) {
        let result = engine
            .evaluate_many(response, std::slice::from_ref(&expression))
            .await
            .into_iter()
            .next()
            .unwrap_or_else(|| Ok(Vec::new()))
            .map_err(|e| format!("{expression}: {}", e.0))?;
        return Ok(matches!(result.first(), Some(FhirPathItem::Boolean(true))));
    }
    let conditions: Vec<&Value> = item["enableWhen"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    if conditions.is_empty() {
        return Ok(true);
    }
    let holds = |condition: &&Value| {
        let given = answers
            .get(condition["question"].as_str().unwrap_or_default())
            .cloned()
            .unwrap_or_default();
        let operator = condition["operator"].as_str().unwrap_or_default();
        if operator == "exists" {
            return (!given.is_empty()) == (condition["answerBoolean"] == true);
        }
        let expected = condition
            .as_object()
            .and_then(|o| o.iter().find(|(k, _)| k.starts_with("answer")))
            .map(|(_, v)| v.clone())
            .unwrap_or(Value::Null);
        given
            .iter()
            .any(|value| compare(value, operator, &expected))
    };
    Ok(if item["enableBehavior"] == "any" {
        conditions.iter().any(holds)
    } else {
        conditions.iter().all(holds)
    })
}

/// The value a FHIRPath result gives an answer of `key`.
fn as_answer(key: &str, item: &FhirPathItem) -> Option<Value> {
    let number = |text: &str| text.parse::<serde_json::Number>().ok().map(Value::Number);
    let value = match item {
        FhirPathItem::Boolean(b) => Value::Bool(*b),
        FhirPathItem::Integer(i) => Value::from(*i),
        FhirPathItem::Decimal(text) => number(text)?,
        FhirPathItem::String(text) | FhirPathItem::Temporal(text) => Value::String(text.clone()),
        _ => return None,
    };
    Some(json!({ key: value }))
}

/// Re-validates `response` against `questionnaire`: the response as the server stores it, with
/// every calculated value recomputed, and any warnings; or every reason it is refused.
pub async fn check_response(
    engine: &dyn FhirPathEngine,
    questionnaire: &Value,
    response: &Value,
) -> Result<(Value, Vec<Issue>), Vec<Issue>> {
    let defined = definitions(questionnaire);
    let items = responses(response);
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut answers: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut instances: BTreeMap<String, usize> = BTreeMap::new();

    for (at, item) in &items {
        let link = item["linkId"].as_str().unwrap_or_default();
        let Some(definition) = defined.get(link) else {
            errors.push(issue(
                Severity::Error,
                IssueCode::Structure,
                at,
                format!("{link} is not an item of the Questionnaire"),
            ));
            continue;
        };
        *instances.entry(link.to_owned()).or_default() += 1;
        let given: Vec<&Value> = item["answer"].as_array().into_iter().flatten().collect();
        let repeats = definition["repeats"] == true;
        if !repeats && given.len() > 1 {
            errors.push(issue(
                Severity::Error,
                IssueCode::Structure,
                at,
                format!("{link} takes one answer"),
            ));
        }
        let kind = definition["type"].as_str().unwrap_or_default();
        let key = answer_key(kind);
        for answer in given {
            let Some((name, value)) = value_of(answer) else {
                continue;
            };
            if key != Some(name) {
                errors.push(issue(
                    Severity::Error,
                    IssueCode::Value,
                    at,
                    format!("{link} takes a {kind} answer, not {name}"),
                ));
                continue;
            }
            let options: Vec<&Value> = definition["answerOption"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|o| o.get("valueCoding"))
                .collect();
            if name == "valueCoding"
                && !options.is_empty()
                && !options.iter().any(|o| {
                    o["code"] == value["code"]
                        && (o.get("system").is_none() || o["system"] == value["system"])
                })
            {
                errors.push(issue(
                    Severity::Error,
                    IssueCode::CodeInvalid,
                    at,
                    format!("{link}'s answer is not one of its options"),
                ));
            }
            answers
                .entry(link.to_owned())
                .or_default()
                .push(value.clone());
        }
    }
    for (link, count) in &instances {
        let repeats = defined.get(link).is_some_and(|d| d["repeats"] == true);
        let group = defined.get(link).is_some_and(|d| d["type"] == "group");
        if group && !repeats && *count > 1 {
            errors.push(issue(
                Severity::Error,
                IssueCode::Structure,
                "QuestionnaireResponse",
                format!("{link} does not repeat"),
            ));
        }
    }

    // Expressions read the answers: a response with misplaced or mistyped answers is refused
    // before any is evaluated.
    if !errors.is_empty() {
        return Err(errors);
    }

    let completed = matches!(response["status"].as_str(), Some("completed" | "amended"));
    for (link, definition) in &defined {
        let kind = definition["type"].as_str().unwrap_or_default();
        if kind == "display" {
            continue;
        }
        let is_enabled = match enabled(engine, definition, response, &answers).await {
            Ok(value) => value,
            Err(error) => {
                errors.push(issue(
                    Severity::Error,
                    IssueCode::Processing,
                    link,
                    format!("enableWhenExpression failed: {error}"),
                ));
                continue;
            }
        };
        let answered = answers.get(link).is_some_and(|a| !a.is_empty());
        if !is_enabled && answered {
            errors.push(issue(
                Severity::Error,
                IssueCode::Value,
                link,
                format!("{link} is not enabled, so it takes no answer"),
            ));
        }
        if is_enabled
            && completed
            && definition["required"] == true
            && kind != "group"
            && !answered
            && extension_expression(definition, &calculated_expression()).is_none()
        {
            errors.push(issue(
                Severity::Error,
                IssueCode::Required,
                link,
                format!("{link} is required"),
            ));
        }
    }

    // Answer constraints, on the Questionnaire and each item.
    let mut constrained: Vec<(String, &Value)> =
        vec![("QuestionnaireResponse".to_owned(), questionnaire)];
    constrained.extend(defined.iter().map(|(link, item)| (link.clone(), item)));
    for (at, holder) in constrained {
        for constraint in holder["extension"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| e["url"] == CONSTRAINT)
        {
            let part = |name: &str| {
                constraint["extension"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|p| p["url"] == name)
                    .cloned()
                    .unwrap_or(Value::Null)
            };
            let Some(expression) = part("expression")["valueString"]
                .as_str()
                .map(str::to_owned)
            else {
                continue;
            };
            let human = part("human")["valueString"]
                .as_str()
                .unwrap_or("a constraint fails")
                .to_owned();
            let severity = if part("severity")["valueCode"] == "warning" {
                Severity::Warning
            } else {
                Severity::Error
            };
            let result = engine
                .evaluate_many(response, std::slice::from_ref(&expression))
                .await
                .into_iter()
                .next();
            let fails = match result {
                Some(Ok(items)) => matches!(items.first(), Some(FhirPathItem::Boolean(false))),
                _ => true,
            };
            if fails {
                let found = issue(severity, IssueCode::Invariant, &at, human);
                if severity == Severity::Error {
                    errors.push(found)
                } else {
                    warnings.push(found)
                }
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    // The server's calculated values win (ADR 0052).
    let mut stored = response.clone();
    for (link, definition) in &defined {
        let Some(expression) = extension_expression(definition, &calculated_expression()) else {
            continue;
        };
        let Some(key) = answer_key(definition["type"].as_str().unwrap_or_default()) else {
            continue;
        };
        let result = engine
            .evaluate_many(response, std::slice::from_ref(&expression))
            .await
            .into_iter()
            .next();
        let value = match result {
            Some(Ok(items)) => items.first().and_then(|first| as_answer(key, first)),
            Some(Err(error)) => {
                return Err(vec![issue(
                    Severity::Error,
                    IssueCode::Processing,
                    link,
                    format!("calculatedExpression failed: {}", error.0),
                )]);
            }
            None => None,
        };
        set_answer(&mut stored["item"], link, value.as_ref());
    }
    Ok((stored, warnings))
}

/// Sets (or, with `None`, clears) the answer of every response item `link`.
fn set_answer(items: &mut Value, link: &str, value: Option<&Value>) {
    for item in items.as_array_mut().into_iter().flatten() {
        if item["linkId"] == link
            && let Some(object) = item.as_object_mut()
        {
            match value {
                Some(value) => {
                    object.insert("answer".into(), Value::Array(vec![value.clone()]));
                }
                None => {
                    object.remove("answer");
                }
            }
        }
        if let Some(children) = item.get_mut("item") {
            set_answer(children, link, value);
        }
    }
}

#[cfg(test)]
mod tests;
