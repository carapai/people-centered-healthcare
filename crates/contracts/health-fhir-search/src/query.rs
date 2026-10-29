//! The search request parser and AST (fhir-r5-model.md § v1 search feature set).
//!
//! Parsing is strict by default: an unknown parameter, an unsupported modifier or prefix, or a
//! malformed value is an error that the REST layer returns as 400 with an OperationOutcome.
//! Lenient handling (`Prefer: handling=lenient`) only ignores unknown parameters; an unsupported
//! modifier on a supported parameter is always an error, as R5 requires.

use rust_decimal::Decimal;

use crate::normalize::{self, DateRange, ReferenceKey};
use crate::registry::{ParamState, ParamType, Registry, SearchParameter};

/// A comparison prefix on a number, date or quantity value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prefix {
    /// `eq`, the default.
    Eq,
    /// `ne`.
    Ne,
    /// `gt`.
    Gt,
    /// `lt`.
    Lt,
    /// `ge`.
    Ge,
    /// `le`.
    Le,
    /// `sa`: starts after.
    Sa,
    /// `eb`: ends before.
    Eb,
    /// `ap`: approximately.
    Ap,
}

impl Prefix {
    fn split(value: &str) -> (Self, &str) {
        let prefix = match value.get(..2) {
            Some("eq") => Self::Eq,
            Some("ne") => Self::Ne,
            Some("gt") => Self::Gt,
            Some("lt") => Self::Lt,
            Some("ge") => Self::Ge,
            Some("le") => Self::Le,
            Some("sa") => Self::Sa,
            Some("eb") => Self::Eb,
            Some("ap") => Self::Ap,
            _ => return (Self::Eq, value),
        };
        (prefix, &value[2..])
    }
}

/// How a string value matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringMatch {
    /// The default: the folded value starts with the folded search text.
    StartsWith,
    /// `:exact`: equal as written, including case and accents.
    Exact,
    /// `:contains`: the folded value contains the folded search text.
    Contains,
}

/// One search value, typed by its parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchValue {
    /// `[system]|[code]`: `system` is `None` for any system and `Some("")` for none (`|code`);
    /// `code` is `None` for any code in the system (`system|`).
    Token {
        /// The system constraint.
        system: Option<String>,
        /// The code constraint.
        code: Option<String>,
    },
    /// A string: folded for [`StringMatch::StartsWith`] and [`StringMatch::Contains`], as
    /// written for [`StringMatch::Exact`].
    String {
        /// The search text.
        text: String,
        /// How it matches.
        mode: StringMatch,
    },
    /// A date with its implicit range.
    Date {
        /// The prefix.
        prefix: Prefix,
        /// The range at the value's precision.
        range: DateRange,
    },
    /// A reference: `target_type` is `None` when the value names only an id and the parameter
    /// allows several target types.
    Reference {
        /// The target type, if known.
        target_type: Option<String>,
        /// The target id, or the whole URL when the value names no type.
        target_id: String,
    },
    /// `[prefix]number|[system]|[code]`; `None` constraints match any.
    Quantity {
        /// The prefix.
        prefix: Prefix,
        /// The value.
        value: Decimal,
        /// The unit system.
        system: Option<String>,
        /// The unit code.
        code: Option<String>,
    },
    /// `[prefix]number`.
    Number {
        /// The prefix.
        prefix: Prefix,
        /// The value.
        value: Decimal,
    },
    /// A URI, matched exactly.
    Uri(String),
    /// A composite value: one per component, `$`-separated.
    Composite(Vec<SearchValue>),
}

/// What one clause requires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Test {
    /// `:missing=true` or `:missing=false`.
    Missing(bool),
    /// Any of the values (comma-separated values are ORed).
    Any(Vec<SearchValue>),
    /// `:not`: none of the values, including resources with no value for the parameter.
    NoneOf(Vec<SearchValue>),
}

/// One parameter of a search. Clauses are ANDed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clause {
    /// The parameter code.
    pub param: String,
    /// The parameter type.
    pub param_type: ParamType,
    /// What it requires.
    pub test: Test,
}

/// Strict or lenient handling of unknown parameters (`Prefer: handling=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Handling {
    /// The default: an unknown parameter is an error.
    #[default]
    Strict,
    /// Unknown parameters are ignored and reported in [`SearchQuery::ignored`].
    Lenient,
}

/// A parsed search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    /// The resource type searched.
    pub resource_type: String,
    /// The clauses, ANDed.
    pub clauses: Vec<Clause>,
    /// `_count`, capped at [`MAX_COUNT`].
    pub count: Option<u32>,
    /// Result-control parameters the REST layer applies (`_sort`, `_include`, `_revinclude`,
    /// `_summary`, `_elements`, `_total`, `_format`, `_pretty`), in request order.
    pub control: Vec<(String, String)>,
    /// Unknown parameters ignored under lenient handling.
    pub ignored: Vec<String>,
}

/// The largest page a search returns.
pub const MAX_COUNT: u32 = 1000;

const CONTROL: &[&str] = &[
    "_sort",
    "_include",
    "_revinclude",
    "_summary",
    "_elements",
    "_total",
    "_format",
    "_pretty",
];

/// Why a search cannot run. Each is a 400 with an OperationOutcome.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SearchError {
    /// The parameter is not supported on this resource type.
    #[error("unknown search parameter `{0}`")]
    UnknownParameter(String),
    /// A supported parameter whose reindex has not completed.
    #[error("search parameter `{0}` is being reindexed and is not searchable yet")]
    NotReady(String),
    /// The modifier is not supported on this parameter.
    #[error("modifier `:{modifier}` is not supported on `{param}`")]
    UnsupportedModifier {
        /// The parameter.
        param: String,
        /// The modifier.
        modifier: String,
    },
    /// The prefix is not supported here.
    #[error("prefix `{prefix}` is not supported on `{param}`")]
    UnsupportedPrefix {
        /// The parameter.
        param: String,
        /// The prefix.
        prefix: String,
    },
    /// A search feature outside the v1 set, or one handled elsewhere.
    #[error("`{0}` uses a search feature that is not supported")]
    UnsupportedFeature(String),
    /// A value that cannot be read for its parameter type.
    #[error("invalid value `{value}` for `{param}`: {reason}")]
    InvalidValue {
        /// The parameter.
        param: String,
        /// The value.
        value: String,
        /// Why.
        reason: String,
    },
}

/// Parses `params` (already percent-decoded name/value pairs) into a search of
/// `resource_type`.
pub fn parse_query(
    registry: &Registry,
    resource_type: &str,
    params: &[(String, String)],
    handling: Handling,
) -> Result<SearchQuery, SearchError> {
    let mut query = SearchQuery {
        resource_type: resource_type.to_owned(),
        clauses: Vec::new(),
        count: None,
        control: Vec::new(),
        ignored: Vec::new(),
    };
    for (name, raw) in params {
        if name == "_count" {
            let count: u32 = raw.parse().map_err(|_| SearchError::InvalidValue {
                param: name.clone(),
                value: raw.clone(),
                reason: "not a non-negative integer".into(),
            })?;
            query.count = Some(count.min(MAX_COUNT));
            continue;
        }
        if CONTROL.contains(&name.as_str()) {
            query.control.push((name.clone(), raw.clone()));
            continue;
        }
        if name.contains('.') || name.starts_with("_has:") {
            return Err(SearchError::UnsupportedFeature(name.clone()));
        }
        let (code, modifier) = name
            .split_once(':')
            .map_or((name.as_str(), None), |(c, m)| (c, Some(m)));
        let Some(supported) = registry.get(resource_type, code) else {
            match handling {
                Handling::Strict => return Err(SearchError::UnknownParameter(code.to_owned())),
                Handling::Lenient => {
                    query.ignored.push(name.clone());
                    continue;
                }
            }
        };
        if supported.state == ParamState::Reindexing {
            return Err(SearchError::NotReady(code.to_owned()));
        }
        query
            .clauses
            .push(clause(registry, &supported.param, modifier, raw)?);
    }
    Ok(query)
}

fn clause(
    registry: &Registry,
    param: &SearchParameter,
    modifier: Option<&str>,
    raw: &str,
) -> Result<Clause, SearchError> {
    let code = param.code.as_str();
    let unsupported = |m: &str| SearchError::UnsupportedModifier {
        param: code.to_owned(),
        modifier: m.to_owned(),
    };
    let invalid = |value: &str, reason: &str| SearchError::InvalidValue {
        param: code.to_owned(),
        value: value.to_owned(),
        reason: reason.to_owned(),
    };
    let make = |test| Clause {
        param: code.to_owned(),
        param_type: param.param_type,
        test,
    };
    let mode = match (param.param_type, modifier) {
        (_, Some("missing")) => {
            return match raw {
                "true" => Ok(make(Test::Missing(true))),
                "false" => Ok(make(Test::Missing(false))),
                _ => Err(invalid(raw, "`:missing` takes true or false")),
            };
        }
        (ParamType::String, None) => StringMatch::StartsWith,
        (ParamType::String, Some("exact")) => StringMatch::Exact,
        (ParamType::String, Some("contains")) => StringMatch::Contains,
        (ParamType::Token, Some("not")) | (_, None) => StringMatch::StartsWith,
        (_, Some(m)) => return Err(unsupported(m)),
    };
    let values = split(raw, ',')
        .into_iter()
        .map(|v| value(registry, param, param.param_type, v, mode, true))
        .collect::<Result<Vec<_>, _>>()?;
    if values.is_empty() {
        return Err(invalid(raw, "no value"));
    }
    Ok(make(if modifier == Some("not") {
        Test::NoneOf(values)
    } else {
        Test::Any(values)
    }))
}

fn value(
    registry: &Registry,
    param: &SearchParameter,
    param_type: ParamType,
    raw: &str,
    mode: StringMatch,
    last: bool,
) -> Result<SearchValue, SearchError> {
    let invalid = |reason: &str| SearchError::InvalidValue {
        param: param.code.clone(),
        value: unescape(raw),
        reason: reason.to_owned(),
    };
    let prefixed = |raw: &str| -> Result<(Prefix, String), SearchError> {
        let (prefix, rest) = Prefix::split(raw);
        if !last && prefix != Prefix::Eq {
            return Err(SearchError::UnsupportedPrefix {
                param: param.code.clone(),
                prefix: raw[..2].to_owned(),
            });
        }
        Ok((prefix, unescape(rest)))
    };
    let number = |text: &str| normalize::decimal(text).map_err(|e| invalid(&e.to_string()));
    Ok(match param_type {
        ParamType::Token => {
            let parts = split(raw, '|');
            let non_empty = |s: &str| (!s.is_empty()).then(|| unescape(s));
            match parts.as_slice() {
                [code] => SearchValue::Token {
                    system: None,
                    code: non_empty(code),
                },
                [system, code] => SearchValue::Token {
                    system: Some(unescape(system)),
                    code: non_empty(code),
                },
                _ => return Err(invalid("a token is [system]|[code]")),
            }
        }
        ParamType::String => {
            let text = unescape(raw);
            SearchValue::String {
                text: if mode == StringMatch::Exact {
                    text
                } else {
                    normalize::fold(&text)
                },
                mode,
            }
        }
        ParamType::Date => {
            let (prefix, text) = prefixed(raw)?;
            let range = normalize::date_range(&text, registry.local_offset())
                .map_err(|e| invalid(&e.to_string()))?;
            SearchValue::Date { prefix, range }
        }
        ParamType::Reference => {
            let text = unescape(raw);
            let key = ReferenceKey::parse(&text);
            if !key.target_type.is_empty() {
                SearchValue::Reference {
                    target_type: Some(key.target_type),
                    target_id: key.target_id,
                }
            } else if text.contains(['/', ':']) {
                SearchValue::Reference {
                    target_type: Some(String::new()),
                    target_id: key.target_id,
                }
            } else {
                SearchValue::Reference {
                    target_type: match param.targets.as_slice() {
                        [only] => Some(only.clone()),
                        _ => None,
                    },
                    target_id: text,
                }
            }
        }
        ParamType::Quantity => {
            let parts = split(raw, '|');
            let (prefix, number_text) = prefixed(parts.first().copied().unwrap_or_default())?;
            let non_empty = |s: Option<&&str>| s.filter(|s| !s.is_empty()).map(|s| unescape(s));
            match parts.len() {
                1 | 3 => SearchValue::Quantity {
                    prefix,
                    value: number(&number_text)?,
                    system: non_empty(parts.get(1)),
                    code: non_empty(parts.get(2)),
                },
                _ => return Err(invalid("a quantity is [prefix]number|[system]|[code]")),
            }
        }
        ParamType::Number => {
            let (prefix, text) = prefixed(raw)?;
            SearchValue::Number {
                prefix,
                value: number(&text)?,
            }
        }
        ParamType::Uri => SearchValue::Uri(unescape(raw)),
        ParamType::Composite => {
            let parts = split(raw, '$');
            if parts.len() != param.components.len() {
                return Err(invalid(&format!(
                    "expected {} `$`-separated components",
                    param.components.len()
                )));
            }
            let count = parts.len();
            SearchValue::Composite(
                parts
                    .into_iter()
                    .zip(&param.components)
                    .enumerate()
                    .map(|(i, (part, component))| {
                        value(
                            registry,
                            param,
                            component.param_type,
                            part,
                            StringMatch::StartsWith,
                            i + 1 == count,
                        )
                    })
                    .collect::<Result<_, _>>()?,
            )
        }
    })
}

/// Splits on unescaped `separator`, keeping escapes for the next split.
fn split(text: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut escaped = false;
    for (at, c) in text.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == separator {
            parts.push(&text[start..at]);
            start = at + c.len_utf8();
        }
    }
    parts.push(&text[start..]);
    parts
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}
