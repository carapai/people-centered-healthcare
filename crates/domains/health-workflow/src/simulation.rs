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
//! Financial Gates are stubbed too (`gates`: URL → `pass`, `defer` or `block`); an unstubbed gate
//! defers, as an unavailable one does in the engine.
//! Guards are stubbed by the scenario (`guards`: expression → outcome), because guards are
//! evaluated against clinical data the sandbox does not have; an unstubbed guard fails the
//! scenario rather than guessing.
//!
//! The Workflow Studio's sandbox (#499; studio.md § The Workflow Studio) plays a draft on the same
//! run with [`play`]: the author's guard answers and gate outcomes, then the moves so far (an
//! event, or a wait), each answered with a [`Snapshot`] of what holds after it. A move that would
//! evaluate a guard the author has not answered is not taken: the guards are reported instead.
//! [`save`] turns the run into a [`Scenario`] whose every step expects what its snapshot showed,
//! so it passes in submission's simulation unchanged.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use health_time::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};

use crate::definition::WorkflowDefinition;
use crate::interpreter::{Effect, Event, Guards, Instance, Machine};
use crate::model::{DataModel, State, Statechart};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<Vec<String>>,
    /// The typed actions the step ran, in order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<String>>,
    /// The events of the pending timers, in any order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timers: Option<Vec<String>>,
    /// The open Tasks, in any order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<ExpectedTask>>,
    /// Whether the instance is still running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running: Option<bool>,
}

/// One step of a scenario.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// The event to send.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// How long to wait, firing the timers that fall due.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub guards: BTreeMap<String, bool>,
    /// The outcome of each Financial Gate, by canonical URL: `pass`, `defer` or `block`. An
    /// unstubbed gate defers, as an unavailable one does.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gates: BTreeMap<String, String>,
    /// The steps.
    pub steps: Vec<Step>,
}

/// Guards answered by the scenario; an unstubbed one is false and noted.
struct Stubbed<'a> {
    outcomes: &'a BTreeMap<String, bool>,
    missing: RefCell<BTreeSet<String>>,
    /// Every stubbed guard evaluated.
    asked: RefCell<BTreeMap<String, bool>>,
}

impl<'a> Stubbed<'a> {
    fn new(outcomes: &'a BTreeMap<String, bool>) -> Self {
        Self {
            outcomes,
            missing: RefCell::new(BTreeSet::new()),
            asked: RefCell::new(BTreeMap::new()),
        }
    }
}

impl Guards for Stubbed<'_> {
    fn holds(&self, guard: &str, _: Option<&Event>, _: &DataModel) -> bool {
        match self.outcomes.get(guard) {
            Some(&outcome) => {
                self.asked.borrow_mut().insert(guard.to_owned(), outcome);
                outcome
            }
            None => {
                self.missing.borrow_mut().insert(guard.to_owned());
                false
            }
        }
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
    gate_outcomes: &'a BTreeMap<String, String>,
    /// The stubbed gates entered, with their outcomes.
    gates_asked: BTreeMap<String, String>,
}

impl<'a> Run<'a> {
    /// Starts an instance of `machine` and answers the gates it entered.
    fn start(
        machine: &'a Machine,
        definition: &'a WorkflowDefinition,
        guards: Stubbed<'a>,
        canonical: &str,
        gate_outcomes: &'a BTreeMap<String, String>,
    ) -> (Self, Result<(), String>) {
        let (state, effects) = machine.start(&guards);
        let mut run = Run {
            machine,
            definition,
            guards,
            record: InstanceRecord {
                id: "simulation".into(),
                definition: canonical.to_owned(),
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
            gate_outcomes,
            gates_asked: BTreeMap::new(),
        };
        run.apply(Event::named(STARTED), state, &effects);
        let started = run.gates(&effects);
        (run, started)
    }

    /// Takes step `index`: sends `event` or waits `by`. A step's actions are those it ran; the
    /// first step, if it sends nothing, checks the start.
    fn take(
        &mut self,
        index: usize,
        event: Option<&str>,
        wait: Option<SignedDuration>,
    ) -> Result<(), String> {
        if event.is_some() || wait.is_some() {
            self.ran.clear();
        }
        match (event, wait) {
            (Some(event), None) => self.send(event),
            (None, Some(by)) => self.wait(by),
            (None, None) if index == 0 => Ok(()),
            _ => Err("a step sends one event or waits, not both or neither".into()),
        }
    }

    /// The open Tasks, by state and status.
    fn tasks(&self) -> Vec<ExpectedTask> {
        project(
            &self.definition.statechart,
            &self.record,
            &self.history,
            self.now,
        )
        .into_iter()
        .filter(|t| t.status.is_open())
        .map(|t| ExpectedTask {
            state: t.state.clone(),
            status: t.status.code().to_owned(),
        })
        .collect()
    }

    /// What holds now.
    fn snapshot(&self) -> Snapshot {
        let mut timers: Vec<&Pending> = self.timers.values().collect();
        timers.sort();
        Snapshot {
            configuration: self.record.state.configuration.clone(),
            actions: self.ran.clone(),
            timers: timers
                .into_iter()
                .map(|t| PendingTimer {
                    event: t.event.clone(),
                    due_in: self.now.duration_until(t.at),
                })
                .collect(),
            tasks: self.tasks(),
            running: self.record.state.running,
            accepts: accepts(
                &self.definition.statechart,
                &self.record.state.configuration,
            ),
            elapsed: Timestamp::UNIX_EPOCH.duration_until(self.now),
        }
    }

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
                Effect::StartInvoke { .. }
                | Effect::CancelInvoke { .. }
                | Effect::EvaluateGate { .. }
                | Effect::GateEvaluated { .. }
                | Effect::Completed => {}
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
        self.gates(&effects)
    }

    /// Answers the gates `effects` entered with the scenario's stubbed outcomes (an unstubbed
    /// gate defers, as an unavailable one does), raising `gate.<outcome>`.
    fn gates(&mut self, effects: &[Effect]) -> Result<(), String> {
        for effect in effects {
            if let Effect::EvaluateGate { gate, .. } = effect {
                let outcome = match self.gate_outcomes.get(gate) {
                    Some(outcome) => {
                        self.gates_asked.insert(gate.clone(), outcome.clone());
                        outcome.clone()
                    }
                    None => "defer".to_owned(),
                };
                self.send(&format!("gate.{outcome}"))?;
            }
        }
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
            let open: Vec<String> = self
                .tasks()
                .into_iter()
                .map(|t| format!("{} {}", t.state, t.status))
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
    let guards = Stubbed::new(&scenario.guards);
    let (mut run, started) = Run::start(
        &machine,
        definition,
        guards,
        &scenario.definition,
        &scenario.gates,
    );
    let mut failed = Vec::new();
    if let Err(problem) = started {
        failed.push(format!("start: {problem}"));
    }
    for (index, step) in scenario.steps.iter().enumerate() {
        let number = index + 1;
        let moved = run.take(index, step.event.as_deref(), step.wait);
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

/// A pending timer, as the sandbox shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingTimer {
    /// The event it raises.
    pub event: String,
    /// How long until it falls due.
    pub due_in: SignedDuration,
}

/// What holds after a sandbox move (or at the start).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// The active states, in document order.
    pub configuration: Vec<String>,
    /// The typed actions the move ran (as `<id>@<major>`, in order), stubbed: recorded, never
    /// carried out.
    pub actions: Vec<String>,
    /// The pending timers, earliest first.
    pub timers: Vec<PendingTimer>,
    /// The open Tasks.
    pub tasks: Vec<ExpectedTask>,
    /// Whether the instance is still running.
    pub running: bool,
    /// The events a transition of an active state is taken on, sorted.
    pub accepts: Vec<String>,
    /// How long since the start.
    pub elapsed: SignedDuration,
}

/// One sandbox move: an event to send, or a wait.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Move {
    /// The event to send.
    #[serde(default)]
    pub event: Option<String>,
    /// How long to wait, firing the timers that fall due.
    #[serde(default)]
    pub wait: Option<SignedDuration>,
}

/// A sandbox run: the author's answers and the moves so far.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Play {
    /// The author's answer to each guard.
    #[serde(default)]
    pub guards: BTreeMap<String, bool>,
    /// The outcome of each Financial Gate, by canonical URL; an unanswered gate defers.
    #[serde(default)]
    pub gates: BTreeMap<String, String>,
    /// The moves, in order.
    #[serde(default)]
    pub moves: Vec<Move>,
}

/// How a sandbox run went.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Played {
    /// What held at the start, then after each move taken. A move is taken only when every
    /// move before it was.
    pub snapshots: Vec<Snapshot>,
    /// The guards the next move (the start, if there is no snapshot) would evaluate without an
    /// answer: it was not taken.
    pub unanswered: Vec<String>,
    /// Why the next move could not be taken, if it could not for another reason.
    pub refused: Option<String>,
    /// The answered guards the run evaluated.
    pub guards: BTreeMap<String, bool>,
    /// The answered gates the run entered.
    pub gates: BTreeMap<String, String>,
}

/// The events a transition of a state in `configuration` is taken on, sorted.
pub fn accepts(chart: &Statechart, configuration: &[String]) -> Vec<String> {
    fn walk(states: &[State], active: &BTreeSet<&str>, found: &mut BTreeSet<String>) {
        for state in states {
            if active.contains(state.id.as_str()) {
                for transition in &state.transitions {
                    found.extend(transition.events.iter().cloned());
                }
            }
            walk(&state.children, active, found);
        }
    }
    let active = configuration.iter().map(String::as_str).collect();
    let mut found = BTreeSet::new();
    walk(&chart.states, &active, &mut found);
    found.into_iter().collect()
}

/// Every guard of `chart`'s transitions, in document order, each once.
pub fn guards(chart: &Statechart) -> Vec<String> {
    fn walk(states: &[State], found: &mut Vec<String>) {
        for state in states {
            for guard in state.transitions.iter().filter_map(|t| t.guard.as_ref()) {
                if !guard.is_empty() && !found.contains(guard) {
                    found.push(guard.clone());
                }
            }
            walk(&state.children, found);
        }
    }
    let mut found = Vec::new();
    walk(&chart.states, &mut found);
    found
}

/// Every Financial Gate of `chart`'s states, by canonical URL, in document order, each once.
pub fn gates(chart: &Statechart) -> Vec<String> {
    fn walk(states: &[State], found: &mut Vec<String>) {
        for state in states {
            if let Some(gate) = &state.gate
                && !found.contains(gate)
            {
                found.push(gate.clone());
            }
            walk(&state.children, found);
        }
    }
    let mut found = Vec::new();
    walk(&chart.states, &mut found);
    found
}

/// Whether the run stops at the move just tried: it needed an unanswered guard, or it failed.
fn halted(run: &Run<'_>, moved: Result<(), String>, played: &mut Played) -> bool {
    let missing = run.guards.missing.borrow();
    if !missing.is_empty() {
        played.unanswered = missing.iter().cloned().collect();
        return true;
    }
    if let Err(problem) = moved {
        played.refused = Some(problem);
        return true;
    }
    false
}

/// Plays `play` on `definition` with every action stubbed (the Workflow Studio's sandbox, #499):
/// the start, then each move in turn until one cannot be taken. A move that would evaluate a
/// guard without an answer is not taken, and the guards are reported, never guessed. `Err` when
/// the statechart is invalid.
pub fn play(definition: &WorkflowDefinition, play: &Play) -> Result<Played, String> {
    let machine = Machine::new(&definition.statechart).map_err(|e| e.to_string())?;
    let canonical = format!("{}|{}", definition.url, definition.version);
    let (mut run, started) = Run::start(
        &machine,
        definition,
        Stubbed::new(&play.guards),
        &canonical,
        &play.gates,
    );
    let mut played = Played::default();
    if !halted(&run, started, &mut played) {
        played.snapshots.push(run.snapshot());
        for step in &play.moves {
            // A sandbox move always sends or waits: the start is taken on its own.
            let moved = run.take(usize::MAX, step.event.as_deref(), step.wait);
            if halted(&run, moved, &mut played) {
                break;
            }
            played.snapshots.push(run.snapshot());
        }
    }
    played.guards = run.guards.asked.borrow().clone();
    played.gates = run.gates_asked.clone();
    Ok(played)
}

/// The run as a `tests/` scenario named `name` (*Save as scenario*): the answers it used, the
/// start, then each move taken, each step expecting what its snapshot showed.
pub fn save(definition: &WorkflowDefinition, play: &Play, played: &Played, name: &str) -> Scenario {
    let expect = |s: &Snapshot| Expect {
        configuration: Some(s.configuration.clone()),
        actions: Some(s.actions.clone()),
        timers: Some(s.timers.iter().map(|t| t.event.clone()).collect()),
        tasks: Some(s.tasks.clone()),
        running: Some(s.running),
    };
    let steps = played
        .snapshots
        .iter()
        .enumerate()
        .map(|(index, snapshot)| {
            let taken = index.checked_sub(1).and_then(|m| play.moves.get(m));
            Step {
                event: taken.and_then(|m| m.event.clone()),
                wait: taken.and_then(|m| m.wait),
                expect: expect(snapshot),
            }
        })
        .collect();
    Scenario {
        kind: SCENARIO_TYPE.to_owned(),
        name: name.to_owned(),
        definition: format!("{}|{}", definition.url, definition.version),
        guards: played.guards.clone(),
        gates: played.gates.clone(),
        steps,
    }
}

#[cfg(test)]
mod tests;
