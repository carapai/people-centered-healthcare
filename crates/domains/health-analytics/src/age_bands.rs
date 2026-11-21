//! Age Band Sets (analytics.md § Age Band Sets and Population Estimates; ADR 0071; #551): a
//! published, named list of age ranges with the unit and the date age is taken at, which an
//! indicator picks, pinned to its `url|version`, to disaggregate by age (#552).
//!
//! - **A definition** ([`AgeBandSetDefinition`]) is what a set says, as written. Its
//!   [`problems`](AgeBandSetDefinition::problems) are what Validation refuses: no unit or age-at
//!   date, no bands, a band without its lower age or going down, and above all **a gap or an
//!   overlap** between bands, each named with the ages it leaves out or counts twice.
//! - **A set** ([`AgeBandSet`]) is a definition without problems: its bands, each from an age to
//!   an age (both included) or open above the last, and [`AgeBandSet::band`] finds the band an
//!   age falls in. Ages below the first band, or above a closed last band, fall in none.
//! - **An age** ([`AgeUnit::age`]) is counted in whole units, as people say it: someone born on
//!   10 March 2010 is 15 on 9 March 2026 and 16 the next day.
//! - **Pinning** ([`pinned`]): an indicator names the set it picked as `url|version`, and keeps
//!   that version's bands whatever is published after it. An unpinned reference is refused.
//!
//! Changing a band is a major version and a rename a patch: the publication's semver check
//! (`health-packages`' `bump`) reads the same content.

use std::fmt;

use jiff::Unit;
use jiff::civil::Date;

/// The platform `Library.type` code of an Age Band Set.
pub const AGE_BAND_SET_TYPE: &str = "age-band-set";
/// The content type of an Age Band Set's inline content.
pub const AGE_BAND_SET_CONTENT: &str = "application/vnd.health.age-band-set+json";

/// The unit ages are counted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AgeUnit {
    /// Whole days.
    Days,
    /// Whole weeks.
    Weeks,
    /// Whole months.
    Months,
    /// Whole years.
    Years,
}

impl AgeUnit {
    /// The unit a code names: `days`, `weeks`, `months` or `years`.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "days" => Some(Self::Days),
            "weeks" => Some(Self::Weeks),
            "months" => Some(Self::Months),
            "years" => Some(Self::Years),
            _ => None,
        }
    }

    /// Its code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Days => "days",
            Self::Weeks => "weeks",
            Self::Months => "months",
            Self::Years => "years",
        }
    }

    const fn singular(self) -> &'static str {
        match self {
            Self::Days => "day",
            Self::Weeks => "week",
            Self::Months => "month",
            Self::Years => "year",
        }
    }

    /// The age on `at` of someone born on `birth`, in whole units; none before they were born.
    pub fn age(self, birth: Date, at: Date) -> Option<u32> {
        if at < birth {
            return None;
        }
        let unit = match self {
            Self::Days | Self::Weeks => Unit::Day,
            Self::Months => Unit::Month,
            Self::Years => Unit::Year,
        };
        let span = birth.until((unit, at)).ok()?;
        let whole = match self {
            Self::Days => i64::from(span.get_days()),
            Self::Weeks => i64::from(span.get_days()) / 7,
            Self::Months => i64::from(span.get_months()),
            Self::Years => i64::from(span.get_years()),
        };
        u32::try_from(whole).ok()
    }
}

/// The date age is taken at, within a Report's period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AgeAt {
    /// The period's first day.
    PeriodStart,
    /// The period's last day.
    PeriodEnd,
    /// The indicator's period date: when the care counted happened.
    PeriodDate,
}

impl AgeAt {
    /// The date a code names: `period-start`, `period-end` or `period-date`.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "period-start" => Some(Self::PeriodStart),
            "period-end" => Some(Self::PeriodEnd),
            "period-date" => Some(Self::PeriodDate),
            _ => None,
        }
    }

    /// Its code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::PeriodStart => "period-start",
            Self::PeriodEnd => "period-end",
            Self::PeriodDate => "period-date",
        }
    }
}

/// One band, from an age to an age, both included, or open above.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgeBand {
    /// The lowest age in it.
    pub from: u32,
    /// The highest age in it; none is every age from `from` up.
    pub to: Option<u32>,
}

impl AgeBand {
    /// Whether `age` is in it.
    pub fn contains(self, age: u32) -> bool {
        age >= self.from && self.to.is_none_or(|to| age <= to)
    }

    /// Its code, what a disaggregation's stratum carries: `15-19`, or `25+` when open.
    pub fn code(self) -> String {
        match self.to {
            Some(to) => format!("{}-{to}", self.from),
            None => format!("{}+", self.from),
        }
    }

    /// What a person reads, in `unit`: `15–19 years`, `0 years`, `1 year`, `25+ years`.
    pub fn label(self, unit: AgeUnit) -> String {
        let name = |n: u32| if n == 1 { unit.singular() } else { unit.code() };
        match self.to {
            Some(to) if to == self.from => format!("{to} {}", name(to)),
            Some(to) => format!("{}–{to} {}", self.from, name(to)),
            None => format!("{}+ {}", self.from, unit.code()),
        }
    }
}

/// One band, as written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BandDefinition {
    /// The lowest age in it.
    pub from: Option<i64>,
    /// The highest age in it; none is open above.
    pub to: Option<i64>,
}

/// What an Age Band Set says, as written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgeBandSetDefinition {
    /// Its canonical URL.
    pub url: Option<String>,
    /// Its version.
    pub version: Option<String>,
    /// What a person reads.
    pub title: Option<String>,
    /// The unit, by code.
    pub unit: Option<String>,
    /// The date age is taken at, by code.
    pub age_at: Option<String>,
    /// The bands, in the order a Report shows them.
    pub bands: Vec<BandDefinition>,
    /// Why its content could not be read, if it could not.
    pub unreadable: Option<String>,
}

/// An Age Band Set without problems.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgeBandSet {
    /// Its canonical URL.
    pub url: String,
    /// Its version.
    pub version: String,
    /// What a person reads.
    pub title: String,
    /// The unit ages are counted in.
    pub unit: AgeUnit,
    /// The date age is taken at.
    pub age_at: AgeAt,
    /// The bands, in the order a Report shows them.
    pub bands: Vec<AgeBand>,
}

impl AgeBandSet {
    /// Its canonical reference, `url|version`.
    pub fn canonical(&self) -> String {
        format!("{}|{}", self.url, self.version)
    }

    /// The band `age` falls in, if any.
    pub fn band(&self, age: u32) -> Option<AgeBand> {
        self.bands.iter().copied().find(|band| band.contains(age))
    }
}

/// Ages `from` to `to` (none: and over) in `unit` are `where`, for a problem.
fn ages(from: u32, to: Option<u32>, unit: AgeUnit, place: &str) -> String {
    let unit = unit.code();
    match to {
        Some(to) if to == from => format!("age {from} ({unit}) is {place}"),
        Some(to) => format!("ages {from} to {to} ({unit}) are {place}"),
        None => format!("ages {from} and over ({unit}) are {place}"),
    }
}

/// The gaps and overlaps between `bands`, each named with the ages concerned.
fn gaps_and_overlaps(bands: &[AgeBand], unit: AgeUnit) -> Vec<String> {
    let mut sorted = bands.to_vec();
    sorted.sort();
    let mut problems = Vec::new();
    for pair in sorted.windows(2) {
        let [before, after] = pair else { continue };
        let label = |band: &AgeBand| band.label(unit);
        match before.to {
            None => problems.push(format!(
                "the bands {} and {} overlap: {}",
                label(before),
                label(after),
                ages(after.from, after.to, unit, "in both")
            )),
            Some(to) if after.from <= to => {
                let end = after.to.map_or(to, |a| a.min(to));
                problems.push(format!(
                    "the bands {} and {} overlap: {}",
                    label(before),
                    label(after),
                    ages(after.from, Some(end), unit, "in both")
                ));
            }
            Some(to) if after.from > to + 1 => problems.push(format!(
                "there is a gap between the bands {} and {}: {}",
                label(before),
                label(after),
                ages(to + 1, Some(after.from - 1), unit, "in neither")
            )),
            Some(_) => {}
        }
    }
    problems
}

impl AgeBandSetDefinition {
    /// Every reason this is not a usable Age Band Set, for Validation.
    pub fn problems(&self) -> Vec<String> {
        match self.age_band_set() {
            Ok(_) => Vec::new(),
            Err(problems) => problems,
        }
    }

    /// The set, or every reason this is not one.
    pub fn age_band_set(&self) -> Result<AgeBandSet, Vec<String>> {
        let mut problems = Vec::new();
        let mut text = |value: &Option<String>, what: &str| match value.as_deref() {
            Some(v) if !v.trim().is_empty() => v.to_owned(),
            _ => {
                problems.push(format!("the Age Band Set has no {what}"));
                String::new()
            }
        };
        let url = text(&self.url, "url");
        let version = text(&self.version, "version");
        let title = text(&self.title, "title");
        if let Some(why) = &self.unreadable {
            problems.push(format!("the Age Band Set's content {why}"));
            return Err(problems);
        }
        let unit = match self.unit.as_deref() {
            None => {
                problems.push("the Age Band Set has no unit (days, weeks, months or years)".into());
                None
            }
            Some(code) => {
                let unit = AgeUnit::from_code(code);
                if unit.is_none() {
                    problems.push(format!(
                        "the Age Band Set's unit {code} is not days, weeks, months or years"
                    ));
                }
                unit
            }
        };
        let age_at = match self.age_at.as_deref() {
            None => {
                problems.push(
                    "the Age Band Set does not say the date age is taken at (period-start, period-end or period-date)"
                        .into(),
                );
                None
            }
            Some(code) => {
                let at = AgeAt::from_code(code);
                if at.is_none() {
                    problems.push(format!(
                        "the Age Band Set takes age at {code}, not period-start, period-end or period-date"
                    ));
                }
                at
            }
        };
        if self.bands.is_empty() {
            problems.push("the Age Band Set has no bands".into());
        }
        let mut bands = Vec::new();
        for (n, band) in self.bands.iter().enumerate() {
            let row = n + 1;
            let Some(from) = band.from else {
                problems.push(format!("band {row} has no lowest age (from)"));
                continue;
            };
            let Ok(from) = u32::try_from(from) else {
                problems.push(format!("band {row} starts at {from}, below zero"));
                continue;
            };
            let to = match band.to.map(u32::try_from) {
                None => None,
                Some(Ok(to)) if to >= from => Some(to),
                Some(_) => {
                    let to = band.to.unwrap_or_default();
                    problems.push(format!("band {row} goes down from {from} to {to}"));
                    continue;
                }
            };
            bands.push(AgeBand { from, to });
        }
        if let Some(unit) = unit {
            problems.extend(gaps_and_overlaps(&bands, unit));
        }
        match (unit, age_at) {
            (Some(unit), Some(age_at)) if problems.is_empty() => Ok(AgeBandSet {
                url,
                version,
                title,
                unit,
                age_at,
                bands,
            }),
            _ => Err(problems),
        }
    }
}

/// Why a reference to an Age Band Set finds none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinError {
    /// It names no version: `url` rather than `url|version`.
    Unpinned(String),
    /// No set has that `url|version`.
    NotFound(String),
}

impl fmt::Display for PinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unpinned(reference) => write!(
                f,
                "the Age Band Set {reference} is not pinned to a version (url|version)"
            ),
            Self::NotFound(reference) => {
                write!(f, "no published Age Band Set is {reference}")
            }
        }
    }
}

impl std::error::Error for PinError {}

/// Whether `reference` names a version: `url|version`, both parts given.
pub fn is_pinned(reference: &str) -> bool {
    reference
        .split_once('|')
        .is_some_and(|(url, version)| !url.trim().is_empty() && !version.trim().is_empty())
}

/// The set `reference` (`url|version`) pins among `sets`: exactly that version, whatever was
/// published after it.
pub fn pinned<'a>(
    reference: &str,
    sets: impl IntoIterator<Item = &'a AgeBandSet>,
) -> Result<&'a AgeBandSet, PinError> {
    if !is_pinned(reference) {
        return Err(PinError::Unpinned(reference.to_owned()));
    }
    sets.into_iter()
        .find(|set| set.canonical() == reference)
        .ok_or_else(|| PinError::NotFound(reference.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn band(from: i64, to: Option<i64>) -> BandDefinition {
        BandDefinition {
            from: Some(from),
            to,
        }
    }

    fn anc(bands: Vec<BandDefinition>) -> AgeBandSetDefinition {
        AgeBandSetDefinition {
            url: Some("https://health.go.ug/fhir/Library/age-bands-anc".into()),
            version: Some("1.0.0".into()),
            title: Some("MoH ANC age bands".into()),
            unit: Some("years".into()),
            age_at: Some("period-end".into()),
            bands,
            unreadable: None,
        }
    }

    fn moh_anc() -> AgeBandSetDefinition {
        anc(vec![
            band(10, Some(14)),
            band(15, Some(19)),
            band(20, Some(24)),
            band(25, None),
        ])
    }

    #[test]
    fn contiguous_bands_are_a_set() {
        let set = moh_anc().age_band_set().expect("a set");
        assert_eq!(set.unit, AgeUnit::Years);
        assert_eq!(set.age_at, AgeAt::PeriodEnd);
        assert_eq!(
            set.canonical(),
            "https://health.go.ug/fhir/Library/age-bands-anc|1.0.0"
        );
        assert_eq!(set.band(9), None);
        assert_eq!(set.band(10).map(AgeBand::code).as_deref(), Some("10-14"));
        assert_eq!(set.band(19).map(AgeBand::code).as_deref(), Some("15-19"));
        assert_eq!(set.band(49).map(AgeBand::code).as_deref(), Some("25+"));
        let labels: Vec<String> = set.bands.iter().map(|b| b.label(set.unit)).collect();
        assert_eq!(
            labels,
            ["10–14 years", "15–19 years", "20–24 years", "25+ years"]
        );
    }

    #[test]
    fn a_gap_fails_validation_naming_the_ages_left_out() {
        let set = anc(vec![band(10, Some(14)), band(20, Some(24))]);
        assert_eq!(
            set.problems(),
            [
                "there is a gap between the bands 10–14 years and 20–24 years: ages 15 to 19 (years) are in neither"
            ]
        );
    }

    #[test]
    fn an_overlap_fails_validation_naming_the_ages_counted_twice() {
        let set = anc(vec![band(10, Some(15)), band(15, Some(19))]);
        assert_eq!(
            set.problems(),
            ["the bands 10–15 years and 15–19 years overlap: age 15 (years) is in both"]
        );
        let open = anc(vec![band(20, None), band(25, Some(29))]);
        assert_eq!(
            open.problems(),
            ["the bands 20+ years and 25–29 years overlap: ages 25 to 29 (years) are in both"]
        );
    }

    #[test]
    fn bands_may_be_written_in_any_order() {
        let set = anc(vec![band(15, Some(19)), band(10, Some(14))]);
        assert!(set.problems().is_empty());
        let built = set.age_band_set().expect("a set");
        assert_eq!(built.bands[0].code(), "15-19", "the written order is kept");
    }

    #[test]
    fn every_other_problem_is_named() {
        let set = AgeBandSetDefinition {
            unit: Some("decades".into()),
            age_at: Some("birthday".into()),
            bands: vec![
                BandDefinition::default(),
                band(-1, Some(4)),
                band(9, Some(5)),
            ],
            ..AgeBandSetDefinition::default()
        };
        assert_eq!(
            set.problems(),
            [
                "the Age Band Set has no url",
                "the Age Band Set has no version",
                "the Age Band Set has no title",
                "the Age Band Set's unit decades is not days, weeks, months or years",
                "the Age Band Set takes age at birthday, not period-start, period-end or period-date",
                "band 1 has no lowest age (from)",
                "band 2 starts at -1, below zero",
                "band 3 goes down from 9 to 5",
            ]
        );
        let empty = AgeBandSetDefinition {
            unit: None,
            age_at: None,
            ..anc(Vec::new())
        };
        assert_eq!(
            empty.problems(),
            [
                "the Age Band Set has no unit (days, weeks, months or years)",
                "the Age Band Set does not say the date age is taken at (period-start, period-end or period-date)",
                "the Age Band Set has no bands",
            ]
        );
    }

    #[test]
    fn age_is_counted_in_whole_units() {
        let birth = Date::constant(2010, 3, 10);
        let years = |at| AgeUnit::Years.age(birth, at);
        assert_eq!(years(Date::constant(2026, 3, 9)), Some(15));
        assert_eq!(years(Date::constant(2026, 3, 10)), Some(16));
        assert_eq!(years(Date::constant(2009, 1, 1)), None);
        let newborn = Date::constant(2026, 3, 1);
        assert_eq!(
            AgeUnit::Days.age(newborn, Date::constant(2026, 3, 29)),
            Some(28)
        );
        assert_eq!(
            AgeUnit::Weeks.age(newborn, Date::constant(2026, 3, 14)),
            Some(1)
        );
        assert_eq!(
            AgeUnit::Months.age(newborn, Date::constant(2026, 4, 30)),
            Some(1)
        );
    }

    #[test]
    fn an_indicator_keeps_the_version_it_pinned() {
        let one = moh_anc().age_band_set().expect("version 1");
        let two = AgeBandSetDefinition {
            version: Some("2.0.0".into()),
            ..anc(vec![band(10, Some(19)), band(20, None)])
        }
        .age_band_set()
        .expect("version 2");
        let sets = [two.clone(), one.clone()];
        let found = pinned(
            "https://health.go.ug/fhir/Library/age-bands-anc|1.0.0",
            &sets,
        )
        .expect("version 1");
        assert_eq!(found.bands, one.bands);
        assert_eq!(
            pinned("https://health.go.ug/fhir/Library/age-bands-anc", &sets),
            Err(PinError::Unpinned(
                "https://health.go.ug/fhir/Library/age-bands-anc".into()
            ))
        );
        assert_eq!(
            pinned(
                "https://health.go.ug/fhir/Library/age-bands-anc|3.0.0",
                &sets
            )
            .map_err(|e| e.to_string()),
            Err("no published Age Band Set is https://health.go.ug/fhir/Library/age-bands-anc|3.0.0".into())
        );
    }
}
