//! Financial Gates in workflows (ADR 0033; workflow-engine.md § Financial Gates; #122).
//!
//! - **On entry** to a gate state the interpreter emits `EvaluateGate`; the application asks
//!   Finance's [`FinancialGates`] for the outcome and raises `gate.pass`, `gate.defer` or
//!   `gate.block` in the same step, so a gate that passes or defers is never seen waiting, and
//!   the history entry records the answer (`GateEvaluated`).
//! - **The query** names the instance's context as the Account, its `patient` data field, and
//!   its `urgency` data field (`routine`, `urgent` or `emergency`, set at triage). Urgent and
//!   emergency care always defers.
//! - **Fail-safe:** when Finance cannot answer, or this node has no gate port, the gate defers;
//!   it never blocks.
//! - **Blocked:** the instance waits in the gate state, whose task is the cashier's.
//!   [`Workflows::recheck_gates`], on a payment event, asks again.

use std::future::Future;
use std::pin::Pin;

use health_billing_contract::{FinancialGates, GateOutcome, GateQuery, GateUnavailable, Urgency};
use health_persistence::{Store, Transaction};
use health_time::Timestamp;
use health_workflow::{
    Effect, Event, Guards, Instance, InstanceRecord, Machine, State, Value, WorkflowStore,
};

use crate::{Actions, Definitions, Outcome, PRINCIPAL, WorkflowError, Workflows, event_key};

/// At most this many gates are answered in one step: a definition cannot loop through gates.
const MOST_GATES: usize = 8;

/// [`FinancialGates`] behind a pointer.
pub trait GatePort: Send + Sync {
    /// The outcome of `query`.
    fn evaluate<'a>(
        &'a self,
        query: &'a GateQuery,
    ) -> Pin<Box<dyn Future<Output = Result<GateOutcome, GateUnavailable>> + Send + 'a>>;
}

impl<T: FinancialGates> GatePort for T {
    fn evaluate<'a>(
        &'a self,
        query: &'a GateQuery,
    ) -> Pin<Box<dyn Future<Output = Result<GateOutcome, GateUnavailable>> + Send + 'a>> {
        Box::pin(self.evaluate_gate(query))
    }
}

fn text(data: &health_workflow::DataModel, field: &str) -> Option<String> {
    match data.get(field) {
        Some(Value::Text(value)) => Some(value.clone()),
        _ => None,
    }
}

fn urgency(data: &health_workflow::DataModel) -> Urgency {
    match text(data, "urgency").as_deref() {
        Some("emergency") => Urgency::Emergency,
        Some("urgent") => Urgency::Urgent,
        _ => Urgency::Routine,
    }
}

fn gate_of<'a>(states: &'a [State], id: &str) -> Option<&'a str> {
    states.iter().find_map(|state| {
        if state.id == id {
            state.gate.as_deref()
        } else {
            gate_of(&state.children, id)
        }
    })
}

impl<S, D, A, G> Workflows<S, D, A, G>
where
    S: Store,
    S::Tx: WorkflowStore,
    D: Definitions,
    A: Actions<S::Tx>,
    G: Guards + Send + Sync,
{
    /// Answers the Financial Gates of this node with `port`.
    #[must_use]
    pub fn with_gates(mut self, port: std::sync::Arc<dyn GatePort>) -> Self {
        self.gates = Some(port);
        self
    }

    /// Finance's outcome at `gate` for the instance about `context` with `data`: deferring when
    /// Finance cannot answer.
    async fn outcome(
        &self,
        gate: &str,
        context: &str,
        data: &health_workflow::DataModel,
    ) -> GateOutcome {
        let Some(port) = &self.gates else {
            return GateOutcome::Defer("Finance does not answer gates here".into());
        };
        let query = GateQuery {
            gate: gate.to_owned(),
            account: context.to_owned(),
            patient: text(data, "patient").unwrap_or_default(),
            urgency: urgency(data),
        };
        port.evaluate(&query)
            .await
            .unwrap_or_else(|error| GateOutcome::Defer(error.to_string()))
    }

    /// Answers the gates a step entered, stepping `gate.<outcome>` in, until none is left.
    pub(crate) async fn settle_gates(
        &self,
        machine: &Machine,
        context: &str,
        mut state: Instance,
        effects: Vec<Effect>,
    ) -> Result<(Instance, Vec<Effect>), WorkflowError> {
        let mut settled = Vec::new();
        let mut pending = effects;
        let mut answered = 0;
        while !pending.is_empty() {
            let mut next = Vec::new();
            for effect in pending {
                let Effect::EvaluateGate { state: at, gate } = effect else {
                    settled.push(effect);
                    continue;
                };
                answered += 1;
                let outcome = if answered > MOST_GATES {
                    GateOutcome::Defer("too many gates in one step".into())
                } else {
                    self.outcome(&gate, context, &state.data).await
                };
                settled.push(Effect::GateEvaluated {
                    state: at,
                    gate,
                    outcome: outcome.code().to_owned(),
                    reason: outcome.reason().to_owned(),
                });
                let mut event = Event::named(&format!("gate.{}", outcome.code()));
                event
                    .data
                    .insert("reason".into(), Value::Text(outcome.reason().to_owned()));
                let (stepped, more) = machine.step(state, &event, &self.guards)?;
                state = stepped;
                next.extend(more);
            }
            pending = next;
        }
        Ok((state, settled))
    }

    /// Asks Finance again for every gate `instance` is waiting at, on payment event `event_id`;
    /// a pass or defer moves it on. Delivered once per event id.
    pub async fn recheck_gates(
        &self,
        event_id: &str,
        instance: &str,
    ) -> Result<Outcome, WorkflowError> {
        let now: Timestamp = self.clock.now();
        let mut tx = self.store.begin().await?;
        if !tx.claim_event(&event_key(event_id), now).await? {
            return Ok(Outcome::Replayed);
        }
        let record: InstanceRecord = tx
            .instance(instance)
            .await?
            .ok_or_else(|| WorkflowError::NotFound(instance.to_owned()))?;
        if !record.state.running {
            tx.commit().await?;
            return Ok(Outcome::Ignored);
        }
        let compiled = self.compiled(&record.definition)?;
        let waiting = record
            .state
            .configuration
            .iter()
            .find_map(|id| gate_of(&compiled.chart.states, id));
        let Some(gate) = waiting else {
            tx.commit().await?;
            return Ok(Outcome::Ignored);
        };
        let outcome = self
            .outcome(gate, &record.context, &record.state.data)
            .await;
        if outcome.code() == "block" {
            tx.commit().await?;
            return Ok(Outcome::Ignored);
        }
        let mut event = Event::named(&format!("gate.{}", outcome.code()));
        event
            .data
            .insert("reason".into(), Value::Text(outcome.reason().to_owned()));
        let moved = self
            .advance(&mut tx, record, &event, PRINCIPAL, event_id, now)
            .await?;
        tx.commit().await?;
        Ok(moved.map_or(Outcome::Ignored, Outcome::Applied))
    }
}
