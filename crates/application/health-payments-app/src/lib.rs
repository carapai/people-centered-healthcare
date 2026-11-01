//! Payments in the financial ledger (billing-and-payments.md § Payments, § Other flows;
//! ADR 0030, ADR 0032). Each step of a payment is a balanced posting:
//!
//! | Step | Debit | Credit |
//! |---|---|---|
//! | Cash taken | `cash/{till}` | the patient's receivable (or deposits) |
//! | Non-cash recorded, pending | payments in transit | the patient's receivable (or deposits) |
//! | Confirmed by the provider | the method's merchant account | payments in transit |
//! | Voided or expired | the patient's receivable (or deposits) | payments in transit |
//! | Reversed, or refunded | the patient's receivable (or deposits) | where the money is |
//! | Till shortage at close | till variance | `cash/{till}` |
//! | Till overage at close | `cash/{till}` | till variance |
//!
//! A pending payment sits in payments in transit, apart from cash and merchant accounts, so the
//! ledger never counts it as received (§68).

use std::collections::BTreeSet;

use health_financial_ledger::{
    Currency, DEPOSITS, Dimensions, IN_TRANSIT, Line, Money, Posting, Side, TILL_VARIANCE,
};
use health_payments::{Method, Payment, Purpose, Shift, Status};

mod service;

use health_time::Timestamp;
use rust_decimal::Decimal;
pub use service::{Payments, PaymentsError, Settings};

/// Where payments land in the chart of accounts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accounts {
    /// The patient's receivable (e.g. `receivable/patient`).
    pub receivable: String,
    /// Each non-cash method's merchant account (e.g. `cash/merchant-mtn`).
    pub merchants: Vec<(Method, String)>,
}

/// A step of a payment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Taken (cash) or recorded pending (non-cash).
    Recorded,
    /// Confirmed by provider evidence.
    Confirmed,
    /// Voided or expired while pending.
    Ended,
    /// Reversed after it was posted.
    Reversed,
    /// Part refunded, by this amount.
    Refunded(Decimal),
}

/// Why a step could not be posted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PostingError {
    /// The method has no merchant account.
    #[error("no merchant account for {0:?}")]
    NoMerchant(Method),
    /// Cash needs its till.
    #[error("cash needs the till of its shift")]
    NoTill,
    /// The step does not fit the payment's status.
    #[error("{0:?} does not fit a payment that is {1:?}")]
    WrongStep(Step, Status),
}

fn dimensions(facility: &str) -> Dimensions {
    Dimensions {
        facility: facility.to_owned(),
        department: None,
        service: None,
        program: None,
        funding_source: Some("patient".to_owned()),
        fund: None,
        encounter: None,
        price_versions: BTreeSet::new(),
    }
}

/// Where the ledger posts a payment.
pub struct Ledgering<'a> {
    /// The accounts.
    pub accounts: &'a Accounts,
    /// The facility.
    pub facility: &'a str,
    /// The ledger's currency.
    pub currency: Currency,
}

impl Ledgering<'_> {
    fn posting(
        &self,
        id: String,
        description: String,
        debit: &str,
        credit: &str,
        amount: Decimal,
        at: Timestamp,
    ) -> Posting {
        let amount = Money::rounded(amount, self.currency);
        let line = |account: &str, side| Line {
            account: account.to_owned(),
            side,
            amount,
            dimensions: dimensions(self.facility),
            original: None,
        };
        Posting {
            id,
            at,
            description,
            lines: vec![line(debit, Side::Debit), line(credit, Side::Credit)],
            reverses: None,
        }
    }

    fn target(&self, payment: &Payment) -> String {
        match payment.purpose {
            Purpose::Settlement => self.accounts.receivable.clone(),
            Purpose::Deposit => DEPOSITS.to_owned(),
        }
    }

    fn merchant(&self, method: Method) -> Result<String, PostingError> {
        self.accounts
            .merchants
            .iter()
            .find(|(m, _)| *m == method)
            .map(|(_, account)| account.clone())
            .ok_or(PostingError::NoMerchant(method))
    }

    /// Where the money of a posted payment is: the till's cash, or the method's merchant account.
    fn holding(&self, payment: &Payment, till: Option<&str>) -> Result<String, PostingError> {
        if payment.method == Method::Cash {
            till.map(|t| format!("cash/{t}"))
                .ok_or(PostingError::NoTill)
        } else {
            self.merchant(payment.method)
        }
    }

    /// The posting for `step` of `payment` (`till` for cash).
    pub fn post(
        &self,
        payment: &Payment,
        step: Step,
        till: Option<&str>,
        at: Timestamp,
    ) -> Result<Posting, PostingError> {
        let target = self.target(payment);
        let wrong = || PostingError::WrongStep(step, payment.status);
        let pending = matches!(payment.status, Status::Pending | Status::PendingUnverified);
        let (debit, credit, amount, suffix) = match step {
            Step::Recorded
                if payment.method == Method::Cash && payment.status == Status::Posted =>
            {
                (
                    self.holding(payment, till)?,
                    target,
                    payment.amount,
                    "taken",
                )
            }
            Step::Recorded if pending => (IN_TRANSIT.to_owned(), target, payment.amount, "pending"),
            Step::Confirmed if payment.status == Status::Posted && payment.method.two_phase() => (
                self.merchant(payment.method)?,
                IN_TRANSIT.to_owned(),
                payment.amount,
                "confirmed",
            ),
            Step::Ended if matches!(payment.status, Status::Voided | Status::Expired) => {
                (target, IN_TRANSIT.to_owned(), payment.amount, "ended")
            }
            Step::Reversed if payment.status == Status::Reversed => (
                target,
                self.holding(payment, till)?,
                payment.amount,
                "reversed",
            ),
            Step::Refunded(amount)
                if payment.status == Status::Posted && amount > Decimal::ZERO =>
            {
                (target, self.holding(payment, till)?, amount, "refunded")
            }
            _ => return Err(wrong()),
        };
        Ok(self.posting(
            format!("{}-{suffix}", payment.id),
            format!("Payment {} {suffix} ({:?})", payment.id, payment.method),
            &debit,
            &credit,
            amount,
            at,
        ))
    }

    /// The till-variance posting of a settled shift, if it had a variance.
    pub fn variance(&self, shift: &Shift, at: Timestamp) -> Option<Posting> {
        let variance = shift.variance()?;
        if variance.is_zero() {
            return None;
        }
        let cash = format!("cash/{}", shift.till);
        let (debit, credit) = if variance < Decimal::ZERO {
            (TILL_VARIANCE.to_owned(), cash)
        } else {
            (cash, TILL_VARIANCE.to_owned())
        };
        Some(self.posting(
            format!("{}-variance", shift.id),
            format!("Till variance of shift {}", shift.id),
            &debit,
            &credit,
            variance.abs(),
            at,
        ))
    }
}
