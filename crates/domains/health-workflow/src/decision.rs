//! Deciding with a decision table, and the checks the Studio's decision-table editor runs
//! (studio.md § Decision tables; #516).
//!
//! - **Inputs.** A table's conditions read the workflow's declared data requirements. Each path
//!   a requirement declares is one input, named as FHIRPath reads it: `%<requirement>.<path>`
//!   (the requirement `patient` of `Patient` with the path `Patient.age` is `%patient.age`).
//!   Sample inputs give each a typed [`Value`].
//! - **Deciding** ([`DecisionTable::decide`]). The first row whose conditions all hold gives the
//!   outcome; when none does, the default (if any). Conditions are FHIRPath, evaluated by the
//!   caller (through `health-fhir-validation`'s engine at central), so this module stays pure.
//! - **Coverage** ([`coverage`]). Over every combination of sample values per input
//!   ([`combinations`]): the combinations no rule covers when there is no default, and the rules
//!   that never fire because a rule above always matches first.
//! - **Test cases** ([`TableTest`]). A `tests/` file of type [`TEST_TYPE`] keeps sample inputs
//!   and the outcome they gave; submission's simulation decides again and fails the package when
//!   the outcome changed.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::definition::{DataRequirement, DecisionTable};
use crate::model::Value;

/// The `type` of a decision-table test case in a package's `tests/` folder.
pub const TEST_TYPE: &str = "decision-table-test";

/// The most combinations of sample values coverage is checked over.
pub const MOST_COMBINATIONS: usize = 1024;

/// Sample inputs: each input (`%<requirement>.<path>`) with its value.
pub type Inputs = BTreeMap<String, Value>;

/// A decision-table test case, as a package's `tests/` file holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableTest {
    /// Always [`TEST_TYPE`].
    #[serde(rename = "type")]
    pub kind: String,
    /// What it shows.
    pub name: String,
    /// The definition holding the table, `url|version`.
    pub definition: String,
    /// The table's id.
    pub table: String,
    /// The sample inputs.
    #[serde(default)]
    pub inputs: Inputs,
    /// The outcome they must give.
    pub expect: Value,
}

/// What a table decided: the outcome and the row that gave it (none when the default did, or
/// nothing did).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Decision {
    /// The outcome, if any.
    pub outcome: Option<Value>,
    /// The index of the row that fired.
    pub row: Option<usize>,
}

impl DecisionTable {
    /// The outcome: the first row whose conditions all hold (`holds` evaluates one condition of
    /// a row), else the default.
    pub fn decide(&self, mut holds: impl FnMut(usize, &str) -> bool) -> Decision {
        for (index, row) in self.rows.iter().enumerate() {
            if row.when.iter().all(|condition| holds(index, condition)) {
                return Decision {
                    outcome: Some(row.then.clone()),
                    row: Some(index),
                };
            }
        }
        Decision {
            outcome: self.default.clone(),
            row: None,
        }
    }
}

fn identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// An input's requirement and the path read from it: `%patient.age` is `("patient", ["age"])`.
/// `None` if `input` is not `%<identifier>(.<identifier>)+`.
pub fn input(input: &str) -> Option<(&str, Vec<&str>)> {
    let mut parts = input.strip_prefix('%')?.split('.');
    let requirement = parts.next().filter(|p| identifier(p))?;
    let path: Vec<&str> = parts.collect();
    (!path.is_empty() && path.iter().all(|p| identifier(p))).then_some((requirement, path))
}

impl DataRequirement {
    /// The inputs this requirement gives decision tables: one per path, as FHIRPath reads it.
    pub fn inputs(&self) -> Vec<String> {
        self.paths
            .iter()
            .filter_map(|path| {
                let rest = match path.strip_prefix(&self.resource_type) {
                    Some("") => return None,
                    Some(rest) => rest.strip_prefix('.').unwrap_or(path),
                    None => path,
                };
                let named = format!("%{}.{rest}", self.name);
                input(&named).is_some().then_some(named)
            })
            .collect()
    }
}

/// Every combination of `samples` (each input's sample values), or `None` when there are more
/// than `most`. No inputs is one empty combination.
pub fn combinations(samples: &BTreeMap<String, Vec<Value>>, most: usize) -> Option<Vec<Inputs>> {
    let mut count: usize = 1;
    for values in samples.values() {
        count = count.checked_mul(values.len())?;
    }
    if count > most {
        return None;
    }
    let mut out = vec![Inputs::new()];
    for (name, values) in samples {
        out = out
            .into_iter()
            .flat_map(|inputs| {
                values.iter().map(move |value| {
                    let mut next = inputs.clone();
                    next.insert(name.clone(), value.clone());
                    next
                })
            })
            .collect();
    }
    Some(out)
}

/// What coverage found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Coverage {
    /// The combinations that gave no outcome: no rule covers them, and there is no default.
    pub gaps: Vec<Inputs>,
    /// The rules (row indexes) that fired for no combination: a rule above always matched
    /// first, or the rule never holds.
    pub never_fires: Vec<usize>,
}

/// Coverage of `table` from what it `decided` for each combination of sample values.
pub fn coverage(table: &DecisionTable, decided: &[(Inputs, Decision)]) -> Coverage {
    let fired: BTreeSet<usize> = decided.iter().filter_map(|(_, d)| d.row).collect();
    Coverage {
        gaps: decided
            .iter()
            .filter(|(_, d)| d.outcome.is_none())
            .map(|(inputs, _)| inputs.clone())
            .collect(),
        never_fires: (0..table.rows.len())
            .filter(|n| !fired.contains(n))
            .collect(),
    }
}

#[cfg(test)]
mod tests;
