//! The facility's financial records go up as operations (#81; write-authority.md: charges,
//! invoices and local payments, up as operations; reversals only, never edits). Each write logs
//! its record, and the receiver replays it through the same store method, so central's copy
//! follows the facility's in commit order. Records created with an id are not created twice;
//! the rest are kept once by the push's receipts.
//!
//! Central checks what it can without the facility's chart: a posting balances, is on the
//! sending facility, and reverses a posting held here once; a payment carries its own provider
//! evidence, only moves on (never edited), and its provider reference names no other payment.

use health_billing::{AccountStore as _, Charge, ChargeStore as _, Deferral, Settlement, Waiver};
use health_financial_ledger::{Posting, PostingStore as _};
use health_payments::{Payment, PaymentStore as _, Shift};
use health_persistence::WriteError;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::store::SurrealTx;

/// A charge (billing), by account.
pub(crate) const CHARGE: &str = "charge";
/// A deferral (billing), by account.
pub(crate) const DEFERRAL: &str = "deferral";
/// A settlement of charges by a payment (billing), by account.
pub(crate) const SETTLEMENT: &str = "settlement";
/// A waiver (billing), by account.
pub(crate) const WAIVER: &str = "waiver";
/// A posting to the financial ledger, by its id.
pub(crate) const POSTING: &str = "posting";
/// A payment, by its id.
pub(crate) const PAYMENT: &str = "payment";
/// A cashier's shift, by its id.
pub(crate) const SHIFT: &str = "shift";

/// The financial ledgers.
pub(crate) const FINANCE: &[&str] = &[
    CHARGE, DEFERRAL, SETTLEMENT, WAIVER, POSTING, PAYMENT, SHIFT,
];

fn parse<T: DeserializeOwned>(body: &Value) -> Result<T, String> {
    serde_json::from_value(body.clone()).map_err(|e| e.to_string())
}

/// A one-column row's text.
fn row_text(row: &[String]) -> &str {
    row.first().map_or("", String::as_str)
}

impl SurrealTx {
    async fn held(&mut self, table: &str, id: &str) -> Result<bool, WriteError> {
        let rows = self
            .rows(
                &format!("SELECT VALUE [<string>record::id(id)] FROM type::record('{table}', $id)"),
                vec![("id", id.to_owned())],
            )
            .await
            .map_err(WriteError::Store)?;
        Ok(!rows.is_empty())
    }

    /// Why central refuses `posting` from `facility`, if it does.
    async fn posting_refused(
        &mut self,
        posting: &Posting,
        facility: Option<&str>,
    ) -> Result<Option<String>, WriteError> {
        if let Err(refused) = posting.balanced() {
            return Ok(Some(refused.to_string()));
        }
        if let Some(facility) = facility
            && let Some(line) = posting
                .lines
                .iter()
                .find(|line| line.dimensions.facility != facility)
        {
            return Ok(Some(format!(
                "posting {} has a line on facility {}",
                posting.id, line.dimensions.facility
            )));
        }
        let Some(original) = &posting.reverses else {
            return Ok(None);
        };
        let rows = self
            .rows(
                "SELECT VALUE [<string>(reverses ?? '')] FROM type::record('ledger_posting', $id)",
                vec![("id", original.clone())],
            )
            .await
            .map_err(WriteError::Store)?;
        if rows.first().is_none_or(|row| !row_text(row).is_empty()) {
            return Ok(Some(format!(
                "posting {original} is not held here, or is itself a reversal"
            )));
        }
        let reversals = self
            .rows(
                "SELECT VALUE [<string>record::id(id)] FROM ledger_posting WHERE reverses = $id",
                vec![("id", original.clone())],
            )
            .await
            .map_err(WriteError::Store)?;
        if !reversals.is_empty() {
            return Ok(Some(format!("posting {original} is already reversed")));
        }
        Ok(None)
    }

    /// Why central refuses this copy of `payment`, if it does.
    async fn payment_refused(&mut self, payment: &Payment) -> Result<Option<String>, WriteError> {
        let held = self.payment(&payment.id).await.map_err(WriteError::Store)?;
        if let Err(refused) = payment.may_follow(held.as_ref()) {
            return Ok(Some(format!("payment {}: {refused}", payment.id)));
        }
        let Some(reference) = &payment.reference else {
            return Ok(None);
        };
        let others = self
            .rows(
                "SELECT VALUE [<string>record::id(id)] FROM payment \
                 WHERE reference = $reference AND record::id(id) != $id",
                vec![("reference", reference.clone()), ("id", payment.id.clone())],
            )
            .await
            .map_err(WriteError::Store)?;
        Ok(others.first().map(|other| {
            format!(
                "provider reference {reference} is payment {}'s already",
                row_text(other)
            )
        }))
    }

    /// Replays a financial operation from `ledger`, sent by `facility` (`None` from central).
    /// `Err(Ok(reason))` when its body is not that ledger's record, or central's checks refuse it.
    pub(crate) async fn replay_finance(
        &mut self,
        ledger: &str,
        body: &Value,
        facility: Option<&str>,
    ) -> Result<(), Result<String, WriteError>> {
        let refused = |reason: String| Err(Ok(reason));
        let failed = |error: WriteError| Err(Err(error));
        let outcome = match ledger {
            CHARGE => match parse::<Charge>(body) {
                Ok(charge) => self.put_charge(&charge).await,
                Err(reason) => return refused(reason),
            },
            DEFERRAL => match parse::<Deferral>(body) {
                Ok(deferral) => self.add_deferral(&deferral).await,
                Err(reason) => return refused(reason),
            },
            SETTLEMENT => match parse::<Settlement>(body) {
                Ok(settlement) => self.add_settlement(&settlement).await,
                Err(reason) => return refused(reason),
            },
            WAIVER => match parse::<Waiver>(body) {
                Ok(waiver) => match self.held("billing_waiver", &waiver.id).await {
                    Ok(true) => Ok(()),
                    Ok(false) => self.add_waiver(&waiver).await,
                    Err(error) => return failed(error),
                },
                Err(reason) => return refused(reason),
            },
            POSTING => match parse::<Posting>(body) {
                Ok(posting) => match self.held("ledger_posting", &posting.id).await {
                    Ok(true) => Ok(()),
                    Ok(false) => match self.posting_refused(&posting, facility).await {
                        Ok(Some(reason)) => return refused(reason),
                        Ok(None) => self.append_posting(&posting).await,
                        Err(error) => return failed(error),
                    },
                    Err(error) => return failed(error),
                },
                Err(reason) => return refused(reason),
            },
            PAYMENT => match parse::<Payment>(body) {
                Ok(payment) => match self.payment_refused(&payment).await {
                    Ok(Some(reason)) => return refused(reason),
                    Ok(None) => self.put_payment(&payment).await,
                    Err(error) => return failed(error),
                },
                Err(reason) => return refused(reason),
            },
            SHIFT => match parse::<Shift>(body) {
                Ok(shift) => self.put_shift(&shift).await,
                Err(reason) => return refused(reason),
            },
            other => return refused(format!("no financial ledger {other}")),
        };
        outcome.map_err(Err)
    }
}
