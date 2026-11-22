//! An indicator's definition (studio.md § Indicators; ADR 0068): a FHIR R5 `Measure` whose
//! populations are FHIRPath criteria over a patient's records, with the period date that places
//! care in a Report's period.
//!
//! [`MeasureDefinition`] is what a Measure says, as written, so that Validation can name each
//! problem ([`MeasureDefinition::problems`]); [`Indicator`] is a definition without problems,
//! which is what evaluation takes.
//!
//! *What it calculates* ([`Calculation`]) is saved as the Measure's scoring: a count is
//! `cohort`, a percentage `proportion`, a rate or a ratio `ratio` (a rate carries its factor, per
//! 100 to 100,000), and a sum, average, median, minimum, maximum or distinct count of a value is
//! `continuous-variable`, the value being its `measure-observation` with its aggregate method.

use std::collections::{BTreeMap, BTreeSet};

use crate::disaggregation::{Disaggregation, DisaggregationDefinition, DisaggregationSource};
use crate::membership::{Reference, references};

/// The FHIRPath expression language, as a FHIR `Expression.language`.
pub const FHIRPATH: &str = "text/fhirpath";

/// How an indicator's populations combine into its figure (FHIR `measure-scoring`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scoring {
    /// A count: how many are in the initial population.
    Cohort,
    /// A percentage: numerator over denominator.
    Proportion,
    /// A rate or a ratio: numerator over denominator, from different counts.
    Ratio,
    /// A value per counted thing, summarised (sum, average, median, …).
    ContinuousVariable,
}

impl Scoring {
    /// The scoring a `measure-scoring` code names.
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "cohort" => Self::Cohort,
            "proportion" => Self::Proportion,
            "ratio" => Self::Ratio,
            "continuous-variable" => Self::ContinuousVariable,
            _ => return None,
        })
    }

    /// Its `measure-scoring` code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Cohort => "cohort",
            Self::Proportion => "proportion",
            Self::Ratio => "ratio",
            Self::ContinuousVariable => "continuous-variable",
        }
    }

    /// The populations an indicator of this scoring must have.
    pub const fn populations(self) -> &'static [PopulationKind] {
        match self {
            Self::Cohort => &[PopulationKind::InitialPopulation],
            Self::Proportion | Self::Ratio => &[
                PopulationKind::InitialPopulation,
                PopulationKind::Denominator,
                PopulationKind::Numerator,
            ],
            Self::ContinuousVariable => &[
                PopulationKind::InitialPopulation,
                PopulationKind::MeasurePopulation,
            ],
        }
    }
}

/// A population of a Measure (FHIR `measure-population`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PopulationKind {
    /// Everything with care in the period that the indicator considers. A count's criteria.
    InitialPopulation,
    /// The denominator of a percentage, rate or ratio.
    Denominator,
    /// The numerator of a percentage, rate or ratio.
    Numerator,
    /// What a continuous-variable indicator takes a value of.
    MeasurePopulation,
    /// The value a continuous-variable indicator takes of each counted thing. Its criteria give
    /// a value, not true or false, so it is never among an [`Indicator`]'s populations.
    MeasureObservation,
}

impl PopulationKind {
    /// The population a `measure-population` code names; codes the platform does not author
    /// (exclusions, exceptions) are none: a *none of these* group does their work.
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "initial-population" => Self::InitialPopulation,
            "denominator" => Self::Denominator,
            "numerator" => Self::Numerator,
            "measure-population" => Self::MeasurePopulation,
            "measure-observation" => Self::MeasureObservation,
            _ => return None,
        })
    }

    /// Its `measure-population` code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InitialPopulation => "initial-population",
            Self::Denominator => "denominator",
            Self::Numerator => "numerator",
            Self::MeasurePopulation => "measure-population",
            Self::MeasureObservation => "measure-observation",
        }
    }
}

/// What one counted thing is (studio.md § Indicators, *What is counted*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Counts {
    /// People, each once however many of their records match.
    People,
    /// Program enrollments, each once.
    Enrollments,
    /// Visits, every matching one.
    Visits,
    /// Observations, every matching one.
    Observations,
}

impl Counts {
    /// What a code names.
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "people" => Self::People,
            "enrollments" => Self::Enrollments,
            "visits" => Self::Visits,
            "observations" => Self::Observations,
            _ => return None,
        })
    }

    /// Its code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::People => "people",
            Self::Enrollments => "enrollments",
            Self::Visits => "visits",
            Self::Observations => "observations",
        }
    }
}

/// How the values of a continuous-variable indicator are summarised: its `measure-observation`'s
/// aggregate method (FHIR `measure-aggregate-method`, and `distinct-count`, which it lacks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Aggregate {
    /// The sum of the values.
    Sum,
    /// Their mean.
    Average,
    /// Their median: the middle value, or the mean of the two middle values.
    Median,
    /// The smallest.
    Minimum,
    /// The largest.
    Maximum,
    /// How many different values there are, e.g. of a coded field.
    DistinctCount,
}

impl Aggregate {
    /// What a code names.
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "sum" => Self::Sum,
            "average" => Self::Average,
            "median" => Self::Median,
            "minimum" => Self::Minimum,
            "maximum" => Self::Maximum,
            "distinct-count" => Self::DistinctCount,
            _ => return None,
        })
    }

    /// Its code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Sum => "sum",
            Self::Average => "average",
            Self::Median => "median",
            Self::Minimum => "minimum",
            Self::Maximum => "maximum",
            Self::DistinctCount => "distinct-count",
        }
    }

    /// Whether facility figures can be combined into a district or national one. A median or a
    /// distinct count cannot: they stay at facility level (analytics.md).
    pub const fn rolls_up(self) -> bool {
        !matches!(self, Self::Median | Self::DistinctCount)
    }
}

/// The factors a rate may be expressed per.
pub const RATE_FACTORS: [u32; 4] = [100, 1_000, 10_000, 100_000];

/// What an indicator calculates (studio.md § Indicators, *What it calculates*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Calculation {
    /// How many are counted.
    Count,
    /// The numerator over the denominator, × 100.
    Percentage,
    /// The numerator over the denominator, × a factor (per 1,000, …).
    Rate {
        /// The factor, one of [`RATE_FACTORS`].
        per: u32,
    },
    /// The numerator over the denominator.
    Ratio,
    /// A value of each counted thing, summarised.
    Value(Aggregate),
}

impl Calculation {
    /// The scoring it is saved as.
    pub const fn scoring(self) -> Scoring {
        match self {
            Self::Count => Scoring::Cohort,
            Self::Percentage => Scoring::Proportion,
            Self::Rate { .. } | Self::Ratio => Scoring::Ratio,
            Self::Value(_) => Scoring::ContinuousVariable,
        }
    }

    /// Whether it has a denominator.
    pub const fn has_denominator(self) -> bool {
        matches!(self, Self::Percentage | Self::Rate { .. } | Self::Ratio)
    }

    /// Its code, as the editor names it.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Percentage => "percentage",
            Self::Rate { .. } => "rate",
            Self::Ratio => "ratio",
            Self::Value(aggregate) => aggregate.code(),
        }
    }
}

/// Which of a counted thing's values is taken, counting people or enrollments: the one from its
/// first record in the period, by the period date, or its last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pick {
    /// The first value.
    First,
    /// The last value.
    Last,
}

impl Pick {
    /// What a code names.
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "first" => Self::First,
            "last" => Self::Last,
            _ => return None,
        })
    }

    /// Its code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Last => "last",
        }
    }
}

/// The unit of a time between two dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TimeUnit {
    /// Days.
    Days,
    /// Whole weeks.
    Weeks,
    /// Whole calendar months.
    Months,
    /// Whole calendar years, as an age is.
    Years,
}

impl TimeUnit {
    /// What a code names.
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "days" => Self::Days,
            "weeks" => Self::Weeks,
            "months" => Self::Months,
            "years" => Self::Years,
            _ => return None,
        })
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

    /// The time from `from` to `to` in this unit, whole units only (negative when `to` is
    /// earlier).
    pub fn between(self, from: jiff::civil::Date, to: jiff::civil::Date) -> Result<i64, String> {
        let largest = match self {
            Self::Days | Self::Weeks => jiff::Unit::Day,
            Self::Months => jiff::Unit::Month,
            Self::Years => jiff::Unit::Year,
        };
        let span = from
            .until((largest, to))
            .map_err(|error| format!("no time between {from} and {to}: {error}"))?;
        Ok(match self {
            Self::Days => i64::from(span.get_days()),
            Self::Weeks => i64::from(span.get_days()) / 7,
            Self::Months => i64::from(span.get_months()),
            Self::Years => i64::from(span.get_years()),
        })
    }
}

/// Where a counted thing's value comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueSource {
    /// A FHIRPath expression giving it: a numeric field such as gestational age, or a code for
    /// a distinct count.
    Expression(String),
    /// The time between two dates, each a FHIRPath expression: days from the last menstrual
    /// period to the visit, or an age (years from `%patient.birthDate`).
    Between {
        /// The earlier date.
        from: String,
        /// The later date.
        to: String,
        /// The unit.
        unit: TimeUnit,
    },
}

/// The value a continuous-variable indicator takes of each counted thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndicatorValue {
    /// Where it comes from.
    pub source: ValueSource,
    /// Which one is taken, counting people or enrollments.
    pub pick: Pick,
}

/// The time between two dates, as written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BetweenDefinition {
    /// The earlier date.
    pub from: Option<Expression>,
    /// The later date.
    pub to: Option<Expression>,
    /// Its unit, by code.
    pub unit: Option<String>,
}

/// A FHIR `Expression`, as written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Expression {
    /// Its `language`.
    pub language: Option<String>,
    /// Its `expression`.
    pub expression: Option<String>,
}

impl Expression {
    /// A FHIRPath expression.
    pub fn fhirpath(expression: impl Into<String>) -> Self {
        Self {
            language: Some(FHIRPATH.to_owned()),
            expression: Some(expression.into()),
        }
    }

    /// The FHIRPath it holds, or why it holds none.
    pub(crate) fn fhirpath_text(&self) -> Result<&str, String> {
        match (self.language.as_deref(), self.expression.as_deref()) {
            (Some(FHIRPATH), Some(text)) if !text.trim().is_empty() => Ok(text),
            (Some(FHIRPATH), _) => Err("has no expression".into()),
            (Some(other), _) => Err(format!("is {other}, not FHIRPath ({FHIRPATH})")),
            (None, _) => Err(format!("names no language ({FHIRPATH})")),
        }
    }
}

/// A population of a Measure, as written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PopulationDefinition {
    /// Its `measure-population` code, if it has one.
    pub code: Option<String>,
    /// Its criteria: true or false of a record, or, for the `measure-observation`, its value.
    pub criteria: Option<Expression>,
    /// The `measure-observation`'s aggregate method, by code.
    pub aggregate: Option<String>,
    /// The `measure-observation`'s value as the time between two dates, instead of criteria.
    pub between: Option<BetweenDefinition>,
    /// Which value the `measure-observation` takes, by code; none is the first.
    pub pick: Option<String>,
}

/// What a Measure says, as written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeasureDefinition {
    /// Its canonical URL.
    pub url: Option<String>,
    /// Its version.
    pub version: Option<String>,
    /// What a person reads.
    pub title: Option<String>,
    /// Its `measure-scoring` code.
    pub scoring: Option<String>,
    /// The type of record its criteria are evaluated on (`Measure.basis`): `Encounter`, …
    pub basis: Option<String>,
    /// What it counts, by code; none is people.
    pub counts: Option<String>,
    /// A rate's factor (per 100, 1,000, …).
    pub rate_per: Option<i64>,
    /// Whether it counts from the first record up to the end of the period, instead of within
    /// it; none is not.
    pub cumulative: Option<bool>,
    /// The period date: the declared date that places care in a Report's period.
    pub period_date: Option<Expression>,
    /// Its populations, in its first group.
    pub populations: Vec<PopulationDefinition>,
    /// How many groups it has.
    pub groups: usize,
    /// What it is broken down by: its first group's stratifiers, in order (#551, #552).
    pub disaggregations: Vec<DisaggregationDefinition>,
}

/// An indicator without problems: what evaluation takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Indicator {
    /// Its canonical URL.
    pub url: String,
    /// Its version.
    pub version: String,
    /// What a person reads.
    pub title: String,
    /// How its populations combine.
    pub scoring: Scoring,
    /// What it calculates.
    pub calculation: Calculation,
    /// The type of record its criteria are evaluated on.
    pub basis: String,
    /// What it counts.
    pub counts: Counts,
    /// Whether it counts from the first record up to the end of the period.
    pub cumulative: bool,
    /// The value it takes of each counted thing, if it is a continuous-variable indicator.
    pub value: Option<IndicatorValue>,
    /// The FHIRPath giving a record's period date.
    pub period_date: String,
    /// Each population's FHIRPath criteria.
    pub populations: BTreeMap<PopulationKind, String>,
    /// The other indicators' sides its criteria require a patient to be in (#550).
    pub references: BTreeSet<Reference>,
    /// What it is broken down by, in order (#552).
    pub disaggregations: Vec<Disaggregation>,
}

impl Indicator {
    /// Its canonical reference, `url|version`.
    pub fn canonical(&self) -> String {
        format!("{}|{}", self.url, self.version)
    }

    /// The Age Band Sets it disaggregates by, each pinned to `url|version` (#551).
    pub fn age_band_sets(&self) -> Vec<&str> {
        self.disaggregations
            .iter()
            .filter_map(|d| match &d.source {
                DisaggregationSource::AgeBands(set) => Some(set.as_str()),
                DisaggregationSource::Coded(_) => None,
            })
            .collect()
    }

    /// The criteria of `population`.
    pub fn criteria(&self, population: PopulationKind) -> Option<&str> {
        self.populations.get(&population).map(String::as_str)
    }
}

fn is_resource_type(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase()) && chars.all(|c| c.is_ascii_alphanumeric())
}

impl MeasureDefinition {
    /// Every reason this Measure is not a usable indicator, for Validation.
    pub fn problems(&self) -> Vec<String> {
        match self.indicator() {
            Ok(_) => Vec::new(),
            Err(problems) => problems,
        }
    }

    /// The indicator, or every reason this Measure is not one.
    pub fn indicator(&self) -> Result<Indicator, Vec<String>> {
        let mut problems = Vec::new();
        let mut text = |value: &Option<String>, what: &str| match value.as_deref() {
            Some(v) if !v.trim().is_empty() => v.to_owned(),
            _ => {
                problems.push(format!("the indicator has no {what}"));
                String::new()
            }
        };
        let url = text(&self.url, "url");
        let version = text(&self.version, "version");
        let title = text(&self.title, "title");
        let scoring = match self.scoring.as_deref() {
            None => {
                problems.push(
                    "the indicator has no scoring (cohort, proportion, ratio or continuous-variable)"
                        .into(),
                );
                None
            }
            Some(code) => {
                let scoring = Scoring::from_code(code);
                if scoring.is_none() {
                    problems.push(format!(
                        "the indicator's scoring {code} is not a measure-scoring code"
                    ));
                }
                scoring
            }
        };
        let basis = match self.basis.as_deref() {
            Some(basis) if is_resource_type(basis) => basis.to_owned(),
            Some(basis) => {
                problems.push(format!(
                    "the indicator's basis {basis} is not the type of record its criteria read"
                ));
                String::new()
            }
            None => {
                problems.push(
                    "the indicator has no basis: the type of record its criteria read (Encounter, Observation, …)"
                        .into(),
                );
                String::new()
            }
        };
        let counts = match self.counts.as_deref() {
            None => Counts::People,
            Some(code) => Counts::from_code(code).unwrap_or_else(|| {
                problems.push(format!(
                    "the indicator counts {code}, which is not people, enrollments, visits or observations"
                ));
                Counts::People
            }),
        };
        let period_date = match &self.period_date {
            None => {
                problems.push(
                    "the indicator has no period date: the declared date that places care in a Report's period"
                        .into(),
                );
                String::new()
            }
            Some(expression) => match expression.fhirpath_text() {
                Ok(text) => text.to_owned(),
                Err(why) => {
                    problems.push(format!("the indicator's period date {why}"));
                    String::new()
                }
            },
        };
        if self.groups > 1 {
            problems.push("the indicator has more than one group".into());
        }
        let mut populations = BTreeMap::new();
        let mut referred = BTreeSet::new();
        let mut observations = Vec::new();
        for population in &self.populations {
            let Some(code) = population.code.as_deref() else {
                problems.push("a population of the indicator has no code".into());
                continue;
            };
            let Some(kind) = PopulationKind::from_code(code) else {
                problems.push(format!(
                    "the indicator's population {code} is not one the platform authors"
                ));
                continue;
            };
            if kind == PopulationKind::MeasureObservation {
                observations.push(population);
                continue;
            }
            let criteria = population.criteria.clone().unwrap_or_default();
            match criteria.fhirpath_text() {
                Ok(text) => {
                    match references(text) {
                        Ok(found) => referred.extend(found),
                        Err(why) => problems.push(format!("the indicator's {code} criteria {why}")),
                    }
                    if populations.insert(kind, text.to_owned()).is_some() {
                        problems.push(format!("the indicator has two {code} populations"));
                    }
                }
                Err(why) => problems.push(format!("the indicator's {code} criteria {why}")),
            }
        }
        if let Some(scoring) = scoring {
            for wanted in scoring.populations() {
                if !populations.contains_key(wanted)
                    && !self
                        .populations
                        .iter()
                        .any(|p| p.code.as_deref() == Some(wanted.code()))
                {
                    problems.push(format!(
                        "the indicator's scoring {} needs a {} population",
                        scoring.code(),
                        wanted.code()
                    ));
                }
            }
            for kind in populations.keys() {
                if !scoring.populations().contains(kind) {
                    problems.push(format!(
                        "the indicator's scoring {} has no {} population",
                        scoring.code(),
                        kind.code()
                    ));
                }
            }
        }
        let value = match (scoring, observations.as_slice()) {
            (Some(Scoring::ContinuousVariable), []) => {
                problems.push(
                    "the indicator's scoring continuous-variable needs a measure-observation population: the value it takes of each counted thing"
                        .into(),
                );
                None
            }
            (Some(Scoring::ContinuousVariable), [observation]) => {
                observation_value(observation, &mut problems)
            }
            (Some(Scoring::ContinuousVariable), _) => {
                problems.push("the indicator has two measure-observation populations".into());
                None
            }
            (Some(scoring), [_, ..]) => {
                problems.push(format!(
                    "the indicator's scoring {} has no measure-observation population",
                    scoring.code()
                ));
                None
            }
            (_, _) => None,
        };
        let calculation = match (scoring, self.rate_per) {
            (Some(Scoring::Ratio), Some(per)) => {
                match u32::try_from(per).ok().filter(|p| RATE_FACTORS.contains(p)) {
                    Some(per) => Some(Calculation::Rate { per }),
                    None => {
                        problems.push(format!(
                            "the indicator's rate is per {per}, not per 100, 1000, 10000 or 100000"
                        ));
                        None
                    }
                }
            }
            (Some(scoring), Some(_)) => {
                problems.push(format!(
                    "the indicator's scoring {} has no rate factor: only a ratio is a rate",
                    scoring.code()
                ));
                None
            }
            (Some(Scoring::Cohort), None) => Some(Calculation::Count),
            (Some(Scoring::Proportion), None) => Some(Calculation::Percentage),
            (Some(Scoring::Ratio), None) => Some(Calculation::Ratio),
            (Some(Scoring::ContinuousVariable), None) => value
                .as_ref()
                .map(|(aggregate, _)| Calculation::Value(*aggregate)),
            (None, _) => None,
        };
        match (counts, basis.as_str()) {
            (Counts::Visits, basis) if !basis.is_empty() && basis != "Encounter" => problems.push(
                format!("the indicator counts visits, so its basis must be Encounter, not {basis}"),
            ),
            (Counts::Observations, basis) if !basis.is_empty() && basis != "Observation" => {
                problems.push(format!(
                    "the indicator counts observations, so its basis must be Observation, not {basis}"
                ));
            }
            _ => {}
        }
        let mut disaggregations: Vec<Disaggregation> = Vec::new();
        for (n, written) in self.disaggregations.iter().enumerate() {
            match written.disaggregation(n + 1) {
                Ok(disaggregation) => {
                    if disaggregations
                        .iter()
                        .any(|d| d.name == disaggregation.name)
                    {
                        problems.push(format!(
                            "the indicator disaggregates by {} twice",
                            disaggregation.name
                        ));
                    }
                    disaggregations.push(disaggregation);
                }
                Err(found) => problems.extend(found),
            }
        }
        let canonical = format!("{url}|{version}");
        if referred.iter().any(|r: &Reference| r.measure == canonical) {
            problems.push(format!(
                "the indicator refers to itself ({canonical}): indicators may not refer to each other in a circle"
            ));
        }
        match (scoring, calculation) {
            (Some(scoring), Some(calculation)) if problems.is_empty() => Ok(Indicator {
                url,
                version,
                title,
                scoring,
                calculation,
                basis,
                counts,
                cumulative: self.cumulative.unwrap_or(false),
                period_date,
                populations,
                references: referred,
                disaggregations,
                value: value.map(|(_, value)| value),
            }),
            _ => Err(problems),
        }
    }
}

/// The aggregate method and the value a `measure-observation` gives, naming its problems.
fn observation_value(
    observation: &PopulationDefinition,
    problems: &mut Vec<String>,
) -> Option<(Aggregate, IndicatorValue)> {
    let aggregate = match observation.aggregate.as_deref() {
        None => {
            problems.push(
                "the indicator's measure-observation has no aggregate method (sum, average, median, minimum, maximum or distinct-count)"
                    .into(),
            );
            None
        }
        Some(code) => {
            let aggregate = Aggregate::from_code(code);
            if aggregate.is_none() {
                problems.push(format!(
                    "the indicator's aggregate method {code} is not sum, average, median, minimum, maximum or distinct-count"
                ));
            }
            aggregate
        }
    };
    let pick = match observation.pick.as_deref() {
        None => Some(Pick::First),
        Some(code) => {
            let pick = Pick::from_code(code);
            if pick.is_none() {
                problems.push(format!(
                    "the indicator takes the {code} value, not the first or the last"
                ));
            }
            pick
        }
    };
    let mut fhirpath = |expression: &Option<Expression>, what: &str| match expression
        .as_ref()
        .map(Expression::fhirpath_text)
    {
        Some(Ok(text)) => Some(text.to_owned()),
        Some(Err(why)) => {
            problems.push(format!("the indicator's {what} {why}"));
            None
        }
        None => {
            problems.push(format!("the indicator's {what} has no expression"));
            None
        }
    };
    let source = match (&observation.criteria, &observation.between) {
        (Some(_), Some(_)) => {
            problems.push(
                "the indicator's value is both an expression and the time between two dates".into(),
            );
            None
        }
        (None, None) => {
            problems.push(
                "the indicator's measure-observation has no value: an expression, or the time between two dates"
                    .into(),
            );
            None
        }
        (criteria @ Some(_), None) => fhirpath(criteria, "value").map(ValueSource::Expression),
        (None, Some(between)) => {
            let from = fhirpath(&between.from, "value's first date");
            let to = fhirpath(&between.to, "value's second date");
            let unit = match between.unit.as_deref() {
                None => {
                    problems.push(
                        "the indicator's value has no unit (days, weeks, months or years)".into(),
                    );
                    None
                }
                Some(code) => {
                    let unit = TimeUnit::from_code(code);
                    if unit.is_none() {
                        problems.push(format!(
                            "the indicator's value is in {code}, not days, weeks, months or years"
                        ));
                    }
                    unit
                }
            };
            match (from, to, unit) {
                (Some(from), Some(to), Some(unit)) => Some(ValueSource::Between { from, to, unit }),
                _ => None,
            }
        }
    };
    Some((
        aggregate?,
        IndicatorValue {
            source: source?,
            pick: pick?,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anc_first_visits() -> MeasureDefinition {
        MeasureDefinition {
            url: Some("https://health.go.ug/fhir/Measure/anc-first-visits".into()),
            version: Some("1.0.0".into()),
            title: Some("ANC 1st visits".into()),
            scoring: Some("cohort".into()),
            basis: Some("Encounter".into()),
            counts: Some("people".into()),
            rate_per: None,
            cumulative: None,
            period_date: Some(Expression::fhirpath("Encounter.period.start")),
            populations: vec![PopulationDefinition {
                code: Some("initial-population".into()),
                criteria: Some(Expression::fhirpath("%patient.gender = 'female'")),
                ..PopulationDefinition::default()
            }],
            groups: 1,
            disaggregations: vec![DisaggregationDefinition {
                age_band_set: Some("https://health.go.ug/fhir/Library/age-bands-anc|1.0.0".into()),
                ..DisaggregationDefinition::default()
            }],
        }
    }

    #[test]
    fn a_count_with_its_period_date_and_criteria_is_an_indicator() {
        let indicator = anc_first_visits().indicator().expect("an indicator");
        assert_eq!(indicator.scoring, Scoring::Cohort);
        assert_eq!(indicator.counts, Counts::People);
        assert_eq!(indicator.basis, "Encounter");
        assert_eq!(indicator.period_date, "Encounter.period.start");
        assert_eq!(
            indicator.criteria(PopulationKind::InitialPopulation),
            Some("%patient.gender = 'female'")
        );
        assert_eq!(indicator.criteria(PopulationKind::Numerator), None);
        assert!(anc_first_visits().problems().is_empty());
    }

    fn criteria(expression: &str) -> MeasureDefinition {
        MeasureDefinition {
            populations: vec![PopulationDefinition {
                code: Some("initial-population".into()),
                criteria: Some(Expression::fhirpath(expression)),
                ..PopulationDefinition::default()
            }],
            ..anc_first_visits()
        }
    }

    #[test]
    fn an_indicator_keeps_the_other_indicators_its_criteria_refer_to() {
        let other = "https://health.go.ug/fhir/Measure/anc-visits|1.0.0";
        let indicator = criteria(&format!(
            "'{other}' in %indicators.denominator and %patient.gender = 'female'"
        ))
        .indicator()
        .expect("an indicator");
        assert_eq!(
            indicator.references,
            BTreeSet::from([Reference {
                measure: other.into(),
                side: crate::membership::Side::Denominator,
            }])
        );
        assert_eq!(
            indicator.canonical(),
            "https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0"
        );
    }

    #[test]
    fn an_indicator_may_not_refer_to_itself_or_to_an_unpinned_one() {
        let itself = criteria(
            "'https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0' in %indicators.numerator",
        );
        assert_eq!(
            itself.problems(),
            [
                "the indicator refers to itself (https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0): indicators may not refer to each other in a circle"
            ]
        );
        let unpinned = criteria("'https://x/Measure/a' in %indicators.numerator");
        assert_eq!(
            unpinned.problems(),
            [
                "the indicator's initial-population criteria refers to the indicator https://x/Measure/a, which is not pinned to a version (url|version)"
            ]
        );
    }

    #[test]
    fn an_indicator_pins_the_age_band_sets_it_disaggregates_by() {
        let indicator = anc_first_visits().indicator().expect("an indicator");
        assert_eq!(
            indicator.age_band_sets(),
            ["https://health.go.ug/fhir/Library/age-bands-anc|1.0.0"]
        );
        let unpinned = MeasureDefinition {
            disaggregations: vec![DisaggregationDefinition {
                age_band_set: Some("https://health.go.ug/fhir/Library/age-bands-anc".into()),
                ..DisaggregationDefinition::default()
            }],
            ..anc_first_visits()
        };
        assert_eq!(
            unpinned.problems(),
            [
                "the indicator disaggregates by the Age Band Set https://health.go.ug/fhir/Library/age-bands-anc, which is not pinned to a version (url|version)"
            ]
        );
    }

    #[test]
    fn an_indicator_disaggregates_by_age_bands_or_by_a_coded_value() {
        let hiv = DisaggregationDefinition {
            code: Some("hiv-status".into()),
            criteria: Some(Expression::fhirpath("%patient.extension.value.code")),
            ..DisaggregationDefinition::default()
        };
        let mut measure = anc_first_visits();
        measure.disaggregations.push(hiv.clone());
        let indicator = measure.indicator().expect("an indicator");
        assert_eq!(
            indicator.disaggregations,
            [
                Disaggregation {
                    name: "https://health.go.ug/fhir/Library/age-bands-anc|1.0.0".into(),
                    source: DisaggregationSource::AgeBands(
                        "https://health.go.ug/fhir/Library/age-bands-anc|1.0.0".into()
                    ),
                },
                Disaggregation {
                    name: "hiv-status".into(),
                    source: DisaggregationSource::Coded("%patient.extension.value.code".into()),
                },
            ]
        );
        let measure = MeasureDefinition {
            disaggregations: vec![
                hiv.clone(),
                hiv.clone(),
                DisaggregationDefinition {
                    criteria: Some(Expression::fhirpath("%patient.gender")),
                    ..DisaggregationDefinition::default()
                },
                DisaggregationDefinition::default(),
                DisaggregationDefinition {
                    age_band_set: Some("https://x/Library/a|1".into()),
                    ..hiv
                },
            ],
            ..anc_first_visits()
        };
        assert_eq!(
            measure.problems(),
            [
                "the indicator disaggregates by hiv-status twice",
                "disaggregation 3 has no code: the name a Report shows it by",
                "disaggregation 4 names neither an Age Band Set nor the expression giving its code",
                "disaggregation 5 is both by an Age Band Set and by a coded value",
            ]
        );
    }

    #[test]
    fn a_measure_without_a_period_date_is_refused() {
        let measure = MeasureDefinition {
            period_date: None,
            ..anc_first_visits()
        };
        assert_eq!(
            measure.problems(),
            [
                "the indicator has no period date: the declared date that places care in a Report's period"
            ]
        );
        let measure = MeasureDefinition {
            period_date: Some(Expression {
                language: Some("text/cql".into()),
                expression: Some("Visit.start".into()),
            }),
            ..anc_first_visits()
        };
        assert_eq!(
            measure.problems(),
            ["the indicator's period date is text/cql, not FHIRPath (text/fhirpath)"]
        );
    }

    #[test]
    fn every_problem_is_named() {
        let measure = MeasureDefinition {
            groups: 2,
            populations: vec![
                PopulationDefinition {
                    code: Some("initial-population".into()),
                    criteria: Some(Expression {
                        language: Some(FHIRPATH.into()),
                        expression: Some(" ".into()),
                    }),
                    ..PopulationDefinition::default()
                },
                PopulationDefinition {
                    code: Some("numerator".into()),
                    criteria: Some(Expression::fhirpath("true")),
                    ..PopulationDefinition::default()
                },
                PopulationDefinition {
                    code: Some("denominator-exclusion".into()),
                    criteria: None,
                    ..PopulationDefinition::default()
                },
                PopulationDefinition::default(),
            ],
            ..MeasureDefinition::default()
        };
        assert_eq!(
            measure.problems(),
            [
                "the indicator has no url",
                "the indicator has no version",
                "the indicator has no title",
                "the indicator has no scoring (cohort, proportion, ratio or continuous-variable)",
                "the indicator has no basis: the type of record its criteria read (Encounter, Observation, …)",
                "the indicator has no period date: the declared date that places care in a Report's period",
                "the indicator has more than one group",
                "the indicator's initial-population criteria has no expression",
                "the indicator's population denominator-exclusion is not one the platform authors",
                "a population of the indicator has no code",
            ]
        );
    }

    #[test]
    fn a_scoring_needs_its_populations_and_only_those() {
        let measure = MeasureDefinition {
            scoring: Some("proportion".into()),
            ..anc_first_visits()
        };
        assert_eq!(
            measure.problems(),
            [
                "the indicator's scoring proportion needs a denominator population",
                "the indicator's scoring proportion needs a numerator population",
            ]
        );
        let mut measure = anc_first_visits();
        measure.populations.push(PopulationDefinition {
            code: Some("numerator".into()),
            criteria: Some(Expression::fhirpath("true")),
            ..PopulationDefinition::default()
        });
        measure.populations.push(PopulationDefinition {
            code: Some("numerator".into()),
            criteria: Some(Expression::fhirpath("false")),
            ..PopulationDefinition::default()
        });
        assert_eq!(
            measure.problems(),
            [
                "the indicator has two numerator populations",
                "the indicator's scoring cohort has no numerator population",
            ]
        );
    }

    #[test]
    fn unknown_codes_are_refused() {
        let measure = MeasureDefinition {
            scoring: Some("tally".into()),
            basis: Some("encounter visits".into()),
            counts: Some("households".into()),
            period_date: Some(Expression::default()),
            ..anc_first_visits()
        };
        assert_eq!(
            measure.problems(),
            [
                "the indicator's scoring tally is not a measure-scoring code",
                "the indicator's basis encounter visits is not the type of record its criteria read",
                "the indicator counts households, which is not people, enrollments, visits or observations",
                "the indicator's period date names no language (text/fhirpath)",
            ]
        );
    }

    #[test]
    fn codes_read_back() {
        for scoring in [
            Scoring::Cohort,
            Scoring::Proportion,
            Scoring::Ratio,
            Scoring::ContinuousVariable,
        ] {
            assert_eq!(Scoring::from_code(scoring.code()), Some(scoring));
        }
        for kind in [
            PopulationKind::InitialPopulation,
            PopulationKind::Denominator,
            PopulationKind::Numerator,
            PopulationKind::MeasurePopulation,
            PopulationKind::MeasureObservation,
        ] {
            assert_eq!(PopulationKind::from_code(kind.code()), Some(kind));
        }
        for aggregate in [
            Aggregate::Sum,
            Aggregate::Average,
            Aggregate::Median,
            Aggregate::Minimum,
            Aggregate::Maximum,
            Aggregate::DistinctCount,
        ] {
            assert_eq!(Aggregate::from_code(aggregate.code()), Some(aggregate));
        }
        for pick in [Pick::First, Pick::Last] {
            assert_eq!(Pick::from_code(pick.code()), Some(pick));
        }
        for unit in [
            TimeUnit::Days,
            TimeUnit::Weeks,
            TimeUnit::Months,
            TimeUnit::Years,
        ] {
            assert_eq!(TimeUnit::from_code(unit.code()), Some(unit));
        }
        for counts in [
            Counts::People,
            Counts::Enrollments,
            Counts::Visits,
            Counts::Observations,
        ] {
            assert_eq!(Counts::from_code(counts.code()), Some(counts));
        }
    }

    fn population(code: &str, expression: &str) -> PopulationDefinition {
        PopulationDefinition {
            code: Some(code.into()),
            criteria: Some(Expression::fhirpath(expression)),
            ..PopulationDefinition::default()
        }
    }

    fn with_denominator(scoring: &str) -> MeasureDefinition {
        let mut measure = anc_first_visits();
        measure.scoring = Some(scoring.into());
        measure.populations.push(population("denominator", "true"));
        measure.populations.push(population("numerator", "false"));
        measure
    }

    fn of_a_value(aggregate: Option<&str>) -> MeasureDefinition {
        let mut measure = anc_first_visits();
        measure.scoring = Some("continuous-variable".into());
        measure
            .populations
            .push(population("measure-population", "true"));
        measure.populations.push(PopulationDefinition {
            aggregate: aggregate.map(str::to_owned),
            ..population("measure-observation", "%records.entry.resource.value")
        });
        measure
    }

    #[test]
    fn what_it_calculates_is_saved_as_its_scoring() {
        let calculation = |measure: MeasureDefinition| {
            let indicator = measure.indicator().expect("an indicator");
            assert_eq!(indicator.calculation.scoring(), indicator.scoring);
            indicator.calculation
        };
        assert_eq!(calculation(anc_first_visits()), Calculation::Count);
        assert_eq!(
            calculation(with_denominator("proportion")),
            Calculation::Percentage
        );
        assert_eq!(calculation(with_denominator("ratio")), Calculation::Ratio);
        let rate = MeasureDefinition {
            rate_per: Some(1_000),
            ..with_denominator("ratio")
        };
        assert_eq!(calculation(rate), Calculation::Rate { per: 1_000 });
        for aggregate in [
            Aggregate::Sum,
            Aggregate::Average,
            Aggregate::Median,
            Aggregate::Minimum,
            Aggregate::Maximum,
            Aggregate::DistinctCount,
        ] {
            assert_eq!(
                calculation(of_a_value(Some(aggregate.code()))),
                Calculation::Value(aggregate)
            );
        }
        assert!(Calculation::Percentage.has_denominator());
        assert!(!Calculation::Value(Aggregate::Sum).has_denominator());
        assert!(Aggregate::Sum.rolls_up() && !Aggregate::Median.rolls_up());
    }

    #[test]
    fn a_value_is_an_expression_or_the_time_between_two_dates_taking_the_first_or_last() {
        let indicator = of_a_value(Some("median")).indicator().expect("indicator");
        assert_eq!(
            indicator.value,
            Some(IndicatorValue {
                source: ValueSource::Expression("%records.entry.resource.value".into()),
                pick: Pick::First,
            })
        );
        let mut age = of_a_value(Some("average"));
        age.populations[2] = PopulationDefinition {
            code: Some("measure-observation".into()),
            aggregate: Some("average".into()),
            pick: Some("last".into()),
            between: Some(BetweenDefinition {
                from: Some(Expression::fhirpath("%patient.birthDate")),
                to: Some(Expression::fhirpath("Encounter.actualPeriod.start")),
                unit: Some("years".into()),
            }),
            ..PopulationDefinition::default()
        };
        assert_eq!(
            age.indicator().expect("indicator").value,
            Some(IndicatorValue {
                source: ValueSource::Between {
                    from: "%patient.birthDate".into(),
                    to: "Encounter.actualPeriod.start".into(),
                    unit: TimeUnit::Years,
                },
                pick: Pick::Last,
            })
        );
        assert_eq!(anc_first_visits().indicator().expect("count").value, None);
    }

    #[test]
    fn a_calculation_that_is_not_whole_is_refused() {
        let measure = MeasureDefinition {
            rate_per: Some(250),
            ..with_denominator("ratio")
        };
        assert_eq!(
            measure.problems(),
            ["the indicator's rate is per 250, not per 100, 1000, 10000 or 100000"]
        );
        let measure = MeasureDefinition {
            rate_per: Some(100),
            ..with_denominator("proportion")
        };
        assert_eq!(
            measure.problems(),
            ["the indicator's scoring proportion has no rate factor: only a ratio is a rate"]
        );
        assert_eq!(
            of_a_value(None).problems(),
            [
                "the indicator's measure-observation has no aggregate method (sum, average, median, minimum, maximum or distinct-count)"
            ]
        );
        assert_eq!(
            of_a_value(Some("count")).problems(),
            [
                "the indicator's aggregate method count is not sum, average, median, minimum, maximum or distinct-count"
            ]
        );
        let mut measure = of_a_value(Some("sum"));
        measure.populations.pop();
        assert_eq!(
            measure.problems(),
            [
                "the indicator's scoring continuous-variable needs a measure-observation population: the value it takes of each counted thing"
            ]
        );
        let mut measure = with_denominator("proportion");
        measure
            .populations
            .push(population("measure-observation", "1"));
        assert_eq!(
            measure.problems(),
            ["the indicator's scoring proportion has no measure-observation population"]
        );
        let mut measure = of_a_value(Some("sum"));
        measure.populations[2] = PopulationDefinition {
            code: Some("measure-observation".into()),
            aggregate: Some("sum".into()),
            pick: Some("middle".into()),
            between: Some(BetweenDefinition {
                from: None,
                to: Some(Expression::fhirpath("Encounter.actualPeriod.start")),
                unit: Some("fortnights".into()),
            }),
            ..PopulationDefinition::default()
        };
        assert_eq!(
            measure.problems(),
            [
                "the indicator takes the middle value, not the first or the last",
                "the indicator's value's first date has no expression",
                "the indicator's value is in fortnights, not days, weeks, months or years",
            ]
        );
        measure.populations[2].between = None;
        measure.populations[2].pick = None;
        assert_eq!(
            measure.problems(),
            [
                "the indicator's measure-observation has no value: an expression, or the time between two dates"
            ]
        );
    }

    #[test]
    fn visits_and_observations_are_counted_from_their_own_records() {
        let measure = MeasureDefinition {
            counts: Some("observations".into()),
            ..anc_first_visits()
        };
        assert_eq!(
            measure.problems(),
            ["the indicator counts observations, so its basis must be Observation, not Encounter"]
        );
        let measure = MeasureDefinition {
            counts: Some("visits".into()),
            basis: Some("Observation".into()),
            ..anc_first_visits()
        };
        assert_eq!(
            measure.problems(),
            ["the indicator counts visits, so its basis must be Encounter, not Observation"]
        );
        let measure = MeasureDefinition {
            counts: Some("enrollments".into()),
            cumulative: Some(true),
            ..anc_first_visits()
        };
        let indicator = measure.indicator().expect("indicator");
        assert_eq!(indicator.counts, Counts::Enrollments);
        assert!(indicator.cumulative);
    }

    #[test]
    fn the_time_between_two_dates_is_in_whole_units() {
        let date = |text: &str| -> jiff::civil::Date { text.parse().expect("date") };
        let (lmp, visit) = (date("2025-12-24"), date("2026-03-04"));
        assert_eq!(TimeUnit::Days.between(lmp, visit), Ok(70));
        assert_eq!(TimeUnit::Weeks.between(lmp, visit), Ok(10));
        assert_eq!(TimeUnit::Months.between(lmp, visit), Ok(2));
        assert_eq!(
            TimeUnit::Years.between(date("2008-06-15"), date("2026-03-11")),
            Ok(17)
        );
        assert_eq!(
            TimeUnit::Years.between(date("2008-03-11"), date("2026-03-11")),
            Ok(18)
        );
        assert_eq!(TimeUnit::Days.between(visit, lmp), Ok(-70));
    }
}
