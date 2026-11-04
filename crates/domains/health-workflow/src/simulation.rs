//! Workflow simulation (#101; workflow-engine.md § Simulation and publication gates): a
//! package's `tests/` scenarios run against its definitions on the same interpreter, with
//! **stubbed actions**. Every scenario must pass before the package may be published.
//!
//! A [`Scenario`] starts an instance of a definition, then takes its steps in order. Each step
//! sends an event, or waits a while (firing the timers that fall due, earliest first), and then
//! checks what it expects:
//! - the active configuration;
//! - the typed actions the step ran (as `<id>@<major>`, in order). Actions are recorded, never
//!   carried out;
//! - the events of the timers still pending;
//! - the open Tasks, by state and status;
//! - whether the instance is still running.
//!
//! The first step may send nothing and wait for nothing: it checks the instance as started.
//! Guards are stubbed by the scenario (`guards`: expression → outcome), because guards are
//! evaluated against clinical data the sandbox does not have; an unstubbed guard fails the
//! scenario rather than guessing.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use health_time::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};

use crate::definition::WorkflowDefinition;
use crate::interpreter::{Effect, Event, Guards, Instance, Machine};
use crate::model::DataModel;
use crate::store::{HistoryEntry, InstanceRecord, STARTED};
use crate::worklists::project;

/// The `type` of a scenario file in a package's `tests/` folder.
pub const SCENARIO_TYPE: &str = "workflow-scenario";

/// A Task a step expects to be open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedTask {
    /// Its state.
    pub state: String,
    /// Its status code: `ready` or `in-progress`.
    pub status: String,
}

/// What a step expects. Anything left out is not checked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Expect {
    /// The active states, in any order.
    #[serde(default)]
    pub configuration: Option<Vec<String>>,
    /// The typed actions the step ran, in order.
    #[serde(default)]
    pub actions: Option<Vec<String>>,
    /// The events of the pending timers, in any order.
    #[serde(default)]
    pub timers: Option<Vec<String>>,
    /// The open Tasks, in any order.
    #[serde(default)]
    pub tasks: Option<Vec<ExpectedTask>>,
    /// Whether the instance is still running.
    #[serde(default)]
    pub running: Option<bool>,
}

/// One step of a scenario.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// The event to send.
    #[serde(default)]
    pub event: Option<String>,
    /// How long to wait, firing the timers that fall due.
    #[serde(default)]
    pub wait: Option<SignedDuration>,
    /// What must then hold.
    #[serde(default)]
    pub expect: Expect,
}

/// A simulation case, as a package's `tests/` file holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scenario {
    /// Always [`SCENARIO_TYPE`].
    #[serde(rename = "type")]
    pub kind: String,
    /// What it shows.
    pub name: String,
    /// The definition it runs, `url|version`.
    pub definition: String,
    /// The outcome of each guard the definition evaluates.
    #[serde(default)]
    pub guards: BTreeMap<String, bool>,
    /// The steps.
    pub steps: Vec<Step>,
}

/// Guards answered by the scenario; an unstubbed one is false and noted.
struct Stubbed<'a> {
    outcomes: &'a BTreeMap<String, bool>,
    missing: RefCell<BTreeSet<String>>,
}

impl Guards for Stubbed<'_> {
    fn holds(&self, guard: &str, _: Option<&Event>, _: &DataModel) -> bool {
        self.outcomes.get(guard).copied().unwrap_or_else(|| {
            self.missing.borrow_mut().insert(guard.to_owned());
            false
        })
    }
}

/// A pending timer: when it fires, and its event.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Pending {
    at: Timestamp,
    id: String,
    event: String,
}

struct Run<'a> {
    machine: &'a Machine,
    definition: &'a WorkflowDefinition,
    guards: Stubbed<'a>,
    record: InstanceRecord,
    history: Vec<HistoryEntry>,
    timers: BTreeMap<String, Pending>,
    now: Timestamp,
    ran: Vec<String>,
}

impl Run<'_> {
    fn apply(&mut self, event: Event, state: Instance, effects: &[Effect]) {
        for effect in effects {
            match effect {
                Effect::Execute { action, .. } => self.ran.push(action.clone()),
                Effect::StartTimer { id, after, event } => {
                    if let Ok(at) = self.now.checked_add(*after) {
                        self.timers.insert(
                            id.clone(),
                            Pending {
                                at,
                                id: id.clone(),
                                event: event.clone(),
                            },
                        );
                    }
                }
                Effect::CancelTimer { id } => {
                    self.timers.remove(id);
                }
                Effect::StartInvoke { .. } | Effect::CancelInvoke { .. } | Effect::Completed => {}
            }
        }
        let from = std::mem::replace(&mut self.record.state, state);
        self.record.version += 1;
        self.history.push(HistoryEntry {
            instance: self.record.id.clone(),
            seq: self.record.version,
            event,
            from: from.configuration,
            to: self.record.state.configuration.clone(),
            effects: effects.to_vec(),
            actor: "simulation".into(),
            at: self.now,
            op_id: format!("simulation-{}", self.record.version),
        });
    }

    fn send(&mut self, name: &str) -> Result<(), String> {
        let event = Event::named(name);
        let (state, effects) = self
            .machine
            .step(self.record.state.clone(), &event, &self.guards)
            .map_err(|e| e.to_string())?;
        self.apply(event, state, &effects);
        Ok(())
    }

    fn wait(&mut self, by: SignedDuration) -> Result<(), String> {
        let until = self.now.checked_add(by).map_err(|e| e.to_string())?;
        loop {
            let next = self
                .timers
                .values()
                .filter(|t| t.at <= until)
                .min()
                .cloned();
            let Some(timer) = next else {
                break;
            };
            self.timers.remove(&timer.id);
            self.now = self.now.max(timer.at);
            self.send(&timer.event)?;
        }
        self.now = until;
        Ok(())
    }

    fn check(&self, number: usize, expect: &Expect) -> Vec<String> {
        let mut failed = Vec::new();
        let mut differs = |what: &str, expected: String, actual: String| {
            if expected != actual {
                failed.push(format!(
                    "step {number}: expected {what} {expected}, got {actual}"
                ));
            }
        };
        let sorted = |mut list: Vec<String>| {
            list.sort();
            format!("{list:?}")
        };
        if let Some(configuration) = &expect.configuration {
            differs(
                "configuration",
                sorted(configuration.clone()),
                sorted(self.record.state.configuration.clone()),
            );
        }
        if let Some(actions) = &expect.actions {
            differs("actions", format!("{actions:?}"), format!("{:?}", self.ran));
        }
        if let Some(timers) = &expect.timers {
            differs(
                "timers",
                sorted(timers.clone()),
                sorted(self.timers.values().map(|t| t.event.clone()).collect()),
            );
        }
        if let Some(tasks) = &expect.tasks {
            let open: Vec<String> = project(
                &self.definition.statechart,
                &self.record,
                &self.history,
                self.now,
            )
            .into_iter()
            .filter(|t| t.status.is_open())
            .map(|t| format!("{} {}", t.state, t.status.code()))
            .collect();
            differs(
                "tasks",
                sorted(
                    tasks
                        .iter()
                        .map(|t| format!("{} {}", t.state, t.status))
                        .collect(),
                ),
                sorted(open),
            );
        }
        if let Some(running) = expect.running {
            differs(
                "running",
                running.to_string(),
                self.record.state.running.to_string(),
            );
        }
        failed
    }
}

/// Runs `scenario` against `definition`: every way it fails, each prefixed with the scenario's
/// name; none if it passes.
pub fn simulate(definition: &WorkflowDefinition, scenario: &Scenario) -> Vec<String> {
    let fail = |problem: String| vec![format!("{}: {problem}", scenario.name)];
    if scenario.kind != SCENARIO_TYPE {
        return fail(format!("its type is not {SCENARIO_TYPE}"));
    }
    let machine = match Machine::new(&definition.statechart) {
        Ok(machine) => machine,
        Err(error) => return fail(error.to_string()),
    };
    let guards = Stubbed {
        outcomes: &scenario.guards,
        missing: RefCell::new(BTreeSet::new()),
    };
    let (state, effects) = machine.start(&guards);
    let mut run = Run {
        machine: &machine,
        definition,
        guards,
        record: InstanceRecord {
            id: "simulation".into(),
            definition: scenario.definition.clone(),
            context: "simulation".into(),
            authority: "simulation".into(),
            state: Instance {
                configuration: Vec::new(),
                data: DataModel::new(),
                sequence: 0,
                invoked: Vec::new(),
                running: true,
            },
            version: 0,
        },
        history: Vec::new(),
        timers: BTreeMap::new(),
        now: Timestamp::UNIX_EPOCH,
        ran: Vec::new(),
    };
    run.apply(Event::named(STARTED), state, &effects);

    let mut failed = Vec::new();
    for (index, step) in scenario.steps.iter().enumerate() {
        let number = index + 1;
        // A step's actions are those it ran; the first step, if it sends nothing, checks the start.
        if step.event.is_some() || step.wait.is_some() {
            run.ran.clear();
        }
        let moved = match (&step.event, step.wait) {
            (Some(event), None) => run.send(event),
            (None, Some(by)) => run.wait(by),
            (None, None) if index == 0 => Ok(()),
            _ => Err("a step sends one event or waits, not both or neither".into()),
        };
        if let Err(problem) = moved {
            failed.push(format!("step {number}: {problem}"));
            break;
        }
        failed.extend(run.check(number, &step.expect));
    }
    let missing = run.guards.missing.borrow();
    if !missing.is_empty() {
        failed.push(format!(
            "guards not stubbed: {}",
            missing.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    failed
        .into_iter()
        .map(|problem| format!("{}: {problem}", scenario.name))
        .collect()
}

#[cfg(test)]
mod tests;
