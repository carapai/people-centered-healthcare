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
            "CREATE type::record('ledger_posting', $id) CONTENT { at: <datetime>$at, data: $data } \
             RETURN VALUE record::id(id)",
            vec![
                ("id", posting.id.clone()),
                ("at", posting.at.to_string()),
                ("data", encode(posting)?),
            ],
        )
        .await?;
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
             recorded_at: <datetime>$recorded_at, data: $data } RETURN VALUE record::id(id)",
            vec![
                ("id", payment.id.clone()),
                ("status", status_code(payment.status).to_owned()),
                ("shift", payment.shift.clone().unwrap_or_default()),
                ("recorded_at", payment.recorded_at.to_string()),
                ("data", encode(payment)?),
            ],
        )
        .await?;
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
