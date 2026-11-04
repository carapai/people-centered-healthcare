//! Financial Gates (ADR 0033; billing-and-payments.md § Financial Gates; #122): configuration
//! records attached to workflow steps, and the outcome Finance gives when a workflow reaches one.
//!
//! The order of the rules is the clinical safety argument:
//! 1. a gate not active here passes;
//! 2. emergency and urgent care is never blocked: whatever is owed or missing, it defers;
//! 3. a needed preauthorization not yet approved blocks where the gate allows it, else defers;
//! 4. nothing owed (paid, covered or free) passes;
//! 5. a deferral an authorized user recorded defers;
//! 6. otherwise the gate blocks where it allows blocking, and defers where it does not.
//!
//! Deferred charges stay on the Account as receivables. Turning one into a waiver
//! ([`Waiver`]) is a separate, accountable decision: it needs a permission (social worker or
//! finance) and is audited. Emergencies are deferred, never waived automatically.

use health_billing_contract::{GateOutcome, Urgency};
use health_time::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// A Financial Gate: a configuration record, per facility, service, payer or program, attached
/// to a workflow step ("before sample collection").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancialGate {
    /// Its canonical URL: what a workflow's gate state names.
    pub url: String,
    /// Its version.
    pub version: String,
    /// Whether it is active here (a configuration override point, ADR 0026).
    pub active: bool,
    /// Whether non-urgent care may wait at it for payment; if not, unpaid care defers.
    pub allows_block: bool,
    /// Whether the step needs an approved preauthorization.
    #[serde(default)]
    pub requires_preauthorization: bool,
}

/// What Finance knows about the Account at the gate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GateFacts {
    /// What the patient still owes on the Account.
    pub owed: Decimal,
    /// Whether an authorized user recorded a deferral for this Account at this gate.
    pub deferred: bool,
    /// Whether the needed preauthorization is approved.
    pub preauthorized: bool,
}

/// The outcome of `gate` for an Account with `facts`, for care of `urgency`.
pub fn evaluate(gate: &FinancialGate, facts: &GateFacts, urgency: Urgency) -> GateOutcome {
    let unpaid = facts.owed > Decimal::ZERO;
    let unauthorized = gate.requires_preauthorization && !facts.preauthorized;
    if !gate.active {
        return GateOutcome::Pass("the gate is not active here".into());
    }
    if urgency.never_blocks() && (unpaid || unauthorized) {
        return GateOutcome::Defer(format!(
            "{} care is never blocked",
            match urgency {
                Urgency::Emergency => "emergency",
                _ => "urgent",
            }
        ));
    }
    if unauthorized {
        return if gate.allows_block {
            GateOutcome::Block("an approved preauthorization is needed".into())
        } else {
            GateOutcome::Defer("the preauthorization is pending".into())
        };
    }
    if !unpaid {
        return GateOutcome::Pass("paid, covered or free".into());
    }
    if facts.deferred {
        return GateOutcome::Defer("a deferral was recorded".into());
    }
    if gate.allows_block {
        GateOutcome::Block(format!("{} is owed before this step", facts.owed))
    } else {
        GateOutcome::Defer(format!("{} is owed; the gate does not block", facts.owed))
    }
}

/// A deferral an authorized user recorded: the care goes ahead and the charges stay owed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deferral {
    /// The Account.
    pub account: String,
    /// The gate, by canonical URL.
    pub gate: String,
    /// Who.
    pub by: String,
    /// Why.
    pub reason: String,
    /// When.
    pub at: Timestamp,
}

/// Turning what a patient owes on an Account into a waiver: written off to the waiver account of
/// the patient's funding source. Who may is the caller's permission check, and it is audited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Waiver {
    /// Its id.
    pub id: String,
    /// The Account.
    pub account: String,
    /// How much.
    pub amount: Decimal,
    /// Who.
    pub by: String,
    /// Why.
    pub reason: String,
    /// When.
    pub at: Timestamp,
}

/// Why a deferral or waiver is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GateRefused {
    /// No reason given, or no author.
    #[error("a deferral or waiver needs who and why")]
    Unexplained,
    /// A waiver of nothing, or of more than is owed.
    #[error("a waiver must be more than nothing and no more than the {0} owed")]
    BadAmount(Decimal),
}

impl Deferral {
    /// Checks it names who and why.
    pub fn check(&self) -> Result<(), GateRefused> {
        if self.by.trim().is_empty() || self.reason.trim().is_empty() {
            return Err(GateRefused::Unexplained);
        }
        Ok(())
    }
}

impl Waiver {
    /// Checks it names who and why, and waives no more than `owed`.
    pub fn check(&self, owed: Decimal) -> Result<(), GateRefused> {
        if self.by.trim().is_empty() || self.reason.trim().is_empty() {
            return Err(GateRefused::Unexplained);
        }
        if self.amount <= Decimal::ZERO || self.amount > owed {
            return Err(GateRefused::BadAmount(owed));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate(allows_block: bool) -> FinancialGate {
        FinancialGate {
            url: "https://health.go.ug/fhir/FinancialGate/before-lab".into(),
            version: "1.0.0".into(),
            active: true,
            allows_block,
            requires_preauthorization: false,
        }
    }

    fn owing(amount: i64) -> GateFacts {
        GateFacts {
            owed: Decimal::from(amount),
            ..GateFacts::default()
        }
    }

    #[test]
    fn urgent_and_emergency_care_always_defers_whatever_is_owed() {
        for urgency in [Urgency::Urgent, Urgency::Emergency] {
            for allows_block in [true, false] {
                let outcome = evaluate(&gate(allows_block), &owing(50_000), urgency);
                assert_eq!(outcome.code(), "defer", "{urgency:?}");
            }
            let mut preauth = gate(true);
            preauth.requires_preauthorization = true;
            assert_eq!(
                evaluate(&preauth, &owing(0), urgency).code(),
                "defer",
                "nor waits for a preauthorization"
            );
        }
    }

    #[test]
    fn routine_care_passes_when_nothing_is_owed_and_otherwise_follows_the_gate() {
        assert_eq!(
            evaluate(&gate(true), &owing(0), Urgency::Routine).code(),
            "pass"
        );
        assert_eq!(
            evaluate(&gate(true), &owing(10_000), Urgency::Routine),
            GateOutcome::Block("10000 is owed before this step".into())
        );
        assert_eq!(
            evaluate(&gate(false), &owing(10_000), Urgency::Routine).code(),
            "defer",
            "a gate that does not block defers"
        );
        let deferred = GateFacts {
            deferred: true,
            ..owing(10_000)
        };
        assert_eq!(
            evaluate(&gate(true), &deferred, Urgency::Routine).code(),
            "defer"
        );
        let mut inactive = gate(true);
        inactive.active = false;
        assert_eq!(
            evaluate(&inactive, &owing(10_000), Urgency::Routine).code(),
            "pass"
        );
    }

    #[test]
    fn a_needed_preauthorization_must_be_approved() {
        let mut preauth = gate(true);
        preauth.requires_preauthorization = true;
        assert_eq!(
            evaluate(&preauth, &owing(0), Urgency::Routine).code(),
            "block"
        );
        let approved = GateFacts {
            preauthorized: true,
            ..owing(0)
        };
        assert_eq!(
            evaluate(&preauth, &approved, Urgency::Routine).code(),
            "pass"
        );
        preauth.allows_block = false;
        assert_eq!(
            evaluate(&preauth, &owing(0), Urgency::Routine).code(),
            "defer"
        );
    }

    #[test]
    fn deferrals_and_waivers_say_who_and_why_and_waive_no_more_than_is_owed() {
        let at: Timestamp = "2026-11-05T09:00:00Z".parse().expect("timestamp");
        let deferral = Deferral {
            account: "Encounter/e-1".into(),
            gate: gate(true).url,
            by: "social-worker-1".into(),
            reason: " ".into(),
            at,
        };
        assert_eq!(deferral.check(), Err(GateRefused::Unexplained));
        let waiver = Waiver {
            id: "w-1".into(),
            account: "Encounter/e-1".into(),
            amount: Decimal::from(5_000),
            by: "social-worker-1".into(),
            reason: "unable to pay".into(),
            at,
        };
        assert_eq!(waiver.check(Decimal::from(10_000)), Ok(()));
        assert_eq!(
            waiver.check(Decimal::from(1_000)),
            Err(GateRefused::BadAmount(Decimal::from(1_000)))
        );
    }
}
