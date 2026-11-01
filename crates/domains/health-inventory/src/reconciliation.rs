//! Reconciliation Records (inventory-and-procurement.md; WHO GSDP §13.2): a discrepancy between
//! what the ledger expects and what is there, open until someone records its root cause.

use health_time::Timestamp;
use serde::{Deserialize, Serialize};

/// What went wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Discrepancy {
    /// Fewer arrived than were sent.
    Short,
    /// Some arrived damaged.
    Damaged,
    /// More arrived than were sent.
    Excess,
    /// A late operation drove the balance negative.
    Negative,
    /// A count found a different quantity.
    CountVariance,
}

/// Why it happened (GSDP §13.2), recorded when the record is resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RootCause {
    /// A movement was recorded wrongly, or not at all.
    RecordingError,
    /// Stock was issued or dispensed without being recorded.
    UnrecordedIssue,
    /// Lost or damaged on the way.
    TransitLoss,
    /// Damaged in storage.
    Damage,
    /// Expired and not written off.
    Expiry,
    /// The supplier sent a different quantity.
    SupplierDiscrepancy,
    /// Theft.
    Theft,
    /// Something else, explained in the note.
    Other,
}

/// How a record was resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    /// The root cause.
    pub cause: RootCause,
    /// What was found.
    pub note: String,
    /// Who resolved it.
    pub by: String,
    /// When.
    pub at: Timestamp,
}

/// Why a record could not be resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ResolveRefused {
    /// It is already resolved.
    #[error("the reconciliation record is already resolved")]
    AlreadyResolved,
    /// `Other` needs a note saying what it was.
    #[error("an `other` root cause needs a note")]
    NoteRequired,
}

/// A discrepancy to reconcile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconciliationRecord {
    /// The record's id.
    pub id: String,
    /// The Stock Unit it concerns ([`crate::StockUnit::key`]).
    pub unit: String,
    /// What went wrong.
    pub discrepancy: Discrepancy,
    /// By how much, in base units: negative for missing stock, positive for excess.
    pub quantity: i64,
    /// What found it, e.g. `Transfer/{id}`, `Movement/{id}` or `Count/{id}`.
    pub source: String,
    /// When it was opened.
    pub opened_at: Timestamp,
    /// Its resolution, once recorded.
    pub resolution: Option<Resolution>,
}

impl ReconciliationRecord {
    /// An open record.
    pub fn open(
        id: &str,
        unit: &str,
        discrepancy: Discrepancy,
        quantity: i64,
        source: &str,
        at: Timestamp,
    ) -> Self {
        Self {
            id: id.to_owned(),
            unit: unit.to_owned(),
            discrepancy,
            quantity,
            source: source.to_owned(),
            opened_at: at,
            resolution: None,
        }
    }

    /// Records the root cause. A record is resolved once.
    pub fn resolve(
        &mut self,
        cause: RootCause,
        note: &str,
        by: &str,
        at: Timestamp,
    ) -> Result<(), ResolveRefused> {
        if self.resolution.is_some() {
            return Err(ResolveRefused::AlreadyResolved);
        }
        if cause == RootCause::Other && note.trim().is_empty() {
            return Err(ResolveRefused::NoteRequired);
        }
        self.resolution = Some(Resolution {
            cause,
            note: note.trim().to_owned(),
            by: by.to_owned(),
            at,
        });
        Ok(())
    }
}
