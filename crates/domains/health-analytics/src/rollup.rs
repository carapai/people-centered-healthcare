//! Roll-up at central (analytics.md § Roll up; ADR 0068, ADR 0075; #555): an indicator's figure
//! for a set of facilities (a district, a region, the country) from the summaries each facility's
//! edge computed and synced.
//!
//! - **Summed parts.** The figure is recomputed from the facilities' summed counts, numerators,
//!   denominators and values ([`Summary::roll_up`]), never by averaging their figures, and each
//!   disaggregation band the same way. A median or a distinct count stays at facility level: one
//!   facility's is its own figure, and over several there is none.
//! - **Missing, never zero.** A facility has reported for a period when it has a summary for it,
//!   or its summaries synced at or after the period's first day (UTC) with none for it: no care
//!   it counts happened there, so it adds nothing. Any other facility is *missing*, shown with its
//!   last sync, and adds nothing either; when every facility is missing there is no figure.
//! - **Freshness.** Each reported facility carries when its summary was computed and when it last
//!   synced, and the roll-up when its newest summary was computed.
//!
//! Which facilities make up a place is the facility registry's (ADR 0094); the caller gives them.

use std::collections::BTreeMap;
use std::future::Future;

use health_persistence::StoreError;
use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::calculate::{RollUpError, Summary};
use crate::indicator::Calculation;
use crate::period::Period;

/// A facility's summary of one indicator and period, as central keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacilitySummary {
    /// The facility.
    pub facility: String,
    /// The summary `MeasureReport`, as JSON.
    pub report: String,
    /// When its edge computed it.
    pub computed_at: Timestamp,
}

/// The summaries central keeps per facility, over a transaction.
pub trait FacilitySummaries {
    /// Every facility's summary of `measure` (`url|version`) for `period`.
    fn facility_summaries(
        &mut self,
        measure: &str,
        period: &Period,
    ) -> impl Future<Output = Result<Vec<FacilitySummary>, StoreError>> + Send;

    /// When each facility's summaries last synced, for those that ever have.
    fn summaries_synced(
        &mut self,
    ) -> impl Future<Output = Result<BTreeMap<String, Timestamp>, StoreError>> + Send;
}

/// A facility that has reported for the period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reported {
    /// The facility.
    pub facility: String,
    /// When its summary was computed; none when it has no summary for the period (nothing it
    /// counts happened there).
    pub computed_at: Option<Timestamp>,
    /// When its summaries last synced.
    pub synced_at: Option<Timestamp>,
}

/// A facility that has not reported for the period: shown as missing, never as zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    /// The facility.
    pub facility: String,
    /// When its summaries last synced, if ever.
    pub last_sync: Option<Timestamp>,
}

/// An indicator's figure for a set of facilities, with its completeness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolledUp {
    /// The figure from the reported facilities' summed parts; none when none reported.
    pub summary: Option<Summary>,
    /// The facilities that reported, in the order given.
    pub reported: Vec<Reported>,
    /// The facilities that did not, in the order given.
    pub missing: Vec<Missing>,
    /// When the newest summary taken was computed.
    pub computed_at: Option<Timestamp>,
}

impl RolledUp {
    /// How many facilities reported, out of how many were expected.
    pub fn completeness(&self) -> (usize, usize) {
        (
            self.reported.len(),
            self.reported.len() + self.missing.len(),
        )
    }
}

/// The figure of an indicator calculating `calculation` for `period` over `facilities`, from
/// each facility's `summaries` (by facility: the summary and when it was computed) and when each
/// facility's summaries last `synced`. A facility named twice counts once.
pub fn roll_up_facilities(
    calculation: Calculation,
    period: &Period,
    facilities: &[String],
    summaries: &BTreeMap<String, (Summary, Timestamp)>,
    synced: &BTreeMap<String, Timestamp>,
) -> Result<RolledUp, RollUpError> {
    let mut parts = Vec::new();
    let mut reported = Vec::new();
    let mut missing = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for facility in facilities {
        if !seen.insert(facility) {
            continue;
        }
        let synced_at = synced.get(facility).copied();
        if let Some((summary, computed_at)) = summaries.get(facility) {
            parts.push(summary.clone());
            reported.push(Reported {
                facility: facility.clone(),
                computed_at: Some(*computed_at),
                synced_at,
            });
        } else if synced_at.is_some_and(|at| at.to_zoned(TimeZone::UTC).date() >= period.start) {
            reported.push(Reported {
                facility: facility.clone(),
                computed_at: None,
                synced_at,
            });
        } else {
            missing.push(Missing {
                facility: facility.clone(),
                last_sync: synced_at,
            });
        }
    }
    let facility_only = match calculation {
        Calculation::Value(aggregate) if !aggregate.rolls_up() => Some(aggregate),
        _ => None,
    };
    let summary = if seen.len() == 1 && facility_only.is_some() {
        // One facility is facility level: its own figure, a median too, needs no rolling up.
        parts.pop()
    } else if let Some(aggregate) = facility_only {
        return Err(RollUpError::FacilityOnly(aggregate));
    } else if reported.is_empty() {
        None
    } else {
        Some(Summary::roll_up(calculation, &parts)?)
    };
    Ok(RolledUp {
        summary,
        computed_at: reported.iter().filter_map(|r| r.computed_at).max(),
        reported,
        missing,
    })
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::*;
    use crate::calculate::{Band, Disaggregated};
    use crate::indicator::Aggregate;
    use crate::period::PeriodType;

    fn march() -> Period {
        Period::parse(PeriodType::Monthly, "2026-03").expect("march")
    }

    fn at(text: &str) -> Timestamp {
        text.parse().expect("timestamp")
    }

    fn percentage(count: u64, denominator: u64) -> Summary {
        Summary {
            calculation: Calculation::Percentage,
            value: None,
            count,
            denominator: Some(denominator),
            values: None,
            disaggregations: Vec::new(),
        }
    }

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    #[test]
    fn a_district_percentage_is_its_summed_numerators_over_its_summed_denominators() {
        // 1 of 2 (50%) and 9 of 10 (90%): 10 of 12, not the 70% averaging gives.
        let summaries = BTreeMap::from([
            (
                "A".to_owned(),
                (percentage(1, 2), at("2026-04-01T06:00:00Z")),
            ),
            (
                "B".to_owned(),
                (percentage(9, 10), at("2026-04-02T06:00:00Z")),
            ),
        ]);
        let synced = BTreeMap::from([
            ("A".to_owned(), at("2026-04-01T07:00:00Z")),
            ("B".to_owned(), at("2026-04-02T07:00:00Z")),
        ]);
        let rolled = roll_up_facilities(
            Calculation::Percentage,
            &march(),
            &names(&["A", "B"]),
            &summaries,
            &synced,
        )
        .expect("rolled up");
        let summary = rolled.summary.as_ref().expect("a figure");
        assert_eq!((summary.count, summary.denominator), (10, Some(12)));
        assert_eq!(summary.value, Some(Decimal::new(8333, 2)));
        assert_eq!(rolled.completeness(), (2, 2));
        assert_eq!(rolled.computed_at, Some(at("2026-04-02T06:00:00Z")));
    }

    #[test]
    fn a_facility_that_has_not_synced_is_missing_never_zero() {
        let summaries = BTreeMap::from([(
            "A".to_owned(),
            (percentage(1, 2), at("2026-04-01T06:00:00Z")),
        )]);
        let synced = BTreeMap::from([
            ("A".to_owned(), at("2026-04-01T07:00:00Z")),
            // Last synced in February: nothing is known of its March.
            ("B".to_owned(), at("2026-02-27T07:00:00Z")),
            // Synced in March with no summary for it: none of its care counts, so it adds zero.
            ("C".to_owned(), at("2026-03-15T07:00:00Z")),
        ]);
        let rolled = roll_up_facilities(
            Calculation::Percentage,
            &march(),
            &names(&["A", "B", "C", "D", "A"]),
            &summaries,
            &synced,
        )
        .expect("rolled up");
        let summary = rolled.summary.as_ref().expect("a figure");
        assert_eq!((summary.count, summary.denominator), (1, Some(2)));
        assert_eq!(
            rolled.missing,
            vec![
                Missing {
                    facility: "B".into(),
                    last_sync: Some(at("2026-02-27T07:00:00Z")),
                },
                Missing {
                    facility: "D".into(),
                    last_sync: None,
                },
            ]
        );
        assert_eq!(
            rolled.reported[1],
            Reported {
                facility: "C".into(),
                computed_at: None,
                synced_at: Some(at("2026-03-15T07:00:00Z")),
            }
        );
        assert_eq!(rolled.completeness(), (2, 4));

        let none = roll_up_facilities(
            Calculation::Count,
            &march(),
            &names(&["B", "D"]),
            &summaries,
            &synced,
        )
        .expect("rolled up");
        assert_eq!(none.summary, None, "no figure, not a zero");
        assert_eq!(none.completeness(), (0, 2));
    }

    #[test]
    fn bands_roll_up_like_the_whole_and_medians_stay_at_the_facility() {
        let banded = |count, denominator| Summary {
            disaggregations: vec![Disaggregated {
                name: "age".into(),
                bands: vec![Band {
                    code: "15-19".into(),
                    summary: percentage(count, denominator),
                }],
            }],
            ..percentage(count, denominator)
        };
        let summaries = BTreeMap::from([
            ("A".to_owned(), (banded(1, 4), at("2026-04-01T06:00:00Z"))),
            ("B".to_owned(), (banded(3, 4), at("2026-04-01T06:00:00Z"))),
        ]);
        let rolled = roll_up_facilities(
            Calculation::Percentage,
            &march(),
            &names(&["A", "B"]),
            &summaries,
            &BTreeMap::new(),
        )
        .expect("rolled up");
        let band = &rolled.summary.expect("figure").disaggregations[0].bands[0].summary;
        assert_eq!((band.count, band.denominator), (4, Some(8)));
        assert_eq!(band.value, Some(Decimal::new(50, 0)));

        for facilities in [names(&["A", "B"]), Vec::new()] {
            assert_eq!(
                roll_up_facilities(
                    Calculation::Value(Aggregate::Median),
                    &march(),
                    &facilities,
                    &BTreeMap::new(),
                    &BTreeMap::new(),
                ),
                Err(RollUpError::FacilityOnly(Aggregate::Median))
            );
        }
    }

    #[test]
    fn one_facility_s_median_is_its_own_figure() {
        let median = |weeks: i64| Summary {
            calculation: Calculation::Value(Aggregate::Median),
            value: Some(Decimal::from(weeks)),
            count: 3,
            denominator: None,
            values: None,
            disaggregations: Vec::new(),
        };
        let summaries = BTreeMap::from([
            ("A".to_owned(), (median(14), at("2026-04-01T06:00:00Z"))),
            ("B".to_owned(), (median(20), at("2026-04-01T06:00:00Z"))),
        ]);
        let synced = BTreeMap::new();
        let one = roll_up_facilities(
            Calculation::Value(Aggregate::Median),
            &march(),
            &names(&["A", "A"]),
            &summaries,
            &synced,
        )
        .expect("facility level");
        assert_eq!(one.summary, Some(median(14)));
        assert_eq!(one.completeness(), (1, 1));
        // One facility that has not reported is missing, with no figure.
        let none = roll_up_facilities(
            Calculation::Value(Aggregate::Median),
            &march(),
            &names(&["C"]),
            &summaries,
            &synced,
        )
        .expect("facility level");
        assert_eq!((none.completeness(), none.summary), ((0, 1), None));
        // Two are above facility level.
        assert_eq!(
            roll_up_facilities(
                Calculation::Value(Aggregate::Median),
                &march(),
                &names(&["A", "B"]),
                &summaries,
                &synced,
            ),
            Err(RollUpError::FacilityOnly(Aggregate::Median))
        );
    }
}
