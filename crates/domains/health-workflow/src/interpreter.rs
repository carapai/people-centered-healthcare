//! The interpreter: SCXML's algorithm for run-to-completion (W3C SCXML 1.0, Appendix D), without
//! history states, over a compiled statechart.
//!
//! **Pure and deterministic.** [`Machine::start`] and [`Machine::step`] take an instance and an
//! event and return what to do: the effects are typed actions, timers and invocations, which the
//! workflow application carries out in the same transaction (ADR 0044). Guards are evaluated
//! through the [`Guards`] port. Nothing here reads a clock or does I/O, so the same instance and
//! event always give the same result.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use health_time::SignedDuration;
use serde::{Deserialize, Serialize};

use crate::model::{Action, DataModel, Kind, State, Statechart, Transition, Value};

/// An event: from a user command, a domain event, a timer or an invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// The name, dot-separated (`triage.completed`, `done.state.consultation`).
    pub name: String,
    /// The payload.
    #[serde(default)]
    pub data: BTreeMap<String, Value>,
}

impl Event {
    /// An event with no payload.
    pub fn named(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            data: BTreeMap::new(),
        }
    }
}

/// Evaluates guards (FHIRPath, outside the domain): whether `guard` holds for `event` over
/// `data`.
pub trait Guards {
    /// Whether `guard` holds.
    fn holds(&self, guard: &str, event: Option<&Event>, data: &DataModel) -> bool;
}

/// What the workflow application must do after a step, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Effect {
    /// Carry out a typed action. `key` (microstep, index) makes the invocation idempotent: the
    /// application derives its UUIDv5 from the instance and this key.
    Execute {
        /// The action id.
        action: String,
        /// Its parameters.
        params: BTreeMap<String, Value>,
        /// The microstep sequence and the action's index in it.
        key: (u64, u32),
    },
    /// Start a durable timer that raises `event` after `after`.
    StartTimer {
        /// The timer id (`{state}#{index}`).
        id: String,
        /// How long from now.
        after: SignedDuration,
        /// The event to raise.
        event: String,
    },
    /// Cancel a timer whose state was left.
    CancelTimer {
        /// The timer id.
        id: String,
    },
    /// Start a sub-workflow.
    StartInvoke {
        /// The invocation id.
        id: String,
        /// The definition `url|version`.
        src: String,
    },
    /// Cancel a sub-workflow whose state was left.
    CancelInvoke {
        /// The invocation id.
        id: String,
    },
    /// The instance reached a top-level final state and is complete.
    Completed,
}

/// A running instance's state: what is persisted between steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instance {
    /// The active states, in document order.
    pub configuration: Vec<String>,
    /// The data model.
    pub data: DataModel,
    /// Microsteps taken so far (the key of each action effect).
    pub sequence: u64,
    /// Invocations started and not yet cancelled.
    pub invoked: Vec<String>,
    /// False once a top-level final state is reached: further events are ignored.
    pub running: bool,
}

/// The definition is not a valid statechart.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DefinitionError {
    /// Two states share an id.
    #[error("state id {0:?} is used twice")]
    DuplicateState(String),
    /// A transition or `initial` names no state.
    #[error("{0:?} names no state")]
    UnknownTarget(String),
    /// A compound state's `initial` is not one of its descendants.
    #[error("the initial state of {0:?} is not one of its descendants")]
    BadInitial(String),
    /// A state of this kind cannot have children, or must have them.
    #[error("{0:?} has children it cannot have, or lacks children it needs")]
    BadChildren(String),
    /// An instance names a state the definition does not have.
    #[error("the instance names a state the definition does not have")]
    UnknownInstanceState,
}

/// One state, flattened. Index 0 is the root, an implicit compound state.
#[derive(Debug, Clone)]
struct Node {
    id: String,
    kind: Kind,
    parent: Option<usize>,
    children: Vec<usize>,
    initial: Vec<usize>,
    on_entry: Vec<Vec<Action>>,
    on_exit: Vec<Vec<Action>>,
    transitions: Vec<Transition>,
    targets: Vec<Vec<usize>>,
    timers: Vec<(SignedDuration, String)>,
    invoke: Vec<(String, String)>,
}

/// A compiled statechart, ready to run instances.
#[derive(Debug, Clone)]
pub struct Machine {
    nodes: Vec<Node>,
    by_id: BTreeMap<String, usize>,
}

/// A transition chosen to run: its source node and index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Chosen {
    source: usize,
    index: usize,
}

/// One run's working state.
struct Run<'a, G: Guards> {
    machine: &'a Machine,
    guards: &'a G,
    configuration: BTreeSet<usize>,
    data: DataModel,
    sequence: u64,
    invoked: Vec<String>,
    running: bool,
    internal: VecDeque<Event>,
    to_invoke: BTreeSet<usize>,
    effects: Vec<Effect>,
    action_index: u32,
}

impl Machine {
    /// Compiles `chart`, checking ids, targets and kinds.
    pub fn new(chart: &Statechart) -> Result<Self, DefinitionError> {
        let mut machine = Self {
            nodes: vec![Node {
                id: String::new(),
                kind: Kind::Compound,
                parent: None,
                children: Vec::new(),
                initial: Vec::new(),
                on_entry: Vec::new(),
                on_exit: Vec::new(),
                transitions: Vec::new(),
                targets: Vec::new(),
                timers: Vec::new(),
                invoke: Vec::new(),
            }],
            by_id: BTreeMap::new(),
        };
        for state in &chart.states {
            machine.add(state, 0)?;
        }
        let resolve = |by_id: &BTreeMap<String, usize>, ids: &[String]| {
            ids.iter()
                .map(|id| {
                    by_id
                        .get(id)
                        .copied()
                        .ok_or_else(|| DefinitionError::UnknownTarget(id.clone()))
                })
                .collect::<Result<Vec<_>, _>>()
        };
        let root_initial = resolve(&machine.by_id, &chart.initial)?;
        machine.nodes[0].initial = root_initial;
        for index in 0..machine.nodes.len() {
            let targets = machine.nodes[index]
                .transitions
                .iter()
                .map(|t| resolve(&machine.by_id, &t.targets))
                .collect::<Result<Vec<_>, _>>()?;
            machine.nodes[index].targets = targets;
        }
        for state in &chart.states {
            machine.resolve_initial(state)?;
        }
        for index in 0..machine.nodes.len() {
            if machine.nodes[index].initial.is_empty()
                && let Some(first) = machine.nodes[index].children.first().copied()
            {
                machine.nodes[index].initial = vec![first];
            }
            let initial = machine.nodes[index].initial.clone();
            if initial.iter().any(|i| !machine.is_descendant(*i, index)) {
                return Err(DefinitionError::BadInitial(machine.nodes[index].id.clone()));
            }
        }
        Ok(machine)
    }

    fn add(&mut self, state: &State, parent: usize) -> Result<usize, DefinitionError> {
        if self.by_id.contains_key(&state.id) {
            return Err(DefinitionError::DuplicateState(state.id.clone()));
        }
        let needs_children = matches!(state.kind, Kind::Compound | Kind::Parallel);
        if needs_children == state.children.is_empty() {
            return Err(DefinitionError::BadChildren(state.id.clone()));
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            id: state.id.clone(),
            kind: state.kind,
            parent: Some(parent),
            children: Vec::new(),
            initial: Vec::new(),
            on_entry: state.on_entry.clone(),
            on_exit: state.on_exit.clone(),
            transitions: state.transitions.clone(),
            targets: Vec::new(),
            timers: state
                .timers
                .iter()
                .map(|t| (t.after, t.event.clone()))
                .collect(),
            invoke: state
                .invoke
                .iter()
                .map(|i| (i.id.clone(), i.src.clone()))
                .collect(),
        });
        self.by_id.insert(state.id.clone(), index);
        self.nodes[parent].children.push(index);
        for child in &state.children {
            self.add(child, index)?;
        }
        Ok(index)
    }

    fn resolve_initial(&mut self, state: &State) -> Result<(), DefinitionError> {
        let index = self.by_id[&state.id];
        if state.kind == Kind::Compound {
            let initial = state
                .initial
                .iter()
                .map(|id| {
                    self.by_id
                        .get(id)
                        .copied()
                        .ok_or_else(|| DefinitionError::UnknownTarget(id.clone()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            self.nodes[index].initial = initial;
        }
        for child in &state.children {
            self.resolve_initial(child)?;
        }
        Ok(())
    }

    fn is_descendant(&self, node: usize, ancestor: usize) -> bool {
        let mut current = self.nodes[node].parent;
        while let Some(parent) = current {
            if parent == ancestor {
                return true;
            }
            current = self.nodes[parent].parent;
        }
        false
    }

    /// Ancestors of `node` up to, not including, `stop` (or the root, included, when `stop` is
    /// `None`), nearest first.
    fn proper_ancestors(&self, node: usize, stop: Option<usize>) -> Vec<usize> {
        let mut ancestors = Vec::new();
        let mut current = self.nodes[node].parent;
        while let Some(parent) = current {
            if Some(parent) == stop {
                break;
            }
            ancestors.push(parent);
            current = self.nodes[parent].parent;
        }
        ancestors
    }

    fn is_compound(&self, node: usize) -> bool {
        node == 0 || self.nodes[node].kind == Kind::Compound
    }

    /// Starts an instance: enters the initial configuration and runs to completion.
    pub fn start<G: Guards>(&self, guards: &G) -> (Instance, Vec<Effect>) {
        let mut run = Run::new(
            self,
            guards,
            Instance {
                configuration: Vec::new(),
                data: DataModel::new(),
                sequence: 0,
                invoked: Vec::new(),
                running: true,
            },
        )
        .unwrap_or_else(|_| unreachable!("an empty instance names no states"));
        run.sequence += 1;
        run.action_index = 0;
        let initial = self.nodes[0].initial.clone();
        run.enter_initial(&initial);
        run.complete();
        run.finish()
    }

    /// Processes one external event: a macrostep, run to completion. A completed instance
    /// ignores it.
    pub fn step<G: Guards>(
        &self,
        instance: Instance,
        event: &Event,
        guards: &G,
    ) -> Result<(Instance, Vec<Effect>), DefinitionError> {
        let mut run = Run::new(self, guards, instance)?;
        if !run.running {
            return Ok(run.finish());
        }
        let enabled = run.select(Some(event));
        if !enabled.is_empty() {
            run.microstep(&enabled, Some(event));
        }
        run.complete();
        Ok(run.finish())
    }
}

/// Whether event descriptor `descriptor` matches `name` (SCXML 3.12.1): `*`, the name itself,
/// or a dot-separated prefix of it (`foo` and `foo.*` match `foo.bar`).
pub fn matches(descriptor: &str, name: &str) -> bool {
    let descriptor = descriptor.trim_end_matches(".*").trim_end_matches('.');
    descriptor == "*"
        || descriptor == name
        || name
            .strip_prefix(descriptor)
            .is_some_and(|rest| rest.starts_with('.'))
}

impl<'a, G: Guards> Run<'a, G> {
    fn new(
        machine: &'a Machine,
        guards: &'a G,
        instance: Instance,
    ) -> Result<Self, DefinitionError> {
        let configuration = instance
            .configuration
            .iter()
            .map(|id| {
                machine
                    .by_id
                    .get(id)
                    .copied()
                    .ok_or(DefinitionError::UnknownInstanceState)
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        Ok(Self {
            machine,
            guards,
            configuration,
            data: instance.data,
            sequence: instance.sequence,
            invoked: instance.invoked,
            running: instance.running,
            internal: VecDeque::new(),
            to_invoke: BTreeSet::new(),
            effects: Vec::new(),
            action_index: 0,
        })
    }

    fn finish(self) -> (Instance, Vec<Effect>) {
        let instance = Instance {
            configuration: self
                .configuration
                .iter()
                .map(|n| self.machine.nodes[*n].id.clone())
                .collect(),
            data: self.data,
            sequence: self.sequence,
            invoked: self.invoked,
            running: self.running,
        };
        (instance, self.effects)
    }

    /// Eventless transitions, then the internal queue, until neither enables anything; then the
    /// invocations of the states entered and still active (SCXML 3.13, 422).
    fn complete(&mut self) {
        while self.running {
            let mut enabled = self.select(None);
            let mut event = None;
            if enabled.is_empty() {
                let Some(next) = self.internal.pop_front() else {
                    break;
                };
                enabled = self.select(Some(&next));
                event = Some(next);
            }
            if !enabled.is_empty() {
                self.microstep(&enabled, event.as_ref());
            }
        }
        let to_invoke = std::mem::take(&mut self.to_invoke);
        for node in to_invoke {
            if !self.configuration.contains(&node) {
                continue;
            }
            for (id, src) in self.machine.nodes[node].invoke.clone() {
                self.invoked.push(id.clone());
                self.effects.push(Effect::StartInvoke { id, src });
            }
        }
    }

    /// The optimal enabled transition set for `event` (`None`: eventless), in document order.
    fn select(&self, event: Option<&Event>) -> Vec<Chosen> {
        let machine = self.machine;
        let mut enabled: Vec<Chosen> = Vec::new();
        let atomic = self
            .configuration
            .iter()
            .copied()
            .filter(|n| machine.nodes[*n].children.is_empty());
        for state in atomic {
            let chain = std::iter::once(state).chain(machine.proper_ancestors(state, None));
            'chain: for node in chain {
                for (index, transition) in machine.nodes[node].transitions.iter().enumerate() {
                    let event_ok = match event {
                        None => transition.events.is_empty(),
                        Some(e) => transition.events.iter().any(|d| matches(d, &e.name)),
                    };
                    let guard_ok = transition
                        .guard
                        .as_deref()
                        .is_none_or(|g| self.guards.holds(g, event, &self.data));
                    if event_ok && guard_ok {
                        let chosen = Chosen {
                            source: node,
                            index,
                        };
                        if !enabled.contains(&chosen) {
                            enabled.push(chosen);
                        }
                        break 'chain;
                    }
                }
            }
        }
        self.remove_conflicts(enabled)
    }

    /// SCXML's removeConflictingTransitions: a transition whose exit set overlaps an earlier one's
    /// replaces it only when its source is a descendant of the earlier one's source.
    fn remove_conflicts(&self, enabled: Vec<Chosen>) -> Vec<Chosen> {
        let mut filtered: Vec<Chosen> = Vec::new();
        for t1 in enabled {
            let exit1 = self.exit_set(t1);
            let mut preempted = false;
            let mut to_remove = Vec::new();
            for t2 in &filtered {
                let exit2 = self.exit_set(*t2);
                if !exit1.is_disjoint(&exit2) {
                    if self.machine.is_descendant(t1.source, t2.source) {
                        to_remove.push(*t2);
                    } else {
                        preempted = true;
                        break;
                    }
                }
            }
            if !preempted {
                filtered.retain(|t| !to_remove.contains(t));
                filtered.push(t1);
            }
        }
        filtered
    }

    fn targets(&self, t: Chosen) -> &[usize] {
        &self.machine.nodes[t.source].targets[t.index]
    }

    /// The transition's domain (SCXML getTransitionDomain).
    fn domain(&self, t: Chosen) -> Option<usize> {
        let machine = self.machine;
        let targets = self.targets(t);
        if targets.is_empty() {
            return None;
        }
        let transition = &machine.nodes[t.source].transitions[t.index];
        if transition.internal
            && machine.is_compound(t.source)
            && targets.iter().all(|s| machine.is_descendant(*s, t.source))
        {
            return Some(t.source);
        }
        let mut states = vec![t.source];
        states.extend_from_slice(targets);
        Some(self.lcca(&states))
    }

    /// The least common compound ancestor (the root at worst).
    fn lcca(&self, states: &[usize]) -> usize {
        let machine = self.machine;
        let (first, rest) = states.split_first().unwrap_or((&0, &[]));
        for ancestor in machine.proper_ancestors(*first, None) {
            if machine.is_compound(ancestor)
                && rest.iter().all(|s| machine.is_descendant(*s, ancestor))
            {
                return ancestor;
            }
        }
        0
    }

    fn exit_set(&self, t: Chosen) -> BTreeSet<usize> {
        match self.domain(t) {
            None => BTreeSet::new(),
            Some(domain) => self
                .configuration
                .iter()
                .copied()
                .filter(|s| self.machine.is_descendant(*s, domain))
                .collect(),
        }
    }

    fn microstep(&mut self, enabled: &[Chosen], event: Option<&Event>) {
        self.sequence += 1;
        self.action_index = 0;
        self.exit_states(enabled);
        for t in enabled {
            let actions = self.machine.nodes[t.source].transitions[t.index]
                .actions
                .clone();
            self.run_block(&actions, event);
        }
        self.enter_states(enabled, event);
    }

    fn exit_states(&mut self, enabled: &[Chosen]) {
        let mut exiting: BTreeSet<usize> = BTreeSet::new();
        for t in enabled {
            exiting.extend(self.exit_set(*t));
        }
        // Exit order: reverse document order (descendants before ancestors).
        for state in exiting.into_iter().rev() {
            self.to_invoke.remove(&state);
            let node = &self.machine.nodes[state];
            for block in node.on_exit.clone() {
                self.run_block(&block, None);
            }
            for (id, _) in &node.invoke {
                if let Some(at) = self.invoked.iter().position(|i| i == id) {
                    self.invoked.remove(at);
                    self.effects.push(Effect::CancelInvoke { id: id.clone() });
                }
            }
            for index in 0..node.timers.len() {
                self.effects.push(Effect::CancelTimer {
                    id: format!("{}#{index}", node.id),
                });
            }
            self.configuration.remove(&state);
        }
    }

    fn enter_initial(&mut self, initial: &[usize]) {
        let mut to_enter = BTreeSet::new();
        let mut default_entry = BTreeSet::new();
        // Every target's descendants first, then the ancestors, as for a transition: otherwise a
        // parallel ancestor would default-enter a region another target is about to enter.
        for state in initial {
            self.add_descendants(*state, &mut to_enter, &mut default_entry);
        }
        for state in initial {
            self.add_ancestors(*state, Some(0), &mut to_enter, &mut default_entry);
        }
        self.enter_set(to_enter, None);
    }

    fn enter_states(&mut self, enabled: &[Chosen], event: Option<&Event>) {
        let mut to_enter = BTreeSet::new();
        let mut default_entry = BTreeSet::new();
        for t in enabled {
            let targets = self.targets(*t).to_vec();
            for state in &targets {
                self.add_descendants(*state, &mut to_enter, &mut default_entry);
            }
            let domain = self.domain(*t);
            for state in &targets {
                self.add_ancestors(*state, domain, &mut to_enter, &mut default_entry);
            }
        }
        self.enter_set(to_enter, event);
    }

    fn add_descendants(
        &self,
        state: usize,
        to_enter: &mut BTreeSet<usize>,
        default_entry: &mut BTreeSet<usize>,
    ) {
        to_enter.insert(state);
        let machine = self.machine;
        match machine.nodes[state].kind {
            Kind::Compound => {
                default_entry.insert(state);
                for child in machine.nodes[state].initial.clone() {
                    self.add_descendants(child, to_enter, default_entry);
                    self.add_ancestors(child, Some(state), to_enter, default_entry);
                }
            }
            Kind::Parallel => {
                for child in machine.nodes[state].children.clone() {
                    if !to_enter
                        .iter()
                        .any(|s| *s == child || machine.is_descendant(*s, child))
                    {
                        self.add_descendants(child, to_enter, default_entry);
                    }
                }
            }
            Kind::Atomic | Kind::Final => {}
        }
    }

    fn add_ancestors(
        &self,
        state: usize,
        stop: Option<usize>,
        to_enter: &mut BTreeSet<usize>,
        default_entry: &mut BTreeSet<usize>,
    ) {
        let machine = self.machine;
        for ancestor in machine.proper_ancestors(state, stop) {
            if ancestor == 0 {
                continue;
            }
            to_enter.insert(ancestor);
            if machine.nodes[ancestor].kind == Kind::Parallel {
                for child in machine.nodes[ancestor].children.clone() {
                    if !to_enter
                        .iter()
                        .any(|s| *s == child || machine.is_descendant(*s, child))
                    {
                        self.add_descendants(child, to_enter, default_entry);
                    }
                }
            }
        }
    }

    /// Enters `states` in document order (entry order), with their handlers, timers and
    /// completion events.
    fn enter_set(&mut self, states: BTreeSet<usize>, event: Option<&Event>) {
        let machine = self.machine;
        for state in states {
            if self.configuration.contains(&state) {
                continue;
            }
            self.configuration.insert(state);
            self.to_invoke.insert(state);
            let node = &machine.nodes[state];
            for block in node.on_entry.clone() {
                self.run_block(&block, event);
            }
            for (index, (after, raised)) in node.timers.iter().enumerate() {
                self.effects.push(Effect::StartTimer {
                    id: format!("{}#{index}", node.id),
                    after: *after,
                    event: raised.clone(),
                });
            }
            if node.kind == Kind::Final {
                let parent = node.parent.unwrap_or(0);
                if parent == 0 {
                    self.running = false;
                    self.effects.push(Effect::Completed);
                    return;
                }
                self.internal.push_back(Event::named(&format!(
                    "done.state.{}",
                    machine.nodes[parent].id
                )));
                if let Some(grandparent) = machine.nodes[parent].parent
                    && grandparent != 0
                    && machine.nodes[grandparent].kind == Kind::Parallel
                    && machine.nodes[grandparent]
                        .children
                        .iter()
                        .all(|c| self.in_final(*c))
                {
                    self.internal.push_back(Event::named(&format!(
                        "done.state.{}",
                        machine.nodes[grandparent].id
                    )));
                }
            }
        }
    }

    /// Whether `state` is complete: a compound with an active final child, or a parallel whose
    /// children are all complete.
    fn in_final(&self, state: usize) -> bool {
        let node = &self.machine.nodes[state];
        match node.kind {
            Kind::Compound => node.children.iter().any(|c| {
                self.machine.nodes[*c].kind == Kind::Final && self.configuration.contains(c)
            }),
            Kind::Parallel => node.children.iter().all(|c| self.in_final(*c)),
            Kind::Atomic | Kind::Final => false,
        }
    }

    /// One block of executable content.
    fn run_block(&mut self, actions: &[Action], _event: Option<&Event>) {
        for action in actions {
            match action {
                Action::Raise { event } => self.internal.push_back(Event::named(event)),
                Action::Assign { field, value } => {
                    self.data.insert(field.clone(), value.clone());
                }
                Action::Execute { action, params } => {
                    self.effects.push(Effect::Execute {
                        action: action.clone(),
                        params: params.clone(),
                        key: (self.sequence, self.action_index),
                    });
                    self.action_index += 1;
                }
            }
        }
    }
}
