//! An indicator's definition (studio.md § Indicators; ADR 0068): a FHIR R5 `Measure` whose
//! populations are FHIRPath criteria over a patient's records, with the period date that places
//! care in a Report's period.
//!
//! [`MeasureDefinition`] is what a Measure says, as written, so that Validation can name each
//! problem ([`MeasureDefinition::problems`]); [`Indicator`] is a definition without problems,
//! which is what evaluation takes.

use std::collections::BTreeMap;

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
    fn fhirpath_text(&self) -> Result<&str, String> {
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
    /// Its criteria.
    pub criteria: Option<Expression>,
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
    /// The period date: the declared date that places care in a Report's period.
    pub period_date: Option<Expression>,
    /// Its populations, in its first group.
    pub populations: Vec<PopulationDefinition>,
    /// How many groups it has.
    pub groups: usize,
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
    /// The type of record its criteria are evaluated on.
    pub basis: String,
    /// What it counts.
    pub counts: Counts,
    /// The FHIRPath giving a record's period date.
    pub period_date: String,
    /// Each population's FHIRPath criteria.
    pub populations: BTreeMap<PopulationKind, String>,
}

impl Indicator {
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
            let criteria = population.criteria.clone().unwrap_or_default();
            match criteria.fhirpath_text() {
                Ok(text) => {
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
        match scoring {
            Some(scoring) if problems.is_empty() => Ok(Indicator {
                url,
                version,
                title,
                scoring,
                basis,
                counts,
                period_date,
                populations,
            }),
            _ => Err(problems),
        }
    }
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
            period_date: Some(Expression::fhirpath("Encounter.period.start")),
            populations: vec![PopulationDefinition {
                code: Some("initial-population".into()),
                criteria: Some(Expression::fhirpath("%patient.gender = 'female'")),
            }],
            groups: 1,
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
                },
                PopulationDefinition {
                    code: Some("numerator".into()),
                    criteria: Some(Expression::fhirpath("true")),
                },
                PopulationDefinition {
                    code: Some("denominator-exclusion".into()),
                    criteria: None,
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
        });
        measure.populations.push(PopulationDefinition {
            code: Some("numerator".into()),
            criteria: Some(Expression::fhirpath("false")),
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
        ] {
            assert_eq!(PopulationKind::from_code(kind.code()), Some(kind));
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
}
