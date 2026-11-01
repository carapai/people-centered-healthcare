//! Stock counts (inventory-and-procurement.md § Stock counts; #129): started without freezing
//! the store, counted, submitted, and approved in one transaction that posts the variances,
//! raises a Reconciliation Record for each, and closes, with its root cause, any record of a
//! flagged negative balance the count resolves (GSDP §13.2).

use health_inventory::counts::{CountMode, StockCount, Variance};
use health_inventory::reconciliation::{Discrepancy, ReconciliationRecord, RootCause};
use health_inventory::{Movement, MovementKind, Origin, StockStore, StockUnit};
use health_persistence::{Store, Transaction};
use health_time::Timestamp;
use serde_json::{Value, json};

use crate::{ATTEMPTS, Attempt, Inventory, InventoryError, Sender};

/// The root cause of a negative-balance Reconciliation Record a count resolves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cause {
    /// The record.
    pub record: String,
    /// Why.
    pub cause: RootCause,
    /// What was found.
    pub note: String,
}

impl<S> Inventory<S>
where
    S: Store,
    S::Tx: StockStore,
{
    async fn load_count(&self, id: &str) -> Result<StockCount, InventoryError> {
        let mut tx = self.store.begin().await?;
        tx.count(id)
            .await?
            .ok_or_else(|| InventoryError::NotFound(format!("Count/{id}")))
    }

    async fn save_count(&self, count: &StockCount) -> Result<(), InventoryError> {
        let mut tx = self.store.begin().await?;
        tx.put_count(count)
            .await
            .map_err(|e| InventoryError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| InventoryError::Store(e.to_string()))
    }

    /// Starts a count of `store` at `at`: every unit with a balance record, or `units` for a
    /// cycle count. Nothing is frozen.
    pub async fn start_count(
        &self,
        id: &str,
        store: &str,
        mode: CountMode,
        units: Option<Vec<StockUnit>>,
        at: Timestamp,
    ) -> Result<StockCount, InventoryError> {
        let mut tx = self.store.begin().await?;
        let units = match units {
            Some(units) => units,
            None => tx.units_of_store(store).await?,
        };
        let mut lines = Vec::with_capacity(units.len());
        for unit in units {
            let balance = tx.stock_balance(&unit.key()).await?.unwrap_or_default();
            lines.push((unit, balance.on_hand));
        }
        drop(tx);
        let count = StockCount::start(id, store, mode, lines, at)?;
        self.save_count(&count).await?;
        Ok(count)
    }

    /// Records what was counted on `line` of count `id`, and the reason for any variance.
    pub async fn record_count(
        &self,
        id: &str,
        line: usize,
        counted: i64,
        reason: Option<&str>,
    ) -> Result<StockCount, InventoryError> {
        let mut count = self.load_count(id).await?;
        count.record(line, counted, reason)?;
        self.save_count(&count).await?;
        Ok(count)
    }

    /// Submits count `id` for approval.
    pub async fn submit_count(&self, id: &str, by: &str) -> Result<StockCount, InventoryError> {
        let mut count = self.load_count(id).await?;
        count.submit(by)?;
        self.save_count(&count).await?;
        Ok(count)
    }

    /// Count `id`.
    pub async fn count(&self, id: &str) -> Result<StockCount, InventoryError> {
        self.load_count(id).await
    }

    /// Approves count `id`. In one transaction: a count-variance movement for each variance, a
    /// Reconciliation Record for each, and the open negative-balance records of the counted
    /// units resolved with their `causes` (each one needs a cause) and their units unflagged.
    pub async fn approve_count(
        &self,
        id: &str,
        sender: &Sender,
        supervisor: bool,
        threshold: i64,
        causes: &[Cause],
        now: Timestamp,
    ) -> Result<(StockCount, Vec<Variance>), InventoryError> {
        for _ in 0..ATTEMPTS {
            let mut tx = self.store.begin().await?;
            let attempt = async {
                let mut count = tx.count(id).await?.ok_or_else(|| {
                    Attempt::Fail(InventoryError::NotFound(format!("Count/{id}")))
                })?;
                let mut since: Vec<Movement> = Vec::new();
                for line in &count.lines {
                    since.extend(tx.movements(&line.unit.key()).await?);
                }
                let variances = count.approve(&sender.actor, supervisor, threshold, &since)?;
                for v in &variances {
                    let movement = Movement {
                        id: format!("{id}-variance-{}", v.line),
                        op_id: format!("{id}-variance-{}", v.line),
                        unit: v.unit.clone(),
                        kind: MovementKind::CountVariance,
                        quantity: v.variance,
                        pack_size: 1,
                        unit_cost: None,
                        occurred_at: now,
                        recorded_at: now,
                        source_document: format!("Count/{id}"),
                        actor: sender.actor.clone(),
                        node: sender.node.clone(),
                        reason: count.lines[v.line].reason.clone(),
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
                    tx.put_reconciliation(&ReconciliationRecord::open(
                        &format!("{id}-{}", v.line),
                        &v.unit.key(),
                        Discrepancy::CountVariance,
                        v.variance,
                        &format!("Count/{id}"),
                        now,
                    ))
                    .await?;
                }
                // The count settles the negative balances of what it counted.
                let counted: Vec<String> = count.lines.iter().map(|l| l.unit.key()).collect();
                for mut record in tx.open_reconciliations().await? {
                    if record.discrepancy != Discrepancy::Negative
                        || !counted.contains(&record.unit)
                    {
                        continue;
                    }
                    let Some(cause) = causes.iter().find(|c| c.record == record.id) else {
                        return Err(Attempt::Fail(InventoryError::CauseRequired(record.id)));
                    };
                    record
                        .resolve(cause.cause, &cause.note, &sender.actor, now)
                        .map_err(|e| Attempt::Fail(e.into()))?;
                    tx.put_reconciliation(&record).await?;
                }
                for line in &count.lines {
                    let key = line.unit.key();
                    if let Some(mut balance) = tx.stock_balance(&key).await?
                        && balance.flagged
                        && balance.on_hand >= 0
                    {
                        balance.flagged = false;
                        tx.put_stock_balance(&line.unit, &balance).await?;
                    }
                }
                tx.put_count(&count).await?;
                Ok::<_, Attempt>((count, variances))
            }
            .await;
            if let Some(done) = Self::settle(tx, attempt).await {
                return done;
            }
        }
        Err(InventoryError::Busy)
    }
}

/// The count as a FHIR `InventoryReport` snapshot of its store: one item per Stock Unit, with
/// the counted quantity and the unit's batch, expiry and funding source.
pub fn inventory_report(count: &StockCount) -> Value {
    let items: Vec<Value> = count
        .lines
        .iter()
        .filter_map(|line| {
            let counted = line.counted?;
            Some(json!({
                "category": { "text": format!("{:?}", line.unit.status).to_lowercase() },
                "quantity": { "value": counted },
                "item": { "concept": { "text": line.unit.product } },
                "extension": [
                    { "url": "urn:health:extension/stock-batch", "valueString": line.unit.batch },
                    { "url": "urn:health:extension/stock-expiry", "valueDate": line.unit.expiry },
                    { "url": "urn:health:extension/funding-source", "valueString": line.unit.funding_source },
                ],
            }))
        })
        .collect();
    json!({
        "resourceType": "InventoryReport",
        "id": count.id,
        "status": if count.status == health_inventory::counts::CountStatus::Approved { "active" } else { "draft" },
        "countType": "snapshot",
        "reportedDateTime": count.started_at.to_string(),
        "inventoryListing": [{
            "location": { "reference": count.store },
            "countingDateTime": count.started_at.to_string(),
            "item": items,
        }],
    })
}
