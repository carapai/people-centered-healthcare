//! Transfers and Reconciliation Records (inventory-and-procurement.md § Transfers; #129).
//!
//! Each step is one transaction: the transfer's state, every movement it makes (each decided by
//! the store's policy against its balance record, so a dispatch beyond what is available is
//! refused as a whole), and the Reconciliation Records for what was found short, damaged or in
//! excess on receipt.

use health_inventory::reconciliation::{ReconciliationRecord, RootCause};
use health_inventory::transfers::{Counted, Leg, Transfer, TransferStatus};
use health_inventory::{Movement, MovementKind, Origin, StockStore};
use health_persistence::Store;
use health_time::Timestamp;

use crate::{ATTEMPTS, Attempt, Inventory, InventoryError};

/// Who records a step, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sender {
    /// The user.
    pub actor: String,
    /// This node.
    pub node: String,
}

fn movement(
    transfer: &Transfer,
    step: &str,
    index: usize,
    leg: &Leg,
    sender: &Sender,
    now: Timestamp,
) -> Movement {
    let id = format!("{}-{step}-{index}", transfer.id);
    Movement {
        op_id: id.clone(),
        id,
        unit: leg.unit.clone(),
        kind: if leg.quantity < 0 {
            MovementKind::TransferOut
        } else {
            MovementKind::TransferIn
        },
        quantity: leg.quantity,
        pack_size: 1,
        unit_cost: None,
        occurred_at: now,
        recorded_at: now,
        source_document: format!("Transfer/{}", transfer.id),
        actor: sender.actor.clone(),
        node: sender.node.clone(),
        reason: transfer.direct.then(|| "direct transfer".to_owned()),
        reverses: None,
        emergency: false,
    }
}

impl<S> Inventory<S>
where
    S: Store,
    S::Tx: StockStore,
{
    async fn step(
        &self,
        transfer: &Transfer,
        step: &str,
        legs: &[Leg],
        records: &[ReconciliationRecord],
        sender: &Sender,
        now: Timestamp,
    ) -> Result<(), InventoryError> {
        for _ in 0..ATTEMPTS {
            let mut tx = self.store.begin().await?;
            let attempt = async {
                for (index, leg) in legs.iter().enumerate() {
                    let movement = movement(transfer, step, index, leg, sender, now);
                    self.apply(
                        &mut tx,
                        &movement,
                        Origin::Authority {
                            may_emergency_dispense: false,
                        },
                    )
                    .await?;
                }
                for record in records {
                    tx.put_reconciliation(record).await?;
                }
                tx.put_transfer(transfer).await?;
                Ok::<(), Attempt>(())
            }
            .await;
            if let Some(done) = Self::settle(tx, attempt).await {
                return done;
            }
        }
        Err(InventoryError::Busy)
    }

    async fn load(&self, id: &str) -> Result<Transfer, InventoryError> {
        let mut tx = self.store.begin().await?;
        tx.transfer(id)
            .await?
            .ok_or_else(|| InventoryError::NotFound(format!("Transfer/{id}")))
    }

    /// Sends `transfer`: a transfer-out of each line at the source, all or nothing.
    pub async fn dispatch(
        &self,
        mut transfer: Transfer,
        sender: &Sender,
        now: Timestamp,
    ) -> Result<Transfer, InventoryError> {
        let legs = transfer.dispatch(&sender.actor)?;
        self.step(&transfer, "out", &legs, &[], sender, now).await?;
        Ok(transfer)
    }

    /// Receives transfer `id` with what was counted: the transfer-ins, and a Reconciliation
    /// Record for each discrepancy.
    pub async fn receive(
        &self,
        id: &str,
        counted: Vec<Counted>,
        sender: &Sender,
        now: Timestamp,
    ) -> Result<(Transfer, Vec<ReconciliationRecord>), InventoryError> {
        let mut transfer = self.load(id).await?;
        let (legs, found) = transfer.receive(counted, &sender.actor)?;
        let records: Vec<ReconciliationRecord> = found
            .iter()
            .enumerate()
            .map(|(n, f)| {
                ReconciliationRecord::open(
                    &format!("{id}-{n}"),
                    &f.unit,
                    f.discrepancy,
                    f.quantity,
                    &format!("Transfer/{id}"),
                    now,
                )
            })
            .collect();
        self.step(&transfer, "in", &legs, &records, sender, now)
            .await?;
        Ok((transfer, records))
    }

    /// A one-step direct transfer within one facility, by the one user who sends and receives.
    pub async fn direct_transfer(
        &self,
        mut transfer: Transfer,
        sender: &Sender,
        now: Timestamp,
    ) -> Result<Transfer, InventoryError> {
        let legs = transfer.direct(&sender.actor)?;
        self.step(&transfer, "direct", &legs, &[], sender, now)
            .await?;
        Ok(transfer)
    }

    /// Records the root cause of Reconciliation Record `id`. A transfer whose records are all
    /// resolved is closed in the same transaction.
    pub async fn resolve(
        &self,
        id: &str,
        cause: RootCause,
        note: &str,
        by: &str,
        now: Timestamp,
    ) -> Result<ReconciliationRecord, InventoryError> {
        for _ in 0..ATTEMPTS {
            let mut tx = self.store.begin().await?;
            let attempt = async {
                let mut record = tx.reconciliation(id).await?.ok_or_else(|| {
                    Attempt::Fail(InventoryError::NotFound(format!("Reconciliation/{id}")))
                })?;
                record
                    .resolve(cause, note, by, now)
                    .map_err(|e| Attempt::Fail(e.into()))?;
                tx.put_reconciliation(&record).await?;
                if let Some(transfer_id) = record.source.strip_prefix("Transfer/") {
                    let others_open = tx
                        .open_reconciliations()
                        .await?
                        .iter()
                        .any(|r| r.source == record.source && r.id != record.id);
                    if let Some(mut transfer) = tx.transfer(transfer_id).await?
                        && transfer.status == TransferStatus::Reconciling
                        && !others_open
                    {
                        transfer.close(true)?;
                        tx.put_transfer(&transfer).await?;
                    }
                }
                Ok::<_, Attempt>(record)
            }
            .await;
            if let Some(done) = Self::settle(tx, attempt).await {
                return done;
            }
        }
        Err(InventoryError::Busy)
    }

    /// The Reconciliation Records still open, oldest first.
    pub async fn open_reconciliations(&self) -> Result<Vec<ReconciliationRecord>, InventoryError> {
        let mut tx = self.store.begin().await?;
        Ok(tx.open_reconciliations().await?)
    }

    /// Transfer `id`.
    pub async fn transfer(&self, id: &str) -> Result<Transfer, InventoryError> {
        self.load(id).await
    }
}
