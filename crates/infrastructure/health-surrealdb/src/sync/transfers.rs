//! Inter-facility transfers through central (#247; inventory-and-procurement.md § Transfers):
//! each side records its own movements at its own Authority Node; the transfer document goes up
//! from both and down to the destination; central matches the source's dispatch with the
//! destination's receipt.
//!
//! - **Queued with the save.** Saving a transfer between facilities queues it for every upstream
//!   (`SurrealTx::put_transfer`).
//! - **At central** ([`SurrealStore::receive_transfer`]): the source's lines are authoritative;
//!   from the destination only its progress is taken (status, who received, what was counted).
//!   Once received, the counts are matched against the source's lines with
//!   `transfers::discrepancies`, and each discrepancy is a Reconciliation Record at central,
//!   against the destination's Stock Unit. A receipt with no dispatch is recorded as such.
//! - **Down to the destination** ([`SurrealStore::inbound_transfers`],
//!   [`SurrealStore::accept_transfer`]): the transfers in transit addressed to its facility, kept
//!   unless it holds the transfer already.

use health_inventory::StockStore as _;
use health_inventory::reconciliation::ReconciliationRecord;
use health_inventory::transfers::{Transfer, TransferStatus, discrepancies};
use health_persistence::{Store as _, StoreError, Transaction as _, WriteError};
use health_time::Timestamp;
use serde_json::Value;
use surrealdb::types::SurrealValue;

use crate::SurrealStore;
use crate::store::{SurrealTx, unavailable, write_error};

/// How far a transfer has come, to keep the later of two copies.
fn rank(status: TransferStatus) -> u8 {
    match status {
        TransferStatus::Picked => 0,
        TransferStatus::InTransit => 1,
        TransferStatus::Received => 2,
        TransferStatus::Reconciling => 3,
        TransferStatus::Closed => 4,
    }
}

/// What central made of a transfer's receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferMatch {
    /// Still in transit, or a transfer within one facility.
    Pending,
    /// Received as dispatched.
    Matched,
    /// Received with discrepancies: the Reconciliation Records opened, by id.
    Mismatch(Vec<String>),
    /// A receipt for which no dispatch has arrived.
    Undispatched,
}

/// Why central refused a transfer.
#[derive(Debug, thiserror::Error)]
pub enum TransferRefused {
    /// Neither end of it is the sending facility.
    #[error("transfer {0} is not this facility's")]
    NotTheirs(String),
    /// A transfer within one facility does not go to central.
    #[error("transfer {0} is within one facility")]
    WithinFacility(String),
    /// The store failed.
    #[error(transparent)]
    Store(#[from] WriteError),
}

impl From<StoreError> for TransferRefused {
    fn from(error: StoreError) -> Self {
        Self::Store(WriteError::Store(error))
    }
}

impl SurrealTx {
    /// Queues transfer `id` for every upstream (#247).
    pub(crate) async fn queue_transfer(&mut self, id: &str) -> Result<(), WriteError> {
        self.tx()?
            .query(
                "FOR $peer IN (SELECT VALUE record::id(id) FROM sync_upstream) { \
                   UPSERT type::record('sync_transfer_outbox', [$peer, $id]) \
                     SET queued_at = time::now(); \
                 };",
            )
            .bind(("id", id.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?;
        Ok(())
    }
}

#[derive(SurrealValue)]
struct StateRow {
    source_seen: bool,
}

impl SurrealStore {
    async fn transfers_where(
        &self,
        sql: &str,
        peer: Option<&str>,
    ) -> Result<Vec<Transfer>, StoreError> {
        let mut query = self.connection.db().query(sql.to_owned());
        if let Some(peer) = peer {
            query = query.bind(("peer", peer.to_owned()));
        }
        let rows: Vec<String> = query
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        rows.iter()
            .map(|data| {
                serde_json::from_str(data)
                    .map_err(|_| StoreError::Unavailable("stock_transfer row is invalid".into()))
            })
            .collect()
    }

    /// The transfers between facilities queued for `peer`.
    pub async fn queued_transfers(&self, peer: &str) -> Result<Vec<Transfer>, StoreError> {
        let transfers = self
            .transfers_where(
                "SELECT VALUE data FROM stock_transfer WHERE record::id(id) INSIDE \
                 (SELECT VALUE record::id(id)[1] FROM sync_transfer_outbox:[$peer]..[$peer, {}])",
                Some(peer),
            )
            .await?;
        Ok(transfers
            .into_iter()
            .filter(|t| t.between.is_some())
            .collect())
    }

    /// Takes transfers off `peer`'s queue once the peer has them.
    pub async fn transfers_sent(&self, peer: &str, ids: &[String]) -> Result<(), StoreError> {
        self.connection
            .db()
            .query(
                "FOR $id IN $ids { DELETE type::record('sync_transfer_outbox', [$peer, $id]); };",
            )
            .bind(("peer", peer.to_owned()))
            .bind(("ids", ids.to_vec()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?;
        Ok(())
    }

    /// Keeps a transfer that `facility` sent, and matches it once received (#247). The source's
    /// lines are authoritative; from the destination only its progress is taken.
    pub async fn receive_transfer(
        &self,
        facility: &str,
        incoming: Transfer,
    ) -> Result<TransferMatch, TransferRefused> {
        let Some((from, to)) = incoming.between.clone() else {
            return Err(TransferRefused::WithinFacility(incoming.id));
        };
        let from_source = from == facility;
        if !from_source && to != facility {
            return Err(TransferRefused::NotTheirs(incoming.id));
        }
        let mut tx = self.begin().await.map_err(WriteError::Store)?;
        let existing = tx.transfer(&incoming.id).await.map_err(WriteError::Store)?;
        let state: Option<StateRow> = tx
            .tx()?
            .query("SELECT source_seen FROM ONLY type::record('sync_transfer_state', $id)")
            .bind(("id", incoming.id.clone()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?
            .take(0)
            .map_err(write_error)?;
        let source_seen = from_source || state.is_some_and(|s| s.source_seen);
        let merged = match existing {
            None => incoming,
            Some(held) => {
                let (lines, between, from_store, to_store) = if from_source {
                    (
                        incoming.lines.clone(),
                        incoming.between.clone(),
                        incoming.from_store.clone(),
                        incoming.to_store.clone(),
                    )
                } else {
                    (
                        held.lines.clone(),
                        held.between.clone(),
                        held.from_store.clone(),
                        held.to_store.clone(),
                    )
                };
                let progress = if rank(incoming.status) >= rank(held.status) {
                    &incoming
                } else {
                    &held
                };
                Transfer {
                    lines,
                    between,
                    from_store,
                    to_store,
                    status: progress.status,
                    received_by: progress.received_by.clone(),
                    counted: progress.counted.clone(),
                    dispatched_by: held
                        .dispatched_by
                        .clone()
                        .or(incoming.dispatched_by.clone()),
                    ..held
                }
            }
        };
        tx.put_transfer(&merged).await?;
        tx.tx()?
            .query(
                "UPSERT type::record('sync_transfer_state', $id) SET source_seen = $seen, \
                 arrived_at = arrived_at ?? time::now()",
            )
            .bind(("id", merged.id.clone()))
            .bind(("seen", source_seen))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?;
        let received = rank(merged.status) >= rank(TransferStatus::Received);
        let outcome = if !received {
            TransferMatch::Pending
        } else if !source_seen {
            record_match(&mut tx, &merged.id, "undispatched", &[]).await?;
            TransferMatch::Undispatched
        } else {
            let found = discrepancies(&merged.lines, &merged.to_store, &merged.counted);
            let now = Timestamp::now();
            let mut opened = Vec::new();
            for (n, f) in found.iter().enumerate() {
                let id = format!("{}-central-{n}", merged.id);
                tx.put_reconciliation(&ReconciliationRecord::open(
                    &id,
                    &f.unit,
                    f.discrepancy,
                    f.quantity,
                    &format!("Transfer/{}", merged.id),
                    now,
                ))
                .await?;
                opened.push(id);
            }
            let detail: Vec<Value> = found
                .iter()
                .map(|f| {
                    serde_json::json!({"unit": f.unit, "quantity": f.quantity, "discrepancy": format!("{:?}", f.discrepancy)})
                })
                .collect();
            if opened.is_empty() {
                record_match(&mut tx, &merged.id, "matched", &detail).await?;
                TransferMatch::Matched
            } else {
                record_match(&mut tx, &merged.id, "mismatch", &detail).await?;
                TransferMatch::Mismatch(opened)
            }
        };
        tx.commit().await?;
        Ok(outcome)
    }

    /// The transfers in transit addressed to `facility`: what its edge pulls to receive.
    pub async fn inbound_transfers(&self, facility: &str) -> Result<Vec<Transfer>, StoreError> {
        let all = self
            .transfers_where("SELECT VALUE data FROM stock_transfer", None)
            .await?;
        Ok(all
            .into_iter()
            .filter(|t| {
                t.status == TransferStatus::InTransit
                    && t.between.as_ref().is_some_and(|(_, to)| to == facility)
            })
            .collect())
    }

    /// Keeps a transfer addressed to this facility, unless it holds the transfer already; it is
    /// not queued back. Returns whether it was new.
    pub async fn accept_transfer(&self, transfer: &Transfer) -> Result<bool, StoreError> {
        let data =
            serde_json::to_string(transfer).map_err(|e| StoreError::Unavailable(e.to_string()))?;
        let new: Option<bool> = self
            .connection
            .db()
            .query(
                "BEGIN; \
                 LET $new = array::len((SELECT id FROM type::record('stock_transfer', $id))) = 0; \
                 IF $new { CREATE type::record('stock_transfer', $id) CONTENT { data: $data }; }; \
                 RETURN $new; \
                 COMMIT;",
            )
            .bind(("id", transfer.id.clone()))
            .bind(("data", data))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(3)
            .map_err(unavailable)?;
        Ok(new.unwrap_or(false))
    }

    /// Transfers dispatched to another facility and not received after `days` days, and receipts
    /// whose dispatch never arrived: the ids, for the hourly alert at central.
    pub async fn stale_transfers(
        &self,
        days: i64,
    ) -> Result<(Vec<String>, Vec<String>), StoreError> {
        let before = Timestamp::now() - health_time::SignedDuration::from_hours(24 * days);
        let in_transit: Vec<String> = self
            .connection
            .db()
            .query(format!(
                "SELECT VALUE record::id(id) FROM sync_transfer_state \
                 WHERE source_seen = true AND arrived_at < d'{before}'"
            ))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        let mut unreceived = Vec::new();
        let mut tx = self.begin().await?;
        for id in in_transit {
            if tx
                .transfer(&id)
                .await?
                .is_some_and(|t| t.status == TransferStatus::InTransit)
            {
                unreceived.push(id);
            }
        }
        drop(tx);
        let undispatched: Vec<String> = self
            .connection
            .db()
            .query("SELECT VALUE record::id(id) FROM transfer_match WHERE status = 'undispatched'")
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        Ok((unreceived, undispatched))
    }
}

async fn record_match(
    tx: &mut SurrealTx,
    id: &str,
    status: &str,
    found: &[Value],
) -> Result<(), WriteError> {
    tx.tx()?
        .query(
            "UPSERT type::record('transfer_match', $id) SET status = $status, found = $found, \
             checked_at = time::now()",
        )
        .bind(("id", id.to_owned()))
        .bind(("status", status.to_owned()))
        .bind(("found", Value::Array(found.to_vec()).to_string()))
        .await
        .and_then(surrealdb::IndexedResults::check)
        .map_err(write_error)?;
    Ok(())
}
