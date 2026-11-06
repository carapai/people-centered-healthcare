//! Reservations (ADR 0035): stock held for a preparation, with a lifetime.

use health_time::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};

use crate::StockUnit;

/// What stock is held for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Purpose {
    /// A dispense being prepared.
    Dispense,
    /// A transfer being picked.
    TransferPick,
    /// A procedure kit being made up.
    ProcedureKit,
}

impl Purpose {
    /// How long a reservation for this lasts: 4 h for a dispense or a kit, 72 h for a transfer
    /// pick (inventory-and-procurement.md).
    #[must_use]
    pub fn lifetime(self) -> SignedDuration {
        match self {
            Self::Dispense | Self::ProcedureKit => SignedDuration::from_hours(4),
            Self::TransferPick => SignedDuration::from_hours(72),
        }
    }
}

/// Where a reservation is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReservationState {
    /// Holding stock until it expires.
    Held,
    /// Given back: the preparation was cancelled.
    Released,
    /// Used: the dispense was completed (in full or in part).
    Consumed,
}

/// Stock held for a preparation (inventory-and-procurement.md § Reservations).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reservation {
    /// The reservation's id.
    pub id: String,
    /// The Stock Unit held from.
    pub unit: StockUnit,
    /// How much, in base units.
    pub quantity: i64,
    /// What it is held for.
    pub purpose: Purpose,
    /// What holds it, e.g. `MedicationDispense/{id}`.
    pub reference: String,
    /// When it was made.
    pub created: Timestamp,
    /// When it lapses if not used.
    pub expires: Timestamp,
    /// Where it is.
    pub state: ReservationState,
}

impl Reservation {
    /// Holds `quantity` of `unit` for `purpose` and `reference`, from `at` for the purpose's
    /// lifetime.
    #[must_use]
    pub fn hold(
        id: &str,
        unit: &StockUnit,
        quantity: i64,
        purpose: Purpose,
        reference: &str,
        at: Timestamp,
    ) -> Self {
        Self {
            id: id.to_owned(),
            unit: unit.clone(),
            quantity,
            purpose,
            reference: reference.to_owned(),
            created: at,
            expires: at.checked_add(purpose.lifetime()).unwrap_or(at),
            state: ReservationState::Held,
        }
    }

    /// Whether it still holds stock at `at`: held, and not yet lapsed.
    #[must_use]
    pub fn live(&self, at: Timestamp) -> bool {
        self.state == ReservationState::Held && at < self.expires
    }
}

/// What `reservations` hold at `at`: only live ones count, so a lapsed reservation releases its
/// stock without anyone releasing it.
pub fn reserved<'a>(reservations: impl IntoIterator<Item = &'a Reservation>, at: Timestamp) -> i64 {
    reservations
        .into_iter()
        .filter(|r| r.live(at))
        .map(|r| r.quantity)
        .sum()
}
