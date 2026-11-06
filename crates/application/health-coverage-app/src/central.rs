//! Coverage over a store (#125): central's accumulators and each coverage's eligibility.
//!
//! - **[`Coverages::settle`]** settles a claim's charges in one transaction. It reads central's
//!   accumulators for the claim's coverages and years, runs [`crate::settle_claim`], and writes
//!   them back. Each accumulator is the guard of its coverage and year, so two settlements of the
//!   same coverage conflict and one is rerun with the other's use. A charge already taken in is
//!   not taken in again.
//! - **[`Coverages::snapshot`]**: an accumulator as a facility caches it, to price estimates.
//! - **[`Coverages::check_eligibility`]** asks the payer and keeps the answer. If the payer cannot
//!   be reached, the kept answer stands, pending verification.

use std::collections::BTreeMap;

use health_coverage::{
    Accumulator, BenefitPlan, Coverage, CoverageStore, Eligibility, Payer, Snapshot, check,
};
use health_persistence::{Store, StoreError, Transaction, WriteError};
use health_pricing::{Applicability, PriceBook, PriceRule};
use health_time::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;

use crate::{ClaimCharge, CoverageError, Settlement, settle_claim};

/// How many times a settlement is rerun after losing a write conflict.
const ATTEMPTS: u32 = 8;

/// Why a stored coverage use case failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CentralError {
    /// The claim could not be settled.
    #[error(transparent)]
    Settle(#[from] CoverageError),
    /// Concurrent settlements kept winning; try again.
    #[error("the coverage is busy; try again")]
    Busy,
    /// The store failed.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for CentralError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

/// Coverage at central, over `store`.
pub struct Coverages<S> {
    store: S,
}

/// What a claim's settlement needs besides its charges.
pub struct Claim<'a> {
    /// The claim's charges.
    pub charges: &'a [ClaimCharge],
    /// The patient's coverages.
    pub coverages: &'a [Coverage],
    /// Their Benefit Plans.
    pub plans: &'a [BenefitPlan],
    /// The Price Books.
    pub books: &'a [PriceBook],
    /// The Price Rules.
    pub rules: &'a [PriceRule],
    /// The rules' applicability.
    pub applicability: &'a (dyn Applicability + Sync),
}

impl<S> Coverages<S>
where
    S: Store,
    S::Tx: CoverageStore,
{
    /// The use cases over `store`.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Settles `claim` against central's accumulators, keeping them.
    pub async fn settle(&self, claim: &Claim<'_>) -> Result<Settlement, CentralError> {
        let years: Vec<i16> = claim
            .charges
            .iter()
            .map(|c| c.request.service_time.to_zoned(TimeZone::UTC).year())
            .collect();
        for _ in 0..ATTEMPTS {
            let mut tx = self.store.begin().await?;
            let mut held = BTreeMap::new();
            for coverage in claim.coverages {
                for year in &years {
                    if let Some(accumulator) = tx.accumulator(&coverage.id, *year).await? {
                        held.insert((coverage.id.clone(), *year), accumulator);
                    }
                }
            }
            let before = held.clone();
            let settled = settle_claim(
                claim.charges,
                claim.coverages,
                claim.plans,
                held,
                claim.books,
                claim.rules,
                claim.applicability,
            )?;
            let mut written = Ok(());
            for (key, accumulator) in &settled.accumulators {
                if before.get(key) != Some(accumulator) {
                    written = tx.put_accumulator(accumulator).await;
                    if written.is_err() {
                        break;
                    }
                }
            }
            match written {
                Ok(()) => match tx.commit().await {
                    Ok(()) => return Ok(settled),
                    Err(WriteError::Conflict) => {}
                    Err(other) => return Err(CentralError::Store(other.to_string())),
                },
                Err(WriteError::Conflict) => {}
                Err(other) => return Err(CentralError::Store(other.to_string())),
            }
        }
        Err(CentralError::Busy)
    }

    /// Central's accumulator of `coverage` for `year` as a facility caches it at `now`.
    pub async fn snapshot(
        &self,
        coverage: &str,
        year: i16,
        now: Timestamp,
    ) -> Result<Snapshot, CentralError> {
        let mut tx = self.store.begin().await?;
        let accumulator = tx
            .accumulator(coverage, year)
            .await?
            .unwrap_or_else(|| Accumulator::new(coverage, year));
        Ok(Snapshot {
            accumulator,
            as_of: now,
        })
    }

    /// Asks `payer` whether `coverage` is eligible on `day`, and keeps the answer. If the payer
    /// cannot be reached, the kept answer stands, pending verification (and is kept as such).
    pub async fn check_eligibility<P: Payer>(
        &self,
        payer: &P,
        coverage: &Coverage,
        day: Date,
        now: Timestamp,
    ) -> Result<Eligibility, CentralError> {
        let cached = {
            let mut tx = self.store.begin().await?;
            tx.eligibility(&coverage.id).await?
        };
        let answer = check(payer, coverage, day, cached.as_ref(), now).await;
        let mut tx = self.store.begin().await?;
        tx.put_eligibility(&answer)
            .await
            .map_err(|e| CentralError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| CentralError::Store(e.to_string()))?;
        Ok(answer)
    }
}
