//! The facility's financial records go up as operations (#81; write-authority.md: charges,
//! invoices and local payments, up as operations; reversals only, never edits). Each write logs
//! its record, and the receiver replays it through the same store method, so central's copy
//! follows the facility's in commit order. Records created with an id are not created twice;
//! the rest are kept once by the push's receipts.

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

    /// Replays a financial operation from `ledger`. `Err(Ok(reason))` when its body is not that
    /// ledger's record.
    pub(crate) async fn replay_finance(
        &mut self,
        ledger: &str,
        body: &Value,
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
                    Ok(false) => self.append_posting(&posting).await,
                    Err(error) => return failed(error),
                },
                Err(reason) => return refused(reason),
            },
            PAYMENT => match parse::<Payment>(body) {
                Ok(payment) => self.put_payment(&payment).await,
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
