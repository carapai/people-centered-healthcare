//! A Price Book's and a Price Rule's life (pricing-and-insurance.md § Records; ADR 0031): drafted
//! at central, approved once by one finance approver who is not the author, and immutable from
//! then on. The only change an approved version takes is retirement from a time not yet passed,
//! so no price a charge was given ever changes. Changing a price is a new version.

use std::collections::BTreeSet;
use std::future::Future;

use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;
use rust_decimal::Decimal;

use crate::book::{Approval, PriceBook, PriceRule, RuleProblem, validate};
use crate::price::Applied;

/// Why a lifecycle step is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleError {
    /// Only a draft may be approved or edited.
    #[error("only a draft may change, and this version is approved")]
    NotDraft,
    /// Only an approved version may be retired.
    #[error("only an approved version may be retired")]
    NotApproved,
    /// The author may not approve their own draft.
    #[error("the author may not approve their own draft")]
    OwnApproval,
    /// Another approved version of the same book or rule is in effect over part of the period.
    #[error("{0:?} is already approved for part of this period")]
    Overlaps(Applied),
    /// A book with no prices, a negative price, a tier below one unit, or an item twice.
    #[error("the book's prices are not well formed: {0}")]
    BadPrices(String),
    /// The rule conflicts with approved rules, or is malformed.
    #[error("the rule is refused: {0:?}")]
    Rule(Vec<RuleProblem>),
    /// A retirement in the past, before the start, or later than an existing end.
    #[error("a retirement must be from now on, after the start, and no later than any end set")]
    BadRetirement,
}

fn approved(approval: &Approval) -> bool {
    matches!(approval, Approval::Approved { .. })
}

fn check_prices(book: &PriceBook) -> Result<(), LifecycleError> {
    if book.entries.is_empty() {
        return Err(LifecycleError::BadPrices("it has no prices".into()));
    }
    let mut items = BTreeSet::new();
    for entry in &book.entries {
        if !items.insert(entry.item.as_str()) {
            return Err(LifecycleError::BadPrices(format!(
                "{} is priced twice",
                entry.item
            )));
        }
        let bad_tier = entry
            .tiers
            .iter()
            .any(|t| t.min < Decimal::ONE || t.unit < Decimal::ZERO);
        if entry.unit < Decimal::ZERO || bad_tier {
            return Err(LifecycleError::BadPrices(format!(
                "{} has a negative price or a tier below one unit",
                entry.item
            )));
        }
    }
    Ok(())
}

/// Approves draft `book` as `by` at `at`, against the versions of every book (`all`).
pub fn approve_book(
    book: &PriceBook,
    all: &[PriceBook],
    by: &str,
    at: Timestamp,
) -> Result<PriceBook, LifecycleError> {
    if approved(&book.approval) {
        return Err(LifecycleError::NotDraft);
    }
    if book.author == by {
        return Err(LifecycleError::OwnApproval);
    }
    check_prices(book)?;
    if let Some(other) = all.iter().find(|other| {
        other.id == book.id
            && other.version != book.version
            && approved(&other.approval)
            && other.effective.overlaps(&book.effective)
    }) {
        return Err(LifecycleError::Overlaps(Applied {
            id: other.id.clone(),
            version: other.version,
        }));
    }
    Ok(PriceBook {
        approval: Approval::Approved {
            by: by.to_owned(),
            at,
        },
        ..book.clone()
    })
}

/// Approves draft `rule` as `by` at `at`, against every rule version (`all`): it may not
/// overlap another approved version of itself, nor conflict with any approved rule.
pub fn approve_rule(
    rule: &PriceRule,
    all: &[PriceRule],
    by: &str,
    at: Timestamp,
) -> Result<PriceRule, LifecycleError> {
    if approved(&rule.approval) {
        return Err(LifecycleError::NotDraft);
    }
    if rule.author == by {
        return Err(LifecycleError::OwnApproval);
    }
    if let Some(other) = all.iter().find(|other| {
        other.id == rule.id
            && other.version != rule.version
            && approved(&other.approval)
            && other.effective.overlaps(&rule.effective)
    }) {
        return Err(LifecycleError::Overlaps(Applied {
            id: other.id.clone(),
            version: other.version,
        }));
    }
    let mut together: Vec<PriceRule> = all
        .iter()
        .filter(|other| approved(&other.approval) && other.id != rule.id)
        .cloned()
        .collect();
    together.push(rule.clone());
    let problems: Vec<RuleProblem> = validate(&together)
        .into_iter()
        .filter(|problem| match problem {
            RuleProblem::Conflict(a, b) => *a == rule.id || *b == rule.id,
            RuleProblem::BadBundle(id) | RuleProblem::BadAdjustment(id) => *id == rule.id,
        })
        .collect();
    if !problems.is_empty() {
        return Err(LifecycleError::Rule(problems));
    }
    Ok(PriceRule {
        approval: Approval::Approved {
            by: by.to_owned(),
            at,
        },
        ..rule.clone()
    })
}

fn retired(
    approval: &Approval,
    effective: crate::book::Effective,
    to: Timestamp,
    now: Timestamp,
) -> Result<crate::book::Effective, LifecycleError> {
    if !approved(approval) {
        return Err(LifecycleError::NotApproved);
    }
    if to < now || to <= effective.from || effective.to.is_some_and(|end| to > end) {
        return Err(LifecycleError::BadRetirement);
    }
    Ok(crate::book::Effective {
        to: Some(to),
        ..effective
    })
}

/// Ends approved `book` at `to`, which may not have passed (`now`).
pub fn retire_book(
    book: &PriceBook,
    to: Timestamp,
    now: Timestamp,
) -> Result<PriceBook, LifecycleError> {
    Ok(PriceBook {
        effective: retired(&book.approval, book.effective, to, now)?,
        ..book.clone()
    })
}

/// Ends approved `rule` at `to`, which may not have passed (`now`).
pub fn retire_rule(
    rule: &PriceRule,
    to: Timestamp,
    now: Timestamp,
) -> Result<PriceRule, LifecycleError> {
    Ok(PriceRule {
        effective: retired(&rule.approval, rule.effective, to, now)?,
        ..rule.clone()
    })
}

/// Price Books and rules, inside the unit of work's transaction (the pricing Module's driven
/// port). Every version is kept.
pub trait PricingStore {
    /// Adds or replaces a book version.
    fn put_book(&mut self, book: &PriceBook)
    -> impl Future<Output = Result<(), WriteError>> + Send;
    /// Every version of every book.
    fn books(&mut self) -> impl Future<Output = Result<Vec<PriceBook>, StoreError>> + Send;
    /// Adds or replaces a rule version.
    fn put_rule(&mut self, rule: &PriceRule)
    -> impl Future<Output = Result<(), WriteError>> + Send;
    /// Every version of every rule.
    fn rules(&mut self) -> impl Future<Output = Result<Vec<PriceRule>, StoreError>> + Send;
}
