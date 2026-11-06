//! Remittance and shortfall postings (pricing-and-insurance.md § Eligibility, preauthorization
//! and claims; #126): what a payer paid against a claim settles its receivable, and **what it
//! did not pay, the patient pays**: each shortfall moves from the coverage's receivable to the
//! patient's.

use health_coverage::claims::{Remittance, Settled};
use health_financial_ledger::{Currency, Dimensions, IN_TRANSIT, Line, Money, Posting, Side};

/// The posting of `remittance` for the claim of `coverage` at `facility`: payments in transit
/// debited (the bank receipt clears them), the coverage's receivable credited. Billing debits
/// `receivable/{coverage}` with each charge's payer share, so this takes off what was paid.
#[must_use]
pub fn remittance_posting(
    remittance: &Remittance,
    coverage: &str,
    facility: &str,
    currency: Currency,
) -> Posting {
    let dimensions = Dimensions {
        facility: facility.to_owned(),
        funding_source: Some(coverage.to_owned()),
        ..Dimensions::default()
    };
    let amount = Money::rounded(remittance.amount, currency);
    Posting {
        id: format!("remittance-{}", remittance.reference),
        at: remittance.at,
        description: format!(
            "Remittance {} from {} for claim {}",
            remittance.reference, remittance.payer, remittance.claim
        ),
        lines: vec![
            Line {
                account: IN_TRANSIT.to_owned(),
                side: Side::Debit,
                amount,
                dimensions: dimensions.clone(),
                original: None,
            },
            Line {
                account: format!("receivable/{coverage}"),
                side: Side::Credit,
                amount,
                dimensions,
                original: None,
            },
        ],
        reverses: None,
    }
}

/// The posting that moves `settled`'s shortfalls (denied, or approved below what was claimed) from
/// the coverage's receivable to the patient's, at `at`. `None` when the payer paid every item in
/// full. One line pair per item, each naming its charge.
#[must_use]
pub fn shortfall_posting(
    settled: &Settled,
    facility: &str,
    currency: Currency,
    at: health_time::Timestamp,
) -> Option<Posting> {
    let coverage = &settled.claim.coverage;
    let mut lines = Vec::new();
    for short in settled
        .shortfalls
        .iter()
        .filter(|s| s.amount > rust_decimal::Decimal::ZERO)
    {
        let amount = Money::rounded(short.amount, currency);
        let dimensions = |funding: &str| Dimensions {
            facility: facility.to_owned(),
            funding_source: Some(funding.to_owned()),
            encounter: Some(settled.claim.account.clone()),
            service: Some(short.charge.clone()),
            ..Dimensions::default()
        };
        lines.push(Line {
            account: format!("receivable/{PATIENT}"),
            side: Side::Debit,
            amount,
            dimensions: dimensions(PATIENT),
            original: None,
        });
        lines.push(Line {
            account: format!("receivable/{coverage}"),
            side: Side::Credit,
            amount,
            dimensions: dimensions(coverage),
            original: None,
        });
    }
    if lines.is_empty() {
        return None;
    }
    Some(Posting {
        id: format!("shortfall-{}", settled.claim.id),
        at,
        description: format!(
            "Shortfall of claim {} ({}) to the patient",
            settled.claim.id, settled.claim.payer
        ),
        lines,
        reverses: None,
    })
}

/// The patient as a funding source, as billing names it.
const PATIENT: &str = "patient";
