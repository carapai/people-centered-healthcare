//! Substitution (inventory-and-procurement.md § Reservations and batch selection): who may
//! approve giving something other than what was prescribed. Every substitution is recorded on the
//! MedicationDispense.

use serde::{Deserialize, Serialize};

/// What is given in place of what was prescribed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Substitution {
    /// A generic equivalent (product registry and formulary).
    Generic,
    /// Another brand.
    Brand,
    /// Another pack.
    Pack,
    /// A different medicine for the same purpose.
    Therapeutic,
}

/// Who approved it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Approval {
    /// No one: the dispenser alone.
    Dispenser,
    /// A pharmacist.
    Pharmacist,
    /// The prescriber confirmed it.
    Prescriber,
}

/// Why a substitution was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SubstitutionRefused {
    /// A brand or pack substitution needs a pharmacist.
    #[error("a brand or pack substitution needs a pharmacist")]
    NeedsPharmacist,
    /// A therapeutic substitution needs the prescriber's confirmation.
    #[error("a therapeutic substitution needs the prescriber's confirmation")]
    NeedsPrescriber,
}

/// Whether `kind` may be given with `approval`. A prescriber's confirmation covers any.
pub fn may_substitute(kind: Substitution, approval: Approval) -> Result<(), SubstitutionRefused> {
    match (kind, approval) {
        (Substitution::Generic, _) | (_, Approval::Prescriber) => Ok(()),
        (Substitution::Brand | Substitution::Pack, Approval::Pharmacist) => Ok(()),
        (Substitution::Brand | Substitution::Pack, Approval::Dispenser) => {
            Err(SubstitutionRefused::NeedsPharmacist)
        }
        (Substitution::Therapeutic, _) => Err(SubstitutionRefused::NeedsPrescriber),
    }
}
