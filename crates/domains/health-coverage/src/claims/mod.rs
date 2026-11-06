//! Claims (pricing-and-insurance.md § Eligibility, preauthorization and claims; #126). No
//! national payer or claim format is assumed.
//!
//! - **[`assemble`]**: a [`Claim`] per coverage from the account's charges that coverage pays
//!   part of, in service order (`use = claim`), or a preauthorization (`use = preauthorization`).
//! - **[`Adjudication`]** (ClaimResponse): per item, what the payer approved. [`Claim::adjudicate`]
//!   settles the claim: complete, partial or denied, and the [`Shortfall`] of each item the payer
//!   did not pay in full.
//! - **[`Claim::appeal`]**: a denied or partly paid claim is resubmitted as a new claim that
//!   references the original.
//! - **[`Remittance`]**: what the payer paid against a claim, for the payer-receivable posting.
//! - **[`ClaimPayer`]**: the port the payer adapters in `health-integration` implement.

use std::future::Future;

use health_time::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::PayerError;

/// What a claim is for (`Claim.use`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClaimUse {
    /// Payment for care given.
    Claim,
    /// Approval before care is given; a financial gate may require it.
    Preauthorization,
}

/// Where a claim is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClaimStatus {
    /// Assembled, not yet sent.
    Draft,
    /// Sent to the payer.
    Submitted,
    /// Adjudicated: paid in full.
    Approved,
    /// Adjudicated: paid in part.
    Partial,
    /// Adjudicated: nothing paid.
    Denied,
}

/// A charge a coverage pays part of, as a claim item's source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claimable {
    /// The charge.
    pub charge: String,
    /// Its item.
    pub item: String,
    /// How many.
    pub quantity: Decimal,
    /// Its price after stages 1–3.
    pub gross: Decimal,
    /// What this coverage was split to pay.
    pub payer_share: Decimal,
    /// When the service was given.
    pub at: Timestamp,
}

/// One item of a claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimItem {
    /// Its sequence, from 1.
    pub sequence: u32,
    /// The charge it claims.
    pub charge: String,
    /// Its item.
    pub item: String,
    /// How many.
    pub quantity: Decimal,
    /// What is claimed: the coverage's share.
    pub claimed: Decimal,
    /// When the service was given.
    pub at: Timestamp,
}

/// A claim to one payer for one coverage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    /// The claim.
    pub id: String,
    /// Claim or preauthorization.
    pub use_: ClaimUse,
    /// The patient.
    pub patient: String,
    /// The Coverage.
    pub coverage: String,
    /// The payer.
    pub payer: String,
    /// The billing account.
    pub account: String,
    /// The items.
    pub items: Vec<ClaimItem>,
    /// Where it is.
    pub status: ClaimStatus,
    /// When it was assembled.
    pub created: Timestamp,
    /// The claim this one appeals, if it does.
    pub appeals: Option<String>,
    /// The payer's reference, once submitted.
    pub payer_reference: Option<String>,
}

/// Why a claim could not be assembled, adjudicated or appealed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClaimRefused {
    /// No charge in it is one the coverage pays part of.
    #[error("there is nothing to claim")]
    Nothing,
    /// The claim is not where that step needs it.
    #[error("the claim is {0:?}")]
    Status(ClaimStatus),
    /// The adjudication names an item the claim does not have, or approves more than claimed or
    /// less than nothing.
    #[error("the adjudication of item {0} does not fit the claim")]
    BadAdjudication(u32),
}

/// Who and what a claim is for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimFor {
    /// The claim's id.
    pub id: String,
    /// Claim or preauthorization.
    pub use_: ClaimUse,
    /// The patient.
    pub patient: String,
    /// The Coverage.
    pub coverage: String,
    /// The payer.
    pub payer: String,
    /// The billing account.
    pub account: String,
}

/// Assembles the claim `header` describes from `charges`: those its coverage pays part of, in
/// service order, then by charge.
pub fn assemble(
    header: &ClaimFor,
    charges: &[Claimable],
    at: Timestamp,
) -> Result<Claim, ClaimRefused> {
    let mut paid: Vec<&Claimable> = charges
        .iter()
        .filter(|c| c.payer_share > Decimal::ZERO)
        .collect();
    if paid.is_empty() {
        return Err(ClaimRefused::Nothing);
    }
    paid.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.charge.cmp(&b.charge)));
    let items = paid
        .into_iter()
        .zip(1..)
        .map(|(c, sequence)| ClaimItem {
            sequence,
            charge: c.charge.clone(),
            item: c.item.clone(),
            quantity: c.quantity,
            claimed: c.payer_share,
            at: c.at,
        })
        .collect();
    Ok(Claim {
        id: header.id.clone(),
        use_: header.use_,
        patient: header.patient.clone(),
        coverage: header.coverage.clone(),
        payer: header.payer.clone(),
        account: header.account.clone(),
        items,
        status: ClaimStatus::Draft,
        created: at,
        appeals: None,
        payer_reference: None,
    })
}

/// The payer's decision on one item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemAdjudication {
    /// The claim item's sequence.
    pub sequence: u32,
    /// What the payer approved.
    pub approved: Decimal,
    /// Why it approved less, if it did.
    pub reason: Option<String>,
}

/// The payer's decision on a claim (ClaimResponse).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adjudication {
    /// The payer's reference for the claim.
    pub payer_reference: String,
    /// Per item; an item not named was approved in nothing.
    pub items: Vec<ItemAdjudication>,
    /// Why the whole claim was denied, if it was.
    pub denied: Option<String>,
}

/// An item the payer did not pay in full: what falls back on the account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shortfall {
    /// The charge.
    pub charge: String,
    /// What was claimed and not approved.
    pub amount: Decimal,
    /// The payer's reason.
    pub reason: Option<String>,
}

/// What an adjudication settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settled {
    /// The claim, adjudicated.
    pub claim: Claim,
    /// What the payer approved in all: the payer receivable.
    pub approved: Decimal,
    /// The items not paid in full.
    pub shortfalls: Vec<Shortfall>,
}

impl Claim {
    /// What is claimed in all.
    #[must_use]
    pub fn total(&self) -> Decimal {
        self.items.iter().map(|i| i.claimed).sum()
    }

    /// The claim, sent: with the payer's reference.
    pub fn submitted(&self, payer_reference: &str) -> Result<Self, ClaimRefused> {
        if self.status != ClaimStatus::Draft {
            return Err(ClaimRefused::Status(self.status));
        }
        Ok(Self {
            status: ClaimStatus::Submitted,
            payer_reference: Some(payer_reference.to_owned()),
            ..self.clone()
        })
    }

    /// Settles a submitted claim with the payer's `adjudication`.
    pub fn adjudicate(&self, adjudication: &Adjudication) -> Result<Settled, ClaimRefused> {
        if self.status != ClaimStatus::Submitted {
            return Err(ClaimRefused::Status(self.status));
        }
        for decided in &adjudication.items {
            let item = self
                .items
                .iter()
                .find(|i| i.sequence == decided.sequence)
                .ok_or(ClaimRefused::BadAdjudication(decided.sequence))?;
            if decided.approved < Decimal::ZERO || decided.approved > item.claimed {
                return Err(ClaimRefused::BadAdjudication(decided.sequence));
            }
        }
        let mut approved = Decimal::ZERO;
        let mut shortfalls = Vec::new();
        for item in &self.items {
            let decided = adjudication
                .items
                .iter()
                .find(|d| d.sequence == item.sequence)
                .filter(|_| adjudication.denied.is_none());
            let given = decided.map_or(Decimal::ZERO, |d| d.approved);
            approved += given;
            if given < item.claimed {
                shortfalls.push(Shortfall {
                    charge: item.charge.clone(),
                    amount: item.claimed - given,
                    reason: decided
                        .and_then(|d| d.reason.clone())
                        .or_else(|| adjudication.denied.clone()),
                });
            }
        }
        let status = if approved == Decimal::ZERO {
            ClaimStatus::Denied
        } else if shortfalls.is_empty() {
            ClaimStatus::Approved
        } else {
            ClaimStatus::Partial
        };
        Ok(Settled {
            claim: Self {
                status,
                payer_reference: Some(adjudication.payer_reference.clone()),
                ..self.clone()
            },
            approved,
            shortfalls,
        })
    }

    /// An appeal of this denied or partly paid claim: a new draft claim `id` of the items not paid
    /// in full, for what was not approved, referencing this one.
    pub fn appeal(&self, id: &str, settled: &Settled, at: Timestamp) -> Result<Self, ClaimRefused> {
        if !matches!(self.status, ClaimStatus::Denied | ClaimStatus::Partial) {
            return Err(ClaimRefused::Status(self.status));
        }
        let items: Vec<ClaimItem> = settled
            .shortfalls
            .iter()
            .filter_map(|short| {
                self.items
                    .iter()
                    .find(|i| i.charge == short.charge)
                    .map(|i| (i, short))
            })
            .zip(1..)
            .map(|((item, short), sequence)| ClaimItem {
                sequence,
                claimed: short.amount,
                ..item.clone()
            })
            .collect();
        if items.is_empty() {
            return Err(ClaimRefused::Nothing);
        }
        Ok(Self {
            id: id.to_owned(),
            items,
            status: ClaimStatus::Draft,
            created: at,
            appeals: Some(self.id.clone()),
            payer_reference: None,
            ..self.clone()
        })
    }
}

/// What a payer paid against a claim (PaymentReconciliation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remittance {
    /// The claim paid.
    pub claim: String,
    /// The payer.
    pub payer: String,
    /// What was paid.
    pub amount: Decimal,
    /// The payer's payment reference.
    pub reference: String,
    /// When.
    pub at: Timestamp,
}

/// The claims port: one adapter per payer in `health-integration`.
pub trait ClaimPayer: Send + Sync {
    /// Sends `claim`: the payer's reference for it.
    fn submit(&self, claim: &Claim) -> impl Future<Output = Result<String, PayerError>> + Send;

    /// The payer's adjudication of the claim it knows as `payer_reference`, once it has one.
    fn adjudication(
        &self,
        payer_reference: &str,
    ) -> impl Future<Output = Result<Option<Adjudication>, PayerError>> + Send;
}

#[cfg(test)]
mod tests;
