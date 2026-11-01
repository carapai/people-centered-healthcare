//! Stock (inventory-and-procurement.md; ADR 0034): an append-only ledger of movements per Stock
//! Unit, and one balance record per Stock Unit that guards concurrent issues.
//!
//! - **[`StockUnit`]**: store × product × batch × expiry × funding source × ownership × status.
//! - **[`Movement`]**: a signed quantity in base units, the pack size, unit cost, `occurred_at`
//!   and `recorded_at`, the source document, actor, node and `op_id`. Corrections are reversals.
//! - **[`Balance`]**: on-hand, reserved and on-hold, with `available` derived. It is written in
//!   every movement's transaction, so two issues of the last pack conflict and one of them sees
//!   the stock gone.
//! - **[`Policy::decide`]**: the negative-stock rules. At the authority, issues, transfers and
//!   adjustments that would go negative are blocked, and a dispense needs the emergency permission
//!   and a reason and is flagged. Late (offline) operations are always appended and flag the unit.
//! - **[`stock_card`]** and **[`rebuild`]**: the running balance by `occurred_at`, and the
//!   nightly drift check.

pub mod reconciliation;
pub mod transfers;

use std::future::Future;

use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Owned stock or consignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ownership {
    /// The facility's own stock.
    Owned,
    /// A supplier's, held on consignment.
    Consignment,
}

/// Whether stock may be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Available for issue.
    Usable,
    /// Held pending inspection (returns arrive here).
    Quarantine,
    /// Restricted to some uses.
    Restricted,
}

/// What a balance is kept for (ADR 0034).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StockUnit {
    /// The store (a Location id).
    pub store: String,
    /// The product.
    pub product: String,
    /// The batch or lot.
    pub batch: String,
    /// The expiry date (`YYYY-MM-DD`).
    pub expiry: String,
    /// The program or donor that funded it.
    pub funding_source: String,
    /// Owned or consignment.
    pub ownership: Ownership,
    /// Usable, quarantine or restricted.
    pub status: Status,
}

impl StockUnit {
    /// The unit's key, unique per Stock Unit.
    pub fn key(&self) -> String {
        let ownership = match self.ownership {
            Ownership::Owned => "owned",
            Ownership::Consignment => "consignment",
        };
        let status = match self.status {
            Status::Usable => "usable",
            Status::Quarantine => "quarantine",
            Status::Restricted => "restricted",
        };
        [
            self.store.as_str(),
            self.product.as_str(),
            self.batch.as_str(),
            self.expiry.as_str(),
            self.funding_source.as_str(),
            ownership,
            status,
        ]
        .join("|")
    }
}

/// What a movement is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MovementKind {
    /// Goods received: increases on-hand.
    Receipt,
    /// Issued to a department or ward: decreases on-hand.
    Issue,
    /// Dispensed to a patient: decreases on-hand.
    Dispense,
    /// Sent to another store: decreases on-hand.
    TransferOut,
    /// Received from another store: increases on-hand.
    TransferIn,
    /// Returned: increases on-hand (into quarantine by default).
    Return,
    /// Corrected: either way, with a reason code.
    Adjustment,
    /// A count's difference: either way.
    CountVariance,
    /// Expired stock written off: decreases on-hand.
    ExpiryWriteOff,
    /// Moved out of or into a status (two movements in one transaction).
    StatusChange,
    /// The reversal of an earlier movement.
    Reversal,
}

impl MovementKind {
    /// Whether the quantity must be positive (+), negative (−), or may be either.
    fn sign(self) -> Option<bool> {
        match self {
            Self::Receipt | Self::TransferIn | Self::Return => Some(true),
            Self::Issue | Self::Dispense | Self::TransferOut | Self::ExpiryWriteOff => Some(false),
            Self::Adjustment | Self::CountVariance | Self::StatusChange | Self::Reversal => None,
        }
    }
}

/// One immutable stock movement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Movement {
    /// Its id.
    pub id: String,
    /// The Stock Unit.
    pub unit: StockUnit,
    /// What it is.
    pub kind: MovementKind,
    /// The signed quantity, in base units.
    pub quantity: i64,
    /// The pack size used (base units per pack).
    pub pack_size: u32,
    /// The unit cost, in the ledger's functional currency.
    #[serde(default)]
    pub unit_cost: Option<Decimal>,
    /// When it happened.
    pub occurred_at: Timestamp,
    /// When it was recorded.
    pub recorded_at: Timestamp,
    /// The source document (a dispense, a goods-received note, a count).
    pub source_document: String,
    /// Who.
    pub actor: String,
    /// On which node.
    pub node: String,
    /// The operation id.
    pub op_id: String,
    /// Why: required for adjustments and emergency dispenses.
    #[serde(default)]
    pub reason: Option<String>,
    /// The movement this reverses.
    #[serde(default)]
    pub reverses: Option<String>,
    /// A dispense beyond the stock on hand, under the emergency permission.
    #[serde(default)]
    pub emergency: bool,
}

/// A Stock Unit's balance record.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Balance {
    /// On hand.
    pub on_hand: i64,
    /// Reserved for pending dispenses.
    pub reserved: i64,
    /// On hold.
    pub on_hold: i64,
    /// Flagged until a count resolves it: an emergency dispense or a late operation took it
    /// negative.
    pub flagged: bool,
}

impl Balance {
    /// `on-hand − reserved − on-hold`.
    pub fn available(&self) -> i64 {
        self.on_hand - self.reserved - self.on_hold
    }
}

/// Where a movement is recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// At the store's authority, now.
    Authority {
        /// Whether the actor holds the emergency-dispense permission.
        may_emergency_dispense: bool,
    },
    /// A late operation: an offline candidate arriving at the authority.
    Late,
}

/// Why a movement was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// It would take the balance below zero (`insufficient_stock`).
    #[error("insufficient stock: {available} available")]
    InsufficientStock {
        /// What is available.
        available: i64,
    },
    /// The quantity's sign does not fit the kind, or it is zero.
    #[error("the quantity does not fit a {0:?}")]
    BadQuantity(MovementKind),
    /// An adjustment or an emergency dispense needs a reason.
    #[error("this movement needs a reason")]
    ReasonRequired,
    /// The store does not allow emergency dispensing.
    #[error("emergency dispensing is blocked at this store")]
    EmergencyBlocked,
}

/// A store's negative-stock configuration (inventory-and-procurement.md § Negative stock).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Policy {
    /// Tighten: no emergency dispense at all.
    pub block_emergency_dispense: bool,
    /// Relax: issues, transfers and adjustments that go negative are allowed and flagged.
    pub allow_negative_issues: bool,
}

/// What recording a movement does to its balance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    /// The new balance.
    pub balance: Balance,
    /// The movement is flagged (an emergency dispense).
    pub emergency: bool,
    /// The balance went negative from a late operation: a Reconciliation Record is due.
    pub reconcile: bool,
}

impl Policy {
    /// Decides whether `movement` may be recorded against `balance`, and the result.
    pub fn decide(
        &self,
        balance: Balance,
        movement: &Movement,
        origin: Origin,
    ) -> Result<Decision, Refused> {
        let quantity = movement.quantity;
        let sign_ok = quantity != 0
            && movement
                .kind
                .sign()
                .is_none_or(|positive| (quantity > 0) == positive);
        if !sign_ok {
            return Err(Refused::BadQuantity(movement.kind));
        }
        let reason = movement
            .reason
            .as_deref()
            .is_some_and(|r| !r.trim().is_empty());
        if movement.kind == MovementKind::Adjustment && !reason {
            return Err(Refused::ReasonRequired);
        }
        let next = Balance {
            on_hand: balance.on_hand + quantity,
            ..balance
        };
        let short = quantity < 0 && next.available() < 0;
        if !short {
            return Ok(Decision {
                balance: next,
                emergency: false,
                reconcile: false,
            });
        }
        match origin {
            // Offline candidates are always appended; the unit is flagged until a count.
            Origin::Late => Ok(Decision {
                balance: Balance {
                    flagged: true,
                    ..next
                },
                emergency: false,
                reconcile: true,
            }),
            Origin::Authority {
                may_emergency_dispense,
            } => match movement.kind {
                MovementKind::Dispense if may_emergency_dispense && movement.emergency => {
                    if self.block_emergency_dispense {
                        return Err(Refused::EmergencyBlocked);
                    }
                    if !reason {
                        return Err(Refused::ReasonRequired);
                    }
                    Ok(Decision {
                        balance: Balance {
                            flagged: true,
                            ..next
                        },
                        emergency: true,
                        reconcile: false,
                    })
                }
                MovementKind::Issue | MovementKind::TransferOut | MovementKind::Adjustment
                    if self.allow_negative_issues =>
                {
                    Ok(Decision {
                        balance: Balance {
                            flagged: true,
                            ..next
                        },
                        emergency: false,
                        reconcile: false,
                    })
                }
                _ => Err(Refused::InsufficientStock {
                    available: balance.available(),
                }),
            },
        }
    }
}

/// The reversal of `movement`: the same quantity, negated. Recorded like any movement.
pub fn reversal(movement: &Movement, id: &str, recorded_at: Timestamp, reason: &str) -> Movement {
    Movement {
        id: id.to_owned(),
        kind: MovementKind::Reversal,
        quantity: -movement.quantity,
        recorded_at,
        reason: Some(reason.to_owned()),
        reverses: Some(movement.id.clone()),
        emergency: false,
        ..movement.clone()
    }
}

/// The stock card: each movement with the running on-hand after it, ordered by `occurred_at`,
/// then `recorded_at`, then id. A backdated movement lands at its own time.
pub fn stock_card(movements: &[Movement]) -> Vec<(&Movement, i64)> {
    let mut ordered: Vec<&Movement> = movements.iter().collect();
    ordered.sort_by(|a, b| {
        a.occurred_at
            .cmp(&b.occurred_at)
            .then(a.recorded_at.cmp(&b.recorded_at))
            .then(a.id.cmp(&b.id))
    });
    let mut running = 0;
    ordered
        .into_iter()
        .map(|m| {
            running += m.quantity;
            (m, running)
        })
        .collect()
}

/// The on-hand the ledger gives, for the nightly drift check against the balance record.
pub fn rebuild(movements: &[Movement]) -> i64 {
    movements.iter().map(|m| m.quantity).sum()
}

/// A Stock Unit whose balance record disagrees with its ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drift {
    /// The unit's key.
    pub unit: String,
    /// What the balance record says.
    pub recorded: i64,
    /// What the ledger gives.
    pub ledger: i64,
}

/// The inventory's persistence port, implemented by the store's transaction.
pub trait StockStore {
    /// The balance record of the unit with `key`, if there is one.
    fn stock_balance(
        &mut self,
        key: &str,
    ) -> impl Future<Output = Result<Option<Balance>, StoreError>> + Send;
    /// Writes the unit's balance record: the guard every movement's transaction writes.
    fn put_stock_balance(
        &mut self,
        unit: &StockUnit,
        balance: &Balance,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;
    /// Appends `movement`.
    fn append_movement(
        &mut self,
        movement: &Movement,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;
    /// The unit's movements, in recording order.
    fn movements(
        &mut self,
        key: &str,
    ) -> impl Future<Output = Result<Vec<Movement>, StoreError>> + Send;
    /// The unit keys that have a balance record.
    fn stock_units(&mut self) -> impl Future<Output = Result<Vec<String>, StoreError>> + Send;
    /// Adds or replaces a transfer.
    fn put_transfer(
        &mut self,
        transfer: &transfers::Transfer,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;
    /// One transfer.
    fn transfer(
        &mut self,
        id: &str,
    ) -> impl Future<Output = Result<Option<transfers::Transfer>, StoreError>> + Send;
    /// Adds or replaces a Reconciliation Record.
    fn put_reconciliation(
        &mut self,
        record: &reconciliation::ReconciliationRecord,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;
    /// One Reconciliation Record.
    fn reconciliation(
        &mut self,
        id: &str,
    ) -> impl Future<Output = Result<Option<reconciliation::ReconciliationRecord>, StoreError>> + Send;
    /// The records still open, oldest first.
    fn open_reconciliations(
        &mut self,
    ) -> impl Future<Output = Result<Vec<reconciliation::ReconciliationRecord>, StoreError>> + Send;
}
