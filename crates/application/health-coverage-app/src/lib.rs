//! Coverage for pricing and claims (pricing-and-insurance.md § Accumulators; ADR 0031; #125).
//!
//! - **[`pricing_terms`]**: a patient's coverages in force on the service day, primary first, as
//!   pricing's stage-4 terms. Where a node prices from a cached accumulator snapshot (a facility,
//!   offline or not), each split is marked an **estimate**; only central's own accumulators give
//!   final terms.
//! - **[`settle_claim`]**: at claim time, central walks a claim's charges in service order with
//!   its own accumulators. A charge priced as an estimate is priced again with central's terms,
//!   and every line whose split differs gives a [`Correction`]: a Reconciliation Record and a
//!   patient-share adjustment posting. Each charge's use (deductible taken, coverage paid) then
//!   runs the accumulators on, so the next charge sees it. A charge already taken in is skipped.
//! - **[`Coverages`]**: the same over a store at central, with eligibility kept per coverage.
//! - **[`remittance_posting`]** (#126): what a payer paid against a claim, against its
//!   receivable; [`shortfall_posting`]: what it did not pay, moved to the patient.

use std::collections::BTreeMap;

mod central;
mod remittance;

pub use central::{CentralError, Claim, Coverages};
pub use remittance::{remittance_posting, shortfall_posting};

use health_coverage::{
    Accumulator, Basis, BenefitPlan, Copay, Correction, Coverage, Snapshot, Split, Terms,
    corrections, in_force, terms,
};
use health_pricing::{
    Applicability, Copay as PricingCopay, CoverageTerms, PriceBook, PriceRule, Priced,
    PricingError, Request, price,
};
use jiff::tz::TimeZone;

/// Why a claim could not be settled.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoverageError {
    /// A charge's coverage has no Benefit Plan at hand.
    #[error("no Benefit Plan {0}")]
    NoPlan(String),
    /// A charge could not be priced again.
    #[error(transparent)]
    Pricing(#[from] PricingError),
}

/// One coverage's terms, in pricing's form.
#[must_use]
pub fn to_pricing(terms: &Terms) -> CoverageTerms {
    CoverageTerms {
        coverage: terms.coverage.clone(),
        excluded: terms.excluded.clone(),
        copay: match terms.copay {
            Copay::None => PricingCopay::None,
            Copay::Percent(p) => PricingCopay::Percent(p),
            Copay::Amount(a) => PricingCopay::Amount(a),
        },
        deductible_remaining: terms.deductible_remaining,
        limit_remaining: terms.limit_remaining,
        estimate: terms.estimate,
    }
}

/// Where a node's accumulators come from.
#[derive(Debug, Clone, Copy)]
pub enum Accumulators<'a> {
    /// Central's own, by coverage and year: final terms.
    Central(&'a BTreeMap<(String, i16), Accumulator>),
    /// A facility's cached snapshots, by coverage: estimated terms.
    Cached(&'a BTreeMap<String, Snapshot>),
}

/// The benefit year of a service time.
fn year(request: &Request) -> i16 {
    request.service_time.to_zoned(TimeZone::UTC).year()
}

/// The terms of `coverages` in force on the day of `request`'s service, primary first, under
/// `plans`, from `accumulators`.
pub fn pricing_terms(
    request: &Request,
    coverages: &[Coverage],
    plans: &[BenefitPlan],
    accumulators: Accumulators<'_>,
) -> Result<Vec<CoverageTerms>, CoverageError> {
    let day = request.service_time.to_zoned(TimeZone::UTC).date();
    in_force(coverages, day)
        .into_iter()
        .map(|coverage| {
            let plan = plans
                .iter()
                .find(|p| p.id == coverage.plan)
                .ok_or_else(|| CoverageError::NoPlan(coverage.plan.clone()))?;
            let basis = match accumulators {
                Accumulators::Central(held) => held
                    .get(&(coverage.id.clone(), year(request)))
                    .map_or(Basis::Unknown, Basis::Authoritative),
                Accumulators::Cached(held) => {
                    held.get(&coverage.id).map_or(Basis::Unknown, Basis::Cached)
                }
            };
            let mut final_terms = terms(coverage, plan, basis);
            // At central with no use yet, the accumulator is simply empty: final, not unknown.
            if matches!(accumulators, Accumulators::Central(_)) {
                final_terms.estimate = false;
            }
            Ok(to_pricing(&final_terms))
        })
        .collect()
}

/// A charge on a claim: what was priced, and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimCharge {
    /// The charge.
    pub charge: String,
    /// The pricing request, with the coverage terms it was priced with.
    pub request: Request,
    /// What was recorded.
    pub priced: Priced,
}

/// What settling a claim found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settlement {
    /// Each charge's corrections, by charge.
    pub corrections: Vec<(String, Correction)>,
    /// Central's accumulators after the claim's charges.
    pub accumulators: BTreeMap<(String, i16), Accumulator>,
    /// The charges priced again (they were estimates).
    pub repriced: Vec<String>,
}

fn splits(priced: &Priced) -> Vec<Split> {
    priced
        .lines
        .iter()
        .flat_map(|line| {
            line.payer_shares.iter().map(move |(coverage, paid)| Split {
                line: line.id.clone(),
                coverage: coverage.clone(),
                payer_share: *paid,
                patient_share: line.patient_share,
            })
        })
        .collect()
}

/// Settles `charges` at central, from its `accumulators`: see the module docs.
pub fn settle_claim(
    charges: &[ClaimCharge],
    coverages: &[Coverage],
    plans: &[BenefitPlan],
    mut accumulators: BTreeMap<(String, i16), Accumulator>,
    books: &[PriceBook],
    rules: &[PriceRule],
    applicability: &dyn Applicability,
) -> Result<Settlement, CoverageError> {
    let mut ordered: Vec<&ClaimCharge> = charges.iter().collect();
    ordered.sort_by(|a, b| {
        a.request
            .service_time
            .cmp(&b.request.service_time)
            .then_with(|| a.charge.cmp(&b.charge))
    });
    let mut found = Vec::new();
    let mut repriced = Vec::new();
    for charge in ordered {
        let year = year(&charge.request);
        // Taken in before (a settlement replayed): nothing to correct or consume again.
        let covered: Vec<&String> = charge
            .priced
            .lines
            .iter()
            .flat_map(|l| l.payer_shares.iter().map(|(c, _)| c))
            .collect();
        if !covered.is_empty()
            && covered.iter().all(|c| {
                accumulators
                    .get(&((*c).clone(), year))
                    .is_some_and(|a| a.has(&charge.charge))
            })
        {
            continue;
        }
        let estimated = charge.priced.lines.iter().any(|l| l.estimate);
        let used = if estimated {
            let request = Request {
                coverages: pricing_terms(
                    &charge.request,
                    coverages,
                    plans,
                    Accumulators::Central(&accumulators),
                )?,
                ..charge.request.clone()
            };
            let again = price(&request, books, rules, applicability)?;
            for correction in corrections(&splits(&charge.priced), &splits(&again)) {
                found.push((charge.charge.clone(), correction));
            }
            repriced.push(charge.charge.clone());
            again
        } else {
            charge.priced.clone()
        };
        for line in &used.lines {
            for (coverage, paid) in &line.payer_shares {
                let deductible = line
                    .deductibles
                    .iter()
                    .find(|(c, _)| c == coverage)
                    .map_or(rust_decimal::Decimal::ZERO, |(_, d)| *d);
                accumulators
                    .entry((coverage.clone(), year))
                    .or_insert_with(|| Accumulator::new(coverage, year))
                    .consume(deductible, *paid);
            }
        }
        for line in &used.lines {
            for (coverage, _) in &line.payer_shares {
                if let Some(accumulator) = accumulators.get_mut(&(coverage.clone(), year)) {
                    accumulator.took(&charge.charge);
                }
            }
        }
    }
    Ok(Settlement {
        corrections: found,
        accumulators,
        repriced,
    })
}
