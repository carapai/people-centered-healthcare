//! Remittance postings (pricing-and-insurance.md § Eligibility, preauthorization and claims;
//! #126): what a payer paid against a claim settles its receivable.

use health_coverage::claims::Remittance;
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
