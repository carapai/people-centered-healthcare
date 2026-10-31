//! A Workflow Definition's statechart (workflow-engine.md § Statechart model).

use std::collections::BTreeMap;

use health_time::SignedDuration;
use serde::{Deserialize, Serialize};

/// A value in an instance's data model or an event's payload: typed fields only, never copies of
/// clinical data.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    /// A flag.
    Bool(bool),
    /// A counter.
    Int(i64),
    /// An id or a code.
    Text(String),
}

/// An instance's data model.
pub type DataModel = BTreeMap<String, Value>;

/// What a state is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// No children (the kind of a state without children is always atomic).
    Atomic,
    /// Exactly one child active at a time.
    Compound,
    /// Every child active at once.
    Parallel,
    /// Entering it completes its parent.
    Final,
}

/// Executable content: what entering, leaving or taking a transition does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Action {
    /// Puts an event on the instance's internal queue.
    Raise {
        /// The event name.
        event: String,
    },
    /// Sets a data-model field.
    Assign {
        /// The field.
        field: String,
        /// Its new value.
        value: Value,
    },
    /// A typed action of another module (ADR 0043), carried out by the workflow application.
    Execute {
        /// The action id, e.g. `patient-flow/open-queue-entry`.
        action: String,
        /// Its parameters.
        #[serde(default)]
        params: BTreeMap<String, Value>,
    },
}

/// A transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    /// Event descriptors it is taken on (`foo` matches `foo` and `foo.bar`; `*` matches any
    /// event). Empty: an eventless transition, taken as soon as its guard holds.
    #[serde(default)]
    pub events: Vec<String>,
    /// A FHIRPath guard, evaluated by the [`Guards`](crate::Guards) port.
    #[serde(default)]
    pub guard: Option<String>,
    /// The target states. Empty: a targetless transition, which exits and enters nothing.
    #[serde(default)]
    pub targets: Vec<String>,
    /// SCXML `type="internal"`: when its source is compound and every target is a descendant,
    /// the source is not exited and re-entered.
    #[serde(default)]
    pub internal: bool,
    /// What it does, after the exits and before the entries.
    #[serde(default)]
    pub actions: Vec<Action>,
}

/// A durable timer: an event raised `after` the state is entered, unless it is left first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timer {
    /// How long after entry.
    pub after: SignedDuration,
    /// The event it raises.
    pub event: String,
}

/// A sub-workflow started while the state is active (`done.invoke.<id>` comes back when it
/// finishes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invoke {
    /// The invocation id, unique in the definition.
    pub id: String,
    /// The sub-workflow's definition `url|version`.
    pub src: String,
}

/// A state and its children.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// The state id, unique in the definition.
    pub id: String,
    /// What it is.
    pub kind: Kind,
    /// For a compound state, the children entered by default; otherwise its first child.
    #[serde(default)]
    pub initial: Vec<String>,
    /// Children, in document order.
    #[serde(default)]
    pub children: Vec<State>,
    /// Entry handlers, each a separate block, run in order.
    #[serde(default)]
    pub on_entry: Vec<Vec<Action>>,
    /// Exit handlers, each a separate block, run in order.
    #[serde(default)]
    pub on_exit: Vec<Vec<Action>>,
    /// Transitions, in document order.
    #[serde(default)]
    pub transitions: Vec<Transition>,
    /// Timers started on entry.
    #[serde(default)]
    pub timers: Vec<Timer>,
    /// Sub-workflows invoked while it is active.
    #[serde(default)]
    pub invoke: Vec<Invoke>,
}

impl State {
    /// An empty state of `kind`.
    pub fn new(id: &str, kind: Kind) -> Self {
        Self {
            id: id.to_owned(),
            kind,
            initial: Vec::new(),
            children: Vec::new(),
            on_entry: Vec::new(),
            on_exit: Vec::new(),
            transitions: Vec::new(),
            timers: Vec::new(),
            invoke: Vec::new(),
        }
    }
}

/// A statechart: the top-level states and where it starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Statechart {
    /// The top-level states, in document order.
    pub states: Vec<State>,
    /// The states entered at start; otherwise the first top-level state.
    #[serde(default)]
    pub initial: Vec<String>,
}
