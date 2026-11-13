//! The financial ledger's postings, payments and cashier shifts (migration `0018_payments`):
//! `health_financial_ledger::PostingStore` and `health_payments::PaymentStore` over the
//! transaction.

use health_financial_ledger::{Posting, PostingStore};
use health_payments::{Payment, PaymentStore, Shift, ShiftStatus, Status};
use health_persistence::{StoreError, WriteError};
use serde::de::DeserializeOwned;

use crate::store::SurrealTx;

fn invalid() -> StoreError {
    StoreError::Unavailable("a payments row has an invalid shape".into())
}

fn encode<T: serde::Serialize>(value: &T) -> Result<String, WriteError> {
    serde_json::to_string(value)
        .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))
}

fn decode<T: DeserializeOwned>(rows: Vec<Vec<String>>) -> Result<Vec<T>, StoreError> {
    rows.into_iter()
        .filter_map(|r| r.into_iter().next())
        .map(|data| serde_json::from_str(&data).map_err(|_| invalid()))
        .collect()
}

fn status_code(status: Status) -> &'static str {
    match status {
        Status::Pending => "pending",
        Status::PendingUnverified => "pending-unverified",
        Status::Posted => "posted",
        Status::Voided => "voided",
        Status::Expired => "expired",
        Status::Reversed => "reversed",
    }
}

impl PostingStore for SurrealTx {
    async fn append_posting(&mut self, posting: &Posting) -> Result<(), WriteError> {
        self.run(
            "CREATE type::record('ledger_posting', $id) CONTENT { at: <datetime>$at, data: $data, \
             reverses: IF $reverses = '' THEN NONE ELSE $reverses END } RETURN VALUE record::id(id)",
            vec![
                ("id", posting.id.clone()),
                ("at", posting.at.to_string()),
                ("reverses", posting.reverses.clone().unwrap_or_default()),
                ("data", encode(posting)?),
            ],
        )
        .await?;
        self.log_op(
            crate::sync::finance::POSTING,
            &posting.id,
            &encode(posting)?,
        );
        Ok(())
    }

    async fn postings(&mut self) -> Result<Vec<Posting>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM ledger_posting ORDER BY at, id",
                vec![],
            )
            .await?;
        decode(rows)
    }
}

impl PaymentStore for SurrealTx {
    async fn put_payment(&mut self, payment: &Payment) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('payment', $id) CONTENT { status: $status, \
             shift: IF $shift = '' THEN NONE ELSE $shift END, \
             reference: IF $reference = '' THEN NONE ELSE $reference END, \
             recorded_at: <datetime>$recorded_at, data: $data } RETURN VALUE record::id(id)",
            vec![
                ("id", payment.id.clone()),
                ("reference", payment.reference.clone().unwrap_or_default()),
                ("status", status_code(payment.status).to_owned()),
                ("shift", payment.shift.clone().unwrap_or_default()),
                ("recorded_at", payment.recorded_at.to_string()),
                ("data", encode(payment)?),
            ],
        )
        .await?;
        self.log_op(
            crate::sync::finance::PAYMENT,
            &payment.id,
            &encode(payment)?,
        );
        Ok(())
    }

    async fn payment(&mut self, id: &str) -> Result<Option<Payment>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('payment', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        Ok(decode(rows)?.into_iter().next())
    }

    async fn payments_of_shift(&mut self, shift: &str) -> Result<Vec<Payment>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM payment WHERE shift = $shift ORDER BY recorded_at, id",
                vec![("shift", shift.to_owned())],
            )
            .await?;
        decode(rows)
    }

    async fn payments_with_references(
        &mut self,
        references: &[String],
    ) -> Result<Vec<Payment>, StoreError> {
        if references.is_empty() {
            return Ok(Vec::new());
        }
        // Binds are strings: the references cross joined by the unit separator, which no
        // reference holds, and are split back into a list.
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM payment \
                 WHERE reference IN string::split($wanted, $separator) ORDER BY recorded_at, id",
                vec![
                    ("wanted", references.join("\u{1f}")),
                    ("separator", "\u{1f}".to_owned()),
                ],
            )
            .await?;
        decode(rows)
    }

    async fn pending_payments(&mut self) -> Result<Vec<Payment>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM payment WHERE status IN ['pending', 'pending-unverified'] \
                 ORDER BY recorded_at, id",
                vec![],
            )
            .await?;
        decode(rows)
    }

    async fn put_shift(&mut self, shift: &Shift) -> Result<(), WriteError> {
        let status = match shift.status {
            ShiftStatus::Open => "open",
            ShiftStatus::AwaitingApproval => "awaiting-approval",
            ShiftStatus::Settled => "settled",
        };
        self.run(
            "UPSERT type::record('cash_shift', $id) CONTENT { till: $till, status: $status, data: $data } \
             RETURN VALUE record::id(id)",
            vec![
                ("id", shift.id.clone()),
                ("till", shift.till.clone()),
                ("status", status.to_owned()),
                ("data", encode(shift)?),
            ],
        )
        .await?;
        self.log_op(crate::sync::finance::SHIFT, &shift.id, &encode(shift)?);
        Ok(())
    }

    async fn shift(&mut self, id: &str) -> Result<Option<Shift>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('cash_shift', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        Ok(decode(rows)?.into_iter().next())
    }

    async fn open_shift(&mut self, till: &str) -> Result<Option<Shift>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM cash_shift WHERE till = $till AND status = 'open' LIMIT 1",
                vec![("till", till.to_owned())],
            )
            .await?;
        Ok(decode(rows)?.into_iter().next())
    }
}

/// One entry of the payment exceptions queue (#124): what a provider statement or answer shows
/// that a person must look at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentException {
    /// `kind|reference`.
    pub id: String,
    /// `mismatched`, `unknown` or `amount`.
    pub kind: String,
    /// The provider reference.
    pub reference: String,
    /// The payment, when one has the reference.
    pub payment: Option<String>,
    /// The amount recorded here, if any.
    pub recorded: Option<String>,
    /// The amount the provider shows, if any.
    pub provider: Option<String>,
    /// `open` or `resolved`.
    pub status: String,
    /// When it was opened.
    pub opened_at: String,
    /// Who resolved it, and how.
    pub resolution: Option<(String, String)>,
}

impl crate::store::SurrealStore {
    /// Opens the exception `kind` for `reference`, unless it is already in the queue.
    pub async fn open_payment_exception(
        &self,
        kind: &str,
        reference: &str,
        payment: Option<&str>,
        recorded: Option<&str>,
        provider: Option<&str>,
    ) -> Result<(), StoreError> {
        self.connection
            .db()
            .query(
                "IF (SELECT VALUE id FROM ONLY type::record('payment_exception', $id)) = NONE { \
                 CREATE type::record('payment_exception', $id) SET kind = $kind, \
                 reference = $reference, payment = $payment, recorded = $recorded, \
                 provider = $provider, status = 'open', opened_at = time::now(); };",
            )
            .bind(("id", format!("{kind}|{reference}")))
            .bind(("kind", kind.to_owned()))
            .bind(("reference", reference.to_owned()))
            .bind(("payment", payment.map(str::to_owned)))
            .bind(("recorded", recorded.map(str::to_owned)))
            .bind(("provider", provider.map(str::to_owned)))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(crate::store::unavailable)?;
        Ok(())
    }

    /// Records that the provider request `reference` (to `provider`) is for `facility`'s
    /// `payment`: written before the request goes out.
    pub async fn put_provider_request(
        &self,
        reference: &str,
        facility: &str,
        payment: &str,
        provider: &str,
    ) -> Result<(), StoreError> {
        self.connection
            .db()
            .query(
                "CREATE type::record('provider_request', $reference) SET facility = $facility, \
                 payment = $payment, provider = $provider, requested_at = time::now();",
            )
            .bind(("reference", reference.to_owned()))
            .bind(("facility", facility.to_owned()))
            .bind(("payment", payment.to_owned()))
            .bind(("provider", provider.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(crate::store::unavailable)?;
        Ok(())
    }

    /// The facility whose payment the provider request `reference` is for, if any.
    pub async fn provider_request_facility(
        &self,
        reference: &str,
    ) -> Result<Option<String>, StoreError> {
        self.connection
            .db()
            .query("SELECT VALUE facility FROM ONLY type::record('provider_request', $reference)")
            .bind(("reference", reference.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(crate::store::unavailable)?
            .take(0)
            .map_err(crate::store::unavailable)
    }

    /// The exceptions still open, oldest first.
    pub async fn open_payment_exceptions(&self) -> Result<Vec<PaymentException>, StoreError> {
        let rows: Vec<Vec<Option<String>>> = self
            .connection
            .db()
            .query(
                "SELECT VALUE [record::id(id), kind, reference, payment, recorded, provider, \
                 status, <string>opened_at, resolved_by, note] FROM payment_exception \
                 WHERE status = 'open' ORDER BY opened_at",
            )
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(crate::store::unavailable)?
            .take(0)
            .map_err(crate::store::unavailable)?;
        rows.into_iter()
            .map(|row| {
                let [
                    id,
                    kind,
                    reference,
                    payment,
                    recorded,
                    provider,
                    status,
                    opened_at,
                    by,
                    note,
                ]: [Option<String>; 10] = row.try_into().map_err(|_| invalid())?;
                Ok(PaymentException {
                    id: id.ok_or_else(invalid)?,
                    kind: kind.ok_or_else(invalid)?,
                    reference: reference.ok_or_else(invalid)?,
                    payment,
                    recorded,
                    provider,
                    status: status.ok_or_else(invalid)?,
                    opened_at: opened_at.ok_or_else(invalid)?,
                    resolution: by.zip(note),
                })
            })
            .collect()
    }

    /// Resolves the open exception `id`, by `by`, with `note`; false if no such open one.
    pub async fn resolve_payment_exception(
        &self,
        id: &str,
        by: &str,
        note: &str,
    ) -> Result<bool, StoreError> {
        let resolved: Vec<String> = self
            .connection
            .db()
            .query(
                "UPDATE type::record('payment_exception', $id) SET status = 'resolved', \
                 resolved_by = $by, note = $note WHERE status = 'open' RETURN VALUE record::id(id)",
            )
            .bind(("id", id.to_owned()))
            .bind(("by", by.to_owned()))
            .bind(("note", note.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(crate::store::unavailable)?
            .take(0)
            .map_err(crate::store::unavailable)?;
        Ok(!resolved.is_empty())
    }
}
