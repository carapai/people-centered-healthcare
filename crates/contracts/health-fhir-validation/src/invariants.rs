//! Tier 2: the R5 core invariants (ADR 0016; fhir-r5-model.md § Validation), evaluated with
//! a [`FhirPathEngine`] after tier 1 has passed.
//!
//! - **Where they apply.** Each invariant is defined at a path of a type: a resource's own
//!   elements (`pat-1` on `Patient.contact`), a datatype (`per-1` on every Period), or an
//!   ancestor (`dom-2` on every DomainResource). The walker finds the paths where each type
//!   actually occurs in this resource, e.g. `Observation.effective.ofType(Period)`, and
//!   evaluates `<path>.all(<invariant>)` at the root. Identical paths are evaluated once, and
//!   the expression strings repeat across resources of the same shape, so the engine's plan
//!   cache serves them.
//! - **Contained resources** are checked from their container, so references to siblings
//!   (`#id`) resolve. **Bundle entry and Parameters resources** are validated as roots of their
//!   own.
//! - **Severity.** Error invariants are enforced. Warnings are not reported yet.
//! - **Fail closed.** A decimal beyond 28 significant digits cannot be evaluated exactly, so the
//!   resource fails with `processing` (ADR 0052). An invariant the engine cannot evaluate also
//!   fails it, unless it is in [`KNOWN_ENGINE_GAPS`] with a reason.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::fhirpath::{FhirPathEngine, FhirPathItem};
use crate::structure::{Issue, IssueCode, Table, is_primitive, resolve, table};

/// Invariants not evaluated, with why. Each is an engine limitation found on the official R5
/// examples. Removing one must keep the corpus test passing.
pub const KNOWN_ENGINE_GAPS: &[(&str, &str)] = &[
    // `ele-1` (every element has a value or children) is what tier 1's "no empty objects,
    // arrays or strings" rule enforces structurally; evaluating it on every element would cost
    // more than the rest of tier 2.
    ("ele-1", "enforced by tier 1"),
    // `exs-1` calls memberOf(), which needs the terminology service (#90).
    ("exs-1", "needs the terminology service (#90)"),
];

fn skipped(key: &str) -> bool {
    KNOWN_ENGINE_GAPS.iter().any(|(k, _)| *k == key)
}

/// One invariant to evaluate at one path of the resource.
struct Check {
    location: String,
    key: String,
    human: String,
    expression: String,
}

/// Validates the core invariants of a resource that passed tier 1. An empty result means it
/// passed tier 2.
pub async fn validate_invariants(engine: &dyn FhirPathEngine, resource: &Value) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut pending = vec![(resource.clone(), None::<String>)];
    while let Some((resource, at)) = pending.pop() {
        let Some(resource_type) = resource
            .get("resourceType")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            continue;
        };
        let here = at.clone().unwrap_or_else(|| resource_type.clone());
        if let Some(location) = unrepresentable_decimal(&resource, &here) {
            issues.push(Issue {
                code: IssueCode::Processing,
                location,
                message: "a decimal has more than 28 significant digits, so the invariants cannot be evaluated exactly".into(),
            });
            continue;
        }
        let mut walker = Walker {
            table: table(),
            checks: Vec::new(),
            seen: BTreeSet::new(),
            nested: Vec::new(),
        };
        if let Some(object) = resource.as_object() {
            walker.resource_root(&resource_type, object, &here);
        }
        let expressions: Vec<String> = walker.checks.iter().map(|c| c.expression.clone()).collect();
        let results = engine.evaluate_many(&resource, &expressions).await;
        for (check, result) in walker.checks.iter().zip(results) {
            match result {
                Ok(items) if holds(&items) => {}
                Ok(_) => issues.push(Issue {
                    code: IssueCode::Invariant,
                    location: check.location.clone(),
                    message: format!("{}: {}", check.key, check.human),
                }),
                Err(failure) => issues.push(Issue {
                    code: IssueCode::Processing,
                    location: check.location.clone(),
                    message: format!("{} could not be evaluated: {failure}", check.key),
                }),
            }
        }
        pending.extend(
            walker
                .nested
                .into_iter()
                .map(|(value, location)| (value, Some(location))),
        );
    }
    issues
}

/// `all()` is true for no instances; the engine returns an empty result for that.
fn holds(items: &[FhirPathItem]) -> bool {
    match items {
        [] => true,
        [FhirPathItem::Boolean(b)] => *b,
        _ => false,
    }
}

/// The location of the first number with more than 28 significant digits.
fn unrepresentable_decimal(value: &Value, location: &str) -> Option<String> {
    match value {
        Value::Number(n) => {
            let text = n.to_string();
            let mantissa = text.split(['e', 'E']).next().unwrap_or(&text);
            let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
            let significant = digits.trim_start_matches('0').len();
            (significant > 28).then(|| location.to_owned())
        }
        Value::Array(items) => items
            .iter()
            .enumerate()
            .find_map(|(i, item)| unrepresentable_decimal(item, &format!("{location}[{i}]"))),
        Value::Object(object) => object
            .iter()
            .find_map(|(key, item)| unrepresentable_decimal(item, &format!("{location}.{key}"))),
        _ => None,
    }
}

/// Adapts an invariant to evaluation by path from the resource root.
///
/// - `%context` is the node the invariant applies to, but the engine binds it to the root.
///   It is bound to the `all()` item with `defineVariable`, so it stays that node inside
///   nested `where()` calls, as in `exs-14`.
/// - Inside a contained resource, `%resource` (the contained resource) is never `%rootResource`
///   (its container). The engine binds both to the root, so the comparison is replaced with its
///   known value, and a `#` reference to the container passes `ref-1`.
fn bind(expression: &str, location: &str) -> String {
    let mut expression = expression.to_owned();
    if location.contains(".contained[") {
        expression = expression.replace("%rootResource!=%resource", "true");
    }
    if expression.contains("%context") {
        expression = format!(
            "defineVariable('invariantContext', $this).select({})",
            expression.replace("%context", "%invariantContext")
        );
    }
    expression
}

struct Walker {
    table: &'static Table,
    checks: Vec<Check>,
    seen: BTreeSet<String>,
    /// Bundle entry and Parameters resources, validated as roots of their own.
    nested: Vec<(Value, String)>,
}

impl Walker {
    /// Adds the error invariants `type_name` defines at `path`, evaluated over `fhirpath`.
    fn add(&mut self, type_name: &str, path: &str, fhirpath: &str, location: &str) {
        let Some(def) = self.table.types.get(type_name) else {
            return;
        };
        for constraint in def.constraints.get(path).into_iter().flatten() {
            if constraint.severity != "error" || skipped(&constraint.key) {
                continue;
            }
            let expression = format!("{fhirpath}.all({})", bind(&constraint.expression, location));
            if self.seen.insert(expression.clone()) {
                self.checks.push(Check {
                    location: location.to_owned(),
                    key: constraint.key.clone(),
                    human: constraint.human.clone(),
                    expression,
                });
            }
        }
    }

    fn resource_root(&mut self, resource_type: &str, object: &Map<String, Value>, location: &str) {
        let lineage: Vec<&str> = self.table.lineage(resource_type).collect();
        for ancestor in lineage {
            self.add(ancestor, "", resource_type, location);
        }
        self.members(resource_type, "", object, resource_type, location);
    }

    /// The members of an object of `type_name` at element path `base`, reached by `fhirpath`.
    fn members(
        &mut self,
        type_name: &str,
        base: &str,
        object: &Map<String, Value>,
        fhirpath: &str,
        location: &str,
    ) {
        let table = self.table;
        let Some(def) = table.types.get(type_name) else {
            return;
        };
        for (key, value) in object {
            if key == "resourceType" || key.starts_with('_') {
                continue;
            }
            let Some((path, element, chosen)) = resolve(def, base, key) else {
                continue;
            };
            let step = match path.strip_suffix("[x]") {
                Some(stem) => format!(
                    "{}.ofType({chosen})",
                    stem.rsplit('.').next().unwrap_or(stem)
                ),
                None => key.clone(),
            };
            let child = format!("{fhirpath}.{step}");
            let child_location = format!("{location}.{key}");
            self.add(type_name, path, &child, &child_location);
            let items: Vec<(usize, &Value)> = match value {
                Value::Array(items) => items.iter().enumerate().collect(),
                single => vec![(0, single)],
            };
            for (index, item) in items {
                let item_location = if value.is_array() {
                    format!("{child_location}[{index}]")
                } else {
                    child_location.clone()
                };
                let Some(object) = item.as_object() else {
                    continue;
                };
                if let Some(target) = &element.content_reference {
                    self.members(type_name, target, object, &child, &item_location);
                } else if matches!(chosen.as_str(), "Resource" | "DomainResource") {
                    let Some(nested_type) = object.get("resourceType").and_then(Value::as_str)
                    else {
                        continue;
                    };
                    if key == "contained" {
                        // Contained resources are checked from their container, so `#id`
                        // references to their siblings resolve (`ref-1`).
                        let nested = format!("{child}.ofType({nested_type})");
                        let lineage: Vec<&str> = table.lineage(nested_type).collect();
                        for ancestor in lineage {
                            self.add(ancestor, "", &nested, &item_location);
                        }
                        self.members(nested_type, "", object, &nested, &item_location);
                    } else {
                        self.nested.push((item.clone(), item_location));
                    }
                } else if matches!(chosen.as_str(), "BackboneElement" | "Element")
                    && def.children.contains_key(path)
                {
                    self.members(type_name, path, object, &child, &item_location);
                } else if !is_primitive(&chosen) {
                    let lineage: Vec<&str> = table.lineage(&chosen).collect();
                    for ancestor in lineage {
                        self.add(ancestor, "", &child, &item_location);
                    }
                    self.members(&chosen, "", object, &child, &item_location);
                }
            }
        }
    }
}
