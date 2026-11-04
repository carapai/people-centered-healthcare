//! Financial Gates at the facility authority (ADR 0033; #122): billing's answer to the gate
//! query, and the decisions that change it.
//!
//! - **The query** ([`FinancialGates`] on [`Billing`]): what the patient owes on the Account (the
//!   patient share of its owed charges, less payments settled against it and waivers), whether a
//!   deferral was recorded there, and the gate's rules. A gate this node does not have is an
//!   error, which the workflow engine meets by deferring, never blocking.
//! - **Settling** ([`Billing::settle`]): a posted payment, or part of it, counts towards an
//!   Account; a payment is never settled beyond its amount.
//! - **Deferring** ([`Billing::defer`]): an authorized user lets the care go ahead; the charges
//!   stay owed.
//! - **Waiving** ([`Billing::waive`]): a separate, accountable decision, posted to the patient's
//!   waiver account against their receivable. Who may defer and waive is the caller's permission
//!   check (social worker or finance), and both are audited by the caller.
//!
//! Preauthorizations come with coverage (#125): until then none counts as approved, so a gate
//! that needs one blocks or defers routine care.

use health_billing::{
    AccountStore, ChargeStore, Deferral, FinancialGate, GateFacts, Settlement, Waiver, evaluate,
    owed,
};
use health_billing_contract::{FinancialGates, GateOutcome, GateQuery, GateUnavailable};
use health_financial_ledger::{Dimensions, Line, Money, Posting, PostingStore, Side};
use health_payments::{PaymentStore, Status};
use health_persistence::{Store, Transaction};
use health_pricing::PricingStore;
use health_time::Timestamp;
use rust_decimal::Decimal;

use crate::{Billing, BillingError, PATIENT, owed as counts};

impl<S> Billing<S>
where
    S: Store,
    S::Tx: ChargeStore + PostingStore + PricingStore + AccountStore + PaymentStore,
{
    /// This billing, answering `gates`.
    #[must_use]
    pub fn with_gates(mut self, gates: Vec<FinancialGate>) -> Self {
        self.gates = gates;
        self
    }

    async fn owed_in(&self, tx: &mut S::Tx, account: &str) -> Result<Decimal, BillingError> {
        let shares: Vec<Decimal> = tx
            .charges_of_account(account)
            .await?
            .iter()
            .filter(|c| counts(c))
            .filter_map(|c| c.price.as_ref().map(|p| p.patient_share))
            .collect();
        let settled = tx.settlements_of_account(account).await?;
        let waived = tx.waivers(account).await?;
        Ok(owed(shares, &settled, &waived))
    }

    /// What the patient still owes on `account`.
    pub async fn owed(&self, account: &str) -> Result<Decimal, BillingError> {
        let mut tx = self.store.begin().await?;
        self.owed_in(&mut tx, account).await
    }

    /// Records that `by` let the care at `gate` go ahead on `account`, unpaid.
    pub async fn defer(
        &self,
        account: &str,
        gate: &str,
        by: &str,
        reason: &str,
        at: Timestamp,
    ) -> Result<Deferral, BillingError> {
        let deferral = Deferral {
            account: account.to_owned(),
            gate: gate.to_owned(),
            by: by.to_owned(),
            reason: reason.to_owned(),
            at,
        };
        deferral.check()?;
        let mut tx = self.store.begin().await?;
        tx.add_deferral(&deferral).await?;
        tx.commit().await?;
        Ok(deferral)
    }

    /// Settles `amount` of posted payment `payment` against `account`.
    pub async fn settle(
        &self,
        payment: &str,
        account: &str,
        amount: Decimal,
        by: &str,
        at: Timestamp,
    ) -> Result<Settlement, BillingError> {
        let mut tx = self.store.begin().await?;
        let found = tx
            .payment(payment)
            .await?
            .filter(|p| p.status == Status::Posted)
            .ok_or_else(|| BillingError::NotPayable(payment.to_owned()))?;
        let settled: Decimal = tx
            .settlements_of_payment(payment)
            .await?
            .iter()
            .map(|s| s.amount)
            .sum();
        let settlement = Settlement {
            payment: payment.to_owned(),
            account: account.to_owned(),
            amount,
            by: by.to_owned(),
            at,
        };
        settlement.check(found.amount - settled)?;
        tx.add_settlement(&settlement).await?;
        tx.commit().await?;
        Ok(settlement)
    }

    /// Waives `amount` of what the patient owes on `account`, posting it to the patient's waiver
    /// account against their receivable.
    pub async fn waive(&self, waiver: Waiver) -> Result<Waiver, BillingError> {
        let mut tx = self.store.begin().await?;
        let owed = self.owed_in(&mut tx, &waiver.account).await?;
        waiver.check(owed)?;
        let currency = self.settings.currency;
        let dimensions = Dimensions {
            facility: self.settings.facility.clone(),
            funding_source: Some(PATIENT.to_owned()),
            encounter: Some(waiver.account.clone()),
            ..Dimensions::default()
        };
        let line = |account: String, side| Line {
            account,
            side,
            amount: Money::rounded(waiver.amount, currency),
            dimensions: dimensions.clone(),
            original: None,
        };
        let posting = Posting {
            id: format!("waiver-{}", waiver.id),
            at: waiver.at,
            description: format!(
                "Waiver {} on {}: {}",
                waiver.id, waiver.account, waiver.reason
            ),
            lines: vec![
                line(format!("waiver/{PATIENT}"), Side::Debit),
                line(format!("receivable/{PATIENT}"), Side::Credit),
            ],
            reverses: None,
        };
        self.ledger()?.check(&posting)?;
        tx.add_waiver(&waiver).await?;
        tx.append_posting(&posting).await?;
        tx.commit().await?;
        Ok(waiver)
    }
}

impl<S> FinancialGates for Billing<S>
where
    S: Store,
    S::Tx: ChargeStore + PostingStore + PricingStore + AccountStore + PaymentStore,
{
    async fn evaluate_gate(&self, query: &GateQuery) -> Result<GateOutcome, GateUnavailable> {
        let unavailable = |error: BillingError| GateUnavailable(error.to_string());
        let gate = self
            .gates
            .iter()
            .find(|g| g.url == query.gate)
            .ok_or_else(|| GateUnavailable(format!("{} is not configured here", query.gate)))?;
        let mut tx = self
            .store
            .begin()
            .await
            .map_err(|e| unavailable(e.into()))?;
        let owed = self
            .owed_in(&mut tx, &query.account)
            .await
            .map_err(unavailable)?;
        let deferred = tx
            .deferrals(&query.account)
            .await
            .map_err(|e| unavailable(e.into()))?
            .iter()
            .any(|d| d.gate == gate.url);
        let facts = GateFacts {
            owed,
            deferred,
            preauthorized: false,
        };
        Ok(evaluate(gate, &facts, query.urgency))
    }
}
