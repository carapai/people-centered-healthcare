//! Reporting periods (studio.md § Indicators): an indicator has no period of its own; a Report
//! chooses the period type and the period, and every indicator in it is evaluated over that
//! period. A period runs from its first day to its last, both included, in the facility's local
//! dates.

use std::fmt;

use jiff::civil::{Date, ISOWeekDate, Weekday};
use serde::{Deserialize, Serialize};

/// A kind of reporting period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PeriodType {
    /// An ISO week, Monday to Sunday: `2026W10`.
    Weekly,
    /// A calendar month: `2026-03`.
    Monthly,
    /// A calendar quarter: `2026Q1`.
    Quarterly,
    /// A half year, January or July onwards: `2026S1`.
    SemiAnnual,
    /// A calendar year: `2026`.
    Annual,
    /// A financial year from July, named by the year it starts in: `2025July` is July 2025 to
    /// June 2026.
    FinancialJuly,
}

impl PeriodType {
    /// Every period type, in the order a Report offers them.
    pub const ALL: [Self; 6] = [
        Self::Weekly,
        Self::Monthly,
        Self::Quarterly,
        Self::SemiAnnual,
        Self::Annual,
        Self::FinancialJuly,
    ];

    /// How a period of this type is written, for a message.
    pub const fn example(self) -> &'static str {
        match self {
            Self::Weekly => "2026W10",
            Self::Monthly => "2026-03",
            Self::Quarterly => "2026Q1",
            Self::SemiAnnual => "2026S1",
            Self::Annual => "2026",
            Self::FinancialJuly => "2025July",
        }
    }
}

/// Why a period could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{id} is not a {kind:?} period (write it like {example})")]
pub struct PeriodError {
    /// What was given.
    pub id: String,
    /// The period type it was read as.
    pub kind: PeriodType,
    /// How one is written.
    pub example: &'static str,
}

/// One reporting period: its type, its id and its first and last days.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Period {
    /// Its type.
    pub kind: PeriodType,
    /// Its id, as written (`2026-03`).
    pub id: String,
    /// Its first day.
    pub start: Date,
    /// Its last day.
    pub end: Date,
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id)
    }
}

fn year(text: &str) -> Option<i16> {
    if text.len() == 4 && text.bytes().all(|b| b.is_ascii_digit()) {
        text.parse().ok()
    } else {
        None
    }
}

fn number(text: &str, most: i8) -> Option<i8> {
    if text.is_empty() || text.len() > 2 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().filter(|n| (1..=most).contains(n))
}

/// The months `first` to `first + months - 1` of `year`, as a period.
fn months(year: i16, first: i8, months: i8) -> Option<(Date, Date)> {
    let start = Date::new(year, first, 1).ok()?;
    let last = start
        .checked_add(jiff::Span::new().months(months - 1))
        .ok()?
        .last_of_month();
    Some((start, last))
}

impl Period {
    /// Reads the period `id` of type `kind`.
    pub fn parse(kind: PeriodType, id: &str) -> Result<Self, PeriodError> {
        let refused = || PeriodError {
            id: id.to_owned(),
            kind,
            example: kind.example(),
        };
        let bounds = match kind {
            PeriodType::Weekly => id.split_once('W').and_then(|(y, w)| {
                let week = ISOWeekDate::new(year(y)?, number(w, 53)?, Weekday::Monday).ok()?;
                let start = week.date();
                Some((start, start.checked_add(jiff::Span::new().days(6)).ok()?))
            }),
            PeriodType::Monthly => id
                .split_once('-')
                .and_then(|(y, m)| (m.len() == 2).then_some((y, m)))
                .and_then(|(y, m)| months(year(y)?, number(m, 12)?, 1)),
            PeriodType::Quarterly => id
                .split_once('Q')
                .and_then(|(y, q)| months(year(y)?, (number(q, 4)? - 1) * 3 + 1, 3)),
            PeriodType::SemiAnnual => id
                .split_once('S')
                .and_then(|(y, s)| months(year(y)?, (number(s, 2)? - 1) * 6 + 1, 6)),
            PeriodType::Annual => year(id).and_then(|y| months(y, 1, 12)),
            PeriodType::FinancialJuly => id
                .strip_suffix("July")
                .and_then(|y| months(year(y)?, 7, 12)),
        };
        let (start, end) = bounds.ok_or_else(refused)?;
        Ok(Self {
            kind,
            id: id.to_owned(),
            start,
            end,
        })
    }

    /// Whether `date` falls in the period.
    pub fn contains(&self, date: Date) -> bool {
        self.start <= date && date <= self.end
    }

    /// The first days of the `n` periods of its type before this one, the nearest first: a
    /// date is in the previous `n` periods when it is on or after the `n`th and before this
    /// period's start (#550).
    pub fn previous_starts(&self, n: usize) -> Vec<Date> {
        let step = match self.kind {
            PeriodType::Weekly => jiff::Span::new().weeks(1),
            PeriodType::Monthly => jiff::Span::new().months(1),
            PeriodType::Quarterly => jiff::Span::new().months(3),
            PeriodType::SemiAnnual => jiff::Span::new().months(6),
            PeriodType::Annual | PeriodType::FinancialJuly => jiff::Span::new().years(1),
        };
        let mut starts = Vec::with_capacity(n);
        let mut start = self.start;
        while starts.len() < n {
            match start.checked_sub(step) {
                Ok(earlier) => {
                    starts.push(earlier);
                    start = earlier;
                }
                Err(_) => break,
            }
        }
        starts
    }
}

/// The id of the period of type `kind` that holds `date`.
fn id_of(kind: PeriodType, date: Date) -> String {
    match kind {
        PeriodType::Weekly => {
            let week = date.iso_week_date();
            format!("{}W{:02}", week.year(), week.week())
        }
        PeriodType::Monthly => format!("{}-{:02}", date.year(), date.month()),
        PeriodType::Quarterly => format!("{}Q{}", date.year(), (date.month() - 1) / 3 + 1),
        PeriodType::SemiAnnual => format!("{}S{}", date.year(), (date.month() - 1) / 6 + 1),
        PeriodType::Annual => date.year().to_string(),
        PeriodType::FinancialJuly if date.month() >= 7 => format!("{}July", date.year()),
        PeriodType::FinancialJuly => format!("{}July", date.year() - 1),
    }
}

impl Period {
    /// The period of type `kind` that holds `date`: where care on that date counts (#554).
    pub fn of(kind: PeriodType, date: Date) -> Self {
        let id = id_of(kind, date);
        Self::parse(kind, &id).unwrap_or(Self {
            kind,
            id,
            start: date,
            end: date,
        })
    }

    /// The period of its type just after this one.
    pub fn next(&self) -> Option<Self> {
        Some(Self::of(self.kind, self.end.tomorrow().ok()?))
    }

    /// The period of its type just before this one, for a Report's % change (ADR 0072).
    pub fn previous(&self) -> Option<Self> {
        Some(Self::of(self.kind, self.start.yesterday().ok()?))
    }
}

/// How many periods back *in the previous N periods* can reach: `%previousPeriods.start` holds
/// this many starts.
pub const PREVIOUS_PERIODS: usize = 60;

/// The local date a FHIR `date` or `dateTime` was written for: its first ten characters, so care
/// at 23:30 on 31 March in the facility's time zone counts toward March. A partial date (a year,
/// or a year and month) places nothing in a period.
pub fn local_date(written: &str) -> Option<Date> {
    let day = written.get(..10)?;
    let bytes = day.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    day.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> Date {
        text.parse().expect("date")
    }

    fn bounds(kind: PeriodType, id: &str) -> (Date, Date) {
        let period = Period::parse(kind, id).expect("period");
        (period.start, period.end)
    }

    #[test]
    fn each_period_type_reads_its_first_and_last_day() {
        let cases = [
            (PeriodType::Weekly, "2026W10", "2026-03-02", "2026-03-08"),
            (PeriodType::Weekly, "2026W01", "2025-12-29", "2026-01-04"),
            (PeriodType::Monthly, "2026-03", "2026-03-01", "2026-03-31"),
            (PeriodType::Monthly, "2028-02", "2028-02-01", "2028-02-29"),
            (PeriodType::Quarterly, "2026Q1", "2026-01-01", "2026-03-31"),
            (PeriodType::Quarterly, "2026Q4", "2026-10-01", "2026-12-31"),
            (PeriodType::SemiAnnual, "2025S2", "2025-07-01", "2025-12-31"),
            (PeriodType::Annual, "2026", "2026-01-01", "2026-12-31"),
            (
                PeriodType::FinancialJuly,
                "2025July",
                "2025-07-01",
                "2026-06-30",
            ),
        ];
        for (kind, id, start, end) in cases {
            assert_eq!(bounds(kind, id), (date(start), date(end)), "{id}");
        }
    }

    #[test]
    fn a_period_written_another_way_is_refused() {
        for (kind, id) in [
            (PeriodType::Monthly, "202603"),
            (PeriodType::Monthly, "2026-13"),
            (PeriodType::Monthly, "2026-3"),
            (PeriodType::Weekly, "2026W54"),
            (PeriodType::Quarterly, "2026Q5"),
            (PeriodType::SemiAnnual, "2026S0"),
            (PeriodType::Annual, "26"),
            (PeriodType::FinancialJuly, "2025"),
        ] {
            let error = Period::parse(kind, id).expect_err(id);
            assert_eq!(error.example, kind.example());
            assert!(error.to_string().contains(id));
        }
    }

    #[test]
    fn the_previous_periods_start_one_period_of_its_type_apart() {
        let starts = |kind, id, n| -> Vec<String> {
            Period::parse(kind, id)
                .expect("period")
                .previous_starts(n)
                .iter()
                .map(ToString::to_string)
                .collect()
        };
        assert_eq!(
            starts(PeriodType::Monthly, "2026-03", 3),
            ["2026-02-01", "2026-01-01", "2025-12-01"]
        );
        assert_eq!(
            starts(PeriodType::Weekly, "2026W10", 2),
            ["2026-02-23", "2026-02-16"]
        );
        assert_eq!(
            starts(PeriodType::Quarterly, "2026Q1", 2),
            ["2025-10-01", "2025-07-01"]
        );
        assert_eq!(starts(PeriodType::SemiAnnual, "2026S1", 1), ["2025-07-01"]);
        assert_eq!(starts(PeriodType::Annual, "2026", 1), ["2025-01-01"]);
        assert_eq!(
            starts(PeriodType::FinancialJuly, "2025July", 2),
            ["2024-07-01", "2023-07-01"]
        );
    }

    #[test]
    fn the_previous_period_is_the_one_before_of_the_same_type() {
        for (kind, id, previous) in [
            (PeriodType::Weekly, "2026W01", "2025W52"),
            (PeriodType::Monthly, "2026-01", "2025-12"),
            (PeriodType::Quarterly, "2026Q1", "2025Q4"),
            (PeriodType::SemiAnnual, "2026S2", "2026S1"),
            (PeriodType::Annual, "2026", "2025"),
            (PeriodType::FinancialJuly, "2025July", "2024July"),
        ] {
            let before = Period::parse(kind, id)
                .expect("period")
                .previous()
                .expect("previous");
            assert_eq!(
                before,
                Period::parse(kind, previous).expect("previous"),
                "{id}"
            );
        }
    }

    #[test]
    fn a_date_is_in_the_period_of_each_type_that_holds_it() {
        for (kind, day, id) in [
            (PeriodType::Weekly, "2026-01-01", "2026W01"),
            (PeriodType::Weekly, "2027-01-01", "2026W53"),
            (PeriodType::Monthly, "2026-03-31", "2026-03"),
            (PeriodType::Quarterly, "2026-04-01", "2026Q2"),
            (PeriodType::SemiAnnual, "2026-06-30", "2026S1"),
            (PeriodType::Annual, "2026-12-31", "2026"),
            (PeriodType::FinancialJuly, "2026-06-30", "2025July"),
            (PeriodType::FinancialJuly, "2026-07-01", "2026July"),
        ] {
            let period = Period::of(kind, date(day));
            assert_eq!(period, Period::parse(kind, id).expect("period"), "{day}");
            assert!(period.contains(date(day)), "{day}");
        }
        let march = Period::parse(PeriodType::Monthly, "2026-03").expect("march");
        assert_eq!(march.next().expect("next").id, "2026-04");
        assert_eq!(march.next().and_then(|p| p.previous()), Some(march));
    }

    #[test]
    fn a_period_holds_its_first_and_last_day_only() {
        let march = Period::parse(PeriodType::Monthly, "2026-03").expect("march");
        assert_eq!(march.to_string(), "2026-03");
        assert!(march.contains(date("2026-03-01")));
        assert!(march.contains(date("2026-03-31")));
        assert!(!march.contains(date("2026-02-28")));
        assert!(!march.contains(date("2026-04-01")));
    }

    #[test]
    fn care_is_placed_by_the_local_date_it_was_written_for() {
        assert_eq!(
            local_date("2026-03-31T23:30:00+03:00"),
            Some(date("2026-03-31"))
        );
        assert_eq!(local_date("2026-03-04"), Some(date("2026-03-04")));
        assert_eq!(local_date("2026-03"), None);
        assert_eq!(local_date("2026"), None);
        assert_eq!(local_date("not a date"), None);
    }

    #[test]
    fn every_period_type_has_an_example_that_reads() {
        for kind in PeriodType::ALL {
            assert!(Period::parse(kind, kind.example()).is_ok(), "{kind:?}");
        }
    }
}
