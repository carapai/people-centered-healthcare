//! Who a count indicator counts in a period (analytics.md; studio.md § Indicators): care counts
//! in the period it happened in, by each record's period date, and the indicator's criteria
//! decide which of those records count. Counting people, a person counts once however many of
//! their records do.
//!
//! The membership is kept per patient, with each record's part in it, so that *Try* can say why
//! each person is counted or not, and so that a later change re-evaluates one patient only
//! (ADR 0068).

use jiff::civil::Date;

use crate::indicator::Counts;
use crate::period::Period;

/// How one record came out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordOutcome {
    /// The record, as a reference (`Encounter/…`).
    pub reference: String,
    /// Its period date, if it has one, or why it could not be evaluated.
    pub date: Result<Option<Date>, String>,
    /// Whether it meets the criteria, or why that could not be evaluated. Only asked of a record
    /// in the period.
    pub meets: Result<bool, String>,
}

/// One patient's records, each as it came out.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Its period date is outside the period.
    OutsidePeriod,
    /// It is in the period and meets the criteria: it counts.
    Meets,
    /// It is in the period but does not meet the criteria.
    DoesNotMeet,
    /// It is in the period, but the criteria could not be evaluated on it.
    Error(String),
}

/// One record's part in the count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordPart {
    /// The record.
    pub reference: String,
    /// Its period date.
    pub date: Option<Date>,
    /// Why it counts or not.
    pub reason: Reason,
}

impl RecordPart {
    /// Whether the record counts.
    pub fn counts(&self) -> bool {
        self.reason == Reason::Meets
    }
}

/// One patient in the count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// The patient.
    pub patient: String,
    /// How many times they count: once counting people, or once per counting record.
    pub count: u64,
    /// Each record's part, in the order given.
    pub records: Vec<RecordPart>,
}

/// A count over a period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Count {
    /// The figure.
    pub value: u64,
    /// Every patient, counted or not, in the order given.
    pub members: Vec<Member>,
}

impl Count {
    /// The patients counted, in the order given.
    pub fn counted(&self) -> impl Iterator<Item = &Member> {
        self.members.iter().filter(|m| m.count > 0)
    }

    /// Every record whose criteria could not be evaluated, with why.
    pub fn errors(&self) -> impl Iterator<Item = (&str, &str)> {
        self.members.iter().flat_map(|m| {
            m.records.iter().filter_map(|r| match &r.reason {
                Reason::Error(why) => Some((r.reference.as_str(), why.as_str())),
                _ => None,
            })
        })
    }
}

/// Why a count was not made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CountError {
    /// Counting this is not built yet.
    #[error("counting {} is not supported yet", .0.code())]
    Unsupported(Counts),
}

fn part(period: &Period, record: &RecordOutcome) -> RecordPart {
    let reason = match &record.date {
        Err(why) => Reason::Error(format!("its period date: {why}")),
        Ok(None) => Reason::NoPeriodDate,
        Ok(Some(date)) if !period.contains(*date) => Reason::OutsidePeriod,
        Ok(Some(_)) => match &record.meets {
            Ok(true) => Reason::Meets,
            Ok(false) => Reason::DoesNotMeet,
            Err(why) => Reason::Error(why.clone()),
        },
    };
    RecordPart {
        reference: record.reference.clone(),
        date: record.date.clone().ok().flatten(),
        reason,
    }
}

/// The count of `counts` over `period` from each patient's records.
pub fn count(
    counts: Counts,
    period: &Period,
    patients: &[PatientOutcome],
) -> Result<Count, CountError> {
    if counts != Counts::People {
        return Err(CountError::Unsupported(counts));
    }
    let members: Vec<Member> = patients
        .iter()
        .map(|patient| {
            let records: Vec<RecordPart> =
                patient.records.iter().map(|r| part(period, r)).collect();
            Member {
                patient: patient.patient.clone(),
                count: u64::from(records.iter().any(RecordPart::counts)),
                records,
            }
        })
        .collect();
    Ok(Count {
        value: members.iter().map(|m| m.count).sum(),
        members,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::period::PeriodType;

    fn record(reference: &str, date: Option<&str>, meets: Result<bool, &str>) -> RecordOutcome {
        RecordOutcome {
            reference: reference.into(),
            date: Ok(date.map(|d| d.parse().expect("date"))),
            meets: meets.map_err(str::to_owned),
        }
    }

    fn march() -> Period {
        Period::parse(PeriodType::Monthly, "2026-03").expect("march")
    }

    #[test]
    fn a_person_counts_once_however_many_of_their_records_count() {
        let patients = [
            PatientOutcome {
                patient: "Patient/a".into(),
                records: vec![
                    record("Encounter/a1", Some("2026-03-04"), Ok(true)),
                    record("Encounter/a2", Some("2026-03-28"), Ok(true)),
                ],
            },
            PatientOutcome {
                patient: "Patient/b".into(),
                records: vec![
                    record("Encounter/b1", Some("2026-02-27"), Ok(true)),
                    record("Encounter/b2", Some("2026-03-10"), Ok(false)),
                    record("Encounter/b3", None, Ok(true)),
                ],
            },
            PatientOutcome {
                patient: "Patient/c".into(),
                records: vec![record("Encounter/c1", Some("2026-03-31"), Ok(true))],
            },
        ];
        let count = count(Counts::People, &march(), &patients).expect("count");
        assert_eq!(count.value, 2);
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
    }

    #[test]
    fn a_criteria_error_counts_nothing_and_is_reported_only_in_the_period() {
        let patients = [PatientOutcome {
            patient: "Patient/a".into(),
            records: vec![
                record("Encounter/a1", Some("2026-03-04"), Err("no such function")),
                record("Encounter/a2", Some("2026-04-01"), Err("no such function")),
                RecordOutcome {
                    reference: "Encounter/a3".into(),
                    date: Err("not a date".into()),
                    meets: Ok(true),
                },
            ],
        }];
        let count = count(Counts::People, &march(), &patients).expect("count");
        assert_eq!(count.value, 0);
        assert_eq!(
            count.errors().collect::<Vec<_>>(),
            [
                ("Encounter/a1", "no such function"),
                ("Encounter/a3", "its period date: not a date")
            ]
        );
        assert!(!count.members[0].records[1].counts());
    }

    #[test]
    fn counting_other_things_waits_for_their_calculations() {
        let error = count(Counts::Visits, &march(), &[]).expect_err("visits");
        assert_eq!(error.to_string(), "counting visits is not supported yet");
    }
}
