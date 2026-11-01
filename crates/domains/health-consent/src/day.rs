use std::fmt;

/// A calendar day (no time zone), for ages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day {
    /// The year.
    pub year: i32,
    /// The month, 1–12.
    pub month: u8,
    /// The day, 1–31.
    pub day: u8,
}

/// A date that is not a FHIR `date`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("not a FHIR date (YYYY, YYYY-MM or YYYY-MM-DD)")]
pub struct InvalidDay;

fn days_in(year: i32, month: u8) -> u8 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 31,
    }
}

impl Day {
    /// A valid day.
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, InvalidDay> {
        if !(1..=12).contains(&month) || day == 0 || day > days_in(year, month) {
            return Err(InvalidDay);
        }
        Ok(Self { year, month, day })
    }

    fn parts(date: &str) -> Result<(i32, Option<u8>, Option<u8>), InvalidDay> {
        let number = |s: &str, len: usize| {
            if s.len() == len && s.bytes().all(|b| b.is_ascii_digit()) {
                s.parse::<i32>().map_err(|_| InvalidDay)
            } else {
                Err(InvalidDay)
            }
        };
        let parts: Vec<&str> = date.split('-').collect();
        let small = |s: &str| number(s, 2).and_then(|n| u8::try_from(n).map_err(|_| InvalidDay));
        match parts.as_slice() {
            [y] => Ok((number(y, 4)?, None, None)),
            [y, m] => Ok((number(y, 4)?, Some(small(m)?), None)),
            [y, m, d] => Ok((number(y, 4)?, Some(small(m)?), Some(small(d)?))),
            _ => Err(InvalidDay),
        }
    }

    /// The earliest day a FHIR `date` (`YYYY`, `YYYY-MM` or `YYYY-MM-DD`) allows.
    pub fn earliest(date: &str) -> Result<Self, InvalidDay> {
        let (year, month, day) = Self::parts(date)?;
        Self::new(year, month.unwrap_or(1), day.unwrap_or(1))
    }

    /// The latest day a FHIR `date` allows.
    pub fn latest(date: &str) -> Result<Self, InvalidDay> {
        let (year, month, day) = Self::parts(date)?;
        let month = month.unwrap_or(12);
        Self::new(year, month, day.unwrap_or_else(|| days_in(year, month)))
    }

    /// Whole years from this day to `on`; 0 before it. Someone born on 29 February has their
    /// birthday on 1 March in other years.
    pub fn age_on(self, on: Self) -> u32 {
        let mut years = on.year - self.year;
        if (on.month, on.day) < (self.month, self.day) {
            years -= 1;
        }
        u32::try_from(years).unwrap_or(0)
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}
