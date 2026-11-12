//! Inventory use cases (inventory-and-procurement.md § Balances and concurrency; ADR 0034).
//!
//! - **[`Inventory::record`]** records one movement in one transaction. It reads the Stock Unit's
//!   balance record, decides with the store's [`Policy`], appends the movement and writes the
//!   balance. Two concurrent issues of the last pack both write the same balance record, so one
//!   conflicts and is rerun; the rerun re-reads the balance and is refused for insufficient stock.
//! - **[`Inventory::change_status`]** moves stock between statuses (usable → quarantine) in one
//!   transaction: out of one Stock Unit and into the other.
//! - **[`Inventory::nightly_check`]** rebuilds each balance from the ledger and reports drift.
//! - **Dispensing** (#128): [`Inventory::prepare`] reserves, [`Inventory::complete_dispense`]
//!   moves stock, [`Inventory::cancel_dispense`] releases, and
//!   [`Inventory::return_to_quarantine`] takes a return. Only live reservations hold stock, in
//!   every movement's decision.

use health_inventory::counts::CountRefused;
use health_inventory::dispensing::{DispenseRefused, SelectRefused};
use health_inventory::receiving::ReceivingRefused;
use health_inventory::reconciliation::{Discrepancy, ReconciliationRecord, ResolveRefused};
use health_inventory::transfers::TransferRefused;
use health_inventory::{
    Balance, Decision, Drift, Movement, MovementKind, Origin, Policy, Refused, Status, StockStore,
    rebuild,
};
use health_persistence::{Store, StoreError, Transaction, WriteError};

mod counts;
mod dispensing;
mod queue;
mod receiving;
mod transfers;
mod valuation;

pub use counts::{Cause, inventory_report};
pub use dispensing::{Completion, DISPENSED_EVENT, PatientReturn, Preparation, Record};
pub use receiving::{OnHand, supply_requests};
pub use transfers::Sender;
pub use valuation::{StoreValuation, inventory_postings};

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
    /// No batch could be chosen for a dispense.
    #[error(transparent)]
    Select(#[from] SelectRefused),
    /// A dispense could not be settled against its reservations.
    #[error(transparent)]
    Dispense(#[from] DispenseRefused),
    /// A reservation lapsed before the dispense was completed: prepare it again.
    #[error("reservation {0} has lapsed; prepare the dispense again")]
    Lapsed(String),
    /// No such transfer, Reconciliation Record or prepared dispense.
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
const ATTEMPTS: u32 = 16;

/// Before attempt `attempt` (from 0): nothing for the first, then a random pause of up to
/// 2^attempt × 4 ms. Writers that lost the same conflict would otherwise rerun in lockstep and
/// collide again; under sixteen concurrent writers on one Stock Unit, most then ran out of
/// attempts (#141, the stock soak).
pub(crate) async fn pause(attempt: u32) {
    use std::hash::{BuildHasher, Hasher};
    if attempt == 0 {
        return;
    }
    let ceiling = 4u64 << attempt.min(7);
    let random = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();
    tokio::time::sleep(std::time::Duration::from_millis(random % ceiling)).await;
}

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
        // Authority Leases (#135): a node moves a leased store's stock only as its active holder.
        // A late operation was checked where it arrived, against the peer that sent it.
        if matches!(origin, Origin::Authority { .. })
            && !tx
                .may_write_store(&movement.unit.store, movement.recorded_at)
                .await?
        {
            return Err(Refused::Leased(movement.unit.store.clone()).into());
        }
        // Only live reservations hold stock (ADR 0035).
        let (balance, _) = dispensing::live_balance(tx, &key, movement.recorded_at, None).await?;
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

    /// Records `movement` from `origin` inside the caller's transaction `tx`, for a caller that
    /// commits it with other writes, such as sync applying a facility's operation with its
    /// receipt (#81). A lost write-write conflict is [`InventoryError::Busy`]; the caller reruns.
    pub async fn record_in(
        &self,
        tx: &mut S::Tx,
        movement: &Movement,
        origin: Origin,
    ) -> Result<Decision, InventoryError> {
        match self.apply(tx, movement, origin).await {
            Ok(decision) => Ok(decision),
            Err(Attempt::Conflict) => Err(InventoryError::Busy),
            Err(Attempt::Fail(error)) => Err(error),
        }
    }

    /// Records `movement` from `origin`: the new balance, and whether it was flagged.
    pub async fn record(
        &self,
        movement: &Movement,
        origin: Origin,
    ) -> Result<Decision, InventoryError> {
        let _turn = queue::stores([movement.unit.store.as_str()]).await;
        for attempt in 0..ATTEMPTS {
            crate::pause(attempt).await;
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
        let _turn = queue::stores([movement.unit.store.as_str()]).await;
        for attempt in 0..ATTEMPTS {
            crate::pause(attempt).await;
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
