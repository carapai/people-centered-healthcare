//! Override points (ADR 0026, ADR 0067; configuration.md § Overrides): the settings a base
//! package lets a district or facility change, and the override packages that change them.
//!
//! - **Declarations.** A base package declares its points in [`OVERRIDE_POINTS`], each a
//!   [`Point`] with a key, a [`Type`], a default, the values allowed ([`Allowed`]: a list or a
//!   range) and a [`Class`]: presentation, operational or clinically visible. A declaration is
//!   checked by [`point_problems`]: its default must be allowed, and no point may hide a question
//!   ([`PointProblem::Hides`]). A list a scope may reorder is an [`Type::Order`]: its value holds
//!   every allowed value once, so reordering never drops one.
//! - **Override packages.** A package of kind [`PackageKind::Override`], one per scope (e.g.
//!   `ug.facility.kam013`), holds only its settings in [`OVERRIDES`], each naming the base
//!   package and the key it sets ([`Setting`]). It has no dependencies, so it keeps working
//!   across base versions while its values stay allowed, and it never carries profiles,
//!   terminology, mappings, workflows or clinical rules ([`package_problems`]).
//! - **Validation** ([`check`]): a setting whose key the base does not declare is refused
//!   ([`SettingProblem::Undeclared`]), as is a value the point does not allow. Publication checks
//!   an override package against the latest published version of each base it names, and
//!   activation of a base version in a scope is blocked while an override there is orphaned or
//!   outside the new values.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::package::{INDEX, MANIFEST, PACKAGE_JSON, PackageKind};

/// Where a base package declares its override points, under the package root.
pub const OVERRIDE_POINTS: &str = "configuration/override-points.json";

/// Where an override package holds its settings, under the package root.
pub const OVERRIDES: &str = "configuration/overrides.json";

/// Who may apply a change to a point (ADR 0067).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    /// Section order and collapsing, widgets and hints, worklist columns: the admin alone.
    Presentation,
    /// Services and service points, queues, clinic days, slot lengths, Financial Gates,
    /// printing: the admin alone.
    Operational,
    /// Programs active here, program-declared ranges: after a district or central reviewer
    /// approves.
    ClinicallyVisible,
}

/// What a point's value is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Type {
    /// On or off.
    Boolean,
    /// A whole number, from a list or within a range.
    Integer,
    /// One code from a list.
    Code,
    /// Some codes from a list, each at most once.
    Codes,
    /// Every code of a list, once each, in the order chosen: a reordering that drops nothing.
    Order,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Code => "code",
            Self::Codes => "codes",
            Self::Order => "order",
        })
    }
}

/// A point's value, a default or a setting.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    /// For a [`Type::Boolean`].
    Boolean(bool),
    /// For a [`Type::Integer`].
    Integer(i64),
    /// For a [`Type::Code`].
    Code(String),
    /// For a [`Type::Codes`] or [`Type::Order`].
    Codes(Vec<String>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boolean(value) => write!(f, "{value}"),
            Self::Integer(value) => write!(f, "{value}"),
            Self::Code(code) => f.write_str(code),
            Self::Codes(codes) => write!(f, "[{}]", codes.join(", ")),
        }
    }
}

/// The values a point allows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Allowed {
    /// Whole numbers from `minimum` to `maximum`, both included.
    Range {
        /// The least allowed.
        minimum: i64,
        /// The most allowed.
        maximum: i64,
    },
    /// These values: integers for a [`Type::Integer`], codes otherwise.
    Values {
        /// The values.
        values: Vec<Value>,
    },
}

impl Allowed {
    fn contains(&self, value: &Value) -> bool {
        match (self, value) {
            (Self::Range { minimum, maximum }, Value::Integer(n)) => {
                (*minimum..=*maximum).contains(n)
            }
            (Self::Values { values }, _) => values.contains(value),
            _ => false,
        }
    }

    fn code(&self, code: &str) -> bool {
        self.contains(&Value::Code(code.to_owned()))
    }
}

impl fmt::Display for Allowed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Range { minimum, maximum } => write!(f, "{minimum} to {maximum}"),
            Self::Values { values } => {
                let shown: Vec<String> = values.iter().map(ToString::to_string).collect();
                write!(f, "one of {}", shown.join(", "))
            }
        }
    }
}

/// One override point a base package declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    /// Its key, e.g. `tracing.missed-visit-days`.
    pub key: String,
    /// What its value is.
    #[serde(rename = "type")]
    pub kind: Type,
    /// Its value where no scope sets one.
    pub default: Value,
    /// The values a scope may choose; none for a [`Type::Boolean`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed: Option<Allowed>,
    /// Who may apply a change.
    pub class: Class,
    /// What it changes, for the Facility settings page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Why a value is not one a point allows.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValueProblem {
    /// Not a value of the point's type.
    #[error("{value} is not a {expected}")]
    WrongType {
        /// The point's type.
        expected: Type,
        /// The value.
        value: Value,
    },
    /// Not among the values, or outside the range, the point allows.
    #[error("{value} is not allowed: {allowed}")]
    NotAllowed {
        /// The value.
        value: Value,
        /// What the point allows.
        allowed: Allowed,
    },
    /// A code chosen twice.
    #[error("{0} is chosen twice")]
    Twice(String),
    /// An order that leaves out allowed codes: it would hide them.
    #[error("the order leaves out {}", .0.join(", "))]
    LeavesOut(Vec<String>),
}

impl Point {
    /// Whether `value` is one this point allows.
    pub fn check(&self, value: &Value) -> Result<(), ValueProblem> {
        let wrong = || ValueProblem::WrongType {
            expected: self.kind,
            value: value.clone(),
        };
        let not_allowed = |value: Value| ValueProblem::NotAllowed {
            value,
            allowed: self
                .allowed
                .clone()
                .unwrap_or(Allowed::Values { values: Vec::new() }),
        };
        match (self.kind, value) {
            (Type::Boolean, Value::Boolean(_)) => Ok(()),
            (Type::Integer, Value::Integer(_)) | (Type::Code, Value::Code(_)) => {
                match &self.allowed {
                    Some(allowed) if allowed.contains(value) => Ok(()),
                    _ => Err(not_allowed(value.clone())),
                }
            }
            (Type::Codes | Type::Order, Value::Codes(codes)) => {
                let Some(allowed) = &self.allowed else {
                    return Err(not_allowed(value.clone()));
                };
                let mut seen = BTreeSet::new();
                for code in codes {
                    if !allowed.code(code) {
                        return Err(not_allowed(Value::Code(code.clone())));
                    }
                    if !seen.insert(code) {
                        return Err(ValueProblem::Twice(code.clone()));
                    }
                }
                if self.kind == Type::Order
                    && let Allowed::Values { values } = allowed
                {
                    let missing: Vec<String> = values
                        .iter()
                        .filter_map(|value| match value {
                            Value::Code(code) if !seen.contains(code) => Some(code.clone()),
                            _ => None,
                        })
                        .collect();
                    if !missing.is_empty() {
                        return Err(ValueProblem::LeavesOut(missing));
                    }
                }
                Ok(())
            }
            _ => Err(wrong()),
        }
    }
}

/// A key: lower-case letters and digits in parts joined by `.` or `-`, e.g.
/// `tracing.missed-visit-days`.
pub fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 128
        && key.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// The words a key may not use: a point that hid a question would make records incomparable
/// across facilities (ADR 0067). Turning a program's forms off means turning the program off.
const HIDING: &[&str] = &[
    "hidden",
    "hide",
    "hides",
    "visible",
    "visibility",
    "show",
    "shown",
];

/// Why a base package's declarations may not be published.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PointProblem {
    /// Not a key.
    #[error("{0:?} is not an override point key")]
    Key(String),
    /// Two points with one key.
    #[error("the override point {0} is declared twice")]
    Duplicate(String),
    /// A point named for hiding something.
    #[error("the override point {0} would hide a question: a point never hides one")]
    Hides(String),
    /// Allowed values that do not suit the point's type.
    #[error("the override point {key}: {detail}")]
    Allowed {
        /// The point.
        key: String,
        /// Why.
        detail: String,
    },
    /// A default the point does not allow.
    #[error("the override point {key}'s default: {problem}")]
    Default {
        /// The point.
        key: String,
        /// Why.
        problem: ValueProblem,
    },
}

fn allowed_problem(point: &Point) -> Option<String> {
    let codes = |values: &[Value]| {
        values
            .iter()
            .all(|value| matches!(value, Value::Code(code) if !code.is_empty()))
    };
    let integers = |values: &[Value]| {
        values
            .iter()
            .all(|value| matches!(value, Value::Integer(_)))
    };
    match (point.kind, &point.allowed) {
        (Type::Boolean, None) => None,
        (Type::Boolean, Some(_)) => {
            Some("a boolean allows true and false, and lists no values".into())
        }
        (_, None) => Some(format!(
            "a point of type {} must say which values it allows",
            point.kind
        )),
        (Type::Integer, Some(Allowed::Range { minimum, maximum })) => {
            (minimum > maximum).then(|| format!("the range {minimum} to {maximum} is empty"))
        }
        (_, Some(Allowed::Range { .. })) => Some(format!(
            "a point of type {} allows a list of codes, not a range",
            point.kind
        )),
        (kind, Some(Allowed::Values { values })) => {
            let distinct: BTreeSet<&Value> = values.iter().collect();
            if values.is_empty() {
                Some("it allows no values".into())
            } else if distinct.len() != values.len() {
                Some("it lists a value twice".into())
            } else if kind == Type::Integer && !integers(values) {
                Some("an integer allows whole numbers".into())
            } else if kind != Type::Integer && !codes(values) {
                Some(format!("a point of type {kind} allows codes"))
            } else {
                None
            }
        }
    }
}

/// The problems with a base package's declarations; none if they may be published.
pub fn point_problems(points: &[Point]) -> Vec<PointProblem> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for point in points {
        let key = &point.key;
        if !valid_key(key) {
            problems.push(PointProblem::Key(key.clone()));
        } else if key.split(['.', '-']).any(|word| HIDING.contains(&word)) {
            problems.push(PointProblem::Hides(key.clone()));
        }
        if !seen.insert(key) {
            problems.push(PointProblem::Duplicate(key.clone()));
        }
        if let Some(detail) = allowed_problem(point) {
            problems.push(PointProblem::Allowed {
                key: key.clone(),
                detail,
            });
        } else if let Err(problem) = point.check(&point.default) {
            problems.push(PointProblem::Default {
                key: key.clone(),
                problem,
            });
        }
    }
    problems
}

/// One value an override package sets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setting {
    /// The base package that declares the point, e.g. `ug.moh.anc`.
    pub package: String,
    /// The point's key.
    pub key: String,
    /// The value chosen.
    pub value: Value,
}

/// Why an override package's settings may not be published or activated.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettingProblem {
    /// A key the base does not declare: never declared, or removed by a newer base (orphaned).
    #[error("{package} declares no override point {key}")]
    Undeclared {
        /// The base package.
        package: String,
        /// The key.
        key: String,
    },
    /// A value the point does not allow, e.g. outside a range a newer base narrowed.
    #[error("{package} {key}: {problem}")]
    Value {
        /// The base package.
        package: String,
        /// The key.
        key: String,
        /// Why.
        problem: ValueProblem,
    },
    /// One point set twice.
    #[error("{package} {key} is set twice")]
    Duplicate {
        /// The base package.
        package: String,
        /// The key.
        key: String,
    },
}

/// The problems an override package's settings have on their own: a point set twice.
pub fn setting_problems(settings: &[Setting]) -> Vec<SettingProblem> {
    let mut seen = BTreeSet::new();
    settings
        .iter()
        .filter(|setting| !seen.insert((&setting.package, &setting.key)))
        .map(|setting| SettingProblem::Duplicate {
            package: setting.package.clone(),
            key: setting.key.clone(),
        })
        .collect()
}

/// The settings for base package `base` checked against the points it declares: each key must be
/// declared and each value allowed. Settings for other packages are not looked at.
pub fn check(base: &str, points: &[Point], settings: &[Setting]) -> Vec<SettingProblem> {
    settings
        .iter()
        .filter(|setting| setting.package == base)
        .filter_map(|setting| {
            let Some(point) = points.iter().find(|point| point.key == setting.key) else {
                return Some(SettingProblem::Undeclared {
                    package: setting.package.clone(),
                    key: setting.key.clone(),
                });
            };
            point
                .check(&setting.value)
                .err()
                .map(|problem| SettingProblem::Value {
                    package: setting.package.clone(),
                    key: setting.key.clone(),
                    problem,
                })
        })
        .collect()
}

/// The base packages `settings` name, each once.
pub fn bases(settings: &[Setting]) -> BTreeSet<&str> {
    settings
        .iter()
        .map(|setting| setting.package.as_str())
        .collect()
}

/// Why a package breaks the override rules, whatever its files say.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OverrideProblem {
    /// An override package with dependencies.
    #[error(
        "an override package has no dependencies: its settings name the packages they override"
    )]
    Dependencies,
    /// An override package carrying anything but its settings: profiles, terminology, mappings,
    /// workflows and clinical rules are never overridden (ADR 0026).
    #[error("an override package carries only {OVERRIDES}, not {0}")]
    Carries(String),
    /// Settings in a package that is not an override package.
    #[error("only an override package carries {OVERRIDES}")]
    NotAnOverride,
    /// Declarations in an override package or a Language Pack.
    #[error("a {0} package declares no override points")]
    Declares(String),
}

/// The override rules a package of `kind` with these `paths` (under the package root) and
/// dependencies breaks. An override package may hold its `package.json`, manifest and index
/// besides its settings.
pub fn package_problems(
    kind: PackageKind,
    has_dependencies: bool,
    paths: &[String],
) -> Vec<OverrideProblem> {
    let mut problems = Vec::new();
    match kind {
        PackageKind::Override => {
            if has_dependencies {
                problems.push(OverrideProblem::Dependencies);
            }
            for path in paths {
                if ![OVERRIDES, PACKAGE_JSON, MANIFEST, INDEX].contains(&path.as_str()) {
                    problems.push(OverrideProblem::Carries(path.clone()));
                }
            }
        }
        other => {
            if paths.iter().any(|path| path == OVERRIDES) {
                problems.push(OverrideProblem::NotAnOverride);
            }
            if other == PackageKind::Language && paths.iter().any(|path| path == OVERRIDE_POINTS) {
                problems.push(OverrideProblem::Declares("language".into()));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests;
