//! The facility's financial ledger (billing-and-payments.md § The ledger; ADR 0030): the financial
//! source of truth, which FHIR financial resources only project.
//!
//! - **Money** ([`Money`], [`Currency`]): `rust_decimal` at the ISO 4217 scale (UGX 0, USD 2),
//!   rounded half-even, and largest-remainder splits whose shares always sum exactly.
//! - **Chart of accounts** ([`ChartOfAccounts::from_template`]): receivables and waivers per
//!   funding source, patient deposits, payments in transit, cash per till, revenue per service
//!   class, rounding and till variance.
//! - **Postings** ([`Ledger::post`]): balanced, in the functional currency, on known accounts,
//!   with [`Dimensions`] on every line; appended once and never changed. Corrections are
//!   [`Ledger::reversal`]s.
//! - **Balances** are always derived from postings ([`Ledger::balance`],
//!   [`Ledger::encounter_balances`] for FHIR `Account.balance[]`).

mod ledger;
mod money;

pub use ledger::{
    Account, AccountKind, ChartOfAccounts, ChartTemplate, DEPOSITS, Dimensions, DuplicateAccount,
    IN_TRANSIT, Ledger, Line, Original, Posting, PostingRefused, ROUNDING, Side, TILL_VARIANCE,
};
pub use money::{Currency, Money, MoneyError, UnknownCurrency};
