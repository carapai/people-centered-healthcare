//! What an indicator calculates over a period (analytics.md; studio.md § Indicators, *What it
//! calculates*): care counts in the period it happened in, by each record's period date (or, for
//! a cumulative indicator, up to the period's end), and the indicator's criteria decide which of
//! those records count.
//!
//! - **What is counted:** people and enrollments each once however many of their records match;
//!   visits and observations each one.
//! - **The denominator** of a percentage, rate or ratio always contains the numerator: a record
//!   is in the numerator only if it is in the denominator and meets the numerator's criteria.
//! - **A value** of each counted thing, counting people or enrollments, is the one from its first
//!   or last record in the period that gives one.
//! - **The summary** ([`Summary`]) carries what roll-up needs: the count (the numerator), the
//!   denominator, and the values' count, sum, minimum and maximum. District and national figures
//!   are recomputed from summed parts ([`Summary::roll_up`]); medians and distinct counts stay at
//!   facility level.
//!
//! The membership is kept per patient, with each record's part in it, so that *Try* can say why
//! each person is counted or not, and so that a later change re-evaluates one patient only
//! (ADR 0068).

use std::collections::{BTreeMap, BTreeSet};

use jiff::civil::Date;
use rust_decimal::{Decimal, RoundingStrategy};

use crate::age_bands::{AgeBandSet, pinned};
use crate::disaggregation::{DisaggregationSource, NOT_RECORDED};
use crate::indicator::{Aggregate, Calculation, Counts, Indicator, Pick, PopulationKind};
use crate::period::Period;

/// A value of a counted thing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Observed {
    /// A number: a sum, average, median, minimum or maximum takes these.
    Number(Decimal),
    /// A code: a distinct count takes these.
    Code(String),
}

impl Observed {
    /// What a distinct count tells apart.
    fn key(&self) -> String {
        match self {
            Self::Number(number) => number.normalize().to_string(),
            Self::Code(code) => code.clone(),
        }
    }
}

/// How one record came out: what an edge keeps of each patient's membership (#554).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecordOutcome {
    /// The record, as a reference (`Encounter/…`).
    pub reference: String,
    /// The enrollment it belongs to (`EpisodeOfCare/…`), if any.
    pub enrollment: Option<String>,
    /// Its period date, if it has one, or why it could not be evaluated.
    pub date: Result<Option<Date>, String>,
    /// Whether it meets each population's criteria, or why that could not be evaluated.
    pub populations: BTreeMap<PopulationKind, Result<bool, String>>,
    /// Its value, if the indicator takes one and it gives one, or why it could not be
    /// evaluated.
    pub value: Result<Option<Observed>, String>,
    /// Its band in each of the indicator's disaggregations, in order: a code, none when it has
    /// no value (*not recorded*), or why it could not be evaluated (#552).
    pub strata: Vec<Result<Option<String>, String>>,
}

/// One patient's records, each as it came out.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PatientOutcome {
    /// The patient, as a reference (`Patient/…`).
    pub patient: String,
    /// Each of their records of the indicator's basis.
    pub records: Vec<RecordOutcome>,
}

/// Why a record counts or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// It has no period date, so it places no care in any period.
    NoPeriodDate,
    /// Its period date is outside the period (after its end, for a cumulative indicator).
    OutsidePeriod,
    /// Counting enrollments, it belongs to none.
    NoEnrollment,
    /// It is in the numerator (and the denominator, if there is one): it counts.
    Meets,
    /// It is in the denominator but not the numerator.
    DenominatorOnly,
    /// It is in the period but does not meet the criteria (of the denominator, if there is
    /// one).
    DoesNotMeet,
    /// It is in the period, but the criteria could not be evaluated on it.
    Error(String),
}

/// One record's part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordPart {
    /// The record.
    pub reference: String,
    /// Its period date.
    pub date: Option<Date>,
    /// Why it counts or not.
    pub reason: Reason,
    /// Its value, or why it could not be evaluated.
    pub value: Result<Option<Observed>, String>,
    /// Whether its value is the one taken for its counted thing.
    pub picked: bool,
    /// Its band in each of the indicator's disaggregations, as evaluated (#552).
    pub strata: Vec<Result<Option<String>, String>>,
}

impl RecordPart {
    /// Whether the record counts.
    pub fn counts(&self) -> bool {
        self.reason == Reason::Meets
    }
}

/// One patient's part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// The patient.
    pub patient: String,
    /// How many of their counted things are in the numerator (or counted, with no
    /// denominator).
    pub count: u64,
    /// How many are in the denominator, if there is one.
    pub denominator: Option<u64>,
    /// Each record's part, in the order given.
    pub records: Vec<RecordPart>,
}

/// The values taken, as roll-up needs them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Values {
    /// How many there are.
    pub count: u64,
    /// Their sum, if they are numbers.
    pub sum: Option<Decimal>,
    /// The smallest, if they are numbers.
    pub minimum: Option<Decimal>,
    /// The largest, if they are numbers.
    pub maximum: Option<Decimal>,
}

/// An indicator's figure for one place and period, with what roll-up needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// What it calculates.
    pub calculation: Calculation,
    /// The figure, to two decimal places; none when the denominator is zero or there is no
    /// value.
    pub value: Option<Decimal>,
    /// How many are counted: the numerator of a percentage, rate or ratio.
    pub count: u64,
    /// The denominator of a percentage, rate or ratio.
    pub denominator: Option<u64>,
    /// The values of a continuous-variable indicator.
    pub values: Option<Values>,
    /// The parts per band of each disaggregation, in the indicator's order; each band's
    /// summary has none of its own (#552).
    pub disaggregations: Vec<Disaggregated>,
}

/// An indicator's parts by one disaggregation (#552).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disaggregated {
    /// The disaggregation's name.
    pub name: String,
    /// Its bands: an Age Band Set's in its order, or the codes found, then
    /// [`NOT_RECORDED`]. They add up to the whole.
    pub bands: Vec<Band>,
}

/// One band's part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Band {
    /// Its code: an age band's (`15-19`, `25+`), a coded value, or [`NOT_RECORDED`].
    pub code: String,
    /// The figure within the band, with what roll-up needs: a percentage's numerator and
    /// denominator are both the band's.
    pub summary: Summary,
}

/// Why summaries could not be rolled up.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RollUpError {
    /// A median or a distinct count stays at facility level.
    #[error("a {} stays at facility level: it cannot be rolled up", .0.code())]
    FacilityOnly(Aggregate),
    /// The parts calculate different things.
    #[error("the parts calculate different things")]
    Mixed,
}

fn rounded(value: Decimal) -> Decimal {
    value
        .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
        .normalize()
}

/// The figure a calculation gives from summable parts; none for a median or a distinct count.
fn figure(
    calculation: Calculation,
    count: u64,
    denominator: Option<u64>,
    values: Option<&Values>,
) -> Option<Decimal> {
    let over = |factor: u32| {
        let denominator = denominator.filter(|d| *d > 0)?;
        Some(Decimal::from(count) * Decimal::from(factor) / Decimal::from(denominator))
    };
    let value = match calculation {
        Calculation::Count => Some(Decimal::from(count)),
        Calculation::Percentage => over(100),
        Calculation::Rate { per } => over(per),
        Calculation::Ratio => over(1),
        Calculation::Value(aggregate) => {
            let values = values.filter(|v| v.count > 0)?;
            match aggregate {
                Aggregate::Sum => values.sum,
                Aggregate::Average => values.sum.map(|sum| sum / Decimal::from(values.count)),
                Aggregate::Minimum => values.minimum,
                Aggregate::Maximum => values.maximum,
                Aggregate::Median | Aggregate::DistinctCount => None,
            }
        }
    };
    value.map(rounded)
}

impl Summary {
    /// The summary of several places' summaries (facilities into a district, …), recomputed
    /// from their summed parts, never by averaging their figures.
    pub fn roll_up(calculation: Calculation, parts: &[Self]) -> Result<Self, RollUpError> {
        if let Calculation::Value(aggregate) = calculation
            && !aggregate.rolls_up()
        {
            return Err(RollUpError::FacilityOnly(aggregate));
        }
        if parts.iter().any(|p| p.calculation != calculation) {
            return Err(RollUpError::Mixed);
        }
        let count = parts.iter().map(|p| p.count).sum();
        // A denominator that is a Population Estimate is not in the parts: it is the place's.
        let denominator = (calculation.has_denominator()
            && (parts.is_empty() || parts.iter().any(|p| p.denominator.is_some())))
        .then(|| parts.iter().filter_map(|p| p.denominator).sum());
        let values = matches!(calculation, Calculation::Value(_)).then(|| {
            let all: Vec<&Values> = parts.iter().filter_map(|p| p.values.as_ref()).collect();
            let numbers =
                |pick: fn(&Values) -> Option<Decimal>| all.iter().filter_map(move |v| pick(v));
            Values {
                count: all.iter().map(|v| v.count).sum(),
                sum: Some(numbers(|v| v.sum).sum()),
                minimum: numbers(|v| v.minimum).min(),
                maximum: numbers(|v| v.maximum).max(),
            }
        });
        let mut disaggregations: Vec<Disaggregated> = Vec::new();
        for part in parts {
            for by in &part.disaggregations {
                let at = match disaggregations.iter().position(|d| d.name == by.name) {
                    Some(at) => at,
                    None => {
                        disaggregations.push(Disaggregated {
                            name: by.name.clone(),
                            bands: Vec::new(),
                        });
                        disaggregations.len() - 1
                    }
                };
                for band in &by.bands {
                    if !disaggregations[at]
                        .bands
                        .iter()
                        .any(|b| b.code == band.code)
                    {
                        disaggregations[at].bands.push(Band {
                            code: band.code.clone(),
                            summary: band.summary.clone(),
                        });
                    }
                }
            }
        }
        for disaggregated in &mut disaggregations {
            // Not recorded stays last whatever each part found.
            disaggregated.bands.sort_by_key(|b| b.code == NOT_RECORDED);
            for band in &mut disaggregated.bands {
                let same: Vec<Self> = parts
                    .iter()
                    .flat_map(|p| &p.disaggregations)
                    .filter(|d| d.name == disaggregated.name)
                    .flat_map(|d| &d.bands)
                    .filter(|b| b.code == band.code)
                    .map(|b| b.summary.clone())
                    .collect();
                band.summary = Self::roll_up(calculation, &same)?;
            }
        }
        Ok(Self {
            calculation,
            value: figure(calculation, count, denominator, values.as_ref()),
            count,
            denominator,
            values,
            disaggregations,
        })
    }
}

/// An indicator evaluated over a period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    /// The figure and what roll-up needs.
    pub summary: Summary,
    /// Every patient, counted or not, in the order given.
    pub members: Vec<Member>,
    /// Each counted thing in the denominator (or counted, with none), with its band in each
    /// disaggregation: what a crossed cell is summarised from ([`Self::crossed`]).
    counted: Vec<Counted>,
}

impl Evaluation {
    /// The patients counted, in the order given.
    pub fn counted(&self) -> impl Iterator<Item = &Member> {
        self.members.iter().filter(|m| m.count > 0)
    }

    /// Every record whose criteria, or whose value as a counted record, could not be
    /// evaluated, with why.
    pub fn errors(&self) -> impl Iterator<Item = (&str, &str)> {
        self.members.iter().flat_map(|m| {
            m.records.iter().flat_map(|r| {
                let value = match (&r.reason, &r.value) {
                    (Reason::Error(why), _) | (Reason::Meets, Err(why)) => Some(why.as_str()),
                    _ => None,
                };
                let placed = matches!(r.reason, Reason::Meets | Reason::DenominatorOnly);
                let strata = r.strata.iter().filter_map(move |s| match s {
                    Err(why) if placed => Some(why.as_str()),
                    _ => None,
                });
                value
                    .into_iter()
                    .chain(strata)
                    .map(move |why| (r.reference.as_str(), why))
            })
        })
    }

    /// The summary of the counted things in band `code` of disaggregation `d` for every
    /// `(d, code)` of `at`, `d` being an index into `indicator`'s disaggregations: a cell
    /// crossing several disaggregations (a Report's age band by HIV status, #867). A
    /// percentage's numerator and denominator are both the cell's, so the cells across one
    /// disaggregation add up to the band of the others. It has no disaggregations of its own.
    pub fn crossed(&self, indicator: &Indicator, at: &[(usize, &str)]) -> Summary {
        summarize(
            indicator,
            self.counted.iter().filter(|c| {
                at.iter()
                    .all(|(d, code)| c.bands.get(*d).is_some_and(|band| band == code))
            }),
        )
    }
}

fn criteria(record: &RecordOutcome, population: PopulationKind) -> Result<bool, String> {
    record
        .populations
        .get(&population)
        .cloned()
        .unwrap_or(Ok(false))
}

/// The record's reason, given that it is in the period.
fn in_period(indicator: &Indicator, record: &RecordOutcome) -> Reason {
    if indicator.counts == Counts::Enrollments && record.enrollment.is_none() {
        return Reason::NoEnrollment;
    }
    let mut sides = vec![PopulationKind::InitialPopulation];
    if indicator.counts_its_denominator() {
        sides.push(PopulationKind::Denominator);
    }
    if matches!(indicator.calculation, Calculation::Value(_)) {
        sides.push(PopulationKind::MeasurePopulation);
    }
    for side in sides {
        match criteria(record, side) {
            Err(why) => return Reason::Error(why),
            Ok(false) => return Reason::DoesNotMeet,
            Ok(true) => {}
        }
    }
    if !indicator.calculation.has_denominator() {
        return Reason::Meets;
    }
    match criteria(record, PopulationKind::Numerator) {
        Err(why) => Reason::Error(why),
        Ok(true) => Reason::Meets,
        Ok(false) => Reason::DenominatorOnly,
    }
}

fn part(indicator: &Indicator, period: &Period, record: &RecordOutcome) -> RecordPart {
    let placed = |date: Date| {
        if indicator.cumulative {
            date <= period.end
        } else {
            period.contains(date)
        }
    };
    let reason = match &record.date {
        Err(why) => Reason::Error(format!("its period date: {why}")),
        Ok(None) => Reason::NoPeriodDate,
        Ok(Some(date)) if !placed(*date) => Reason::OutsidePeriod,
        Ok(Some(_)) => in_period(indicator, record),
    };
    RecordPart {
        reference: record.reference.clone(),
        date: record.date.clone().ok().flatten(),
        reason,
        value: record.value.clone(),
        picked: false,
        strata: record.strata.clone(),
    }
}

/// The counted thing a record belongs to.
fn unit<'a>(indicator: &Indicator, patient: &'a str, record: &'a RecordOutcome) -> &'a str {
    match indicator.counts {
        Counts::People => patient,
        Counts::Enrollments => record.enrollment.as_deref().unwrap_or_default(),
        Counts::Visits | Counts::Observations => &record.reference,
    }
}

fn median(mut numbers: Vec<Decimal>) -> Option<Decimal> {
    numbers.sort();
    let middle = numbers.len() / 2;
    match numbers.len() {
        0 => None,
        n if n % 2 == 1 => numbers.get(middle).copied(),
        _ => Some((numbers[middle - 1] + numbers[middle]) / Decimal::TWO),
    }
}

/// One counted thing (a person, an enrollment, a visit or an observation) in the denominator,
/// or counted when there is none.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Counted {
    /// Whether it is in the numerator (or counted).
    numerator: bool,
    /// The value taken of it, if any.
    value: Option<Observed>,
    /// Its band in each disaggregation.
    bands: Vec<String>,
}

/// The summary of `counted`, without disaggregations.
fn summarize<'a>(indicator: &Indicator, counted: impl Iterator<Item = &'a Counted>) -> Summary {
    let mut count = 0;
    let mut in_denominator = 0;
    let mut taken = Vec::new();
    for thing in counted {
        in_denominator += 1;
        if thing.numerator {
            count += 1;
        }
        taken.extend(thing.value.iter());
    }
    let denominator = indicator.counts_its_denominator().then_some(in_denominator);
    let numbers: Vec<Decimal> = taken
        .iter()
        .filter_map(|o| match o {
            Observed::Number(n) => Some(*n),
            Observed::Code(_) => None,
        })
        .collect();
    let values = indicator.value.as_ref().map(|_| {
        let numeric = numbers.len() == taken.len();
        Values {
            count: taken.len() as u64,
            sum: numeric.then(|| numbers.iter().sum()),
            minimum: numbers.iter().min().copied().filter(|_| numeric),
            maximum: numbers.iter().max().copied().filter(|_| numeric),
        }
    });
    let value = match indicator.calculation {
        Calculation::Value(Aggregate::Median) => median(numbers).map(rounded),
        Calculation::Value(Aggregate::DistinctCount) => Some(Decimal::from(
            taken.iter().map(|o| o.key()).collect::<BTreeSet<_>>().len(),
        )),
        calculation => figure(calculation, count, denominator, values.as_ref()),
    };
    Summary {
        calculation: indicator.calculation,
        value,
        count,
        denominator,
        values,
        disaggregations: Vec::new(),
    }
}

/// The band a record's stratum puts it in.
fn band(stratum: Option<&Result<Option<String>, String>>) -> String {
    match stratum {
        Some(Ok(Some(code))) => code.clone(),
        _ => NOT_RECORDED.to_owned(),
    }
}

/// `indicator` calculated over `period` from each patient's records. Each record's strata are
/// its band in each of the indicator's disaggregations; `sets` are the Age Band Sets they may
/// pin, so that a pinned set's every band is shown, counted or not, in its order.
pub fn calculate(
    indicator: &Indicator,
    period: &Period,
    patients: &[PatientOutcome],
    sets: &[AgeBandSet],
) -> Evaluation {
    let mut members = Vec::with_capacity(patients.len());
    let mut counted: Vec<Counted> = Vec::new();
    for patient in patients {
        let mut records: Vec<RecordPart> = patient
            .records
            .iter()
            .map(|r| part(indicator, period, r))
            .collect();
        // Each counted thing's records in the denominator (or counted), and those in the
        // numerator, as indexes, in the order given.
        let mut by_unit: BTreeMap<&str, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
        for (index, (record, outcome)) in records.iter().zip(&patient.records).enumerate() {
            let key = unit(indicator, &patient.patient, outcome);
            match record.reason {
                Reason::Meets => {
                    let entry = by_unit.entry(key).or_default();
                    entry.0.push(index);
                    entry.1.push(index);
                }
                Reason::DenominatorOnly => by_unit.entry(key).or_default().0.push(index),
                _ => {}
            }
        }
        let mut count = 0;
        for (within, numerator) in by_unit.values() {
            let mut value = None;
            if let Some(how) = &indicator.value {
                let mut with_value: Vec<usize> = numerator
                    .iter()
                    .copied()
                    .filter(|i| matches!(records[*i].value, Ok(Some(_))))
                    .collect();
                // Stable: records on the same date keep the order given.
                with_value.sort_by_key(|i| records[*i].date);
                let picked = match how.pick {
                    Pick::First => with_value.first(),
                    Pick::Last => with_value.last(),
                };
                if let Some(&i) = picked {
                    records[i].picked = true;
                    if let Ok(Some(observed)) = &records[i].value {
                        value = Some(observed.clone());
                    }
                }
            }
            let mut first = within.clone();
            first.sort_by_key(|i| records[*i].date);
            let strata = first.first().map(|i| &patient.records[*i].strata);
            let bands = (0..indicator.disaggregations.len())
                .map(|d| band(strata.and_then(|s| s.get(d))))
                .collect();
            if !numerator.is_empty() {
                count += 1;
            }
            counted.push(Counted {
                numerator: !numerator.is_empty(),
                value,
                bands,
            });
        }
        members.push(Member {
            patient: patient.patient.clone(),
            count,
            denominator: indicator
                .counts_its_denominator()
                .then_some(by_unit.len() as u64),
            records,
        });
    }
    let mut summary = summarize(indicator, counted.iter());
    summary.disaggregations = indicator
        .disaggregations
        .iter()
        .enumerate()
        .map(|(d, disaggregation)| {
            let mut codes: Vec<String> = match &disaggregation.source {
                DisaggregationSource::AgeBands(set) => pinned(set, sets)
                    .map(|set| set.bands.iter().map(|b| b.code()).collect())
                    .unwrap_or_default(),
                DisaggregationSource::Coded(_) => Vec::new(),
            };
            let mut found: Vec<&String> = counted.iter().map(|c| &c.bands[d]).collect();
            found.sort();
            found.dedup();
            for code in found {
                if code != NOT_RECORDED && !codes.contains(code) {
                    codes.push(code.clone());
                }
            }
            codes.push(NOT_RECORDED.to_owned());
            Disaggregated {
                name: disaggregation.name.clone(),
                bands: codes
                    .into_iter()
                    .map(|code| {
                        let summary =
                            summarize(indicator, counted.iter().filter(|c| c.bands[d] == code));
                        Band { code, summary }
                    })
                    .collect(),
            }
        })
        .collect();
    Evaluation {
        summary,
        members,
        counted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indicator::{IndicatorValue, Scoring, ValueSource};
    use crate::period::PeriodType;

    fn date(text: &str) -> Date {
        text.parse().expect("date")
    }

    fn record(
        reference: &str,
        day: Option<&str>,
        sides: &[(PopulationKind, Result<bool, &str>)],
    ) -> RecordOutcome {
        RecordOutcome {
            reference: reference.into(),
            enrollment: None,
            date: Ok(day.map(date)),
            populations: sides
                .iter()
                .map(|(k, r)| (*k, r.map_err(str::to_owned)))
                .collect(),
            value: Ok(None),
            strata: Vec::new(),
        }
    }

    fn ip(meets: Result<bool, &str>) -> Vec<(PopulationKind, Result<bool, &str>)> {
        vec![(PopulationKind::InitialPopulation, meets)]
    }

    fn march() -> Period {
        Period::parse(PeriodType::Monthly, "2026-03").expect("march")
    }

    fn indicator(calculation: Calculation, counts: Counts) -> Indicator {
        Indicator {
            url: "u".into(),
            version: "1".into(),
            title: "t".into(),
            scoring: calculation.scoring(),
            calculation,
            basis: "Encounter".into(),
            counts,
            cumulative: false,
            period_date: "Encounter.actualPeriod.start".into(),
            populations: BTreeMap::new(),
            references: BTreeSet::new(),
            disaggregations: Vec::new(),
            thresholds: Vec::new(),
            estimate: None,
            value: matches!(calculation, Calculation::Value(_)).then(|| IndicatorValue {
                source: ValueSource::Expression("v".into()),
                pick: Pick::First,
            }),
        }
    }

    #[test]
    fn a_person_counts_once_however_many_of_their_records_count() {
        let patients = [
            PatientOutcome {
                patient: "Patient/a".into(),
                records: vec![
                    record("Encounter/a1", Some("2026-03-04"), &ip(Ok(true))),
                    record("Encounter/a2", Some("2026-03-28"), &ip(Ok(true))),
                ],
            },
            PatientOutcome {
                patient: "Patient/b".into(),
                records: vec![
                    record("Encounter/b1", Some("2026-02-27"), &ip(Ok(true))),
                    record("Encounter/b2", Some("2026-03-10"), &ip(Ok(false))),
                    record("Encounter/b3", None, &ip(Ok(true))),
                ],
            },
            PatientOutcome {
                patient: "Patient/c".into(),
                records: vec![record("Encounter/c1", Some("2026-03-31"), &ip(Ok(true)))],
            },
        ];
        let count = calculate(
            &indicator(Calculation::Count, Counts::People),
            &march(),
            &patients,
            &[],
        );
        assert_eq!(count.summary.count, 2);
        assert_eq!(count.summary.value, Some(Decimal::from(2)));
        let counted: Vec<&str> = count.counted().map(|m| m.patient.as_str()).collect();
        assert_eq!(counted, ["Patient/a", "Patient/c"]);
        let why: Vec<&Reason> = count.members[1].records.iter().map(|r| &r.reason).collect();
        assert_eq!(
            why,
            [
                &Reason::OutsidePeriod,
                &Reason::DoesNotMeet,
                &Reason::NoPeriodDate
            ]
        );
        assert_eq!(count.errors().count(), 0);
        // Counting visits, each of A's visits counts; cumulative, B's February visit counts too.
        let visits = calculate(
            &indicator(Calculation::Count, Counts::Visits),
            &march(),
            &patients,
            &[],
        );
        assert_eq!(visits.summary.count, 3);
        let mut cumulative = indicator(Calculation::Count, Counts::Visits);
        cumulative.cumulative = true;
        assert_eq!(
            calculate(&cumulative, &march(), &patients, &[])
                .summary
                .count,
            4
        );
    }

    #[test]
    fn enrollments_count_once_each_and_records_without_one_do_not_count() {
        let enrolled = |reference: &str, enrollment: Option<&str>| RecordOutcome {
            enrollment: enrollment.map(str::to_owned),
            ..record(reference, Some("2026-03-10"), &ip(Ok(true)))
        };
        let patients = [PatientOutcome {
            patient: "Patient/a".into(),
            records: vec![
                enrolled("Encounter/1", Some("EpisodeOfCare/p1")),
                enrolled("Encounter/2", Some("EpisodeOfCare/p1")),
                enrolled("Encounter/3", Some("EpisodeOfCare/p2")),
                enrolled("Encounter/4", None),
            ],
        }];
        let count = calculate(
            &indicator(Calculation::Count, Counts::Enrollments),
            &march(),
            &patients,
            &[],
        );
        assert_eq!(count.summary.count, 2);
        assert_eq!(count.members[0].records[3].reason, Reason::NoEnrollment);
    }

    #[test]
    fn the_numerator_is_always_within_the_denominator() {
        use PopulationKind::{Denominator, InitialPopulation, Numerator};
        let patients = [
            PatientOutcome {
                patient: "Patient/a".into(),
                records: vec![record(
                    "Encounter/a1",
                    Some("2026-03-04"),
                    &[
                        (InitialPopulation, Ok(true)),
                        (Denominator, Ok(true)),
                        (Numerator, Ok(true)),
                    ],
                )],
            },
            PatientOutcome {
                patient: "Patient/b".into(),
                records: vec![record(
                    "Encounter/b1",
                    Some("2026-03-05"),
                    &[
                        (InitialPopulation, Ok(true)),
                        (Denominator, Ok(true)),
                        (Numerator, Ok(false)),
                    ],
                )],
            },
            PatientOutcome {
                patient: "Patient/c".into(),
                records: vec![record(
                    "Encounter/c1",
                    Some("2026-03-06"),
                    &[
                        (InitialPopulation, Ok(true)),
                        (Denominator, Ok(false)),
                        (Numerator, Ok(true)),
                    ],
                )],
            },
            PatientOutcome {
                patient: "Patient/d".into(),
                records: vec![record(
                    "Encounter/d1",
                    Some("2026-03-07"),
                    &[
                        (InitialPopulation, Ok(true)),
                        (Denominator, Ok(true)),
                        (Numerator, Ok(true)),
                    ],
                )],
            },
        ];
        let figure = |calculation| {
            let evaluation = calculate(
                &indicator(calculation, Counts::People),
                &march(),
                &patients,
                &[],
            );
            (
                evaluation.summary.count,
                evaluation.summary.denominator,
                evaluation.summary.value.map(|v| v.to_string()),
            )
        };
        assert_eq!(
            figure(Calculation::Percentage),
            (2, Some(3), Some("66.67".into()))
        );
        assert_eq!(
            figure(Calculation::Rate { per: 1_000 }),
            (2, Some(3), Some("666.67".into()))
        );
        assert_eq!(
            figure(Calculation::Ratio),
            (2, Some(3), Some("0.67".into()))
        );
        let evaluation = calculate(
            &indicator(Calculation::Percentage, Counts::People),
            &march(),
            &patients,
            &[],
        );
        assert_eq!(
            evaluation.members[1].records[0].reason,
            Reason::DenominatorOnly
        );
        assert_eq!(evaluation.members[2].records[0].reason, Reason::DoesNotMeet);
        assert_eq!(evaluation.members[2].denominator, Some(0));
        // With an empty denominator there is no figure.
        assert_eq!(
            calculate(
                &indicator(Calculation::Percentage, Counts::People),
                &march(),
                &[],
                &[]
            )
            .summary
            .value,
            None
        );
    }

    fn valued(reference: &str, day: &str, value: Option<Observed>) -> RecordOutcome {
        let mut sides = ip(Ok(true));
        sides.push((PopulationKind::MeasurePopulation, Ok(true)));
        RecordOutcome {
            value: Ok(value),
            ..record(reference, Some(day), &sides)
        }
    }

    fn number(n: i64) -> Option<Observed> {
        Some(Observed::Number(Decimal::from(n)))
    }

    fn valued_patients() -> Vec<PatientOutcome> {
        vec![
            PatientOutcome {
                patient: "Patient/a".into(),
                // The later record is given first: the first value is by date.
                records: vec![
                    valued("Encounter/a2", "2026-03-20", number(14)),
                    valued("Encounter/a1", "2026-03-04", number(10)),
                ],
            },
            PatientOutcome {
                patient: "Patient/b".into(),
                records: vec![
                    valued("Encounter/b1", "2026-03-02", None),
                    valued("Encounter/b2", "2026-03-09", number(8)),
                ],
            },
            PatientOutcome {
                patient: "Patient/c".into(),
                records: vec![valued("Encounter/c1", "2026-03-11", number(16))],
            },
            PatientOutcome {
                patient: "Patient/d".into(),
                records: vec![valued("Encounter/d1", "2026-03-12", number(9))],
            },
        ]
    }

    #[test]
    fn a_value_per_person_is_their_first_or_last_and_is_summarised() {
        let of = |aggregate, pick| {
            let mut indicator = indicator(Calculation::Value(aggregate), Counts::People);
            if let Some(value) = indicator.value.as_mut() {
                value.pick = pick;
            }
            calculate(&indicator, &march(), &valued_patients(), &[])
        };
        let value = |aggregate, pick| of(aggregate, pick).summary.value.map(|v| v.to_string());
        // First values: 10, 8, 16, 9. Last: 14, 8, 16, 9.
        assert_eq!(value(Aggregate::Median, Pick::First), Some("9.5".into()));
        assert_eq!(value(Aggregate::Median, Pick::Last), Some("11.5".into()));
        assert_eq!(value(Aggregate::Sum, Pick::First), Some("43".into()));
        assert_eq!(value(Aggregate::Average, Pick::First), Some("10.75".into()));
        assert_eq!(value(Aggregate::Minimum, Pick::First), Some("8".into()));
        assert_eq!(value(Aggregate::Maximum, Pick::Last), Some("16".into()));
        let first = of(Aggregate::Median, Pick::First);
        assert_eq!(first.summary.count, 4);
        assert_eq!(
            first.summary.values,
            Some(Values {
                count: 4,
                sum: Some(Decimal::from(43)),
                minimum: Some(Decimal::from(8)),
                maximum: Some(Decimal::from(16)),
            })
        );
        let picked: Vec<bool> = first.members[0].records.iter().map(|r| r.picked).collect();
        assert_eq!(picked, [false, true]);
        assert!(
            first.members[1].records[1].picked,
            "the first record with a value"
        );
        // Counting visits, every record is its own counted thing.
        let mut visits = indicator(Calculation::Value(Aggregate::Sum), Counts::Visits);
        visits.basis = "Encounter".into();
        assert_eq!(
            calculate(&visits, &march(), &valued_patients(), &[])
                .summary
                .value,
            Some(Decimal::from(57))
        );
    }

    #[test]
    fn a_distinct_count_counts_different_values() {
        let code = |c: &str| Some(Observed::Code(c.into()));
        let patients = [PatientOutcome {
            patient: "Patient/a".into(),
            records: vec![
                valued("Encounter/1", "2026-03-01", code("first")),
                valued("Encounter/2", "2026-03-02", code("follow-up")),
                valued("Encounter/3", "2026-03-03", code("first")),
            ],
        }];
        let evaluation = calculate(
            &indicator(Calculation::Value(Aggregate::DistinctCount), Counts::Visits),
            &march(),
            &patients,
            &[],
        );
        assert_eq!(evaluation.summary.value, Some(Decimal::from(2)));
        assert_eq!(evaluation.summary.values.and_then(|v| v.sum), None);
    }

    #[test]
    fn errors_are_reported_per_record_and_count_nothing() {
        let mut bad_value = valued("Encounter/a3", "2026-03-05", None);
        bad_value.value = Err("not a number".into());
        let patients = [PatientOutcome {
            patient: "Patient/a".into(),
            records: vec![
                record(
                    "Encounter/a1",
                    Some("2026-03-04"),
                    &ip(Err("no such function")),
                ),
                RecordOutcome {
                    date: Err("not a date".into()),
                    ..record("Encounter/a2", None, &ip(Ok(true)))
                },
                bad_value,
            ],
        }];
        let evaluation = calculate(
            &indicator(Calculation::Value(Aggregate::Sum), Counts::Visits),
            &march(),
            &patients,
            &[],
        );
        assert_eq!(
            evaluation.errors().collect::<Vec<_>>(),
            [
                ("Encounter/a1", "no such function"),
                ("Encounter/a2", "its period date: not a date"),
                ("Encounter/a3", "not a number"),
            ]
        );
        assert_eq!(evaluation.summary.count, 1);
        assert_eq!(evaluation.summary.value, None, "no value was taken");
    }

    #[test]
    fn each_summary_carries_what_roll_up_needs() {
        let summary = |calculation, count, denominator, values| Summary {
            calculation,
            value: None,
            count,
            denominator,
            values,
            disaggregations: Vec::new(),
        };
        // Two facilities' percentages, 1 of 4 and 3 of 6, give 4 of 10 (40%), not the average
        // of 25% and 50%.
        let rolled = Summary::roll_up(
            Calculation::Percentage,
            &[
                summary(Calculation::Percentage, 1, Some(4), None),
                summary(Calculation::Percentage, 3, Some(6), None),
            ],
        )
        .expect("rolled up");
        assert_eq!((rolled.count, rolled.denominator), (4, Some(10)));
        assert_eq!(rolled.value, Some(Decimal::from(40)));
        let values = |count, sum, minimum, maximum| {
            Some(Values {
                count,
                sum: Some(Decimal::from(sum)),
                minimum: Some(Decimal::from(minimum)),
                maximum: Some(Decimal::from(maximum)),
            })
        };
        let average = Calculation::Value(Aggregate::Average);
        let rolled = Summary::roll_up(
            average,
            &[
                summary(average, 2, None, values(2, 20, 8, 12)),
                summary(average, 3, None, values(3, 45, 9, 20)),
            ],
        )
        .expect("rolled up");
        assert_eq!(rolled.value, Some(Decimal::from(13)));
        assert_eq!(rolled.values, values(5, 65, 8, 20));
        let maximum = Calculation::Value(Aggregate::Maximum);
        assert_eq!(
            Summary::roll_up(
                maximum,
                &[
                    summary(maximum, 2, None, values(2, 20, 8, 12)),
                    summary(maximum, 3, None, values(3, 45, 9, 20)),
                ],
            )
            .expect("rolled up")
            .value,
            Some(Decimal::from(20))
        );
        let median = Calculation::Value(Aggregate::Median);
        assert_eq!(
            Summary::roll_up(median, &[]),
            Err(RollUpError::FacilityOnly(Aggregate::Median))
        );
        assert_eq!(
            Summary::roll_up(
                Calculation::Count,
                &[summary(Calculation::Ratio, 1, Some(1), None)]
            ),
            Err(RollUpError::Mixed)
        );
        // A calculation's summary from evaluation carries the same parts.
        let evaluated = calculate(
            &indicator(average, Counts::People),
            &march(),
            &valued_patients(),
            &[],
        );
        let rolled =
            Summary::roll_up(average, std::slice::from_ref(&evaluated.summary)).expect("rolled up");
        assert_eq!(rolled, evaluated.summary);
        assert_eq!(Scoring::Cohort, Calculation::Count.scoring());
    }

    fn age_bands() -> AgeBandSet {
        use crate::age_bands::{AgeAt, AgeBand, AgeUnit};
        AgeBandSet {
            url: "https://health.go.ug/fhir/Library/age-bands-anc".into(),
            version: "1.0.0".into(),
            title: "MoH ANC age bands".into(),
            unit: AgeUnit::Years,
            age_at: AgeAt::PeriodEnd,
            bands: vec![
                AgeBand {
                    from: 10,
                    to: Some(14),
                },
                AgeBand {
                    from: 15,
                    to: Some(19),
                },
                AgeBand {
                    from: 20,
                    to: Some(24),
                },
                AgeBand { from: 25, to: None },
            ],
        }
    }

    fn by(name: &str, source: DisaggregationSource) -> crate::disaggregation::Disaggregation {
        crate::disaggregation::Disaggregation {
            name: name.into(),
            source,
        }
    }

    fn in_band(
        reference: &str,
        day: &str,
        sides: &[(PopulationKind, Result<bool, &str>)],
        strata: &[Result<Option<&str>, &str>],
    ) -> RecordOutcome {
        RecordOutcome {
            strata: strata
                .iter()
                .map(|s| s.map(|c| c.map(str::to_owned)).map_err(str::to_owned))
                .collect(),
            ..record(reference, Some(day), sides)
        }
    }

    fn bands(summary: &Summary, at: usize) -> Vec<(&str, u64, Option<u64>, Option<Decimal>)> {
        summary.disaggregations[at]
            .bands
            .iter()
            .map(|b| {
                (
                    b.code.as_str(),
                    b.summary.count,
                    b.summary.denominator,
                    b.summary.value,
                )
            })
            .collect()
    }

    #[test]
    fn the_bands_add_up_to_the_total_with_a_not_recorded_band() {
        let set = age_bands();
        let mut count = indicator(Calculation::Count, Counts::People);
        count.disaggregations = vec![
            by("age", DisaggregationSource::AgeBands(set.canonical())),
            by("hiv-status", DisaggregationSource::Coded("v".into())),
        ];
        let one = |patient: &str, strata: &[Result<Option<&str>, &str>]| PatientOutcome {
            patient: format!("Patient/{patient}"),
            records: vec![in_band(
                &format!("Encounter/{patient}"),
                "2026-03-10",
                &ip(Ok(true)),
                strata,
            )],
        };
        let patients = [
            one("a", &[Ok(Some("15-19")), Ok(Some("positive"))]),
            one("b", &[Ok(Some("25+")), Ok(Some("negative"))]),
            one("c", &[Ok(Some("25+")), Ok(None)]),
            // No birth date, and an HIV status that could not be read.
            one("d", &[Ok(None), Err("the code gave 2 items")]),
        ];
        let evaluated = calculate(&count, &march(), &patients, std::slice::from_ref(&set));
        let n = |n: i64| Some(Decimal::from(n));
        assert_eq!(evaluated.summary.count, 4);
        assert_eq!(
            bands(&evaluated.summary, 0),
            [
                ("10-14", 0, None, n(0)),
                ("15-19", 1, None, n(1)),
                ("20-24", 0, None, n(0)),
                ("25+", 2, None, n(2)),
                (NOT_RECORDED, 1, None, n(1)),
            ]
        );
        assert_eq!(
            bands(&evaluated.summary, 1),
            [
                ("negative", 1, None, n(1)),
                ("positive", 1, None, n(1)),
                (NOT_RECORDED, 2, None, n(2)),
            ]
        );
        for by in &evaluated.summary.disaggregations {
            let total: u64 = by.bands.iter().map(|b| b.summary.count).sum();
            assert_eq!(total, evaluated.summary.count, "{} adds up", by.name);
        }
        assert_eq!(
            evaluated.errors().collect::<Vec<_>>(),
            [("Encounter/d", "the code gave 2 items")]
        );
        // Without the pinned set at hand, only the bands found are shown.
        assert_eq!(
            bands(&calculate(&count, &march(), &patients, &[]).summary, 0)
                .iter()
                .map(|b| b.0)
                .collect::<Vec<_>>(),
            ["15-19", "25+", NOT_RECORDED]
        );
    }

    #[test]
    fn a_counted_thing_falls_in_the_band_of_its_first_record() {
        let mut count = indicator(Calculation::Count, Counts::People);
        count.disaggregations = vec![by("visit-type", DisaggregationSource::Coded("v".into()))];
        let patients = [PatientOutcome {
            patient: "Patient/a".into(),
            records: vec![
                in_band(
                    "Encounter/2",
                    "2026-03-20",
                    &ip(Ok(true)),
                    &[Ok(Some("follow-up"))],
                ),
                in_band(
                    "Encounter/1",
                    "2026-03-02",
                    &ip(Ok(true)),
                    &[Ok(Some("first"))],
                ),
                // Not counted, so not her band.
                in_band(
                    "Encounter/0",
                    "2026-03-01",
                    &ip(Ok(false)),
                    &[Ok(Some("other"))],
                ),
            ],
        }];
        let evaluated = calculate(&count, &march(), &patients, &[]);
        assert_eq!(
            bands(&evaluated.summary, 0),
            [
                ("first", 1, None, Some(Decimal::ONE)),
                (NOT_RECORDED, 0, None, Some(Decimal::ZERO)),
            ]
        );
    }

    #[test]
    fn a_percentage_per_band_uses_the_denominator_within_that_band() {
        use PopulationKind::{Denominator, InitialPopulation, Numerator};
        let mut percentage = indicator(Calculation::Percentage, Counts::People);
        percentage.disaggregations =
            vec![by("hiv-status", DisaggregationSource::Coded("v".into()))];
        let sides = |numerator: bool| {
            vec![
                (InitialPopulation, Ok(true)),
                (Denominator, Ok(true)),
                (Numerator, Ok(numerator)),
            ]
        };
        let one = |patient: &str, numerator: bool, status: Option<&str>| PatientOutcome {
            patient: format!("Patient/{patient}"),
            records: vec![in_band(
                &format!("Encounter/{patient}"),
                "2026-03-10",
                &sides(numerator),
                &[Ok(status)],
            )],
        };
        // Positive: 1 of 1; negative: 1 of 3; not recorded: 0 of 1. In all, 2 of 5.
        let patients = [
            one("a", true, Some("positive")),
            one("b", true, Some("negative")),
            one("c", false, Some("negative")),
            one("d", false, Some("negative")),
            one("e", false, None),
        ];
        let evaluated = calculate(&percentage, &march(), &patients, &[]);
        assert_eq!(
            (evaluated.summary.count, evaluated.summary.denominator),
            (2, Some(5))
        );
        assert_eq!(evaluated.summary.value, Some(Decimal::from(40)));
        assert_eq!(
            bands(&evaluated.summary, 0),
            [
                (
                    "negative",
                    1,
                    Some(3),
                    Some("33.33".parse().expect("decimal"))
                ),
                ("positive", 1, Some(1), Some(Decimal::from(100))),
                (NOT_RECORDED, 0, Some(1), Some(Decimal::ZERO)),
            ]
        );
        // Facilities roll up band by band, from the summed parts.
        let rolled = Summary::roll_up(
            Calculation::Percentage,
            &[evaluated.summary.clone(), evaluated.summary.clone()],
        )
        .expect("rolled up");
        assert_eq!(
            bands(&rolled, 0),
            [
                (
                    "negative",
                    2,
                    Some(6),
                    Some("33.33".parse().expect("decimal"))
                ),
                ("positive", 2, Some(2), Some(Decimal::from(100))),
                (NOT_RECORDED, 0, Some(2), Some(Decimal::ZERO)),
            ]
        );
        let alone = Summary::roll_up(
            Calculation::Percentage,
            std::slice::from_ref(&evaluated.summary),
        )
        .expect("rolled up");
        assert_eq!(alone, evaluated.summary);
    }

    #[test]
    fn crossed_bands_add_up_to_each_band_and_keep_the_denominator_within_the_cell() {
        use PopulationKind::{Denominator, InitialPopulation, Numerator};
        let mut percentage = indicator(Calculation::Percentage, Counts::People);
        percentage.disaggregations = vec![
            by("age", DisaggregationSource::AgeBands("a|1".into())),
            by("hiv-status", DisaggregationSource::Coded("v".into())),
        ];
        let one =
            |patient: &str, numerator: bool, age: &str, status: Option<&str>| PatientOutcome {
                patient: format!("Patient/{patient}"),
                records: vec![in_band(
                    &format!("Encounter/{patient}"),
                    "2026-03-10",
                    &[
                        (InitialPopulation, Ok(true)),
                        (Denominator, Ok(true)),
                        (Numerator, Ok(numerator)),
                    ],
                    &[Ok(Some(age)), Ok(status)],
                )],
            };
        let patients = [
            one("a", true, "15-19", Some("positive")),
            one("b", false, "15-19", Some("positive")),
            one("c", true, "15-19", Some("negative")),
            one("d", true, "25+", Some("negative")),
            one("e", false, "25+", None),
        ];
        let evaluated = calculate(&percentage, &march(), &patients, &[]);
        let cell = |age: &str, status: &str| {
            let s = evaluated.crossed(&percentage, &[(0, age), (1, status)]);
            (s.count, s.denominator, s.value)
        };
        let n = |n: i64| Some(Decimal::from(n));
        assert_eq!(cell("15-19", "positive"), (1, Some(2), n(50)));
        assert_eq!(cell("15-19", "negative"), (1, Some(1), n(100)));
        assert_eq!(cell("25+", "negative"), (1, Some(1), n(100)));
        assert_eq!(cell("25+", NOT_RECORDED), (0, Some(1), n(0)));
        assert_eq!(cell("15-19", NOT_RECORDED), (0, Some(0), None));
        // Across the HIV statuses, each age band's crossed cells add up to its band.
        for band in &evaluated.summary.disaggregations[0].bands {
            let statuses = &evaluated.summary.disaggregations[1].bands;
            let (count, denominator) = statuses.iter().fold((0, 0), |(c, d), status| {
                let s = evaluated.crossed(&percentage, &[(0, &band.code), (1, &status.code)]);
                (c + s.count, d + s.denominator.unwrap_or(0))
            });
            assert_eq!(
                (Some(count), Some(denominator)),
                (Some(band.summary.count), band.summary.denominator),
                "{} adds up",
                band.code
            );
        }
        // One band alone is that band; none is the whole.
        let alone = evaluated.crossed(&percentage, &[(1, "negative")]);
        assert_eq!(alone, evaluated.summary.disaggregations[1].bands[0].summary);
        let mut whole = evaluated.summary.clone();
        whole.disaggregations = Vec::new();
        assert_eq!(evaluated.crossed(&percentage, &[]), whole);
    }

    #[test]
    fn a_denominator_that_is_an_estimate_is_not_counted_from_records() {
        use PopulationKind::{InitialPopulation, Numerator};
        // ANC 1st visit coverage over expected pregnancies (#553): the facility counts the
        // numerator; the denominator is the place's estimate, which roll-up takes.
        let patients =
            [("a", true), ("b", false), ("c", true)].map(|(name, first)| PatientOutcome {
                patient: format!("Patient/{name}"),
                records: vec![record(
                    &format!("Encounter/{name}1"),
                    Some("2026-03-04"),
                    &[(InitialPopulation, Ok(true)), (Numerator, Ok(first))],
                )],
            });
        let coverage = Indicator {
            estimate: Some("expected-pregnancies".into()),
            ..indicator(Calculation::Percentage, Counts::People)
        };
        let evaluation = calculate(&coverage, &march(), &patients, &[]);
        assert_eq!(
            (
                evaluation.summary.count,
                evaluation.summary.denominator,
                evaluation.summary.value
            ),
            (2, None, None)
        );
        assert_eq!(evaluation.members[0].denominator, None);
        assert_eq!(
            evaluation.members[1].records[0].reason,
            Reason::DenominatorOnly
        );
        let rolled = Summary::roll_up(
            Calculation::Percentage,
            &[evaluation.summary.clone(), evaluation.summary],
        )
        .expect("rolled up");
        assert_eq!(
            (rolled.count, rolled.denominator, rolled.value),
            (4, None, None)
        );
    }
}
