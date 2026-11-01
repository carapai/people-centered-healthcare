//! Receiving, requisitions and reorder suggestions (inventory-and-procurement.md § Receiving,
//! requisitions and reorder; #130).

use std::collections::BTreeMap;

use health_inventory::receiving::{
    GoodsReceived, ReceivingRefused, ReorderPolicy, Requisition, Suggestion, suggest,
};
use health_inventory::transfers::{Transfer, TransferLine};
use health_inventory::{Movement, MovementKind, Origin, Status, StockStore};
use health_persistence::Store;
use health_time::{SignedDuration, Timestamp};
use serde_json::{Value, json};

use crate::transfers::Step;
use crate::{ATTEMPTS, Attempt, Inventory, InventoryError, Sender};

impl<S> Inventory<S>
where
    S: Store,
    S::Tx: StockStore,
{
    /// Receives a goods received note in one transaction: a receipt of each line at its unit
    /// cost, and the note. A note is received once.
    pub async fn receive_goods(
        &self,
        note: &GoodsReceived,
        sender: &Sender,
        now: Timestamp,
    ) -> Result<(), InventoryError> {
        note.validate()?;
        for _ in 0..ATTEMPTS {
            let mut tx = self.store.begin().await?;
            let attempt = async {
                if tx.goods_received(&note.id).await?.is_some() {
                    return Err(Attempt::Fail(
                        ReceivingRefused::Invalid(format!("{} is already received", note.id))
                            .into(),
                    ));
                }
                for (index, line) in note.lines.iter().enumerate() {
                    let id = format!("{}-{index}", note.id);
                    let movement = Movement {
                        op_id: id.clone(),
                        id,
                        unit: line.unit(&note.store),
                        kind: MovementKind::Receipt,
                        quantity: line.quantity,
                        pack_size: line.pack_size,
                        unit_cost: Some(line.unit_cost),
                        occurred_at: now,
                        recorded_at: now,
                        source_document: format!("GoodsReceived/{}", note.id),
                        actor: sender.actor.clone(),
                        node: sender.node.clone(),
                        reason: None,
                        reverses: None,
                        emergency: false,
                    };
                    self.apply(
                        &mut tx,
                        &movement,
                        Origin::Authority {
                            may_emergency_dispense: false,
                        },
                    )
                    .await?;
                }
                tx.put_goods_received(note).await?;
                Ok::<(), Attempt>(())
            }
            .await;
            if let Some(done) = Self::settle(tx, attempt).await {
                return done;
            }
        }
        Err(InventoryError::Busy)
    }

    async fn load_requisition(&self, id: &str) -> Result<Requisition, InventoryError> {
        let mut tx = self.store.begin().await?;
        tx.requisition(id)
            .await?
            .ok_or_else(|| InventoryError::NotFound(format!("SupplyRequest/{id}")))
    }

    async fn save_requisition(&self, requisition: &Requisition) -> Result<(), InventoryError> {
        let mut tx = self.store.begin().await?;
        tx.put_requisition(requisition)
            .await
            .map_err(|e| InventoryError::Store(e.to_string()))?;
        health_persistence::Transaction::commit(tx)
            .await
            .map_err(|e| InventoryError::Store(e.to_string()))
    }

    /// Records a ward's requisition.
    pub async fn request(&self, requisition: &Requisition) -> Result<(), InventoryError> {
        self.save_requisition(requisition).await
    }

    /// The supplying store approves requisition `id` (`approve`) or refuses it.
    pub async fn decide_requisition(
        &self,
        id: &str,
        approve: bool,
        by: &str,
    ) -> Result<Requisition, InventoryError> {
        let mut requisition = self.load_requisition(id).await?;
        if approve {
            requisition.approve(by)?;
        } else {
            requisition.reject(by)?;
        }
        self.save_requisition(&requisition).await?;
        Ok(requisition)
    }

    /// Fulfils approved requisition `id` by dispatching `lines` (the batches picked) from the
    /// supplying store to the requesting one, as transfer `transfer_id`, in one transaction.
    pub async fn fulfil_requisition(
        &self,
        id: &str,
        transfer_id: &str,
        lines: Vec<TransferLine>,
        sender: &Sender,
        now: Timestamp,
    ) -> Result<(Requisition, Transfer), InventoryError> {
        let mut requisition = self.load_requisition(id).await?;
        let mut transfer = Transfer::new(
            transfer_id,
            &requisition.supplying_store,
            &requisition.requesting_store,
            None,
            lines,
        )?;
        requisition.fulfil(transfer_id)?;
        let legs = transfer.dispatch(&sender.actor)?;
        self.step(
            &transfer,
            Step {
                name: "out",
                legs: &legs,
                records: &[],
                requisition: Some(&requisition),
            },
            sender,
            now,
        )
        .await?;
        Ok((requisition, transfer))
    }

    /// Requisition `id`.
    pub async fn requisition(&self, id: &str) -> Result<Requisition, InventoryError> {
        self.load_requisition(id).await
    }

    /// The reorder suggestions for `store`: per product, usable stock on hand and what was
    /// issued or dispensed over the last `days` days, against `policy`.
    pub async fn reorder(
        &self,
        store: &str,
        policy: ReorderPolicy,
        days: u32,
        now: Timestamp,
    ) -> Result<Vec<Suggestion>, InventoryError> {
        let mut tx = self.store.begin().await?;
        let since = now - SignedDuration::from_hours(24 * i64::from(days));
        let mut products: BTreeMap<String, (i64, i64)> = BTreeMap::new();
        for unit in tx.units_of_store(store).await? {
            if unit.status != Status::Usable {
                continue;
            }
            let key = unit.key();
            let balance = tx.stock_balance(&key).await?.unwrap_or_default();
            let consumed: i64 = tx
                .movements(&key)
                .await?
                .iter()
                .filter(|m| {
                    matches!(m.kind, MovementKind::Issue | MovementKind::Dispense)
                        && m.occurred_at >= since
                        && m.occurred_at <= now
                })
                .map(|m| -m.quantity)
                .sum();
            let entry = products.entry(unit.product.clone()).or_default();
            entry.0 += balance.on_hand;
            entry.1 += consumed;
        }
        Ok(products
            .into_iter()
            .map(|(product, (on_hand, consumed))| {
                suggest(&product, on_hand, consumed, days, policy)
            })
            .collect())
    }
}

/// A requisition as FHIR SupplyRequests, one per product, grouped by the requisition's id.
pub fn supply_requests(requisition: &Requisition) -> Vec<Value> {
    let status = match requisition.status {
        health_inventory::receiving::RequisitionStatus::Requested => "active",
        health_inventory::receiving::RequisitionStatus::Approved => "active",
        health_inventory::receiving::RequisitionStatus::Fulfilled => "completed",
        health_inventory::receiving::RequisitionStatus::Rejected => "cancelled",
    };
    requisition
        .lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            json!({
                "resourceType": "SupplyRequest",
                "id": format!("{}-{index}", requisition.id),
                "status": status,
                "groupIdentifier": { "value": requisition.id },
                "item": { "concept": { "text": line.product } },
                "quantity": { "value": line.quantity },
                "deliverFrom": { "reference": requisition.supplying_store },
                "deliverTo": { "reference": requisition.requesting_store },
                "requester": { "display": requisition.requested_by },
            })
        })
        .collect()
}
