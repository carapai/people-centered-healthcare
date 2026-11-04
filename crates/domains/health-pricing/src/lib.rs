//! Pricing (pricing-and-insurance.md; ADR 0031; #120): Price Books and Price Rules, and the six
//! fixed stages that turn them into a charge's price.
//!
//! - **Price Books** ([`PriceBook`]) are effective-dated, immutable once approved by one finance
//!   approver, and scoped. The base price comes from the most specific book that holds the item:
//!   payer contract → program → department → facility → organization → tenant. Quantity tiers
//!   apply within the base price.
//! - **Price Rules** ([`PriceRule`]) are effective-dated, with a stage and a priority. Their
//!   FHIRPath applicability is evaluated through the [`Applicability`] port. [`validate`]
//!   refuses two rules of the same stage and priority that could both apply.
//! - **The stages** ([`price`]):
//!   1. base price;
//!   2. surcharges;
//!   3. bundles, then discounts;
//!   4. the coverage split, primary first, then secondary on the remainder: deductible, copay,
//!      limits and exclusions from each Benefit Plan's cached terms (an estimate when cached);
//!   5. subsidies and waivers, on the patient share only;
//!   6. manual overrides of the patient share, with who and why.
//! - **Every version is recorded:** each component names the book or rule version it came from,
//!   and [`Priced`] lists them all. [`check`] is the authority's check that each was approved and
//!   in effect at the service time; a mismatch becomes a Reconciliation Record, never a silent
//!   re-price, so historical pricing is preserved.
//!
//! - **Their life** ([`approve_book`], [`approve_rule`], [`retire_book`], [`retire_rule`]):
//!   approved once by a finance approver who is not the author, never overlapping another
//!   approved version of themselves, and immutable after that except for retirement from a time
//!   not yet passed. [`PricingStore`] keeps every version.
//!
//! Amounts are rounded to the currency's minor units, half away from zero.

mod book;
mod lifecycle;
mod price;

pub use book::{
    Adjustment, Approval, Effective, Entry, PriceBook, PriceRule, RuleProblem, Scope, Stage, Tier,
    validate,
};
pub use lifecycle::{
    LifecycleError, PricingStore, approve_book, approve_rule, retire_book, retire_rule,
};
pub use price::{
    Applicability, Applied, Component, Copay, CoverageTerms, Kind, Line, Mismatch, Override,
    Priced, PricedLine, PricingError, Request, check, price,
};

#[cfg(test)]
mod tests;
