//! What a node keeps of its indicators (analytics.md § Compute and Store; ADR 0068; #554): each
//! patient's membership per indicator and period, and a summary per indicator, period and
//! facility.
//!
//! - **Membership** is each patient's records of the indicator's basis as they came out for one
//!   period ([`PatientOutcome`]), kept only for the periods the patient has care in. A change to
//!   a patient re-evaluates that patient alone; the period's summary is then calculated again
//!   from every patient's kept membership, so it is the same as evaluating the period afresh.
//! - **Summaries** are the figures with what roll-up needs, written as a summary FHIR
//!   `MeasureReport`, per indicator (`url|version`), period and facility: what dashboards read
//!   and what syncs to central. Membership never leaves the node. A period no patient has care
//!   in has no summary, however it came to have none, so the summaries kept are always those a
//!   computation from scratch gives.
//! - **Where it stands** on the node's change log ([`Position`]): the epoch and the last `seq`
//!   taken into account. A new epoch, or a log pruned past it, means computing everything
//!   again.
//!
//! The patients and their records are read here too, as stored JSON, so this crate stays pure.

use std::future::Future;

use health_persistence::{StoreError, WriteError};
use jiff::civil::Date;

use crate::calculate::PatientOutcome;
use crate::indicator::Indicator;
use crate::period::{Period, PeriodType};

/// An indicator's summary for one period at one facility, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSummary {
    /// The indicator, `url|version`.
    pub measure: String,
    /// The period.
    pub period: Period,
    /// The facility it is for.
    pub facility: String,
    /// The summary `MeasureReport`, as JSON.
    pub report: String,
}

/// Where a node's indicators stand on its change log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    /// The change log's epoch.
    pub epoch: String,
    /// The last `seq` taken into account.
    pub seq: u64,
}

/// One patient's membership in one indicator and period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberKey {
    /// The indicator, `url|version`.
    pub measure: String,
    /// The period's type.
    pub kind: PeriodType,
    /// The period's id.
    pub period: String,
}

/// The node's indicator records, over a transaction.
pub trait IndicatorStore {
    /// Every patient's kept membership in `measure` for `period`.
    fn members(
        &mut self,
        measure: &str,
        period: &Period,
    ) -> impl Future<Output = Result<Vec<PatientOutcome>, StoreError>> + Send;

    /// Every indicator and period `patient` (`Patient/<id>`) has a membership kept in.
    fn memberships_of(
        &mut self,
        patient: &str,
    ) -> impl Future<Output = Result<Vec<MemberKey>, StoreError>> + Send;

    /// Keeps `outcome` as its patient's membership in `measure` for `period`, replacing any.
    fn put_member(
        &mut self,
        measure: &str,
        period: &Period,
        outcome: &PatientOutcome,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Forgets `patient`'s membership in `measure` for `period`.
    fn remove_member(
        &mut self,
        measure: &str,
        period: &Period,
        patient: &str,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Keeps `summary`, replacing the one for its indicator, period and facility.
    fn put_summary(
        &mut self,
        summary: &StoredSummary,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Forgets the summary of `measure` for `period` at `facility`: no patient has care in it
    /// any more, as if it had never been computed.
    fn remove_summary(
        &mut self,
        measure: &str,
        period: &Period,
        facility: &str,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Every summary kept for the period `period` of type `kind`, by indicator.
    fn summaries(
        &mut self,
        kind: PeriodType,
        period: &str,
    ) -> impl Future<Output = Result<Vec<StoredSummary>, StoreError>> + Send;

    /// Forgets every membership and summary, to compute them all again.
    fn clear_indicators(&mut self) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Where the indicators stand on the change log, if they have been computed.
    fn indicator_position(
        &mut self,
    ) -> impl Future<Output = Result<Option<Position>, StoreError>> + Send;

    /// Records where the indicators stand on the change log.
    fn set_indicator_position(
        &mut self,
        position: &Position,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// The id of every live Patient.
    fn patient_ids(&mut self) -> impl Future<Output = Result<Vec<String>, StoreError>> + Send;

    /// The live Patient `id` as JSON, then each live resource that references it, or none if
    /// there is no such Patient.
    fn patient_records(
        &mut self,
        id: &str,
    ) -> impl Future<Output = Result<Option<Vec<String>>, StoreError>> + Send;

    /// The JSON of version `version_seq` of a resource and of the version before it, those
    /// that exist: who a change concerned, before and after it.
    fn changed_versions(
        &mut self,
        resource_type: &str,
        id: &str,
        version_seq: u64,
    ) -> impl Future<Output = Result<Vec<String>, StoreError>> + Send;
}

/// The periods of type `kind` in which `outcome`'s records place care for `indicator`: each
/// record's period date's period, or, for a cumulative indicator, every period from the first
/// one's up to the one holding `today`. A patient's membership is kept for these only: in any
/// other period none of their records is in the period, so they add nothing to it.
pub fn periods_with_care(
    indicator: &Indicator,
    kind: PeriodType,
    outcome: &PatientOutcome,
    today: Date,
) -> Vec<Period> {
    let dates = outcome
        .records
        .iter()
        .filter_map(|r| r.date.as_ref().ok().copied().flatten());
    let mut periods: Vec<Period> = if indicator.cumulative {
        let Some(first) = dates.min() else {
            return Vec::new();
        };
        let last = Period::of(kind, today);
        let mut period = Period::of(kind, first);
        let mut found = Vec::new();
        while period.start <= last.start {
            let next = period.next();
            found.push(period);
            match next {
                Some(next) => period = next,
                None => break,
            }
        }
        found
    } else {
        dates.map(|date| Period::of(kind, date)).collect()
    };
    periods.sort_by_key(|p| p.start);
    periods.dedup();
    periods
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::calculate::RecordOutcome;
    use crate::indicator::{Calculation, Counts};

    fn date(text: &str) -> Date {
        text.parse().expect("date")
    }

    fn indicator(cumulative: bool) -> Indicator {
        Indicator {
            url: "u".into(),
            version: "1".into(),
            title: "t".into(),
            scoring: Calculation::Count.scoring(),
            calculation: Calculation::Count,
            basis: "Encounter".into(),
            counts: Counts::People,
            cumulative,
            period_date: "Encounter.actualPeriod.start".into(),
            populations: BTreeMap::new(),
            references: BTreeSet::new(),
            disaggregations: Vec::new(),
            thresholds: Vec::new(),
            estimate: None,
            value: None,
        }
    }

    fn outcome(dates: &[Option<&str>]) -> PatientOutcome {
        PatientOutcome {
            patient: "Patient/p".into(),
            records: dates
                .iter()
                .enumerate()
                .map(|(n, day)| RecordOutcome {
                    reference: format!("Encounter/{n}"),
                    enrollment: None,
                    date: Ok(day.map(date)),
                    populations: BTreeMap::new(),
                    value: Ok(None),
                    strata: Vec::new(),
                })
                .collect(),
        }
    }

    fn ids(periods: &[Period]) -> Vec<&str> {
        periods.iter().map(|p| p.id.as_str()).collect()
    }

    #[test]
    fn care_is_kept_in_the_periods_its_dates_fall_in() {
        let patient = outcome(&[
            Some("2026-03-30"),
            None,
            Some("2026-03-02"),
            Some("2026-05-01"),
        ]);
        let today = date("2026-04-03");
        assert_eq!(
            ids(&periods_with_care(
                &indicator(false),
                PeriodType::Monthly,
                &patient,
                today
            )),
            ["2026-03", "2026-05"]
        );
        assert_eq!(
            ids(&periods_with_care(
                &indicator(false),
                PeriodType::Quarterly,
                &patient,
                today
            )),
            ["2026Q1", "2026Q2"]
        );
        assert!(
            periods_with_care(
                &indicator(false),
                PeriodType::Monthly,
                &outcome(&[None]),
                today
            )
            .is_empty()
        );
    }

    #[test]
    fn cumulative_care_is_kept_in_every_period_up_to_today() {
        assert_eq!(
            ids(&periods_with_care(
                &indicator(true),
                PeriodType::Monthly,
                &outcome(&[Some("2026-01-15"), Some("2026-02-01")]),
                date("2026-04-03"),
            )),
            ["2026-01", "2026-02", "2026-03", "2026-04"]
        );
    }
}
