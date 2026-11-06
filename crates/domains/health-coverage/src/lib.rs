//! Coverage (pricing-and-insurance.md § Records, § Accumulators, § Eligibility; ADR 0031; #125).
//!
//! - **[`BenefitPlan`]** (InsurancePlan): the copay, deductible, limit and exclusions.
//! - **[`Coverage`]**: a beneficiary's coverage by a payer under a plan, with its `order`
//!   (primary first) and period. [`in_force`] gives those that apply on a day, in order.
//! - **[`Accumulator`]**: what a coverage's deductible and limit have used in a benefit year. It
//!   is **kept at central**, across facilities. A facility prices from its last cached
//!   [`Snapshot`], so the split is an **estimate**; only central's own accumulator gives
//!   [`Basis::Authoritative`] terms ([`terms`]).
//! - **Eligibility** ([`check`]): asked of the payer through the [`Payer`] port. Offline, or when
//!   the payer cannot be reached, it is the last cached answer, marked
//!   [`EligibilityStatus::PendingVerification`], with its age.
//! - **Correction at claim time** ([`corrections`]): central re-prices the estimated charges with
//!   its own accumulators, and each difference in a line's split is a [`Correction`]: a
//!   Reconciliation Record, and an adjustment of the patient's share.

use std::future::Future;

use health_time::{SignedDuration, Timestamp};
use jiff::civil::Date;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// A plan's copay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "value")]
pub enum Copay {
    /// None.
    None,
    /// A percentage of what the coverage would pay.
    Percent(Decimal),
    /// A fixed amount per line.
    Amount(Decimal),
}

/// A Benefit Plan (InsurancePlan): what a coverage pays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenefitPlan {
    /// The plan.
    pub id: String,
    /// Its version.
    pub version: u32,
    /// The copay.
    pub copay: Copay,
    /// The deductible each benefit year.
    pub deductible: Decimal,
    /// The most it pays each benefit year, if it has a limit.
    pub limit: Option<Decimal>,
    /// The items it does not cover.
    pub excluded: Vec<String>,
}

/// Where a Coverage is (R5 `Coverage.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoverageStatus {
    /// In force.
    Active,
    /// Not yet in force.
    Draft,
    /// Ended.
    Cancelled,
    /// Recorded by mistake.
    EnteredInError,
}

/// A beneficiary's coverage by a payer under a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    /// The Coverage.
    pub id: String,
    /// The patient covered, `Patient/{id}`.
    pub beneficiary: String,
    /// The payer.
    pub payer: String,
    /// The Benefit Plan.
    pub plan: String,
    /// Primary (1), secondary (2), …
    pub order: u32,
    /// Its status.
    pub status: CoverageStatus,
    /// The first day it applies.
    pub start: Date,
    /// The last day it applies, if it ends.
    pub end: Option<Date>,
}

impl Coverage {
    /// Whether it applies on `day`.
    #[must_use]
    pub fn applies(&self, day: Date) -> bool {
        self.status == CoverageStatus::Active
            && self.start <= day
            && self.end.is_none_or(|end| day <= end)
    }
}

/// The coverages of `coverages` that apply on `day`, primary first (then by id).
pub fn in_force(coverages: &[Coverage], day: Date) -> Vec<&Coverage> {
    let mut applying: Vec<&Coverage> = coverages.iter().filter(|c| c.applies(day)).collect();
    applying.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
    applying
}

/// What a coverage has used in one benefit year (kept at central).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accumulator {
    /// The Coverage.
    pub coverage: String,
    /// The benefit year.
    pub year: i16,
    /// What the deductible has taken.
    pub deductible_used: Decimal,
    /// What the coverage has paid.
    pub paid: Decimal,
}

impl Accumulator {
    /// Nothing used yet.
    #[must_use]
    pub fn new(coverage: &str, year: i16) -> Self {
        Self {
            coverage: coverage.to_owned(),
            year,
            deductible_used: Decimal::ZERO,
            paid: Decimal::ZERO,
        }
    }

    /// What remains of `plan`'s deductible, and of its limit if it has one.
    #[must_use]
    pub fn remaining(&self, plan: &BenefitPlan) -> (Decimal, Option<Decimal>) {
        (
            (plan.deductible - self.deductible_used).max(Decimal::ZERO),
            plan.limit
                .map(|limit| (limit - self.paid).max(Decimal::ZERO)),
        )
    }

    /// Takes in one priced line's use: what the deductible took and what the coverage paid.
    pub fn consume(&mut self, deductible: Decimal, paid: Decimal) {
        self.deductible_used += deductible;
        self.paid += paid;
    }
}

/// An accumulator as a facility last received it from central.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// The accumulator then.
    pub accumulator: Accumulator,
    /// When central sent it.
    pub as_of: Timestamp,
}

/// What a split is priced from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis<'a> {
    /// Central's own accumulator: the split is final.
    Authoritative(&'a Accumulator),
    /// A facility's cached snapshot: the split is an estimate.
    Cached(&'a Snapshot),
    /// No accumulator at hand: the whole deductible is assumed left, and the split is an estimate.
    Unknown,
}

/// One coverage's terms for pricing's stage 4, in coverage order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Terms {
    /// The Coverage.
    pub coverage: String,
    /// Items the plan excludes.
    pub excluded: Vec<String>,
    /// The copay.
    pub copay: Copay,
    /// What remains of the deductible.
    pub deductible_remaining: Decimal,
    /// What remains of the limit, if it has one.
    pub limit_remaining: Option<Decimal>,
    /// The split is an estimate, to be corrected at claim time (ADR 0031).
    pub estimate: bool,
}

/// The terms `coverage` under `plan` prices with, from `basis`.
#[must_use]
pub fn terms(coverage: &Coverage, plan: &BenefitPlan, basis: Basis<'_>) -> Terms {
    let (deductible_remaining, limit_remaining, estimate) = match basis {
        Basis::Authoritative(accumulator) => {
            let (deductible, limit) = accumulator.remaining(plan);
            (deductible, limit, false)
        }
        Basis::Cached(snapshot) => {
            let (deductible, limit) = snapshot.accumulator.remaining(plan);
            (deductible, limit, true)
        }
        Basis::Unknown => (plan.deductible, plan.limit, true),
    };
    Terms {
        coverage: coverage.id.clone(),
        excluded: plan.excluded.clone(),
        copay: plan.copay,
        deductible_remaining,
        limit_remaining,
        estimate,
    }
}

/// What a payer said of a coverage, or why it could not be asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EligibilityStatus {
    /// The payer confirmed it.
    Eligible,
    /// The payer refused it.
    NotEligible,
    /// The payer could not be asked: the last answer stands until it can be.
    PendingVerification,
}

/// A coverage's eligibility (CoverageEligibilityResponse), as the node holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Eligibility {
    /// The Coverage.
    pub coverage: String,
    /// Its status.
    pub status: EligibilityStatus,
    /// What the payer last said, if it ever answered.
    pub last_answer: Option<bool>,
    /// When the payer last answered.
    pub answered_at: Option<Timestamp>,
}

impl Eligibility {
    /// How old the payer's last answer is at `now`, if there is one.
    #[must_use]
    pub fn age(&self, now: Timestamp) -> Option<SignedDuration> {
        self.answered_at.map(|at| now.duration_since(at))
    }
}

/// Why the payer could not answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PayerError {
    /// The payer, or central, could not be reached.
    #[error("the payer could not be reached")]
    Unreachable,
    /// The payer refused the request itself.
    #[error("the payer refused the request: {0}")]
    Refused(String),
}

/// The payer port: one adapter per payer in `health-integration` (FHIR
/// CoverageEligibilityRequest where the payer supports it, a payer format otherwise).
pub trait Payer: Send + Sync {
    /// Whether `coverage` is eligible on `day`.
    fn eligible(
        &self,
        coverage: &Coverage,
        day: Date,
    ) -> impl Future<Output = Result<bool, PayerError>> + Send;
}

/// Asks `payer` whether `coverage` is eligible on `day`. If it cannot be asked, the `cached`
/// answer stands, marked pending verification.
pub async fn check<P: Payer>(
    payer: &P,
    coverage: &Coverage,
    day: Date,
    cached: Option<&Eligibility>,
    now: Timestamp,
) -> Eligibility {
    match payer.eligible(coverage, day).await {
        Ok(eligible) => Eligibility {
            coverage: coverage.id.clone(),
            status: if eligible {
                EligibilityStatus::Eligible
            } else {
                EligibilityStatus::NotEligible
            },
            last_answer: Some(eligible),
            answered_at: Some(now),
        },
        Err(_) => pending(coverage, cached),
    }
}

/// The eligibility of `coverage` offline: the cached answer, pending verification.
#[must_use]
pub fn pending(coverage: &Coverage, cached: Option<&Eligibility>) -> Eligibility {
    Eligibility {
        coverage: coverage.id.clone(),
        status: EligibilityStatus::PendingVerification,
        last_answer: cached.and_then(|c| c.last_answer),
        answered_at: cached.and_then(|c| c.answered_at),
    }
}

/// One line's split for one coverage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Split {
    /// The charge line.
    pub line: String,
    /// The Coverage.
    pub coverage: String,
    /// What the coverage pays.
    pub payer_share: Decimal,
    /// What the patient pays of the line.
    pub patient_share: Decimal,
}

/// A difference found at claim time between a line's estimated split and central's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Correction {
    /// The charge line.
    pub line: String,
    /// The Coverage.
    pub coverage: String,
    /// What the coverage pays more (negative: less) than estimated.
    pub payer_delta: Decimal,
    /// What the patient pays more (negative: less) than estimated: the adjustment posting.
    pub patient_delta: Decimal,
}

/// The corrections from `estimated` to `actual`, by line and coverage. A line and coverage with
/// the same split needs none.
pub fn corrections(estimated: &[Split], actual: &[Split]) -> Vec<Correction> {
    let mut found = Vec::new();
    for now in actual {
        let before = estimated
            .iter()
            .find(|e| e.line == now.line && e.coverage == now.coverage);
        let (payer, patient) = before.map_or((Decimal::ZERO, Decimal::ZERO), |e| {
            (e.payer_share, e.patient_share)
        });
        if payer != now.payer_share || patient != now.patient_share {
            found.push(Correction {
                line: now.line.clone(),
                coverage: now.coverage.clone(),
                payer_delta: now.payer_share - payer,
                patient_delta: now.patient_share - patient,
            });
        }
    }
    found
}

#[cfg(test)]
mod tests;
