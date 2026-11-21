//! The Workflow Studio's sandbox (studio.md § The Workflow Studio; #499), at
//! `POST /packages/{id}/workflows/sandbox`. It plays the draft Workflow Definition as the Studio
//! holds it, unsaved edits included, in the engine's simulation (`health_workflow::simulation`)
//! with every action stubbed: nothing is carried out.
//!
//! The run lives in the browser as the author's guard answers, gate outcomes and the moves so far
//! (each an event or a wait). Each request replays it from the start and answers what held after
//! each move, so every step comes from the server: undo drops the last move, restart sends none.
//! A move that would evaluate a guard the author has not answered is not taken and the guards are
//! reported. The answer carries the run as a `workflow-scenario` file, for *Save as scenario*.

use std::collections::BTreeMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use health_access_policy::Subject;
use health_time::SignedDuration;
use health_workflow::definition::WorkflowDefinition;
use health_workflow::simulation::{self, Move, Play, Snapshot};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use super::{Studio, StudioProblem, author, failure, problem};

/// The name a saved run takes when none is given.
const UNNAMED: &str = "Sandbox run";

/// One move: an event to send, or a wait.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, ToSchema)]
pub(crate) struct StudioSandboxMove {
    /// The event to send.
    #[serde(default)]
    pub(crate) event: Option<String>,
    /// How long to wait (ISO 8601), firing the timers that fall due.
    #[serde(default)]
    pub(crate) wait: Option<String>,
}

/// A sandbox run of a draft definition.
#[derive(Debug, Clone, PartialEq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioSandboxInput {
    /// The draft Workflow Definition, as the Studio holds it.
    #[schema(value_type = Object)]
    pub(crate) definition: Value,
    /// The author's answer to each guard, by its expression.
    #[serde(default)]
    pub(crate) guards: BTreeMap<String, bool>,
    /// The outcome of each Financial Gate, by canonical URL: `pass`, `defer` or `block`. An
    /// unanswered gate defers.
    #[serde(default)]
    pub(crate) gates: BTreeMap<String, String>,
    /// The moves so far, in order.
    #[serde(default)]
    pub(crate) moves: Vec<StudioSandboxMove>,
    /// The name the run takes as a scenario.
    #[serde(default)]
    pub(crate) name: Option<String>,
}

/// A pending timer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioSandboxTimer {
    /// The event it raises.
    pub(crate) event: String,
    /// How long until it falls due (ISO 8601).
    pub(crate) due_in: String,
}

/// An open Task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct StudioSandboxTask {
    /// Its state.
    pub(crate) state: String,
    /// `ready` or `in-progress`.
    pub(crate) status: String,
}

/// What held at the start, or after a move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioSandboxStep {
    /// The active states.
    pub(crate) configuration: Vec<String>,
    /// The typed actions the move ran, stubbed (recorded, never carried out).
    pub(crate) actions: Vec<String>,
    /// The pending timers, earliest first.
    pub(crate) timers: Vec<StudioSandboxTimer>,
    /// The open Tasks.
    pub(crate) tasks: Vec<StudioSandboxTask>,
    /// Whether the instance is still running.
    pub(crate) running: bool,
    /// The events the configuration accepts.
    pub(crate) accepts: Vec<String>,
    /// How long since the start (ISO 8601).
    pub(crate) elapsed: String,
}

/// How the run went.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioSandbox {
    /// Every guard of the definition, for the author to answer.
    pub(crate) guards: Vec<String>,
    /// Every Financial Gate of the definition.
    pub(crate) gates: Vec<String>,
    /// The start, then each move taken.
    pub(crate) steps: Vec<StudioSandboxStep>,
    /// The guards the next move needs answered: it was not taken.
    pub(crate) unanswered: Vec<String>,
    /// Why the next move could not be taken, if it could not for another reason.
    pub(crate) refused: Option<String>,
    /// The run as the text of a `workflow-scenario` file of the package's `tests/` folder.
    pub(crate) scenario: String,
}

fn duration(d: SignedDuration) -> String {
    d.to_string()
}

fn step(snapshot: &Snapshot) -> StudioSandboxStep {
    StudioSandboxStep {
        configuration: snapshot.configuration.clone(),
        actions: snapshot.actions.clone(),
        timers: snapshot
            .timers
            .iter()
            .map(|t| StudioSandboxTimer {
                event: t.event.clone(),
                due_in: duration(t.due_in),
            })
            .collect(),
        tasks: snapshot
            .tasks
            .iter()
            .map(|t| StudioSandboxTask {
                state: t.state.clone(),
                status: t.status.clone(),
            })
            .collect(),
        running: snapshot.running,
        accepts: snapshot.accepts.clone(),
        elapsed: duration(snapshot.elapsed),
    }
}

fn unprocessable(detail: impl Into<String>) -> Response {
    problem(StatusCode::UNPROCESSABLE_ENTITY, detail, None)
}

/// The run `input` asks for, or why it is malformed.
fn read(input: &StudioSandboxInput) -> Result<Play, String> {
    let moves = input
        .moves
        .iter()
        .enumerate()
        .map(|(n, m)| {
            let wait = m
                .wait
                .as_deref()
                .map(|w| {
                    w.parse::<SignedDuration>()
                        .map_err(|e| format!("move {}: {w:?} is not a duration: {e}", n + 1))
                })
                .transpose()?;
            Ok(Move {
                event: m.event.clone(),
                wait,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(Play {
        guards: input.guards.clone(),
        gates: input.gates.clone(),
        moves,
    })
}

/// Documents and serves the sandbox.
#[utoipa::path(post, path = "/api/studio/packages/{id}/workflows/sandbox", tag = "studio", operation_id = "playStudioWorkflowSandbox",
    description = "The Workflow Studio's sandbox: plays the posted (unsaved) draft Workflow Definition in the engine's simulation with every action stubbed, from the start through each move (an event or an ISO 8601 wait), with the author's guard answers and gate outcomes. It answers what held after each move: the configuration, the stubbed actions, the pending timers, the open Tasks and the events the configuration accepts. A move that would evaluate an unanswered guard is not taken and the guards are reported, never guessed. The run comes back as a `workflow-scenario` file for *Save as scenario*.",
    params(("id" = String, Path, description = "`name@version`")),
    request_body = StudioSandboxInput,
    responses(
        (status = 200, description = "The run", body = StudioSandbox),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version", body = StudioProblem),
        (status = 422, description = "Not a Workflow Definition, an invalid statechart, or a malformed wait", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn play(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(input): Json<StudioSandboxInput>,
) -> Response {
    if let Err(response) = author(subject) {
        return *response;
    }
    if let Err(error) = studio.publications.draft_files(&id).await {
        return failure(&error);
    }
    let definition: WorkflowDefinition = match serde_json::from_value(input.definition.clone()) {
        Ok(definition) => definition,
        Err(error) => return unprocessable(format!("not a Workflow Definition: {error}")),
    };
    let run = match read(&input) {
        Ok(run) => run,
        Err(error) => return unprocessable(error),
    };
    let played = match simulation::play(&definition, &run) {
        Ok(played) => played,
        Err(error) => return unprocessable(error),
    };
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(UNNAMED);
    let scenario = simulation::save(&definition, &run, &played, name);
    let scenario = match serde_json::to_string_pretty(&scenario) {
        Ok(scenario) => scenario,
        Err(error) => return unprocessable(error.to_string()),
    };
    Json(StudioSandbox {
        guards: simulation::guards(&definition.statechart),
        gates: simulation::gates(&definition.statechart),
        steps: played.snapshots.iter().map(step).collect(),
        unanswered: played.unanswered,
        refused: played.refused,
        scenario,
    })
    .into_response()
}
