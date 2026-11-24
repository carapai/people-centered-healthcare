//! Population Estimates (ADR 0071; analytics.md § Age Band Sets and Population Estimates; #553):
//! the denominators of coverage indicators, shipped as a yearly Population package.
//!
//! - **A year's estimates** ([`EstimatesDefinition`]) are written as JSON
//!   (`configuration/population.json`, schema `population/1`): the year, the estimate types and
//!   the figures. A type is either **given** per place or **derived** as a percentage of another
//!   type, as MoH does (expected pregnancies = 5% of total population). Percentages are written
//!   as exact decimal text.
//! - **Figures** ([`Figure`]) are given at the lowest level available (a facility's catchment,
//!   a sub-county, a district). [`Estimates`] sums them upward through the place hierarchy and
//!   never splits a figure down: a place below the figures given has *no estimate*.
//! - **Periods.** Estimates are annual. A Report's period takes them pro rata
//!   ([`pro_rata`]): a month 1/12, a quarter 1/4, a half year 1/2, a week 7/365.
//! - **The import** ([`parse_estimates_csv`]): the HMIS lead's CSV template has a `place`
//!   column and one column per given type. A place code the registry does not hold, a derived
//!   or unknown type's column, a repeated place or a value that is not a whole number refuses
//!   the file, each problem naming its row.
//! - **As a denominator** ([`over_estimate`]): a percentage or rate whose denominator names an
//!   estimate type is its count over the place's estimate for the period.
//!
//! Which places are below which is the facility registry's (ADR 0094); the caller gives it.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::indicator::{Calculation, Indicator};
use crate::period::{Period, PeriodType};

/// Where a Population package keeps its estimates.
pub const ESTIMATES_FILE: &str = "configuration/population.json";

/// The CSV template's column naming the place.
pub const PLACE_COLUMN: &str = "place";

/// One estimate type a Population package declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimateTypeDefinition {
    /// Its code, what a denominator names (`expected-pregnancies`).
    pub code: String,
    /// What a person reads.
    #[serde(default)]
    pub title: String,
    /// The type it is derived from, if it is not given per place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<String>,
    /// The percentage of that type it is, as exact decimal text (`"5"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<String>,
}

impl EstimateTypeDefinition {
    /// Whether it is derived from another type rather than given per place.
    pub fn is_derived(&self) -> bool {
        self.derived_from.is_some()
    }
}

/// One place's annual figure for one given type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Figure {
    /// The place's code in the facility registry.
    pub place: String,
    /// The estimate type's code.
    #[serde(rename = "type")]
    pub kind: String,
    /// The annual figure.
    pub value: u64,
}

/// A year's Population Estimates, as written.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimatesDefinition {
    /// The year they are for.
    pub year: i32,
    /// The estimate types, given or derived.
    #[serde(default)]
    pub types: Vec<EstimateTypeDefinition>,
    /// The figures of the given types, by place.
    #[serde(default)]
    pub figures: Vec<Figure>,
}

fn percent(text: &str) -> Option<Decimal> {
    Decimal::from_str(text.trim())
        .ok()
        .filter(|p| *p > Decimal::ZERO && *p <= Decimal::ONE_HUNDRED)
}

impl EstimatesDefinition {
    /// The type with `code`.
    pub fn estimate_type(&self, code: &str) -> Option<&EstimateTypeDefinition> {
        self.types.iter().find(|t| t.code == code)
    }

    /// Every reason these estimates cannot be published.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !(1900..=9999).contains(&self.year) {
            problems.push(format!("the year {} is not a year", self.year));
        }
        if self.types.is_empty() {
            problems.push("the estimates declare no estimate type".into());
        }
        let mut codes = BTreeSet::new();
        for kind in &self.types {
            if kind.code.trim().is_empty() {
                problems.push("an estimate type has no code".into());
            } else if !codes.insert(kind.code.as_str()) {
                problems.push(format!("the estimate type {} is declared twice", kind.code));
            }
        }
        for kind in &self.types {
            match (&kind.derived_from, &kind.percent) {
                (None, None) => {}
                (None, Some(_)) => problems.push(format!(
                    "the estimate type {} has a percentage but is derived from no type",
                    kind.code
                )),
                (Some(from), percentage) => {
                    if !codes.contains(from.as_str()) {
                        problems.push(format!(
                            "the estimate type {} is derived from {from}, which is not declared",
                            kind.code
                        ));
                    }
                    if percentage.as_deref().and_then(percent).is_none() {
                        problems.push(format!(
                            "the estimate type {} needs a percentage of {from} above 0 and at most 100",
                            kind.code
                        ));
                    }
                }
            }
            if self.derives_in_a_circle(&kind.code) {
                problems.push(format!(
                    "the estimate type {} is derived from itself, through the types it is derived from",
                    kind.code
                ));
            }
        }
        let mut seen = BTreeSet::new();
        for figure in &self.figures {
            match self.estimate_type(&figure.kind) {
                None => problems.push(format!(
                    "{} has a figure for {}, which is not a declared estimate type",
                    figure.place, figure.kind
                )),
                Some(kind) if kind.is_derived() => problems.push(format!(
                    "{} has a figure for {}, which is derived, not given",
                    figure.place, figure.kind
                )),
                Some(_) => {}
            }
            if figure.place.trim().is_empty() {
                problems.push(format!("a figure for {} names no place", figure.kind));
            } else if !seen.insert((figure.place.as_str(), figure.kind.as_str())) {
                problems.push(format!(
                    "{} has two figures for {}",
                    figure.place, figure.kind
                ));
            }
        }
        problems
    }

    fn derives_in_a_circle(&self, code: &str) -> bool {
        let mut at = self
            .estimate_type(code)
            .and_then(|t| t.derived_from.as_deref());
        let mut steps = 0;
        while let Some(next) = at {
            if next == code {
                return true;
            }
            steps += 1;
            if steps > self.types.len() {
                return true;
            }
            at = self
                .estimate_type(next)
                .and_then(|t| t.derived_from.as_deref());
        }
        false
    }
}

/// Reads the HMIS lead's CSV template: a `place` column and one column per given estimate type
/// of `definition`, one row per place, an empty cell where a place has no figure for that type.
/// Each place code must be one `known` holds (the facility registry, with the NHFR's
/// facilities). Every problem names its row (the header is row 1).
pub fn parse_estimates_csv(
    text: &str,
    definition: &EstimatesDefinition,
    known: impl Fn(&str) -> bool,
) -> Result<Vec<Figure>, Vec<String>> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(n, line)| (n + 1, line.trim_end_matches('\r')))
        .filter(|(_, line)| !line.trim().is_empty());
    let Some((_, header)) = lines.next() else {
        return Err(vec!["the file is empty".into()]);
    };
    let columns: Vec<String> = cells(header);
    let mut problems = Vec::new();
    if columns.first().map(String::as_str) != Some(PLACE_COLUMN) {
        problems.push(format!(
            "row 1: the first column must be {PLACE_COLUMN}, the place's code"
        ));
    }
    let mut types = Vec::new();
    for column in columns.iter().skip(1) {
        match definition.estimate_type(column) {
            None => problems.push(format!(
                "row 1: the column {column} is not one of the year's estimate types"
            )),
            Some(kind) if kind.is_derived() => problems.push(format!(
                "row 1: the column {column} is derived from {}, not given",
                kind.derived_from.as_deref().unwrap_or_default()
            )),
            Some(_) if types.contains(column) => {
                problems.push(format!("row 1: the column {column} is there twice"));
            }
            Some(_) => {}
        }
        types.push(column.clone());
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let mut figures = Vec::new();
    let mut places = BTreeMap::new();
    for (row, line) in lines {
        let values = cells(line);
        let place = values.first().cloned().unwrap_or_default();
        if place.is_empty() {
            problems.push(format!("row {row}: the row names no place"));
            continue;
        }
        if !known(&place) {
            problems.push(format!(
                "row {row}: the place {place} is not in the facility registry or the NHFR"
            ));
        }
        if let Some(first) = places.insert(place.clone(), row) {
            problems.push(format!(
                "row {row}: the place {place} is already on row {first}"
            ));
        }
        if values.len() > columns.len() {
            problems.push(format!(
                "row {row}: the row has {} cells, more than the {} columns",
                values.len(),
                columns.len()
            ));
        }
        for (kind, cell) in types.iter().zip(values.iter().skip(1)) {
            if cell.is_empty() {
                continue;
            }
            let digits: String = cell.chars().filter(|c| *c != ',' && *c != '_').collect();
            match digits.parse::<u64>() {
                Ok(value) => figures.push(Figure {
                    place: place.clone(),
                    kind: kind.clone(),
                    value,
                }),
                Err(_) => problems.push(format!(
                    "row {row}: {kind} for {place} is {cell}, not a whole number"
                )),
            }
        }
    }
    if problems.is_empty() {
        Ok(figures)
    } else {
        Err(problems)
    }
}

/// A CSV line's cells, trimmed; a cell in double quotes may hold commas.
fn cells(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => found.push(std::mem::take(&mut cell).trim().to_owned()),
            c => cell.push(c),
        }
    }
    found.push(cell.trim().to_owned());
    found
}

/// The share of a year a period of `kind` takes of an annual estimate: a month 1/12, a quarter
/// 1/4, a half year 1/2, a week 7/365, a year (calendar or financial) all of it.
pub fn pro_rata(annual: Decimal, kind: PeriodType) -> Decimal {
    match kind {
        PeriodType::Weekly => annual * Decimal::from(7) / Decimal::from(365),
        PeriodType::Monthly => annual / Decimal::from(12),
        PeriodType::Quarterly => annual / Decimal::from(4),
        PeriodType::SemiAnnual => annual / Decimal::TWO,
        PeriodType::Annual | PeriodType::FinancialJuly => annual,
    }
}

/// A year's estimates over the place hierarchy: each given figure where it was given, summed
/// upward, and the derived types from them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estimates {
    definition: EstimatesDefinition,
    /// The given types' annual figures, by type and place, summed upward.
    given: BTreeMap<String, BTreeMap<String, Decimal>>,
}

impl Estimates {
    /// The estimates of `definition`, summed upward from each place to the places above it,
    /// `parent` naming the place directly above a code (none at the top). A place given its own
    /// figure keeps it, whatever is given below it.
    pub fn new(definition: EstimatesDefinition, parent: impl Fn(&str) -> Option<String>) -> Self {
        let mut own: BTreeMap<String, BTreeMap<String, Decimal>> = BTreeMap::new();
        for figure in &definition.figures {
            own.entry(figure.kind.clone())
                .or_default()
                .insert(figure.place.clone(), Decimal::from(figure.value));
        }
        let mut given: BTreeMap<String, BTreeMap<String, Decimal>> = BTreeMap::new();
        for (kind, places) in &own {
            let summed = given.entry(kind.clone()).or_default();
            for (place, value) in places {
                *summed.entry(place.clone()).or_default() += *value;
                // Up the hierarchy, stopping at a place given its own figure: that figure
                // already counts what is below it.
                let mut at = parent(place);
                let mut steps = 0;
                while let Some(above) = at {
                    steps += 1;
                    if places.contains_key(&above) || steps > definition.figures.len() + 64 {
                        break;
                    }
                    *summed.entry(above.clone()).or_default() += *value;
                    at = parent(&above);
                }
            }
        }
        Self { definition, given }
    }

    /// The estimates, as written.
    pub fn definition(&self) -> &EstimatesDefinition {
        &self.definition
    }

    /// The annual estimate of type `kind` for `place`; none when nothing was given at or below
    /// it (*no estimate*), or the type is not declared.
    pub fn annual(&self, place: &str, kind: &str) -> Option<Decimal> {
        self.annual_within(place, kind, 0)
    }

    fn annual_within(&self, place: &str, kind: &str, depth: usize) -> Option<Decimal> {
        let declared = self.definition.estimate_type(kind)?;
        match (&declared.derived_from, &declared.percent) {
            (Some(from), Some(percentage)) if depth <= self.definition.types.len() => {
                let base = self.annual_within(place, from, depth + 1)?;
                Some(base * percent(percentage)? / Decimal::ONE_HUNDRED)
            }
            (Some(_), _) => None,
            (None, _) => self.given.get(kind)?.get(place).copied(),
        }
    }

    /// The estimate of type `kind` for `place` over `period`, pro rata to its length; none when
    /// there is no estimate.
    pub fn for_period(&self, place: &str, kind: &str, period: &Period) -> Option<Decimal> {
        self.annual(place, kind).map(|a| pro_rata(a, period.kind))
    }

    /// The figure at `place` for `period` of `indicator`, whose denominator is an estimate type,
    /// from the `count` its facilities' summaries add up to; none when the indicator's
    /// denominator is not an estimate or the place has *no estimate*.
    pub fn figure(
        &self,
        indicator: &Indicator,
        count: u64,
        place: &str,
        period: &Period,
    ) -> Option<Decimal> {
        let kind = indicator.estimate.as_deref()?;
        over_estimate(
            indicator.calculation,
            count,
            self.for_period(place, kind, period),
        )
    }
}

/// The figure of an indicator calculating `calculation` whose denominator is an estimate:
/// `count` over `estimate` (× 100 for a percentage, × the factor for a rate), to two decimal
/// places. None when there is no estimate, it is zero, or the calculation has no denominator.
pub fn over_estimate(
    calculation: Calculation,
    count: u64,
    estimate: Option<Decimal>,
) -> Option<Decimal> {
    let factor = match calculation {
        Calculation::Percentage => 100,
        Calculation::Rate { per } => per,
        Calculation::Ratio => 1,
        Calculation::Count | Calculation::Value(_) => return None,
    };
    let estimate = estimate.filter(|e| *e > Decimal::ZERO)?;
    Some(
        (Decimal::from(count) * Decimal::from(factor) / estimate)
            .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
            .normalize(),
    )
}

#[cfg(test)]
mod tests;
