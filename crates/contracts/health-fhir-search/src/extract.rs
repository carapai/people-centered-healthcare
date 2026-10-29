//! Index-entry extraction at write time (fhir-r5-model.md § Extraction at write time).

use rust_decimal::Decimal;
use serde_json::Value;

use crate::normalize::{self, DateRange, ReferenceKey};
use crate::path::Node;
use crate::registry::{ParamType, Registry, SearchParameter};

/// One normalized value in an index key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexValue {
    /// A token: `system` is empty when the element carries none (a `code`, `boolean` or
    /// `string`).
    Token {
        /// The system, or empty.
        system: String,
        /// The code or identifier value.
        code: String,
    },
    /// A string, folded for the default and `:contains` searches and as written for `:exact`.
    String {
        /// Case- and accent-folded.
        folded: String,
        /// As written.
        exact: String,
    },
    /// A date's implicit range, or a Period.
    Date(DateRange),
    /// A reference target.
    Reference(ReferenceKey),
    /// A quantity: `code` is the unit code, or the unit text when there is no code.
    Quantity {
        /// The unit system, or empty.
        system: String,
        /// The unit code.
        code: String,
        /// The exact value.
        value: Decimal,
    },
    /// An exact number.
    Number(Decimal),
    /// A URI as stored.
    Uri(String),
}

/// One index entry: a parameter and its key, one value per component (a single value for
/// every type but composite).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    /// The parameter code.
    pub param: String,
    /// The parameter type, which picks the index table.
    pub param_type: ParamType,
    /// The key values.
    pub key: Vec<IndexValue>,
}

/// A value that matched a parameter's expression but cannot be indexed, such as a decimal
/// beyond 28 significant digits (ADR 0052). The resource is still stored; it is not findable
/// by that value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unindexable {
    /// The parameter code.
    pub param: String,
    /// Why.
    pub reason: String,
}

/// Everything a write puts in the index for one resource.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extraction {
    /// The entries.
    pub entries: Vec<IndexEntry>,
    /// Supported parameters with no value, for `:missing`.
    pub missing: Vec<String>,
    /// Values that could not be indexed.
    pub unindexable: Vec<Unindexable>,
}

/// Extracts the index entries of every parameter `registry` indexes for this resource, or only
/// of `only` when given (a reindex job backfilling new parameters).
pub fn extract(
    registry: &Registry,
    resource_type: &str,
    resource: &Value,
    only: Option<&[String]>,
) -> Extraction {
    let mut extraction = Extraction::default();
    for supported in registry.indexed(resource_type) {
        let param = &supported.param;
        if only.is_some_and(|codes| !codes.contains(&param.code)) {
            continue;
        }
        let before = extraction.entries.len();
        let nodes = param.path.evaluate(resource);
        if param.param_type == ParamType::Composite {
            for node in &nodes {
                composite(registry, param, node.value, &mut extraction);
            }
        } else {
            for node in &nodes {
                match values(registry, param.param_type, node) {
                    Ok(values) => {
                        extraction
                            .entries
                            .extend(values.into_iter().map(|v| IndexEntry {
                                param: param.code.clone(),
                                param_type: param.param_type,
                                key: vec![v],
                            }))
                    }
                    Err(reason) => extraction.unindexable.push(Unindexable {
                        param: param.code.clone(),
                        reason,
                    }),
                }
            }
        }
        if extraction.entries.len() == before {
            extraction.missing.push(param.code.clone());
        }
    }
    extraction
}

fn composite(
    registry: &Registry,
    param: &SearchParameter,
    focus: &Value,
    extraction: &mut Extraction,
) {
    let mut keys: Vec<Vec<IndexValue>> = vec![Vec::new()];
    for component in &param.components {
        let mut component_values = Vec::new();
        for node in component.path.evaluate_relative(focus) {
            match values(registry, component.param_type, &node) {
                Ok(values) => component_values.extend(values),
                Err(reason) => extraction.unindexable.push(Unindexable {
                    param: param.code.clone(),
                    reason,
                }),
            }
        }
        keys = keys
            .into_iter()
            .flat_map(|prefix| {
                component_values.iter().map(move |value| {
                    let mut key = prefix.clone();
                    key.push(value.clone());
                    key
                })
            })
            .collect();
    }
    extraction
        .entries
        .extend(keys.into_iter().map(|key| IndexEntry {
            param: param.code.clone(),
            param_type: ParamType::Composite,
            key,
        }));
}

const CONTACT_POINT_SYSTEMS: &[&str] = &["phone", "fax", "email", "pager", "url", "sms", "other"];

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn strings<'a>(value: &'a Value, key: &str) -> Vec<&'a str> {
    match value.get(key) {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).collect(),
        Some(Value::String(s)) => vec![s.as_str()],
        _ => Vec::new(),
    }
}

fn values(
    registry: &Registry,
    param_type: ParamType,
    node: &Node<'_>,
) -> Result<Vec<IndexValue>, String> {
    let value = node.value;
    Ok(match param_type {
        ParamType::Token => tokens(value),
        ParamType::String => match value {
            Value::String(s) => vec![string(s)],
            Value::Object(_) => {
                let keys: &[&str] = if value.get("family").is_some() || value.get("given").is_some()
                {
                    &["family", "given", "prefix", "suffix", "text"]
                } else {
                    &[
                        "line",
                        "city",
                        "district",
                        "state",
                        "postalCode",
                        "country",
                        "text",
                    ]
                };
                keys.iter()
                    .flat_map(|k| strings(value, k))
                    .map(string)
                    .collect()
            }
            _ => Vec::new(),
        },
        ParamType::Date => dates(registry, value)?,
        ParamType::Reference => match value {
            Value::String(s) => vec![IndexValue::Reference(ReferenceKey::parse(s))],
            // A CodeableReference nests its Reference.
            Value::Object(_) => match value.get("reference") {
                Some(inner @ Value::Object(_)) => inner.get("reference"),
                other => other,
            }
            .and_then(Value::as_str)
            .map(|r| IndexValue::Reference(ReferenceKey::parse(r)))
            .into_iter()
            .collect(),
            _ => Vec::new(),
        },
        ParamType::Quantity => {
            let Some(number) = value.get("value").and_then(number_text) else {
                return Ok(Vec::new());
            };
            let value_decimal = normalize::decimal(&number).map_err(|e| e.to_string())?;
            let (system, code) = match text(value, "currency") {
                Some(currency) => ("urn:iso:std:iso:4217", currency),
                None => (
                    text(value, "system").unwrap_or_default(),
                    text(value, "code")
                        .or_else(|| text(value, "unit"))
                        .unwrap_or_default(),
                ),
            };
            vec![IndexValue::Quantity {
                system: system.to_owned(),
                code: code.to_owned(),
                value: value_decimal,
            }]
        }
        ParamType::Number => match number_text(value) {
            Some(number) => vec![IndexValue::Number(
                normalize::decimal(&number).map_err(|e| e.to_string())?,
            )],
            None => Vec::new(),
        },
        ParamType::Uri => value
            .as_str()
            .map(|s| IndexValue::Uri(s.to_owned()))
            .into_iter()
            .collect(),
        ParamType::Composite => Vec::new(),
    })
}

fn number_text(value: &Value) -> Option<String> {
    match value {
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn string(text: &str) -> IndexValue {
    IndexValue::String {
        folded: normalize::fold(text),
        exact: text.to_owned(),
    }
}

fn token(system: &str, code: &str) -> IndexValue {
    IndexValue::Token {
        system: system.to_owned(),
        code: code.to_owned(),
    }
}

fn tokens(value: &Value) -> Vec<IndexValue> {
    match value {
        Value::String(s) => vec![token("", s)],
        Value::Bool(b) => vec![token("", if *b { "true" } else { "false" })],
        Value::Object(_) => {
            if let Some(concept) = value.get("concept") {
                return tokens(concept);
            }
            if let Some(Value::Array(codings)) = value.get("coding") {
                return codings.iter().flat_map(tokens).collect();
            }
            if let Some(code) = text(value, "code") {
                return vec![token(text(value, "system").unwrap_or_default(), code)];
            }
            if let Some(identifier_value) = text(value, "value") {
                let system = text(value, "system").unwrap_or_default();
                // A ContactPoint's system is a kind of contact, not a namespace.
                let system = if CONTACT_POINT_SYSTEMS.contains(&system) {
                    ""
                } else {
                    system
                };
                return vec![token(system, identifier_value)];
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn dates(registry: &Registry, value: &Value) -> Result<Vec<IndexValue>, String> {
    let read =
        |s: &str| normalize::date_range(s, registry.local_offset()).map_err(|e| e.to_string());
    Ok(match value {
        Value::String(s) => vec![IndexValue::Date(read(s)?)],
        Value::Object(_) if value.get("start").is_some() || value.get("end").is_some() => {
            let start = text(value, "start")
                .map(read)
                .transpose()?
                .map_or_else(normalize::date_floor, |r| r.start);
            let end = text(value, "end")
                .map(read)
                .transpose()?
                .map_or_else(normalize::date_ceiling, |r| r.end);
            vec![IndexValue::Date(DateRange { start, end })]
        }
        Value::Object(_) => strings(value, "event")
            .into_iter()
            .map(|s| read(s).map(IndexValue::Date))
            .collect::<Result<_, _>>()?,
        _ => Vec::new(),
    })
}
