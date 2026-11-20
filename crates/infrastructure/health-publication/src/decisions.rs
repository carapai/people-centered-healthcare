//! Decision tables evaluated on the server's FHIRPath engine (studio.md § Decision tables;
//! #516): *Try a patient* and the editor's checks in the Studio, and the `tests/` cases
//! submission's simulation runs ([`crate::WorkflowSandbox`]).
//!
//! Sample inputs (`%<requirement>.<path>` → a value) are bound as FHIRPath variables: each data
//! requirement the definition declares is `%<requirement>`, an element holding the values given
//! for its paths. A condition holds when it evaluates to `true`; empty, `false` or an error does
//! not hold (an error is reported).

use std::collections::BTreeMap;

use health_fhir_validation::{FhirPathEngine, FhirPathItem};
use health_workflow::Value as Sample;
use health_workflow::decision::{
    Coverage, Decision, Inputs, MOST_COMBINATIONS, TableTest, combinations, coverage, input,
};
use health_workflow::definition::{DecisionTable, WorkflowDefinition};
use serde_json::{Map, Value, json};

/// How one expression came out for the sample inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It evaluated to `true`.
    Holds,
    /// It evaluated to `false`, or to nothing.
    Fails,
    /// It could not be evaluated.
    Error(String),
}

impl Outcome {
    /// Whether the expression holds.
    pub fn holds(&self) -> bool {
        matches!(self, Self::Holds)
    }
}

/// What *Try a patient* found: the decision and how each expression asked about came out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tried {
    /// The outcome, and the row that fired.
    pub decision: Decision,
    /// Each expression asked about, in order, with how it came out.
    pub nodes: Vec<(String, Outcome)>,
}

/// The FHIRPath variables `inputs` give: each data requirement of `definition` as an element
/// holding its inputs' values. An input that is not `%<requirement>.<path>` of a declared
/// requirement is refused.
pub fn variables(
    definition: &WorkflowDefinition,
    inputs: &Inputs,
) -> Result<BTreeMap<String, Value>, String> {
    let mut variables: BTreeMap<String, Value> = definition
        .data_requirements
        .iter()
        .map(|r| (r.name.clone(), Value::Object(Map::new())))
        .collect();
    for (name, sample) in inputs {
        let Some((requirement, path)) = input(name) else {
            return Err(format!("{name} is not an input (%<requirement>.<path>)"));
        };
        let Some(mut at) = variables.get_mut(requirement) else {
            return Err(format!(
                "{name} reads {requirement}, which is not a declared data requirement"
            ));
        };
        let (last, parents) = path
            .split_last()
            .ok_or_else(|| format!("{name} has no path"))?;
        for part in parents {
            let Value::Object(object) = at else {
                return Err(format!("{name} reads into a value"));
            };
            at = object
                .entry((*part).to_owned())
                .or_insert_with(|| Value::Object(Map::new()));
        }
        let Value::Object(object) = at else {
            return Err(format!("{name} reads into a value"));
        };
        object.insert(
            (*last).to_owned(),
            match sample {
                Sample::Bool(b) => json!(b),
                Sample::Int(i) => json!(i),
                Sample::Text(t) => json!(t),
            },
        );
    }
    Ok(variables)
}

/// Evaluates every one of `expressions` for `inputs`.
async fn evaluate(
    engine: &dyn FhirPathEngine,
    variables: &BTreeMap<String, Value>,
    expressions: &[String],
) -> Vec<Outcome> {
    let root = json!({ "resourceType": "Parameters" });
    engine
        .evaluate_with(&root, expressions, variables)
        .await
        .into_iter()
        .map(|result| match result {
            Ok(items) if items == [FhirPathItem::Boolean(true)] => Outcome::Holds,
            Ok(_) => Outcome::Fails,
            Err(failure) => Outcome::Error(failure.0),
        })
        .collect()
}

/// Every condition of `table`, in row order.
fn conditions(table: &DecisionTable) -> Vec<String> {
    table.rows.iter().flat_map(|r| r.when.clone()).collect()
}

/// `table` decided from the outcomes of its conditions, in row order.
fn decide(table: &DecisionTable, outcomes: &[Outcome]) -> Decision {
    let mut at = 0;
    let starts: Vec<usize> = table
        .rows
        .iter()
        .map(|row| {
            let start = at;
            at += row.when.len();
            start
        })
        .collect();
    let mut seen = vec![0; table.rows.len()];
    table.decide(|row, _| {
        let index = starts[row] + seen[row];
        seen[row] += 1;
        outcomes.get(index).is_some_and(Outcome::holds)
    })
}

/// *Try a patient*: `table` of `definition` decided for `inputs`, and how each of `nodes` (the
/// parts of a rule's condition tree) came out.
pub async fn try_table(
    engine: &dyn FhirPathEngine,
    definition: &WorkflowDefinition,
    table: &DecisionTable,
    inputs: &Inputs,
    nodes: &[String],
) -> Result<Tried, String> {
    let variables = variables(definition, inputs)?;
    let mut expressions = conditions(table);
    let count = expressions.len();
    expressions.extend(nodes.iter().cloned());
    let outcomes = evaluate(engine, &variables, &expressions).await;
    let (rows, asked) = outcomes.split_at(count.min(outcomes.len()));
    Ok(Tried {
        decision: decide(table, rows),
        nodes: nodes.iter().cloned().zip(asked.iter().cloned()).collect(),
    })
}

/// What the editor's checks found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Checked {
    /// The gaps and the rules that never fire, unless there were too many combinations.
    pub coverage: Option<Coverage>,
    /// How many combinations of sample values were tried.
    pub combinations: usize,
    /// Conditions that could not be evaluated, each once.
    pub errors: Vec<String>,
}

/// Coverage of `table` over every combination of `samples` (each input's sample values), at
/// most [`MOST_COMBINATIONS`].
pub async fn check(
    engine: &dyn FhirPathEngine,
    definition: &WorkflowDefinition,
    table: &DecisionTable,
    samples: &BTreeMap<String, Vec<Sample>>,
) -> Result<Checked, String> {
    let Some(all) = combinations(samples, MOST_COMBINATIONS) else {
        return Ok(Checked::default());
    };
    let expressions = conditions(table);
    let mut decided = Vec::with_capacity(all.len());
    let mut errors = Vec::new();
    for inputs in all {
        let variables = variables(definition, &inputs)?;
        let outcomes = evaluate(engine, &variables, &expressions).await;
        for (expression, outcome) in expressions.iter().zip(&outcomes) {
            if let Outcome::Error(error) = outcome {
                let error = format!("{expression}: {error}");
                if !errors.contains(&error) {
                    errors.push(error);
                }
            }
        }
        decided.push((inputs, decide(table, &outcomes)));
    }
    Ok(Checked {
        coverage: Some(coverage(table, &decided)),
        combinations: decided.len(),
        errors,
    })
}

/// Runs a test case against `definition`: the outcome it now gives, or why it cannot run.
pub async fn run_test(
    engine: &dyn FhirPathEngine,
    definition: &WorkflowDefinition,
    test: &TableTest,
) -> Result<Decision, String> {
    let Some(table) = definition
        .decision_tables
        .iter()
        .find(|t| t.id == test.table)
    else {
        return Err(format!(
            "the definition has no decision table {}",
            test.table
        ));
    };
    Ok(try_table(engine, definition, table, &test.inputs, &[])
        .await?
        .decision)
}

/// A value as the editor and the failures show it.
pub fn shown(value: Option<&Sample>) -> String {
    match value {
        None => "no outcome".to_owned(),
        Some(Sample::Bool(b)) => b.to_string(),
        Some(Sample::Int(i)) => i.to_string(),
        Some(Sample::Text(t)) => t.clone(),
    }
}

#[cfg(test)]
mod tests;
