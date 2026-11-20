//! A profile's snapshot checked against its differential, at package Validation (ADR 0053;
//! #520).
//!
//! Packages ship snapshots and nothing generates them in the product, so a stale or
//! hand-edited snapshot is caught here instead: every differential element must appear in the
//! snapshot with the constraints the differential gives it.
//!
//! - **Profiles only.** A definition that defines a type (`derivation = specialization`) has
//!   no base to constrain, so it is not checked.
//! - **Matching.** A differential element is found in the snapshot by `id` (by `path` when it
//!   has none).
//!   The R4 spelling of a choice type (`Observation.valueQuantity`) finds its type slice
//!   (`Observation.value[x]:valueQuantity`), as the HL7 snapshot generator writes it, and a
//!   choice named without `[x]` finds it with; a choice narrowed to one type is `value[x]`
//!   itself.
//! - **Generator narrowing.** A choice's type whose type slice is closed (`max = 0`) may be
//!   left out of the choice's snapshot types, and a choice's open slicing by type may be closed,
//!   as the HL7 generators do.
//! - **Compared**, each only when the differential sets it: `min`, `max`, `type` (codes,
//!   profiles and target profiles), every `fixed[x]` and `pattern[x]` value, `binding`
//!   (strength and value set), `slicing` (discriminators, rules, ordered), and each
//!   `constraint` by key (expression and severity).
//! - **Canonicals.** The generator pins versions (`…/Patient|5.0.0`), so a canonical written
//!   without a version matches the same canonical with one; two versions must agree.
//!
//! Nothing else is compared: documentation, `mustSupport` and the rest are not constraints the
//! validator enforces (ADR 0053).

use std::collections::HashSet;
use std::fmt;

use serde_json::{Map, Value};

/// Why a profile's snapshot does not carry its differential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotProblem {
    /// A differential and no snapshot: it cannot be published (ADR 0053).
    NoSnapshot,
    /// A differential element with no counterpart in the snapshot.
    Missing {
        /// The differential element's `id` (or `path`).
        element: String,
    },
    /// A constraint the snapshot does not carry as the differential gives it.
    Differs {
        /// The differential element's `id` (or `path`).
        element: String,
        /// Which: `min`, `max`, `type`, the `fixed[x]` or `pattern[x]` key, `binding`,
        /// `slicing`, or `constraint <key>`.
        constraint: String,
    },
}

impl fmt::Display for SnapshotProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSnapshot => write!(
                f,
                "the profile has a differential but no snapshot (differential-only profiles \
                 cannot be published)"
            ),
            Self::Missing { element } => write!(f, "{element}: not in the snapshot"),
            Self::Differs {
                element,
                constraint,
            } => write!(
                f,
                "{element}: the snapshot's {constraint} is not the differential's"
            ),
        }
    }
}

/// Every way `definition`'s snapshot fails to carry its differential, in differential order;
/// none for a definition without a differential, or one that defines a type
/// (`derivation = specialization`) rather than constraining one.
pub fn check_snapshot(definition: &Value) -> Vec<SnapshotProblem> {
    if definition["derivation"] == "specialization" {
        return Vec::new();
    }
    let Some(differential) = definition["differential"]["element"].as_array() else {
        return Vec::new();
    };
    let Some(snapshot) = definition["snapshot"]["element"].as_array() else {
        return vec![SnapshotProblem::NoSnapshot];
    };
    let ids: HashSet<&str> = snapshot.iter().filter_map(key).collect();
    let mut problems = Vec::new();
    for element in differential {
        let Some(name) = key(element) else {
            continue;
        };
        let found = resolve(name, &ids)
            .and_then(|id| snapshot.iter().find(|e| key(e) == Some(id.as_str())));
        let Some(generated) = found else {
            problems.push(SnapshotProblem::Missing {
                element: name.to_owned(),
            });
            continue;
        };
        for constraint in differences(element, generated, &excluded(generated, snapshot)) {
            problems.push(SnapshotProblem::Differs {
                element: name.to_owned(),
                constraint,
            });
        }
    }
    problems
}

/// An element's `id`, or its `path` when it has none.
fn key(element: &Value) -> Option<&str> {
    element["id"].as_str().or_else(|| element["path"].as_str())
}

/// The snapshot id of the differential's `name`: itself, or with each R4-spelled choice
/// segment (`valueQuantity`) as its type slice (`value[x]:valueQuantity`).
fn resolve(name: &str, ids: &HashSet<&str>) -> Option<String> {
    if ids.contains(name) {
        return Some(name.to_owned());
    }
    let mut segments = name.split('.');
    let mut resolved = segments.next()?.to_owned();
    for segment in segments {
        let plain = format!("{resolved}.{segment}");
        if ids.contains(plain.as_str()) {
            resolved = plain;
            continue;
        }
        let (field, slice) = segment.split_once(':').unwrap_or((segment, ""));
        let bare = if slice.is_empty() {
            format!("{resolved}.{field}[x]")
        } else {
            format!("{resolved}.{field}[x]:{slice}")
        };
        // `valueBoolean:valueBoolean` is how some IGs name the type slice itself.
        let named = if slice == field { field } else { segment };
        let bases: Vec<&str> = field
            .char_indices()
            .filter(|&(i, c)| i > 0 && c.is_ascii_uppercase())
            .map(|(i, _)| &field[..i])
            .collect();
        // A choice narrowed to one type has no type slice: `value[x]` itself.
        let single = bases
            .iter()
            .filter(|_| slice.is_empty())
            .map(|base| format!("{resolved}.{base}[x]"));
        resolved = bases
            .iter()
            .map(|base| format!("{resolved}.{base}[x]:{named}"))
            .chain(single)
            .chain([bare])
            .find(|choice| ids.contains(choice.as_str()))?;
    }
    Some(resolved)
}

/// The type slices of the choice `element` the snapshot closes (`max = 0`), as their type
/// suffix (`DateTime`): the HL7 generator drops those types from the choice's own (`deceased[x]`
/// without `dateTime` when `deceased[x]:deceasedDateTime` has `max = 0`).
fn excluded<'a>(element: &Value, snapshot: &'a [Value]) -> Vec<&'a str> {
    let Some(id) = key(element) else {
        return Vec::new();
    };
    let Some(field) = id
        .rsplit('.')
        .next()
        .and_then(|last| last.split(':').next())
        .and_then(|last| last.strip_suffix("[x]"))
    else {
        return Vec::new();
    };
    snapshot
        .iter()
        .filter(|e| e["max"] == "0")
        .filter_map(|e| {
            key(e)?
                .strip_prefix(id)?
                .strip_prefix(':')?
                .strip_prefix(field)
        })
        .collect()
}

/// A type code as a choice's suffix: `dateTime` → `DateTime`.
fn suffix(code: &str) -> String {
    let mut chars = code.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_ascii_uppercase().to_string() + chars.as_str()
    })
}

/// The constraints `differential` sets that `snapshot` does not carry; `excluded` are the types
/// the snapshot may leave out of a choice ([`excluded`]).
fn differences(differential: &Value, snapshot: &Value, excluded: &[&str]) -> Vec<String> {
    let (Some(wanted), Some(generated)) = (differential.as_object(), snapshot.as_object()) else {
        return Vec::new();
    };
    let mut differs = Vec::new();
    for (name, value) in wanted {
        let other = generated.get(name);
        let same = match name.as_str() {
            "min" | "max" => other == Some(value),
            "type" => other.is_some_and(|other| same_types(value, other, excluded)),
            "binding" => other.is_some_and(|other| same_binding(value, other)),
            "slicing" => other.is_some_and(|other| same_slicing(value, other)),
            "constraint" => {
                differs.extend(missing_invariants(value, generated));
                true
            }
            fixed if fixed.starts_with("fixed") || fixed.starts_with("pattern") => {
                other == Some(value)
            }
            _ => true,
        };
        if !same {
            differs.push(name.clone());
        }
    }
    differs
}

/// Whether two canonicals name the same thing: equal, or one without a version.
fn same_canonical(a: &str, b: &str) -> bool {
    match (a.split_once('|'), b.split_once('|')) {
        (Some(_), Some(_)) | (None, None) => a == b,
        (Some((a, _)), None) => a == b,
        (None, Some((b, _))) => a == b,
    }
}

fn same_canonicals(a: &Value, b: &Value) -> bool {
    let list = |v: &Value| -> Vec<String> {
        v.as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let (a, b) = (list(a), list(b));
    a.len() == b.len()
        && a.iter().all(|x| b.iter().any(|y| same_canonical(x, y)))
        && b.iter().all(|y| a.iter().any(|x| same_canonical(x, y)))
}

/// The same type codes, each with the same profiles and target profiles, in any order, but for
/// the `excluded` codes the snapshot may leave out.
fn same_types(differential: &Value, snapshot: &Value, excluded: &[&str]) -> bool {
    let (Some(wanted), Some(generated)) = (differential.as_array(), snapshot.as_array()) else {
        return false;
    };
    let wanted: Vec<&Value> = wanted
        .iter()
        .filter(|t| {
            !(t["code"]
                .as_str()
                .is_some_and(|code| excluded.contains(&suffix(code).as_str()))
                && !generated.iter().any(|g| g["code"] == t["code"]))
        })
        .collect();
    let same = |a: &Value, b: &Value| {
        a["code"] == b["code"]
            && same_canonicals(&a["profile"], &b["profile"])
            && same_canonicals(&a["targetProfile"], &b["targetProfile"])
    };
    wanted.len() == generated.len() && wanted.iter().all(|a| generated.iter().any(|b| same(a, b)))
}

fn same_binding(differential: &Value, snapshot: &Value) -> bool {
    differential["strength"] == snapshot["strength"]
        && match (
            differential["valueSet"].as_str(),
            snapshot["valueSet"].as_str(),
        ) {
            (Some(a), Some(b)) => same_canonical(a, b),
            (None, _) => true,
            (Some(_), None) => false,
        }
}

/// The slicing's discriminators, rules and ordering, each where the differential sets it (the
/// generator fills in the defaults it leaves out); its description is documentation.
///
/// A choice's slicing by type (`$this`) may be closed in the snapshot when the differential
/// leaves it open: the HL7 generators close it, since the choice's types are listed.
fn same_slicing(differential: &Value, snapshot: &Value) -> bool {
    let by_type = differential["discriminator"].as_array().is_some_and(|d| {
        d.iter()
            .all(|d| d["type"] == "type" && d["path"] == "$this")
    });
    ["discriminator", "rules", "ordered"].iter().all(|field| {
        differential.get(*field).is_none_or(|value| {
            let other = snapshot.get(*field);
            other == Some(value)
                || (*field == "rules" && by_type && other.is_some_and(|o| o == "closed"))
        })
    })
}

/// `constraint <key>` for each differential invariant the snapshot lacks, or has with another
/// expression or severity.
fn missing_invariants(differential: &Value, snapshot: &Map<String, Value>) -> Vec<String> {
    let generated = snapshot
        .get("constraint")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    differential
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|invariant| {
            let key = invariant["key"].as_str()?;
            let carried = generated.iter().any(|other| {
                other["key"] == invariant["key"]
                    && ["expression", "severity"].iter().all(|field| {
                        invariant
                            .get(*field)
                            .is_none_or(|v| other.get(*field) == Some(v))
                    })
            });
            (!carried).then(|| format!("constraint {key}"))
        })
        .collect()
}
