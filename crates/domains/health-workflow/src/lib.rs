//! The workflow engine's statechart interpreter (workflow-engine.md § Statechart model; ADR 0042).
//!
//! A Workflow Definition is a [`Statechart`] of atomic, compound, parallel and final states, with
//! transitions (event descriptors, a FHIRPath guard, typed actions, targets), entry and exit
//! handlers, timers and invocations. [`Machine`] runs it with SCXML-style semantics:
//! run-to-completion, one event queue per instance, transitions chosen by document order, and
//! deterministic. The behaviour is pinned by a curated subset of the W3C SCXML conformance
//! assertions (`tests/scxml.rs`).
//!
//! The interpreter is pure: it returns [`Effect`]s (typed actions, timers, invocations) for the
//! workflow application to carry out in its transaction (ADR 0044), and evaluates guards through
//! the [`Guards`] port.

mod interpreter;
mod model;

pub use interpreter::{DefinitionError, Effect, Event, Guards, Instance, Machine, matches};
pub use model::{Action, DataModel, Invoke, Kind, State, Statechart, Timer, Transition, Value};
