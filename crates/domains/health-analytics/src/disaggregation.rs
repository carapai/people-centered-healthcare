//! Disaggregations (analytics.md; studio.md § Indicators, *Disaggregations*; ADR 0068, ADR 0071;
//! #552): an indicator lists what it is broken down by, each a Measure stratifier.
//!
//! - **By age** ([`DisaggregationSource::AgeBands`]): an Age Band Set, pinned to `url|version`,
//!   fixes the bands, the unit and the date age is taken at.
//! - **By a coded value** ([`DisaggregationSource::Coded`]): any FHIRPath expression giving a
//!   code of the record or its patient (sex, HIV status, visit type, …); its bands are the codes
//!   found.
//! - **Not recorded** ([`NOT_RECORDED`]): every disaggregation has this band, for a counted thing
//!   with no value (no birth date, no HIV status) or an age in none of the set's bands, so the
//!   bands always add up to the total.
//!
//! A counted thing (a person, an enrollment, a visit, an observation) falls in exactly one band
//! of each disaggregation: the band of its first record in the denominator (or counted, with
//! no denominator), by period date. A percentage, rate or ratio per band therefore divides the
//! band's numerator by the band's own denominator, and the numerator stays within it.

use crate::age_bands::is_pinned;
use crate::indicator::Expression;

/// The band of a counted thing with no value for a disaggregation.
pub const NOT_RECORDED: &str = "not-recorded";

/// Where a disaggregation's bands come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisaggregationSource {
    /// An Age Band Set, pinned to `url|version`.
    AgeBands(String),
    /// A FHIRPath expression giving a record's code.
    Coded(String),
}

/// What an indicator is broken down by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disaggregation {
    /// Its name: the stratifier's code, or for an Age Band Set without one, the set's
    /// `url|version`.
    pub name: String,
    /// Where its bands come from.
    pub source: DisaggregationSource,
}

/// A disaggregation (a Measure stratifier), as written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisaggregationDefinition {
    /// Its code (`Measure.group.stratifier.code`), by code or text.
    pub code: Option<String>,
    /// The Age Band Set it disaggregates by, as written: `url|version` (#551).
    pub age_band_set: Option<String>,
    /// The expression giving each record's code.
    pub criteria: Option<Expression>,
}

impl DisaggregationDefinition {
    /// The disaggregation, or every reason this stratifier is not one.
    pub(crate) fn disaggregation(&self, row: usize) -> Result<Disaggregation, Vec<String>> {
        let code = self
            .code
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty());
        match (&self.age_band_set, &self.criteria) {
            (Some(_), Some(_)) => Err(vec![format!(
                "disaggregation {row} is both by an Age Band Set and by a coded value"
            )]),
            (None, None) => Err(vec![format!(
                "disaggregation {row} names neither an Age Band Set nor the expression giving its code"
            )]),
            (Some(set), None) if !is_pinned(set) => Err(vec![format!(
                "the indicator disaggregates by the Age Band Set {set}, which is not pinned to a version (url|version)"
            )]),
            (Some(set), None) => Ok(Disaggregation {
                name: code.unwrap_or(set).to_owned(),
                source: DisaggregationSource::AgeBands(set.clone()),
            }),
            (None, Some(criteria)) => {
                let mut problems = Vec::new();
                let expression = criteria
                    .fhirpath_text()
                    .map_err(|why| {
                        problems.push(format!("disaggregation {row}'s expression {why}"))
                    })
                    .ok();
                if code.is_none() {
                    problems.push(format!(
                        "disaggregation {row} has no code: the name a Report shows it by"
                    ));
                }
                match (code, expression) {
                    (Some(code), Some(expression)) if problems.is_empty() => Ok(Disaggregation {
                        name: code.to_owned(),
                        source: DisaggregationSource::Coded(expression.to_owned()),
                    }),
                    _ => Err(problems),
                }
            }
        }
    }
}
