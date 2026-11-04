//! Price Books and Price Rules (pricing-and-insurance.md § Records; ADR 0031): effective-dated,
//! and immutable once one finance approver approves them.

use health_time::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Where a Price Book applies, most specific first: a payer contract's book beats a program's,
/// and so on down to the tenant's.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "level", content = "id")]
pub enum Scope {
    /// A payer contract.
    PayerContract(String),
    /// A program, e.g. ART.
    Program(String),
    /// A department.
    Department(String),
    /// A facility.
    Facility(String),
    /// An organization.
    Organization(String),
    /// The whole tenant.
    Tenant,
}

impl Scope {
    /// How specific it is: 0 is the most specific.
    pub fn rank(&self) -> u8 {
        match self {
            Self::PayerContract(_) => 0,
            Self::Program(_) => 1,
            Self::Department(_) => 2,
            Self::Facility(_) => 3,
            Self::Organization(_) => 4,
            Self::Tenant => 5,
        }
    }
}

/// An effective period: from `from`, until `to` (exclusive) if set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effective {
    /// When it starts.
    pub from: Timestamp,
    /// When it ends, exclusive.
    #[serde(default)]
    pub to: Option<Timestamp>,
}

impl Effective {
    /// Whether it is in effect at `at`.
    pub fn covers(&self, at: Timestamp) -> bool {
        self.from <= at && self.to.is_none_or(|to| at < to)
    }

    /// Whether it shares any instant with `other`.
    pub fn overlaps(&self, other: &Self) -> bool {
        self.to.is_none_or(|to| other.from < to) && other.to.is_none_or(|to| self.from < to)
    }
}

/// Where a book or rule is in its life.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "status")]
pub enum Approval {
    /// Being edited.
    Draft,
    /// Approved, and immutable from then on.
    Approved {
        /// The finance approver.
        by: String,
        /// When.
        at: Timestamp,
    },
}

/// A quantity tier: from `min` units, each costs `unit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tier {
    /// The least quantity it applies to.
    pub min: Decimal,
    /// The unit price from there.
    pub unit: Decimal,
}

/// One priced item of a book: a Healthcare Service or a product.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// The item's code.
    pub item: String,
    /// The unit price.
    pub unit: Decimal,
    /// Quantity tiers within the base price: the highest tier the quantity reaches sets the unit
    /// price.
    #[serde(default)]
    pub tiers: Vec<Tier>,
}

/// A versioned Price Book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceBook {
    /// Its id.
    pub id: String,
    /// Its version.
    pub version: u32,
    /// Where it applies.
    pub scope: Scope,
    /// Its currency (ISO 4217): the facility ledger's functional currency.
    pub currency: String,
    /// When it is in effect.
    pub effective: Effective,
    /// Its approval.
    pub approval: Approval,
    /// Its prices.
    pub entries: Vec<Entry>,
}

/// The stage a rule adjusts at (stages 1, 4 and 6 have no rules: base price, coverage split and
/// manual override).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// Stage 2: surcharges, e.g. after hours.
    Surcharge,
    /// Stage 3: discounts and bundles.
    Discount,
    /// Stage 5: subsidies and waivers, on the patient share only.
    Subsidy,
}

/// What a rule does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Adjustment {
    /// A percentage of the amount it applies to (a surcharge adds it, a discount or subsidy
    /// takes it off).
    Percent {
        /// The percentage, e.g. `20`.
        percent: Decimal,
    },
    /// A fixed amount per line.
    Amount {
        /// The amount.
        amount: Decimal,
    },
    /// Stage 3 only: these items, all on the same charge, cost `price` together. The bundled
    /// lines are priced at zero, each referencing the bundle line.
    Bundle {
        /// The bundled items.
        items: Vec<String>,
        /// The bundle's price.
        price: Decimal,
        /// The bundle line's item code.
        item: String,
    },
}

/// A versioned Price Rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceRule {
    /// Its id.
    pub id: String,
    /// Its version.
    pub version: u32,
    /// Its stage.
    pub stage: Stage,
    /// Its priority within the stage: lower runs first.
    pub priority: u32,
    /// The items it may apply to; empty for any item.
    #[serde(default)]
    pub items: Vec<String>,
    /// When it applies, as FHIRPath over the pricing context; `None` always applies.
    #[serde(default)]
    pub applicability: Option<String>,
    /// What it does.
    pub adjustment: Adjustment,
    /// When it is in effect.
    pub effective: Effective,
    /// Its approval.
    pub approval: Approval,
}

impl PriceRule {
    /// Whether it may apply to `item` (before its applicability).
    pub fn names(&self, item: &str) -> bool {
        match &self.adjustment {
            Adjustment::Bundle { items, .. } => items.iter().any(|i| i == item),
            _ => self.items.is_empty() || self.items.iter().any(|i| i == item),
        }
    }

    /// Whether two rules could both apply to the same item.
    fn shares_items(&self, other: &Self) -> bool {
        let items = |rule: &Self| match &rule.adjustment {
            Adjustment::Bundle { items, .. } => items.clone(),
            _ => rule.items.clone(),
        };
        let (mine, theirs) = (items(self), items(other));
        mine.is_empty() || theirs.is_empty() || mine.iter().any(|i| theirs.contains(i))
    }
}

/// Why a set of rules is refused at validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleProblem {
    /// Two rules of the same stage and priority could both apply.
    #[error("{0} and {1} share a stage and a priority and could both apply")]
    Conflict(String, String),
    /// A bundle outside stage 3, or a bundle of fewer than two items.
    #[error("{0} is a bundle that is not a stage-3 rule of two or more items")]
    BadBundle(String),
    /// A negative or out-of-range adjustment.
    #[error("{0} adjusts by a negative amount or by more than 100%")]
    BadAdjustment(String),
}

/// Every reason `rules` may not be approved together. Applicability cannot be decided ahead of
/// time, so two rules of the same stage and priority whose periods and items overlap conflict.
pub fn validate(rules: &[PriceRule]) -> Vec<RuleProblem> {
    let mut problems = Vec::new();
    for (index, rule) in rules.iter().enumerate() {
        match &rule.adjustment {
            Adjustment::Bundle { items, price, .. } => {
                if rule.stage != Stage::Discount || items.len() < 2 || *price < Decimal::ZERO {
                    problems.push(RuleProblem::BadBundle(rule.id.clone()));
                }
            }
            Adjustment::Percent { percent } => {
                let over = rule.stage != Stage::Surcharge && *percent > Decimal::ONE_HUNDRED;
                if *percent < Decimal::ZERO || over {
                    problems.push(RuleProblem::BadAdjustment(rule.id.clone()));
                }
            }
            Adjustment::Amount { amount } => {
                if *amount < Decimal::ZERO {
                    problems.push(RuleProblem::BadAdjustment(rule.id.clone()));
                }
            }
        }
        for other in &rules[index + 1..] {
            if rule.id != other.id
                && rule.stage == other.stage
                && rule.priority == other.priority
                && rule.effective.overlaps(&other.effective)
                && rule.shares_items(other)
            {
                problems.push(RuleProblem::Conflict(rule.id.clone(), other.id.clone()));
            }
        }
    }
    problems
}
