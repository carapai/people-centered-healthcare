//! Inventory use cases (inventory-and-procurement.md § Balances and concurrency; ADR 0034).
//!
//! - **[`Inventory::record`]** records one movement in one transaction. It reads the Stock Unit's
//!   balance record, decides with the store's [`Policy`], appends the movement and writes the
//!   balance. Two concurrent issues of the last pack both write the same balance record, so one
//!   conflicts and is rerun; the rerun re-reads the balance and is refused for insufficient stock.
//! - **[`Inventory::change_status`]** moves stock between statuses (usable → quarantine) in one
//!   transaction: out of one Stock Unit and into the other.
//! - **[`Inventory::nightly_check`]** rebuilds each balance from the ledger and reports drift.

use health_inventory::counts::CountRefused;
use health_inventory::receiving::ReceivingRefused;
use health_inventory::reconciliation::{Discrepancy, ReconciliationRecord, ResolveRefused};
use health_inventory::transfers::TransferRefused;
use health_inventory::{
    Balance, Decision, Drift, Movement, MovementKind, Origin, Policy, Refused, Status, StockStore,
    rebuild,
};
use health_persistence::{Store, StoreError, Transaction, WriteError};

mod counts;
mod receiving;
mod transfers;

pub use counts::{Cause, inventory_report};
pub use receiving::{OnHand, supply_requests};
pub use transfers::Sender;

/// Why a movement was not recorded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InventoryError {
    /// The policy refused it.
    #[error(transparent)]
    Refused(#[from] Refused),
    /// Concurrent writers kept winning; the caller may try again.
    #[error("the stock unit is busy; try again")]
    Busy,
    /// A transfer step was refused.
    #[error(transparent)]
    Transfer(#[from] TransferRefused),
    /// A Reconciliation Record could not be resolved.
    #[error(transparent)]
    Resolve(#[from] ResolveRefused),
    /// A goods received note or requisition was refused.
    #[error(transparent)]
    Receiving(#[from] ReceivingRefused),
    /// A count step was refused.
    #[error(transparent)]
    Count(#[from] CountRefused),
    /// A count that resolves a flagged negative balance needs its root cause (GSDP §13.2).
    #[error("reconciliation record {0} needs its root cause")]
    CauseRequired(String),
    /// No such transfer or Reconciliation Record.
    #[error("{0} is not known")]
    NotFound(String),
    /// The store failed.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for InventoryError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

/// One attempt's failure: a conflict reruns the use case.
pub(crate) enum Attempt {
    Conflict,
    Fail(InventoryError),
}

impl From<WriteError> for Attempt {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::Conflict => Self::Conflict,
            other => Self::Fail(InventoryError::Store(other.to_string())),
        }
    }
}

impl From<StoreError> for Attempt {
    fn from(error: StoreError) -> Self {
        Self::Fail(error.into())
    }
}

impl From<Refused> for Attempt {
    fn from(refused: Refused) -> Self {
        Self::Fail(refused.into())
    }
}

impl From<ReceivingRefused> for Attempt {
    fn from(refused: ReceivingRefused) -> Self {
        Self::Fail(refused.into())
    }
}

impl From<CountRefused> for Attempt {
    fn from(refused: CountRefused) -> Self {
        Self::Fail(refused.into())
    }
}

impl From<TransferRefused> for Attempt {
    fn from(refused: TransferRefused) -> Self {
        Self::Fail(refused.into())
    }
}

/// How many times a use case is rerun after losing a write-write conflict.
const ATTEMPTS: u32 = 8;

/// The inventory of one store (or a node's stores) over `store`.
pub struct Inventory<S> {
    store: S,
    policy: Policy,
}

impl<S> Inventory<S>
where
    S: Store,
    S::Tx: StockStore,
{
    /// The use cases over `store`, under the store's negative-stock `policy`.
    pub fn new(store: S, policy: Policy) -> Self {
        Self { store, policy }
    }

    async fn apply(
        &self,
        tx: &mut S::Tx,
        movement: &Movement,
        origin: Origin,
    ) -> Result<Decision, Attempt> {
        let key = movement.unit.key();
        let balance = tx.stock_balance(&key).await?.unwrap_or_default();
        let decision = self.policy.decide(balance, movement, origin)?;
        let recorded = Movement {
            emergency: decision.emergency,
            ..movement.clone()
        };
        tx.append_movement(&recorded).await?;
        tx.put_stock_balance(&movement.unit, &decision.balance)
            .await?;
        // A late operation that drove the balance negative is reconciled by a count.
        if decision.reconcile {
            tx.put_reconciliation(&ReconciliationRecord::open(
                &format!("{}-negative", movement.id),
                &key,
                Discrepancy::Negative,
                decision.balance.on_hand,
                &format!("Movement/{}", movement.id),
                movement.recorded_at,
            ))
            .await?;
        }
        Ok(decision)
    }

    /// Commits a successful attempt. `None`: it lost a conflict and is rerun.
    async fn settle<T>(
        tx: S::Tx,
        attempt: Result<T, Attempt>,
    ) -> Option<Result<T, InventoryError>> {
        match attempt {
            Ok(value) => match tx.commit().await {
                Ok(()) => Some(Ok(value)),
                Err(WriteError::Conflict) => None,
                Err(other) => Some(Err(InventoryError::Store(other.to_string()))),
            },
            Err(Attempt::Conflict) => None,
            Err(Attempt::Fail(error)) => Some(Err(error)),
        }
    }

    /// Records `movement` from `origin`: the new balance, and whether it was flagged.
    pub async fn record(
        &self,
        movement: &Movement,
        origin: Origin,
    ) -> Result<Decision, InventoryError> {
        for _ in 0..ATTEMPTS {
            let mut tx = self.store.begin().await?;
            let attempt = self.apply(&mut tx, movement, origin).await;
            if let Some(done) = Self::settle(tx, attempt).await {
                return done;
            }
        }
        Err(InventoryError::Busy)
    }

    /// Moves `quantity` of `movement.unit` to `status`, in one transaction: a status-change out
    /// of the unit and one into the unit with the new status. `movement` carries the shared
    /// fields; its id is suffixed `-out` and `-in`.
    pub async fn change_status(
        &self,
        movement: &Movement,
        status: Status,
        origin: Origin,
    ) -> Result<(Decision, Decision), InventoryError> {
        let quantity = movement.quantity.abs();
        let out = Movement {
            id: format!("{}-out", movement.id),
            kind: MovementKind::StatusChange,
            quantity: -quantity,
            ..movement.clone()
        };
        let into = Movement {
            id: format!("{}-in", movement.id),
            kind: MovementKind::StatusChange,
            quantity,
            unit: health_inventory::StockUnit {
                status,
                ..movement.unit.clone()
            },
            ..movement.clone()
        };
        for _ in 0..ATTEMPTS {
            let mut tx = self.store.begin().await?;
            let attempt = match self.apply(&mut tx, &out, origin).await {
                Ok(left) => self
                    .apply(&mut tx, &into, origin)
                    .await
                    .map(|arrived| (left, arrived)),
                Err(failed) => Err(failed),
            };
            if let Some(done) = Self::settle(tx, attempt).await {
                return done;
            }
        }
        Err(InventoryError::Busy)
    }

    /// The unit's balance record (zero when it has none).
    pub async fn balance(&self, key: &str) -> Result<Balance, InventoryError> {
        let mut tx = self.store.begin().await?;
        Ok(tx.stock_balance(key).await?.unwrap_or_default())
    }

    /// Rebuilds every balance from its movements and reports the units whose balance record
    /// differs (the nightly check).
    pub async fn nightly_check(&self) -> Result<Vec<Drift>, InventoryError> {
        let mut tx = self.store.begin().await?;
        let mut drift = Vec::new();
        for key in tx.stock_units().await? {
            let recorded = tx.stock_balance(&key).await?.unwrap_or_default().on_hand;
            let ledger = rebuild(&tx.movements(&key).await?);
            if recorded != ledger {
                drift.push(Drift {
                    unit: key,
                    recorded,
                    ledger,
                });
            }
        }
        Ok(drift)
    }
}
