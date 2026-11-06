//! Batch selection: first expiry, first out, with eligibility and an expiry buffer (ADR 0035;
//! WHO GSDP §12.30).

use std::collections::BTreeMap;

use jiff::ToSpan;
use jiff::civil::Date;

use crate::{Status, StockUnit};

/// What a dispense needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Need {
    /// The product.
    pub product: String,
    /// How much, in base units.
    pub quantity: i64,
    /// The last day of the course.
    pub course_ends: Date,
    /// The programs the patient is enrolled in (for donor and program stock).
    pub programs: Vec<String>,
}

/// The store's batch rule: how far past the course a batch must last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BatchRule {
    /// Days after the course ends that a batch must still be in date.
    pub buffer_days: i32,
}

/// The funding-source rules: which program a patient must be in to receive stock a funding
/// source paid for. A funding source without a rule is open to everyone.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Eligibility {
    rules: BTreeMap<String, String>,
}

impl Eligibility {
    /// The rules, `(funding source, program)`.
    pub fn new<'a>(rules: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        Self {
            rules: rules
                .into_iter()
                .map(|(f, p)| (f.to_owned(), p.to_owned()))
                .collect(),
        }
    }

    /// Whether a patient in `programs` may receive stock `funding_source` paid for.
    #[must_use]
    pub fn allows(&self, funding_source: &str, programs: &[String]) -> bool {
        self.rules
            .get(funding_source)
            .is_none_or(|program| programs.iter().any(|p| p == program))
    }
}

/// A Stock Unit that might be used, with what is available of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The unit.
    pub unit: StockUnit,
    /// What is available of it now.
    pub available: i64,
}

/// How much to take from one unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    /// The unit.
    pub unit: StockUnit,
    /// How much, in base units.
    pub quantity: i64,
}

/// Why no batch could be chosen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SelectRefused {
    /// The need is not a positive quantity.
    #[error("the quantity must be positive")]
    BadQuantity,
    /// Not enough stock may be used for this patient and course.
    #[error("not enough usable stock: {usable} may be used")]
    NotEnough {
        /// What may be used.
        usable: i64,
    },
    /// The unit is not the product, not usable, or not in date for the course.
    #[error("{0} may not be used for this dispense")]
    Unsuitable(String),
    /// The unit's funding source is for patients in a program this patient is not in.
    #[error("{0} is kept for patients in its program")]
    NotEligible(String),
    /// A choice other than first expiry, first out, needs a reason.
    #[error("choosing other than first expiry needs a reason")]
    ReasonRequired,
}

/// Whether `unit` may be used for `need`.
fn suitable(
    unit: &StockUnit,
    need: &Need,
    eligibility: &Eligibility,
    rule: BatchRule,
) -> Result<(), SelectRefused> {
    let lasts_until = need
        .course_ends
        .checked_add(rule.buffer_days.days())
        .unwrap_or(need.course_ends);
    let in_date = unit
        .expiry
        .parse::<Date>()
        .is_ok_and(|expiry| expiry > lasts_until);
    if unit.product != need.product || unit.status != Status::Usable || !in_date {
        return Err(SelectRefused::Unsuitable(unit.key()));
    }
    if !eligibility.allows(&unit.funding_source, &need.programs) {
        return Err(SelectRefused::NotEligible(unit.key()));
    }
    Ok(())
}

/// The batches to take `need` from, first expiry first, then the smallest quantity, then by key;
/// several when one is not enough.
pub fn fefo(
    candidates: &[Candidate],
    need: &Need,
    eligibility: &Eligibility,
    rule: BatchRule,
) -> Result<Vec<Pick>, SelectRefused> {
    if need.quantity <= 0 {
        return Err(SelectRefused::BadQuantity);
    }
    let mut usable: Vec<&Candidate> = candidates
        .iter()
        .filter(|c| c.available > 0 && suitable(&c.unit, need, eligibility, rule).is_ok())
        .collect();
    usable.sort_by(|a, b| {
        a.unit
            .expiry
            .cmp(&b.unit.expiry)
            .then(a.available.cmp(&b.available))
            .then_with(|| a.unit.key().cmp(&b.unit.key()))
    });
    let mut left = need.quantity;
    let mut picks = Vec::new();
    for candidate in usable {
        if left == 0 {
            break;
        }
        let take = left.min(candidate.available);
        picks.push(Pick {
            unit: candidate.unit.clone(),
            quantity: take,
        });
        left -= take;
    }
    if left > 0 {
        return Err(SelectRefused::NotEnough {
            usable: need.quantity - left,
        });
    }
    Ok(picks)
}

/// A pharmacist's own choice of batches. Each must be suitable and eligible, and available in
/// that quantity; together they must meet the need. A choice that differs from [`fefo`]'s needs a
/// `reason`.
pub fn chosen(
    candidates: &[Candidate],
    choice: &[Pick],
    need: &Need,
    eligibility: &Eligibility,
    rule: BatchRule,
    reason: Option<&str>,
) -> Result<Vec<Pick>, SelectRefused> {
    let total: i64 = choice.iter().map(|p| p.quantity).sum();
    if need.quantity <= 0 || choice.iter().any(|p| p.quantity <= 0) || total != need.quantity {
        return Err(SelectRefused::BadQuantity);
    }
    for pick in choice {
        suitable(&pick.unit, need, eligibility, rule)?;
        let available = candidates
            .iter()
            .find(|c| c.unit == pick.unit)
            .map_or(0, |c| c.available);
        if available < pick.quantity {
            return Err(SelectRefused::NotEnough { usable: available });
        }
    }
    let first = fefo(candidates, need, eligibility, rule)?;
    if first != choice && reason.is_none_or(|r| r.trim().is_empty()) {
        return Err(SelectRefused::ReasonRequired);
    }
    Ok(choice.to_vec())
}
