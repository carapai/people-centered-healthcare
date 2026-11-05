//! The publication gates of an Extraction Mapping (questionnaire-extraction.md § Publication
//! gates; ADR 0052). Publication runs them, with the Owners' opt-ins, which the application
//! layer holds:
//!
//! - [`unparsable`]: every expression parses and evaluates;
//! - [`unknown_items`]: every `linkId` an expression names is an item of the target
//!   Questionnaire;
//! - [`unrounded`]: a value expression that does arithmetic or calls a floating-point function
//!   wraps its whole result in `round(n)`;
//! - [`fixture`]: a fixture response gives exactly its expected Bundle, and every output passes
//!   tier 1.

use health_fhir_validation::{FhirPathEngine, validate_structure};
use serde_json::{Value, json};

use crate::{ExpressionKind, Mapping, bundle, extract};

/// The expressions that cannot be evaluated, against an empty completed response.
pub async fn unparsable(engine: &dyn FhirPathEngine, mapping: &Mapping) -> Vec<String> {
    let expressions = mapping.expressions_by_template();
    let texts: Vec<String> = expressions.iter().map(|e| e.expression.clone()).collect();
    let empty = json!({ "resourceType": "QuestionnaireResponse", "status": "completed" });
    let results = engine.evaluate_many(&empty, &texts).await;
    expressions
        .iter()
        .zip(results)
        .filter_map(|(e, result)| {
            result.err().map(|failure| {
                format!(
                    "template {}: `{}` does not evaluate: {}",
                    e.template, e.expression, failure.0
                )
            })
        })
        .collect()
}

/// `expression` with its string literals, delimited identifiers and `@` literals blanked, so
/// what is left is operators, paths and function names.
fn code(expression: &str) -> String {
    let mut out = String::with_capacity(expression.len());
    let mut chars = expression.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' | '`' => {
                out.push(' ');
                while let Some(inner) = chars.next() {
                    if inner == '\\' {
                        chars.next();
                    } else if inner == c {
                        break;
                    }
                }
            }
            '@' => {
                out.push(' ');
                while chars.peek().is_some_and(|n| {
                    n.is_ascii_alphanumeric() || matches!(n, '-' | ':' | '.' | 'T' | '+' | 'Z')
                }) {
                    chars.next();
                }
            }
            other => out.push(other),
        }
    }
    out
}

const FLOATING: [&str; 5] = ["sqrt(", "ln(", "log(", "exp(", "power("];

/// Whether `code` (blanked) does arithmetic or calls a floating-point function.
fn computes(code: &str) -> bool {
    let words = format!(" {code} ");
    code.contains(['+', '-', '*', '/'])
        || words.contains(" div ")
        || words.contains(" mod ")
        || FLOATING.iter().any(|f| code.contains(f))
}

/// Whether `code` (blanked) has a binary operator outside any parentheses.
fn operator_at_top(code: &str) -> bool {
    let mut depth = 0i32;
    let padded = format!(" {code} ");
    for (at, c) in padded.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            '+' | '-' | '*' | '/' | '|' | '&' | '=' | '<' | '>' if depth == 0 => return true,
            _ if depth == 0
                && [
                    " div ",
                    " mod ",
                    " and ",
                    " or ",
                    " xor ",
                    " implies ",
                    " is ",
                    " as ",
                ]
                .iter()
                .any(|w| padded[at..].starts_with(w)) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Whether `code` (blanked) ends in `.round(n)` applied to all that comes before it.
fn rounded_whole(code: &str) -> bool {
    let code = code.trim();
    let Some(open) = code.rfind(".round(") else {
        return false;
    };
    let argument = &code[open + ".round(".len()..];
    let Some(digits) = argument.strip_suffix(')') else {
        return false;
    };
    if digits.trim().is_empty() || !digits.trim().bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    !operator_at_top(&code[..open])
}

/// Value expressions that compute without rounding their whole result (ADR 0052).
pub fn unrounded(mapping: &Mapping) -> Vec<String> {
    mapping
        .expressions_by_template()
        .into_iter()
        .filter(|e| e.kind == ExpressionKind::Value)
        .filter(|e| {
            let blanked = code(&e.expression);
            computes(&blanked) && !rounded_whole(&blanked)
        })
        .map(|e| {
            format!(
                "template {}: `{}` computes a value, so it must end in an explicit round(n) of its whole result",
                e.template, e.expression
            )
        })
        .collect()
}

fn link_ids(items: &Value, out: &mut Vec<String>) {
    for item in items.as_array().into_iter().flatten() {
        if let Some(link) = item["linkId"].as_str() {
            out.push(link.to_owned());
        }
        link_ids(&item["item"], out);
    }
}

/// The `linkId`s an expression compares with: `linkId = 'x'` (or `~`).
fn named_link_ids(expression: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = expression;
    while let Some(at) = rest.find("linkId") {
        rest = &rest[at + "linkId".len()..];
        let after = rest.trim_start();
        let Some(after) = after.strip_prefix(['=', '~']) else {
            continue;
        };
        let Some(after) = after.trim_start().strip_prefix('\'') else {
            continue;
        };
        if let Some(end) = after.find('\'') {
            found.push(after[..end].to_owned());
        }
    }
    found
}

/// The `linkId`s the mapping names that `questionnaire` does not have.
pub fn unknown_items(mapping: &Mapping, questionnaire: &Value) -> Vec<String> {
    let mut known = Vec::new();
    link_ids(&questionnaire["item"], &mut known);
    let version = questionnaire["version"].as_str().unwrap_or("?");
    let mut problems = Vec::new();
    for e in mapping.expressions_by_template() {
        for link in named_link_ids(&e.expression) {
            if !known.contains(&link) {
                problems.push(format!(
                    "template {}: `{}` names linkId '{link}', which version {version} of the Questionnaire does not have",
                    e.template, e.expression
                ));
            }
        }
    }
    problems.dedup();
    problems
}

/// A fixture: a response and the Bundle it must give.
#[derive(Debug, Clone, PartialEq)]
pub struct Fixture {
    /// The response.
    pub response: Value,
    /// The transaction Bundle it gives ([`bundle`]).
    pub expected: Value,
}

/// Every way `fixture` fails under `mapping`; none if it gives exactly its expected Bundle.
pub async fn fixture(
    engine: &dyn FhirPathEngine,
    mapping: &Mapping,
    fixture: &Fixture,
) -> Vec<String> {
    let plan = match extract(engine, &fixture.response, mapping).await {
        Ok(plan) => plan,
        Err(error) => return vec![format!("the response does not extract: {error}")],
    };
    let mut problems = Vec::new();
    for planned in &plan {
        for issue in validate_structure(&planned.resource.to_string()) {
            problems.push(format!(
                "template {}: the output is not valid FHIR: {issue:?}",
                planned.template
            ));
        }
    }
    let got = bundle(&plan);
    if got != fixture.expected {
        let entries = |b: &Value| b["entry"].as_array().cloned().unwrap_or_default();
        let (got, expected) = (entries(&got), entries(&fixture.expected));
        if got.len() == expected.len() {
            for (index, (g, e)) in got.iter().zip(&expected).enumerate() {
                if g != e {
                    problems.push(format!("entry {index} differs: expected {e}, got {g}"));
                }
            }
        } else {
            problems.push(format!(
                "expected {} entries, got {}",
                expected.len(),
                got.len()
            ));
        }
    }
    problems
}
