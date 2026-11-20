//! The decision-table editor's server side (studio.md § Decision tables; #516), under
//! `/packages/{id}/decision-tables/`. Both take the draft Workflow Definition as the Studio
//! holds it, unsaved edits included, and evaluate on central's FHIRPath engine, the one
//! submission's simulation uses:
//!
//! - `POST …/try` is *Try a patient*: the outcome for sample inputs, the rule that fired, and
//!   how each part of a rule's condition tree came out.
//! - `POST …/check` is the editor's checks: the inputs no rule covers when the table has no
//!   default and the rules that never fire, over every combination of sample values per input,
//!   and the package's saved test cases for the table, run against the draft.

use std::collections::BTreeMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use health_access_policy::Subject;
use health_publication::decisions::{self, Outcome};
use health_workflow::Value as Sample;
use health_workflow::decision::{Inputs, MOST_COMBINATIONS, TEST_TYPE, TableTest};
use health_workflow::definition::{DecisionTable, WorkflowDefinition};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use super::{Studio, StudioProblem, author, failure, problem};

/// The most gaps an answer lists.
const MOST_GAPS: usize = 20;

/// A sample value: a flag, a whole number or a code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub(crate) enum StudioSample {
    /// A flag.
    Boolean(bool),
    /// A whole number.
    Integer(i64),
    /// A code or a text.
    Text(String),
}

impl From<&Sample> for StudioSample {
    fn from(value: &Sample) -> Self {
        match value {
            Sample::Bool(b) => Self::Boolean(*b),
            Sample::Int(i) => Self::Integer(*i),
            Sample::Text(t) => Self::Text(t.clone()),
        }
    }
}

impl From<StudioSample> for Sample {
    fn from(value: StudioSample) -> Self {
        match value {
            StudioSample::Boolean(b) => Self::Bool(b),
            StudioSample::Integer(i) => Self::Int(i),
            StudioSample::Text(t) => Self::Text(t),
        }
    }
}

fn inputs(values: BTreeMap<String, StudioSample>) -> Inputs {
    values.into_iter().map(|(k, v)| (k, v.into())).collect()
}

fn shown(inputs: &Inputs) -> BTreeMap<String, StudioSample> {
    inputs
        .iter()
        .map(|(k, v)| (k.clone(), StudioSample::from(v)))
        .collect()
}

/// *Try a patient* on a draft table.
#[derive(Debug, Clone, PartialEq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDecisionTryInput {
    /// The draft Workflow Definition, as the Studio holds it.
    #[schema(value_type = Object)]
    pub(crate) definition: Value,
    /// The table's id.
    pub(crate) table: String,
    /// The sample inputs, by input (`%<requirement>.<path>`).
    #[serde(default)]
    pub(crate) inputs: BTreeMap<String, StudioSample>,
    /// The parts of a rule's condition tree to mark, each as FHIRPath.
    #[serde(default)]
    pub(crate) nodes: Vec<String>,
}

/// How one part of a condition tree came out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioNodeResult {
    /// The part, as FHIRPath.
    pub(crate) expression: String,
    /// Whether it holds for the sample inputs.
    pub(crate) holds: bool,
    /// Why it could not be evaluated, if it could not.
    pub(crate) error: Option<String>,
}

/// What a draft table decided for sample inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDecisionTry {
    /// The outcome; none when no rule holds and there is no default.
    pub(crate) outcome: Option<StudioSample>,
    /// The index of the rule that fired; none when the default (or nothing) decided.
    pub(crate) row: Option<u32>,
    /// Each part asked about, in order.
    pub(crate) nodes: Vec<StudioNodeResult>,
}

/// The editor's checks on a draft table.
#[derive(Debug, Clone, PartialEq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDecisionCheckInput {
    /// The draft Workflow Definition, as the Studio holds it.
    #[schema(value_type = Object)]
    pub(crate) definition: Value,
    /// The table's id.
    pub(crate) table: String,
    /// Each input's sample values: coverage tries every combination.
    #[serde(default)]
    pub(crate) samples: BTreeMap<String, Vec<StudioSample>>,
}

/// One saved test case of the table, run against the draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDecisionTest {
    /// Its file in the package.
    pub(crate) path: String,
    /// What it shows.
    pub(crate) name: String,
    /// Its sample inputs.
    pub(crate) inputs: BTreeMap<String, StudioSample>,
    /// The outcome it keeps.
    pub(crate) expect: StudioSample,
    /// The outcome the draft gives.
    pub(crate) outcome: Option<StudioSample>,
    /// Whether they are the same.
    pub(crate) passes: bool,
    /// Why it could not run, if it could not.
    pub(crate) error: Option<String>,
}

/// What the checks found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDecisionCheck {
    /// Whether coverage was checked: not when the combinations of sample values exceed
    /// `mostCombinations`.
    pub(crate) covered: bool,
    /// How many combinations were tried.
    pub(crate) combinations: u32,
    /// The most combinations coverage tries.
    pub(crate) most_combinations: u32,
    /// How many combinations no rule covers (there is no default).
    pub(crate) gap_count: u32,
    /// The first of them.
    pub(crate) gaps: Vec<BTreeMap<String, StudioSample>>,
    /// The rules (indexes) that fire for no combination.
    pub(crate) never_fires: Vec<u32>,
    /// Conditions that could not be evaluated.
    pub(crate) errors: Vec<String>,
    /// The package's saved test cases of this table, run against the draft.
    pub(crate) tests: Vec<StudioDecisionTest>,
}

fn unprocessable(detail: impl Into<String>) -> Response {
    problem(StatusCode::UNPROCESSABLE_ENTITY, detail, None)
}

fn refused(detail: impl Into<String>) -> Box<Response> {
    Box::new(unprocessable(detail))
}

/// The draft definition and its table `id`, or why not.
fn table(
    definition: Value,
    id: &str,
) -> Result<(WorkflowDefinition, DecisionTable), Box<Response>> {
    let definition: WorkflowDefinition = serde_json::from_value(definition)
        .map_err(|e| refused(format!("not a Workflow Definition: {e}")))?;
    let table = definition
        .decision_tables
        .iter()
        .find(|t| t.id == id)
        .cloned()
        .ok_or_else(|| refused(format!("the definition has no decision table {id}")))?;
    Ok((definition, table))
}

const fn count(n: usize) -> u32 {
    if n > u32::MAX as usize {
        u32::MAX
    } else {
        n as u32
    }
}

/// Documents and serves *Try a patient*.
#[utoipa::path(post, path = "/api/studio/packages/{id}/decision-tables/try", tag = "studio", operation_id = "tryStudioDecisionTable",
    description = "*Try a patient* on a draft decision table: the server evaluates the table of the posted (unsaved) Workflow Definition for sample inputs, each data requirement bound as `%<requirement>`, and answers the outcome, the rule that fired and how each part of a rule's condition tree came out.",
    params(("id" = String, Path, description = "`name@version`")),
    request_body = StudioDecisionTryInput,
    responses(
        (status = 200, description = "The decision", body = StudioDecisionTry),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version", body = StudioProblem),
        (status = 422, description = "No such table, or an input of no declared data requirement", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn try_table(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(input): Json<StudioDecisionTryInput>,
) -> Response {
    if let Err(response) = author(subject) {
        return *response;
    }
    if let Err(error) = studio.publications.draft_files(&id).await {
        return failure(&error);
    }
    let (definition, table) = match table(input.definition, &input.table) {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let tried = match decisions::try_table(
        studio.engine.as_ref(),
        &definition,
        &table,
        &inputs(input.inputs),
        &input.nodes,
    )
    .await
    {
        Ok(tried) => tried,
        Err(error) => return unprocessable(error),
    };
    Json(StudioDecisionTry {
        outcome: tried.decision.outcome.as_ref().map(StudioSample::from),
        row: tried.decision.row.map(count),
        nodes: tried
            .nodes
            .into_iter()
            .map(|(expression, outcome)| StudioNodeResult {
                expression,
                holds: outcome.holds(),
                error: match outcome {
                    Outcome::Error(error) => Some(error),
                    Outcome::Holds | Outcome::Fails => None,
                },
            })
            .collect(),
    })
    .into_response()
}

/// Documents and serves the editor's checks.
#[utoipa::path(post, path = "/api/studio/packages/{id}/decision-tables/check", tag = "studio", operation_id = "checkStudioDecisionTable",
    description = "The decision-table editor's checks on a draft table of the posted (unsaved) Workflow Definition: over every combination of the sample values per input (at most `mostCombinations`), the combinations no rule covers when the table has no default and the rules that never fire because a rule above always matches first; and the package version's saved test cases of the table (`tests/` files of type `decision-table-test`), run against the draft. *Check now* remains the authority.",
    params(("id" = String, Path, description = "`name@version`")),
    request_body = StudioDecisionCheckInput,
    responses(
        (status = 200, description = "What the checks found", body = StudioDecisionCheck),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version", body = StudioProblem),
        (status = 422, description = "No such table, or an input of no declared data requirement", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn check(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(input): Json<StudioDecisionCheckInput>,
) -> Response {
    if let Err(response) = author(subject) {
        return *response;
    }
    let files = match studio.publications.draft_files(&id).await {
        Ok((files, _)) => files,
        Err(error) => return failure(&error),
    };
    let (definition, table) = match table(input.definition, &input.table) {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let samples = input
        .samples
        .into_iter()
        .map(|(k, values)| (k, values.into_iter().map(Sample::from).collect()))
        .collect();
    let engine = studio.engine.as_ref();
    let checked = match decisions::check(engine, &definition, &table, &samples).await {
        Ok(checked) => checked,
        Err(error) => return unprocessable(error),
    };
    let canonical = format!("{}|{}", definition.url, definition.version);
    let mut tests = Vec::new();
    for (path, bytes) in &files {
        if !path.starts_with("tests/") || !path.ends_with(".json") {
            continue;
        }
        let Ok(test) = serde_json::from_slice::<TableTest>(bytes) else {
            continue;
        };
        if test.kind != TEST_TYPE || test.definition != canonical || test.table != table.id {
            continue;
        }
        let ran = decisions::run_test(engine, &definition, &test).await;
        let outcome = ran.as_ref().ok().and_then(|d| d.outcome.as_ref());
        tests.push(StudioDecisionTest {
            path: path.clone(),
            name: test.name.clone(),
            inputs: shown(&test.inputs),
            expect: StudioSample::from(&test.expect),
            outcome: outcome.map(StudioSample::from),
            passes: outcome == Some(&test.expect),
            error: ran.err(),
        });
    }
    let covered = checked.coverage.is_some();
    let coverage = checked.coverage.unwrap_or_default();
    Json(StudioDecisionCheck {
        covered,
        combinations: count(checked.combinations),
        most_combinations: count(MOST_COMBINATIONS),
        gap_count: count(coverage.gaps.len()),
        gaps: coverage.gaps.iter().take(MOST_GAPS).map(shown).collect(),
        never_fires: coverage.never_fires.into_iter().map(count).collect(),
        errors: checked.errors,
        tests,
    })
    .into_response()
}
