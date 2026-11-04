//! Worklists, the Read Model the catalogue names `health-worklists` (workflow-engine.md § Worklists and queues; ADRs 0002, 0044): Task projections and
//! the worklists over them.
//!
//! - **Projections rebuild from instance state.** [`project`] derives every Task of an instance
//!   from its definition, its record and its history alone: one Task per entry into a human-task
//!   state. Nothing else is stored, so a projection can always be thrown away and rebuilt.
//! - **Status** uses FHIR R5's 12 Task codes ([`TaskStatus`]): an open task is `ready`, or
//!   `in-progress` while claimed; a task whose state was left is `completed`, or `cancelled` if
//!   the instance was cancelled. `businessStatus` is the engine state's id. Workflow state is never
//!   written into a clinical resource's status.
//! - **Claims and escalation** are workflow commands that set reserved data-model fields
//!   ([`owner_field`], [`claimed_field`], [`priority_field`]). A claim lapses after the task's
//!   claim timeout without activity (15 minutes by default), and the task is back on the
//!   worklist for anyone.
//! - **Worklists** ([`worklist`]) hold a worklist's open tasks, most urgent first, then by
//!   arrival.

use std::collections::BTreeMap;

use crate::model::{HumanTask, Priority, State, Statechart, Value};
use crate::store::{CANCELLED, HistoryEntry, InstanceRecord};
use health_time::{SignedDuration, Timestamp};
use uuid::Uuid;

/// The namespace of Task ids.
const NAMESPACE: Uuid = Uuid::from_u128(0x3c9e_51a2_7d04_4b8f_a6e1_92c0_d47b_15e3);

/// FHIR R5 Task status: all 12 codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskStatus {
    /// Not yet actionable.
    Draft,
    /// Ready for the performer to decide whether to act.
    Requested,
    /// Received by the performer.
    Received,
    /// Accepted by the performer.
    Accepted,
    /// Rejected by the performer.
    Rejected,
    /// Ready to be acted on.
    Ready,
    /// Cancelled before completion.
    Cancelled,
    /// Being acted on.
    InProgress,
    /// Paused.
    OnHold,
    /// Attempted and failed.
    Failed,
    /// Done.
    Completed,
    /// Should never have existed.
    EnteredInError,
}

impl TaskStatus {
    /// The FHIR code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Requested => "requested",
            Self::Received => "received",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Ready => "ready",
            Self::Cancelled => "cancelled",
            Self::InProgress => "in-progress",
            Self::OnHold => "on-hold",
            Self::Failed => "failed",
            Self::Completed => "completed",
            Self::EnteredInError => "entered-in-error",
        }
    }

    /// Whether the task is still to be done.
    pub fn is_open(self) -> bool {
        matches!(self, Self::Ready | Self::InProgress)
    }
}

/// A Task projected from a workflow instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskView {
    /// The Task id: the UUIDv5 of instance, state and the history `seq` that entered it.
    pub id: String,
    /// The workflow instance.
    pub instance: String,
    /// The instance's definition, `url|version`.
    pub definition: String,
    /// What the instance is about, e.g. `Encounter/123` (`Task.focus`).
    pub context: String,
    /// The engine state (`Task.businessStatus`).
    pub state: String,
    /// The Task status.
    pub status: TaskStatus,
    /// Who does it: a role or performer type (`Task.requestedPerformer`).
    pub role: String,
    /// The form to fill, a Questionnaire `url|version` (`Task.input`).
    pub form: Option<String>,
    /// The worklist it is on.
    pub worklist: String,
    /// How urgent it is, after any escalation.
    pub priority: Priority,
    /// Who has claimed it, while the claim lasts (`Task.owner`).
    pub owner: Option<String>,
    /// When its state was entered (`Task.authoredOn`): its place in the worklist.
    pub authored_on: Timestamp,
    /// When it last changed (`Task.lastModified`).
    pub last_modified: Timestamp,
}

/// The data-model field holding who claimed the task of `state`.
pub fn owner_field(state: &str) -> String {
    format!("task.{state}.owner")
}

/// The data-model field holding when the task of `state` was claimed or last worked on, in Unix
/// seconds.
pub fn claimed_field(state: &str) -> String {
    format!("task.{state}.claimed")
}

/// The data-model field holding the escalated priority of the task of `state`.
pub fn priority_field(state: &str) -> String {
    format!("task.{state}.priority")
}

/// Whether `field` is one of the reserved task fields, which only task commands set.
pub fn is_task_field(field: &str) -> bool {
    field.starts_with("task.")
}

/// The priority with FHIR code `code`.
pub fn priority(code: &str) -> Option<Priority> {
    [
        Priority::Routine,
        Priority::Urgent,
        Priority::Asap,
        Priority::Stat,
    ]
    .into_iter()
    .find(|p| p.code() == code)
}

/// The id of the Task for the entry into `state` at history `seq` of `instance`.
pub fn task_id(instance: &str, state: &str, seq: u64) -> String {
    Uuid::new_v5(
        &NAMESPACE,
        format!("task|{instance}|{state}|{seq}").as_bytes(),
    )
    .to_string()
}

fn tasks_of<'a>(states: &'a [State], into: &mut BTreeMap<&'a str, &'a HumanTask>) {
    for state in states {
        if let Some(task) = &state.task {
            into.insert(state.id.as_str(), task);
        }
        tasks_of(&state.children, into);
    }
}

/// The human tasks of a definition, by state.
pub fn human_tasks(chart: &Statechart) -> BTreeMap<&str, &HumanTask> {
    let mut found = BTreeMap::new();
    tasks_of(&chart.states, &mut found);
    found
}

/// The live claim on the task of `state`, if any: its owner and when it was last worked on.
pub fn claim(
    record: &InstanceRecord,
    state: &str,
    timeout: SignedDuration,
    now: Timestamp,
) -> Option<(String, Timestamp)> {
    let data = &record.state.data;
    let Some(Value::Text(owner)) = data.get(&owner_field(state)) else {
        return None;
    };
    let Some(Value::Int(seconds)) = data.get(&claimed_field(state)) else {
        return None;
    };
    let at = Timestamp::from_second(*seconds).ok()?;
    let lapses = at.checked_add(timeout).ok()?;
    (now < lapses).then(|| (owner.clone(), at))
}

struct Open {
    seq: u64,
    at: Timestamp,
}

/// Every Task of `record`, rebuilt from its definition's `chart`, the record and its `history`
/// (in `seq` order), as at `now`: closed ones first, in the order they closed, then open ones.
pub fn project(
    chart: &Statechart,
    record: &InstanceRecord,
    history: &[HistoryEntry],
    now: Timestamp,
) -> Vec<TaskView> {
    let tasks = human_tasks(chart);
    let view = |state: &str, task: &HumanTask, open: &Open| TaskView {
        id: task_id(&record.id, state, open.seq),
        instance: record.id.clone(),
        definition: record.definition.clone(),
        context: record.context.clone(),
        state: state.to_owned(),
        status: TaskStatus::Ready,
        role: task.role.clone(),
        form: task.form.clone(),
        worklist: task.worklist.clone(),
        priority: task.priority,
        owner: None,
        authored_on: open.at,
        last_modified: open.at,
    };

    let mut projected = Vec::new();
    let mut active: BTreeMap<&str, Open> = BTreeMap::new();
    for entry in history {
        let left: Vec<&str> = active
            .keys()
            .copied()
            .filter(|state| !entry.to.iter().any(|s| s == state))
            .collect();
        for state in left {
            if let (Some(open), Some(task)) = (active.remove(state), tasks.get(state)) {
                let status = if entry.event.name == CANCELLED {
                    TaskStatus::Cancelled
                } else {
                    TaskStatus::Completed
                };
                projected.push(TaskView {
                    status,
                    last_modified: entry.at,
                    ..view(state, task, &open)
                });
            }
        }
        for state in &entry.to {
            if tasks.contains_key(state.as_str()) && !active.contains_key(state.as_str()) {
                active.insert(
                    state.as_str(),
                    Open {
                        seq: entry.seq,
                        at: entry.at,
                    },
                );
            }
        }
    }

    for (state, open) in &active {
        let Some(task) = tasks.get(state) else {
            continue;
        };
        let mut task_view = view(state, task, open);
        if let Some(Value::Text(code)) = record.state.data.get(&priority_field(state)) {
            task_view.priority = priority(code).unwrap_or(task.priority).max(task.priority);
        }
        if let Some((owner, at)) = claim(record, state, task.claim_timeout, now) {
            task_view.status = TaskStatus::InProgress;
            task_view.owner = Some(owner);
            task_view.last_modified = task_view.last_modified.max(at);
        }
        projected.push(task_view);
    }
    projected
}

/// The open tasks on `worklist`, most urgent first, then by arrival.
pub fn worklist<'a>(
    tasks: impl IntoIterator<Item = &'a TaskView>,
    worklist: &str,
) -> Vec<&'a TaskView> {
    let mut found: Vec<&TaskView> = tasks
        .into_iter()
        .filter(|t| t.worklist == worklist && t.status.is_open())
        .collect();
    found.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then(a.authored_on.cmp(&b.authored_on))
            .then(a.id.cmp(&b.id))
    });
    found
}

#[cfg(test)]
mod tests;
