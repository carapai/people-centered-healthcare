//! A Workflow Definition (ADR 0042, 0043, 0049; workflow-engine.md): what a
//! `WorkflowDefinitionLibrary`'s `application/vnd.health.workflow+json` content holds, and the
//! checks publication runs on it (workflow-engine.md § Simulation and publication gates).
//!
//! - **The statechart** ([`Statechart`]). Its typed actions name an action by `<id>@<major>`.
//! - **The data model:** declared, typed fields. An `assign` may only set a declared field, with
//!   a value of its kind.
//! - **Data requirements:** the resources and paths the guards need, so the engine loads only
//!   those.
//! - **Decision tables:** rows of FHIRPath conditions, each with an outcome, for WHO SMART
//!   decision logic. The first row whose conditions all hold gives the outcome.
//! - **The workflow principal:** the permissions timer and system transitions may use, approved
//!   at publication.
//!
//! [`WorkflowDefinition::problems`] is the publication gate's check, against the actions the
//! release knows ([`ActionDescriptor`]): every action reference and its parameters valid, every
//! assignment typed, no unreachable states, no non-final state without an exit, decision tables
//! well formed, and no principal permission that no action needs. There is no scripting: guards
//! and conditions are FHIRPath, evaluated through a port. Type-checking guards against the data
//! requirements comes with workflow simulation (#101).

use std::collections::{BTreeMap, BTreeSet};

use health_workflow_actions::{ActionDescriptor, ParamKind};
use serde::{Deserialize, Serialize};

use crate::interpreter::{DefinitionError, Machine};
use crate::model::{Action, Kind, State, Statechart, Value};

/// The kind of a data-model field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FieldKind {
    /// A flag.
    Bool,
    /// A counter.
    Int,
    /// An id or a code.
    Text,
}

/// A declared data-model field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    /// Its name.
    pub name: String,
    /// Its kind.
    pub kind: FieldKind,
}

/// What the guards need loaded: a resource and the paths read from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataRequirement {
    /// The name guards use for it, e.g. `encounter`.
    pub name: String,
    /// Its resource type, e.g. `Encounter`.
    pub resource_type: String,
    /// The FHIRPath paths read, e.g. `Encounter.priority`.
    #[serde(default)]
    pub paths: Vec<String>,
}

/// One row of a decision table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// FHIRPath conditions, all of which must hold.
    pub when: Vec<String>,
    /// The outcome.
    pub then: Value,
}

/// A decision table: the first row whose conditions hold gives the outcome, else the default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionTable {
    /// Its id, unique in the definition.
    pub id: String,
    /// The rows, in order.
    pub rows: Vec<Row>,
    /// The outcome when no row holds.
    pub default: Option<Value>,
}

/// A Workflow Definition, as its Library's content holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDefinition {
    /// Its canonical URL.
    pub url: String,
    /// Its version.
    pub version: String,
    /// The statechart.
    pub statechart: Statechart,
    /// The data-model fields.
    #[serde(default)]
    pub data_model: Vec<Field>,
    /// What the guards need loaded.
    #[serde(default)]
    pub data_requirements: Vec<DataRequirement>,
    /// Decision tables.
    #[serde(default)]
    pub decision_tables: Vec<DecisionTable>,
    /// The workflow principal's permissions.
    #[serde(default)]
    pub principal: Vec<String>,
}

/// Why a definition may not be published.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Problem {
    /// Not a valid statechart.
    #[error(transparent)]
    Statechart(#[from] DefinitionError),
    /// An action reference that is not `<id>@<major>`.
    #[error("{0:?} is not an action reference (<id>@<major>)")]
    BadReference(String),
    /// An action this release does not know, at that major version.
    #[error("unknown action {0}")]
    UnknownAction(String),
    /// A parameter the action does not have.
    #[error("{action} has no parameter {param}")]
    UnknownParameter {
        /// The action.
        action: String,
        /// The parameter.
        param: String,
    },
    /// A required parameter not given.
    #[error("{action} needs {param}")]
    MissingParameter {
        /// The action.
        action: String,
        /// The parameter.
        param: String,
    },
    /// A parameter of the wrong kind, or a code the action does not allow.
    #[error("{action}: {param} is not a valid value")]
    InvalidParameter {
        /// The action.
        action: String,
        /// The parameter.
        param: String,
    },
    /// An assignment to a field the data model does not declare, or of another kind.
    #[error("{0} is not a declared field of that kind")]
    BadAssignment(String),
    /// A state nothing can reach.
    #[error("state {0} cannot be reached")]
    Unreachable(String),
    /// A non-final state with no transition out, on it or any ancestor.
    #[error("state {0} has no way out")]
    DeadEnd(String),
    /// A decision table with no rows, an empty condition, or an id used twice.
    #[error("decision table {0} is not well formed")]
    BadDecisionTable(String),
    /// A principal permission no action of the definition needs.
    #[error("the workflow principal declares {0}, which no action needs")]
    UnneededPermission(String),
}

fn states<'a>(list: &'a [State], into: &mut Vec<(&'a State, Vec<&'a State>)>, path: &[&'a State]) {
    for state in list {
        into.push((state, path.to_vec()));
        let mut next = path.to_vec();
        next.push(state);
        states(&state.children, into, &next);
    }
}

fn actions(state: &State) -> impl Iterator<Item = &Action> {
    state
        .on_entry
        .iter()
        .chain(&state.on_exit)
        .flatten()
        .chain(state.transitions.iter().flat_map(|t| &t.actions))
}

fn fits(value: &Value, kind: ParamKind) -> bool {
    match (value, kind) {
        (Value::Bool(_), ParamKind::Bool) | (Value::Int(_), ParamKind::Int) => true,
        (Value::Text(text), ParamKind::Text) => !text.is_empty(),
        (Value::Text(text), ParamKind::Code(codes)) => codes.contains(&text.as_str()),
        _ => false,
    }
}

impl WorkflowDefinition {
    /// Every reason this definition may not be published against the actions in `catalog`.
    pub fn problems(&self, catalog: &[ActionDescriptor]) -> Vec<Problem> {
        let mut problems = Vec::new();
        if let Err(error) = Machine::new(&self.statechart) {
            problems.push(Problem::Statechart(error));
            return problems;
        }
        let mut all = Vec::new();
        states(&self.statechart.states, &mut all, &[]);

        let mut needed = BTreeSet::new();
        let fields: BTreeMap<&str, FieldKind> = self
            .data_model
            .iter()
            .map(|f| (f.name.as_str(), f.kind))
            .collect();
        for (state, _) in &all {
            for action in actions(state) {
                match action {
                    Action::Execute { action, params } => {
                        self.check_action(action, params, catalog, &mut needed, &mut problems);
                    }
                    Action::Assign { field, value } => {
                        let kind = match value {
                            Value::Bool(_) => FieldKind::Bool,
                            Value::Int(_) => FieldKind::Int,
                            Value::Text(_) => FieldKind::Text,
                        };
                        if fields.get(field.as_str()) != Some(&kind) {
                            problems.push(Problem::BadAssignment(field.clone()));
                        }
                    }
                    Action::Raise { .. } => {}
                }
            }
        }

        self.check_reachability(&all, &mut problems);

        let mut ids = BTreeSet::new();
        for table in &self.decision_tables {
            let malformed = !ids.insert(table.id.as_str())
                || table.rows.is_empty()
                || table
                    .rows
                    .iter()
                    .any(|row| row.when.is_empty() || row.when.iter().any(|c| c.trim().is_empty()));
            if malformed {
                problems.push(Problem::BadDecisionTable(table.id.clone()));
            }
        }

        for permission in &self.principal {
            if !needed.contains(permission.as_str()) {
                problems.push(Problem::UnneededPermission(permission.clone()));
            }
        }
        problems
    }

    fn check_action<'a>(
        &self,
        reference: &str,
        params: &BTreeMap<String, Value>,
        catalog: &'a [ActionDescriptor],
        needed: &mut BTreeSet<&'a str>,
        problems: &mut Vec<Problem>,
    ) {
        let Some((id, major)) = reference
            .split_once('@')
            .and_then(|(id, major)| Some((id, major.parse::<u16>().ok()?)))
        else {
            problems.push(Problem::BadReference(reference.to_owned()));
            return;
        };
        let Some(descriptor) = catalog.iter().find(|d| d.id == id && d.major == major) else {
            problems.push(Problem::UnknownAction(reference.to_owned()));
            return;
        };
        needed.insert(descriptor.permission);
        for name in params.keys() {
            if !descriptor.params.iter().any(|p| p.name == name) {
                problems.push(Problem::UnknownParameter {
                    action: reference.to_owned(),
                    param: name.clone(),
                });
            }
        }
        for spec in &descriptor.params {
            match params.get(spec.name) {
                None if spec.required => problems.push(Problem::MissingParameter {
                    action: reference.to_owned(),
                    param: spec.name.to_owned(),
                }),
                Some(value) if !fits(value, spec.kind) => {
                    problems.push(Problem::InvalidParameter {
                        action: reference.to_owned(),
                        param: spec.name.to_owned(),
                    });
                }
                _ => {}
            }
        }
    }

    /// Unreachable states, and non-final atomic states with no transition out.
    fn check_reachability(&self, all: &[(&State, Vec<&State>)], problems: &mut Vec<Problem>) {
        let by_id: BTreeMap<&str, (&State, &Vec<&State>)> = all
            .iter()
            .map(|(state, path)| (state.id.as_str(), (*state, path)))
            .collect();
        let mut reached = BTreeSet::new();
        let mut pending: Vec<&str> = if self.statechart.initial.is_empty() {
            self.statechart
                .states
                .first()
                .map(|s| s.id.as_str())
                .into_iter()
                .collect()
        } else {
            self.statechart.initial.iter().map(String::as_str).collect()
        };
        while let Some(id) = pending.pop() {
            let Some((state, path)) = by_id.get(id) else {
                continue;
            };
            if !reached.insert(id) {
                continue;
            }
            // Entering a state enters its ancestors, and its default descendants.
            pending.extend(path.iter().map(|s| s.id.as_str()));
            match state.kind {
                Kind::Parallel => pending.extend(state.children.iter().map(|c| c.id.as_str())),
                Kind::Compound if state.initial.is_empty() => {
                    pending.extend(state.children.first().map(|c| c.id.as_str()));
                }
                Kind::Compound => pending.extend(state.initial.iter().map(String::as_str)),
                Kind::Atomic | Kind::Final => {}
            }
            for transition in &state.transitions {
                pending.extend(transition.targets.iter().map(String::as_str));
            }
        }
        for (state, path) in all {
            if !reached.contains(state.id.as_str()) {
                problems.push(Problem::Unreachable(state.id.clone()));
                continue;
            }
            let leaf = state.children.is_empty() && state.kind != Kind::Final;
            // A way out is a transition on the state or any ancestor.
            let exits = !state.transitions.is_empty()
                || path.iter().any(|ancestor| !ancestor.transitions.is_empty());
            if leaf && !exits {
                problems.push(Problem::DeadEnd(state.id.clone()));
            }
        }
    }
}

#[cfg(test)]
mod tests;
