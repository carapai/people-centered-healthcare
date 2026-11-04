//! Billing's contract (ADR 0033; billing-and-payments.md § Financial Gates): the query the
//! workflow engine asks when an instance reaches a gate state, so workflows contain no payment
//! steps and never read ledger internals.
//!
//! - [`GateQuery`]: which gate, for which Account and patient, and how urgent the care is.
//! - [`GateOutcome`]: `pass` (paid, covered or free), `defer` (with a reason: urgent care, or a
//!   recorded deferral) or `block` (non-urgent care only, and only where the gate allows it).
//! - [`FinancialGates`]: the port Finance implements. An emergency or urgent triage category
//!   always gives `defer` when anything is owed: urgent care is never blocked.

use std::future::Future;

use serde::{Deserialize, Serialize};

/// How urgent the care is, from the triage category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Urgency {
    /// Not urgent.
    Routine,
    /// Urgent triage.
    Urgent,
    /// An emergency.
    Emergency,
}

impl Urgency {
    /// Whether care must never wait for payment.
    pub fn never_blocks(self) -> bool {
        self >= Self::Urgent
    }
}

/// What the workflow engine asks at a gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateQuery {
    /// The gate's canonical URL.
    pub gate: String,
    /// The Account, e.g. `Encounter/123`.
    pub account: String,
    /// The patient.
    pub patient: String,
    /// How urgent the care is.
    pub urgency: Urgency,
}

/// Finance's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "outcome", content = "reason")]
pub enum GateOutcome {
    /// Paid, covered or free: continue.
    Pass(String),
    /// Continue, with the deferral recorded.
    Defer(String),
    /// Wait for payment: a cashier Task, re-evaluated on payment events.
    Block(String),
}

impl GateOutcome {
    /// The outcome's code, which is also the event a gate state raises: `gate.pass`,
    /// `gate.defer` or `gate.block`.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Pass(_) => "pass",
            Self::Defer(_) => "defer",
            Self::Block(_) => "block",
        }
    }

    /// Why.
    pub fn reason(&self) -> &str {
        match self {
            Self::Pass(reason) | Self::Defer(reason) | Self::Block(reason) => reason,
        }
    }
}

/// The gate query could not be answered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the financial gate could not be evaluated: {0}")]
pub struct GateUnavailable(pub String);

/// Finance's gate query, implemented by billing.
pub trait FinancialGates: Send + Sync {
    /// The outcome of `query`.
    fn evaluate_gate(
        &self,
        query: &GateQuery,
    ) -> impl Future<Output = Result<GateOutcome, GateUnavailable>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urgent_and_emergency_care_never_block() {
        assert!(!Urgency::Routine.never_blocks());
        assert!(Urgency::Urgent.never_blocks());
        assert!(Urgency::Emergency.never_blocks());
    }

    #[test]
    fn outcomes_name_their_events() {
        let defer = GateOutcome::Defer("urgent care".into());
        assert_eq!((defer.code(), defer.reason()), ("defer", "urgent care"));
        assert_eq!(GateOutcome::Pass(String::new()).code(), "pass");
        assert_eq!(GateOutcome::Block(String::new()).code(), "block");
    }
}
