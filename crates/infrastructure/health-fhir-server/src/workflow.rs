//! Workflows, tasks and worklists over HTTP (#99; workflow-engine.md § Worklists and queues;
//! ADRs 0002, 0044), behind `subject_layer`. Working tasks needs [`WORK_PERMISSION`];
//! reassigning, escalating and cancelling need [`SUPERVISE_PERMISSION`]. The signed-in account
//! is the actor, and every command carries the client's `opId`, so a retry is a no-op.
//!
//! - `POST /instances` `{definition, context, opId}`: starts an instance;
//! - `GET /instances/{id}/tasks`: its Tasks, rebuilt from instance state;
//! - `POST /instances/{id}/events` `{event, opId}`: completes a step;
//! - `POST /instances/{id}/cancel` `{reason, opId}`;
//! - `POST /instances/{id}/tasks/{state}/claim` `{opId}`, `…/release` `{opId}`,
//!   `…/reassign` `{to, opId}`, `…/escalate` `{priority, opId}`;
//! - `GET /worklists/{key}`: the open tasks, most urgent first, then by arrival;
//! - `GET /definitions`: the definitions this node runs, with each human task's state, role
//!   and worklist.
//!
//! Each task is returned with `events`: the events its state's transitions take, so a client
//! can offer them without knowing the definition.
//!
//! **Guards and actions.** FHIRPath guards over data requirements and the contexts' typed
//! actions are not wired yet, so the node's definitions must use neither: guards evaluate as
//! false and actions fail, and [`WorkflowConfig::new`] refuses definitions that use them.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_surrealdb::{SurrealStore, SurrealTx};
use health_time::{Clock, SystemClock};
use health_workflow::definition::WorkflowDefinition;
use health_workflow::worklists::{TaskView, human_tasks, priority};
use health_workflow::{Action, DataModel, Event, Guards, State as ChartState, Statechart};
use health_workflow_app::{
    ActionCall, ActionFailed, Actions, Compiled, Outcome, WorkflowError, Workflows,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::FhirServer;

/// The permission to start workflows and work their tasks.
pub const WORK_PERMISSION: &str = "workflow.work";
/// The permission to reassign and escalate tasks and cancel workflows.
pub const SUPERVISE_PERMISSION: &str = "workflow.supervise";

/// The definitions this node runs, compiled, and the node that decides their instances.
#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    definitions: Arc<BTreeMap<String, Arc<Compiled>>>,
    authority: String,
}

/// Why the node's definitions cannot run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkflowConfigError {
    /// The definition fails its publication checks.
    #[error("{definition}: {problem}")]
    Problem {
        /// `url|version`.
        definition: String,
        /// The first problem.
        problem: String,
    },
    /// The definition uses guards or typed actions, which this node cannot run yet.
    #[error("{0} uses guards or typed actions, which are not wired yet")]
    Unsupported(String),
}

fn uses_unwired(states: &[ChartState]) -> bool {
    states.iter().any(|state| {
        let actions = state
            .on_entry
            .iter()
            .chain(&state.on_exit)
            .flatten()
            .chain(state.transitions.iter().flat_map(|t| &t.actions));
        state.transitions.iter().any(|t| t.guard.is_some())
            || actions
                .into_iter()
                .any(|a| matches!(a, Action::Execute { .. }))
            || uses_unwired(&state.children)
    })
}

impl WorkflowConfig {
    /// Runs `definitions` at node `authority`. Each must pass its publication checks and use no
    /// guards or typed actions.
    pub fn new(
        definitions: Vec<WorkflowDefinition>,
        authority: &str,
    ) -> Result<Self, WorkflowConfigError> {
        let mut compiled = BTreeMap::new();
        for definition in definitions {
            let key = format!("{}|{}", definition.url, definition.version);
            if let Some(problem) = definition.problems(&[]).first() {
                return Err(WorkflowConfigError::Problem {
                    definition: key,
                    problem: problem.to_string(),
                });
            }
            if uses_unwired(&definition.statechart.states) {
                return Err(WorkflowConfigError::Unsupported(key));
            }
            let chart =
                Compiled::new(definition.statechart).map_err(|e| WorkflowConfigError::Problem {
                    definition: key.clone(),
                    problem: e.to_string(),
                })?;
            compiled.insert(key, Arc::new(chart));
        }
        Ok(Self {
            definitions: Arc::new(compiled),
            authority: authority.to_owned(),
        })
    }
}

/// Guards are not wired yet: none holds (and [`WorkflowConfig::new`] refuses definitions with
/// guards).
struct NoGuards;

impl Guards for NoGuards {
    fn holds(&self, _: &str, _: Option<&Event>, _: &DataModel) -> bool {
        false
    }
}

/// Typed actions are not wired yet: every one fails.
struct NoActions;

impl Actions<SurrealTx> for NoActions {
    async fn execute(&self, _: &mut SurrealTx, call: &ActionCall) -> Result<(), ActionFailed> {
        Err(ActionFailed(format!(
            "{} is not available here",
            call.action
        )))
    }
}

type Service = Workflows<SurrealStore, Arc<BTreeMap<String, Arc<Compiled>>>, NoActions, NoGuards>;

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn failed(error: &WorkflowError) -> Response {
    let status = match error {
        WorkflowError::NotFound(_) | WorkflowError::UnknownDefinition(_) => StatusCode::NOT_FOUND,
        WorkflowError::Claimed(_) | WorkflowError::Conflict => StatusCode::CONFLICT,
        WorkflowError::NotOwner(_) => StatusCode::FORBIDDEN,
        WorkflowError::Store(_) => return refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    };
    refused(status, &error.to_string())
}

/// The member of staff with `permission`, and the use cases.
fn staff(
    server: &FhirServer,
    subject: Option<Extension<Subject>>,
    permission: &str,
) -> Result<(Subject, Service, Arc<WorkflowConfig>), Box<Response>> {
    let Some(config) = server.workflows.clone() else {
        return Err(Box::new(refused(
            StatusCode::NOT_FOUND,
            "workflows are not enabled",
        )));
    };
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(refused(StatusCode::UNAUTHORIZED, "sign in")));
    };
    if !subject.permissions.has(permission) {
        return Err(Box::new(refused(
            StatusCode::FORBIDDEN,
            &format!("this needs the {permission} permission"),
        )));
    }
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let service = Workflows::new(
        server.store.clone(),
        config.definitions.clone(),
        NoActions,
        NoGuards,
        clock,
        Vec::new(),
        &config.authority,
    );
    Ok((subject, service, config))
}

macro_rules! staff_or_return {
    ($server:expr, $subject:expr, $permission:expr) => {
        match staff(&$server, $subject, $permission) {
            Ok(found) => found,
            Err(response) => return *response,
        }
    };
}

fn find<'a>(states: &'a [ChartState], id: &str) -> Option<&'a ChartState> {
    states.iter().find_map(|state| {
        if state.id == id {
            Some(state)
        } else {
            find(&state.children, id)
        }
    })
}

/// The events the transitions of `state` take.
fn events(chart: &Statechart, state: &str) -> Vec<String> {
    let mut found: Vec<String> = find(&chart.states, state)
        .into_iter()
        .flat_map(|s| &s.transitions)
        .flat_map(|t| t.events.iter().cloned())
        .collect();
    found.dedup();
    found
}

fn task_json(config: &WorkflowConfig, task: &TaskView) -> Value {
    let mut value = serde_json::to_value(task).unwrap_or(Value::Null);
    let open = task.status.is_open();
    if let Some(object) = value.as_object_mut() {
        let offered = config
            .definitions
            .get(&task.definition)
            .filter(|_| open)
            .map(|c| events(&c.chart, &task.state))
            .unwrap_or_default();
        object.insert("events".into(), json!(offered));
    }
    value
}

fn instance_json(outcome: Outcome) -> Response {
    match outcome {
        Outcome::Applied(record) => Json(json!({
            "id": record.id,
            "definition": record.definition,
            "context": record.context,
            "configuration": record.state.configuration,
            "running": record.state.running,
            "version": record.version,
        }))
        .into_response(),
        Outcome::Replayed => Json(json!({ "replayed": true })).into_response(),
        Outcome::Ignored => Json(json!({ "ignored": true })).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartBody {
    definition: String,
    context: String,
    op_id: String,
}

async fn start(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<StartBody>,
) -> Response {
    let (subject, service, _) = staff_or_return!(server, subject, WORK_PERMISSION);
    match service
        .start(
            &body.definition,
            &body.context,
            &subject.account,
            &body.op_id,
        )
        .await
    {
        Ok(Outcome::Applied(record)) => {
            (StatusCode::CREATED, instance_json(Outcome::Applied(record))).into_response()
        }
        Ok(other) => instance_json(other),
        Err(error) => failed(&error),
    }
}

async fn tasks(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
) -> Response {
    let (_, service, config) = staff_or_return!(server, subject, WORK_PERMISSION);
    match service.tasks(&id).await {
        Ok(tasks) => Json(
            tasks
                .iter()
                .map(|t| task_json(&config, t))
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => failed(&error),
    }
}

async fn worklist(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(key): Path<String>,
) -> Response {
    let (_, service, config) = staff_or_return!(server, subject, WORK_PERMISSION);
    match service.worklist(&key).await {
        Ok(tasks) => Json(
            tasks
                .iter()
                .map(|t| task_json(&config, t))
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => failed(&error),
    }
}

async fn definitions(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
) -> Response {
    let (_, _, config) = staff_or_return!(server, subject, WORK_PERMISSION);
    let listed: Vec<Value> = config
        .definitions
        .iter()
        .map(|(key, compiled)| {
            let tasks: Vec<Value> = human_tasks(&compiled.chart)
                .into_iter()
                .map(|(state, task)| {
                    json!({ "state": state, "role": task.role, "worklist": task.worklist })
                })
                .collect();
            json!({ "definition": key, "tasks": tasks })
        })
        .collect();
    Json(listed).into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventBody {
    event: String,
    op_id: String,
}

async fn event(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<EventBody>,
) -> Response {
    let (subject, service, _) = staff_or_return!(server, subject, WORK_PERMISSION);
    match service
        .command(
            &id,
            &Event::named(&body.event),
            &subject.account,
            &body.op_id,
        )
        .await
    {
        Ok(outcome) => instance_json(outcome),
        Err(error) => failed(&error),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CancelBody {
    reason: String,
    op_id: String,
}

async fn cancel(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<CancelBody>,
) -> Response {
    let (subject, service, _) = staff_or_return!(server, subject, SUPERVISE_PERMISSION);
    match service
        .cancel(&id, &body.reason, &subject.account, &body.op_id)
        .await
    {
        Ok(outcome) => instance_json(outcome),
        Err(error) => failed(&error),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskBody {
    op_id: String,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    priority: Option<String>,
}

async fn task_command(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path((id, state, command)): Path<(String, String, String)>,
    Json(body): Json<TaskBody>,
) -> Response {
    let permission = match command.as_str() {
        "claim" | "release" => WORK_PERMISSION,
        "reassign" | "escalate" => SUPERVISE_PERMISSION,
        _ => return refused(StatusCode::NOT_FOUND, "no such task command"),
    };
    let (subject, service, _) = staff_or_return!(server, subject, permission);
    let who = subject.account.as_str();
    let op = body.op_id.as_str();
    let outcome = match command.as_str() {
        "claim" => service.claim(&id, &state, who, op).await,
        "release" => service.release(&id, &state, who, op).await,
        "reassign" => {
            let Some(to) = body.to.as_deref().filter(|t| !t.is_empty()) else {
                return refused(StatusCode::BAD_REQUEST, "reassign needs `to`");
            };
            service.reassign(&id, &state, to, who, op).await
        }
        _ => {
            let Some(to) = body.priority.as_deref().and_then(priority) else {
                return refused(
                    StatusCode::BAD_REQUEST,
                    "escalate needs a `priority`: routine, urgent, asap or stat",
                );
            };
            service.escalate(&id, &state, to, who, op).await
        }
    };
    match outcome {
        Ok(outcome) => instance_json(outcome),
        Err(error) => failed(&error),
    }
}

impl FhirServer {
    /// This server, running `config`'s workflows ([`Self::workflow_router`]).
    #[must_use]
    pub fn with_workflows(mut self, config: WorkflowConfig) -> Self {
        self.workflows = Some(Arc::new(config));
        self
    }

    /// The workflow, task and worklist routes, to mount behind `subject_layer`.
    pub fn workflow_router(&self) -> Router {
        Router::new()
            .route("/instances", post(start))
            .route("/instances/{id}/tasks", get(tasks))
            .route("/instances/{id}/events", post(event))
            .route("/instances/{id}/cancel", post(cancel))
            .route(
                "/instances/{id}/tasks/{state}/{command}",
                post(task_command),
            )
            .route("/worklists/{key}", get(worklist))
            .route("/definitions", get(definitions))
            .with_state(self.clone())
    }
}

/// Starts the Workflow Definition `canonical` (`url|version`, or `url` for the newest version
/// the node runs) about `context`, for another module's use case such as a program enrolment
/// (#116). `None` when the node does not run that definition; replaying `op_id` changes nothing.
pub(crate) async fn start_for(
    server: &FhirServer,
    canonical: &str,
    context: &str,
    actor: &str,
    op_id: &str,
) -> Result<Option<String>, String> {
    let Some(config) = server.workflows.clone() else {
        return Ok(None);
    };
    let key = if config.definitions.contains_key(canonical) {
        Some(canonical.to_owned())
    } else {
        let prefix = format!("{canonical}|");
        config
            .definitions
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .max_by_key(|k| {
                k[prefix.len()..]
                    .split('.')
                    .map(|p| p.parse::<u64>().unwrap_or(0))
                    .collect::<Vec<_>>()
            })
            .cloned()
    };
    let Some(key) = key else {
        return Ok(None);
    };
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let service = Workflows::new(
        server.store.clone(),
        config.definitions.clone(),
        NoActions,
        NoGuards,
        clock,
        Vec::new(),
        &config.authority,
    );
    service
        .start(&key, context, actor, op_id)
        .await
        .map(|_| Some(key))
        .map_err(|e| e.to_string())
}

/// Delivers another module's event `name` to the instance started by `start_op_id`, once per
/// `event_id` (#109). `false` when the node runs no workflows or no such instance exists.
pub(crate) async fn deliver_for(
    server: &FhirServer,
    start_op_id: &str,
    name: &str,
    event_id: &str,
) -> Result<bool, String> {
    let Some(config) = server.workflows.clone() else {
        return Ok(false);
    };
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let service = Workflows::new(
        server.store.clone(),
        config.definitions.clone(),
        NoActions,
        NoGuards,
        clock,
        Vec::new(),
        &config.authority,
    );
    match service
        .deliver(
            event_id,
            &health_workflow_app::instance_id(start_op_id),
            &Event::named(name),
        )
        .await
    {
        Ok(_) => Ok(true),
        Err(health_workflow_app::WorkflowError::NotFound(_)) => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}
