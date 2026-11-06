//! Dispensing (inventory-and-procurement.md § Reservations and batch selection, § Dispensing;
//! ADR 0035; MASTER_PROMPT §73; #128). **Stock moves only when a dispense is completed.** A
//! MedicationRequest never touches stock, and preparation only reserves it.
//!
//! - **[`reservation`]:** preparing a dispense holds stock with a [`Reservation`] for a lifetime
//!   (4 h for a dispense, 72 h for a transfer pick). It reduces *available*, never *on-hand*, and
//!   is released on cancellation, completion or expiry. [`reserved`] counts only live ones, so an
//!   abandoned preparation never causes a phantom stock-out.
//! - **[`fefo`](mod@fefo):** the batch to use. Candidates are usable, of the product, eligible under the
//!   funding-source rules (donor stock only for patients in its program), and expire after the
//!   course ends plus a buffer. The earliest expiry wins, then the smallest quantity. Overriding
//!   that choice needs a reason, but never reaches ineligible or expiring stock.
//! - **[`substitution`]:** generic equivalence is allowed, brand or pack substitution needs a
//!   pharmacist, and therapeutic substitution needs the prescriber.
//! - **[`settle`]:** completing a dispense consumes its reservations, fully or in part. What was
//!   given becomes dispense movements, and the rest is released.
//! - **[`returned`]:** a patient's return goes to quarantine.

pub mod fefo;
pub mod reservation;
pub mod substitution;

use std::collections::BTreeMap;

pub use fefo::{BatchRule, Candidate, Eligibility, Need, Pick, SelectRefused, chosen, fefo};
pub use reservation::{Purpose, Reservation, ReservationState, reserved};
pub use substitution::{Approval, Substitution, SubstitutionRefused, may_substitute};

use crate::{Status, StockUnit};

/// Why a dispense could not be settled.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DispenseRefused {
    /// More was given from a reservation than it holds, or a negative amount.
    #[error("reservation {0} holds less than that")]
    Exceeds(String),
    /// A reservation named is not one of this dispense's live reservations.
    #[error("reservation {0} is not held for this dispense")]
    NotHeld(String),
    /// Nothing was given: cancel the dispense instead.
    #[error("nothing was dispensed")]
    Nothing,
}

/// What completing a dispense does to one of its reservations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settled {
    /// The reservation, now consumed.
    pub reservation: Reservation,
    /// What was given from it: the dispense movement's quantity (positive here).
    pub given: i64,
}

/// Completes a dispense against its live reservations `held`. `given` is what was handed over
/// from each (by reservation id); a reservation not named gave nothing. Each reservation is
/// consumed, and what it gave is its dispense movement. A partial dispense gives less than it
/// held, and the rest goes back to available.
pub fn settle(
    held: &[Reservation],
    given: &BTreeMap<String, i64>,
) -> Result<Vec<Settled>, DispenseRefused> {
    for id in given.keys() {
        if !held.iter().any(|r| &r.id == id) {
            return Err(DispenseRefused::NotHeld(id.clone()));
        }
    }
    let mut settled = Vec::new();
    for reservation in held {
        let amount = given.get(&reservation.id).copied().unwrap_or(0);
        if amount < 0 || amount > reservation.quantity {
            return Err(DispenseRefused::Exceeds(reservation.id.clone()));
        }
        settled.push(Settled {
            reservation: Reservation {
                state: ReservationState::Consumed,
                ..reservation.clone()
            },
            given: amount,
        });
    }
    if settled.iter().all(|s| s.given == 0) {
        return Err(DispenseRefused::Nothing);
    }
    Ok(settled)
}

/// The Stock Unit a patient's return of `unit` goes into: the same batch, in quarantine, until a
/// pharmacist releases it (a status change).
#[must_use]
pub fn returned(unit: &StockUnit) -> StockUnit {
    StockUnit {
        status: Status::Quarantine,
        ..unit.clone()
    }
}

#[cfg(test)]
mod tests;
