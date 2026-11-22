//! Reports (analytics.md § Reports; ADR 0070, ADR 0072; #556): configuration in a Reporting
//! package that lays out indicators for one period type and the levels it runs for.
//!
//! - **A Report** ([`ReportDefinition`]) has a title, exactly one [`PeriodType`], the
//!   [`Level`]s it runs for, and sections, each a table or a line list ([`Section`]). It is
//!   written as JSON (`configuration/reports/<id>.json`, schema `report/1`) and read as written.
//! - **A table section** ([`TableSection`]) has indicator rows pinned to `url|version`, and its
//!   own Disaggregations, each an Age Band Set pinned to `url|version` or a coded value, laid out
//!   as rows or columns ([`Axis`]); several on one side are crossed. It may show column totals
//!   (a Total row, over its row bands), row totals (a Total column), % change from the previous
//!   period, and is or isn't sent to DHIS2.
//! - **A line list section** ([`LineListSection`]) has one row per enrollment in a program
//!   (started in, or active during, the period) or per visit (of chosen types, in the period),
//!   optionally only those an indicator counted, and attribute or concept columns.
//! - **Validation** ([`ReportDefinition::problems`]) checks a Report against the indicators and
//!   Age Band Sets it can pin ([`Catalogue`]). Above all, a Report that runs above facility
//!   level may not hold a median or a distinct count (they cannot be rebuilt from facility
//!   summaries), nor a line list (patient-level records never leave the facility).
//!
//! What a Report sends to DHIS2 cell by cell is #560; running one is #557.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::age_bands::is_pinned;
use crate::indicator::{Aggregate, Calculation};
use crate::period::PeriodType;

/// Where Reports live in a package: `configuration/reports/<id>.json`.
pub const REPORTS_FOLDER: &str = "configuration/reports/";

/// A level of the health system a Report runs for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    /// One facility, on its edge.
    Facility,
    /// A district, at central.
    District,
    /// A region, at central.
    Region,
    /// The country, at central.
    National,
}

impl Level {
    /// Its code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Facility => "facility",
            Self::District => "district",
            Self::Region => "region",
            Self::National => "national",
        }
    }
}

/// Which side of a table a Disaggregation's bands are laid out on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Axis {
    /// Each band is a row under the indicator.
    Rows,
    /// Each band is a column.
    Columns,
}

/// One of a table section's Disaggregations: an Age Band Set or a coded value, on rows or
/// columns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionDisaggregation {
    /// The Age Band Set, `url|version`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_band_set: Option<String>,
    /// A coded value's name, what the section shows it by (e.g. `hiv-status`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The FHIRPath expression giving a record's code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
    /// Where its bands go.
    pub axis: Axis,
}

impl SectionDisaggregation {
    /// What it is, for a problem and to find it twice: the set or the code.
    fn name(&self) -> Option<&str> {
        self.age_band_set.as_deref().or(self.code.as_deref())
    }
}

/// One indicator row of a table section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndicatorRow {
    /// Its id in the section.
    pub id: String,
    /// The indicator, `url|version`.
    pub indicator: String,
    /// What a person reads.
    pub label: String,
}

/// A table of indicator rows sharing the section's Disaggregations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSection {
    /// Its id in the Report.
    pub id: String,
    /// Its heading.
    pub title: String,
    /// What every row is broken down by, in order.
    #[serde(default)]
    pub disaggregations: Vec<SectionDisaggregation>,
    /// The indicators, in order.
    #[serde(default)]
    pub rows: Vec<IndicatorRow>,
    /// A Total row under each indicator, over its row bands.
    #[serde(default)]
    pub column_totals: bool,
    /// A Total column, over the column bands.
    #[serde(default)]
    pub row_totals: bool,
    /// A % change from the previous period column.
    #[serde(default)]
    pub change: bool,
    /// Whether its cells are sent to DHIS2.
    #[serde(default, rename = "sendToDhis2")]
    pub send_to_dhis2: bool,
}

/// What a line list has one row per.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RowPer {
    /// An enrollment in the program.
    Enrollment,
    /// A visit.
    Visit,
}

/// Which enrollments an enrollment line list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Enrollments {
    /// Started in the period.
    Started,
    /// Active at any time during the period.
    Active,
}

/// Which of an enrollment's values a concept column shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ListPick {
    /// The latest.
    Latest,
    /// The first.
    First,
    /// All of them, with their dates.
    All,
    /// How many times it was recorded.
    Count,
}

/// A code: a concept, or a visit type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Code {
    /// Its code system.
    pub system: String,
    /// The code.
    pub code: String,
}

/// What an attribute is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttributeOf {
    /// The person.
    Person,
    /// The enrollment.
    Enrollment,
    /// The visit.
    Visit,
}

/// The attributes a line list column may show, as `of.name`, and what each is of.
pub const ATTRIBUTES: [(&str, AttributeOf); 16] = [
    ("person.name", AttributeOf::Person),
    ("person.sex", AttributeOf::Person),
    ("person.birthDate", AttributeOf::Person),
    ("person.age", AttributeOf::Person),
    ("person.phone", AttributeOf::Person),
    ("person.address", AttributeOf::Person),
    ("person.identifier", AttributeOf::Person),
    ("enrollment.start", AttributeOf::Enrollment),
    ("enrollment.end", AttributeOf::Enrollment),
    ("enrollment.status", AttributeOf::Enrollment),
    ("enrollment.visits", AttributeOf::Enrollment),
    ("enrollment.lastVisit", AttributeOf::Enrollment),
    ("visit.date", AttributeOf::Visit),
    ("visit.type", AttributeOf::Visit),
    ("visit.facility", AttributeOf::Visit),
    ("visit.recorded", AttributeOf::Visit),
];

/// What `attribute` is of, if it is one a line list knows.
pub fn attribute(attribute: &str) -> Option<AttributeOf> {
    ATTRIBUTES
        .iter()
        .find(|(name, _)| *name == attribute)
        .map(|(_, of)| *of)
}

/// One column of a line list: an attribute, or a concept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListColumn {
    /// Its id in the section.
    pub id: String,
    /// Its heading.
    pub label: String,
    /// An attribute of the person, enrollment or visit ([`ATTRIBUTES`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute: Option<String>,
    /// A concept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concept: Option<Code>,
    /// On an enrollment row, which of a concept's values it shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pick: Option<ListPick>,
}

/// One row per enrollment or per visit, with attribute or concept columns. Facility only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineListSection {
    /// Its id in the Report.
    pub id: String,
    /// Its heading.
    pub title: String,
    /// What it has one row per.
    pub row_per: RowPer,
    /// The program, by canonical URL.
    pub program: String,
    /// On an enrollment list, which enrollments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrollments: Option<Enrollments>,
    /// On a visit list, the visit types; none is every visit in the program.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visit_types: Vec<Code>,
    /// Only those an indicator counted, `url|version`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counted_by: Option<String>,
    /// Its columns, in order.
    #[serde(default)]
    pub columns: Vec<ListColumn>,
}

/// A section of a Report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Section {
    /// Indicator rows.
    Table(TableSection),
    /// Patient-level rows.
    LineList(LineListSection),
}

impl Section {
    /// Its id.
    pub fn id(&self) -> &str {
        match self {
            Self::Table(s) => &s.id,
            Self::LineList(s) => &s.id,
        }
    }

    /// Its heading.
    pub fn title(&self) -> &str {
        match self {
            Self::Table(s) => &s.title,
            Self::LineList(s) => &s.title,
        }
    }
}

/// A Report, as written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDefinition {
    /// Its id in the package: its file is `configuration/reports/<id>.json`.
    pub id: String,
    /// What a person reads.
    pub title: String,
    /// Its one period type.
    pub period_type: PeriodType,
    /// The levels it runs for.
    #[serde(default)]
    pub levels: Vec<Level>,
    /// Its sections, in order.
    #[serde(default)]
    pub sections: Vec<Section>,
}

/// The indicators and Age Band Sets a Report can pin: those published, and the package's own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalogue {
    /// Each indicator's calculation, by `url|version`.
    pub indicators: BTreeMap<String, Calculation>,
    /// Every Age Band Set, by `url|version`.
    pub age_band_sets: BTreeSet<String>,
}

/// Why a calculation cannot run above facility level, if it cannot.
fn facility_only(calculation: Calculation) -> Option<&'static str> {
    match calculation {
        Calculation::Value(Aggregate::Median) => Some("a median"),
        Calculation::Value(Aggregate::DistinctCount) => Some("a distinct count"),
        _ => None,
    }
}

fn blank(text: &str) -> bool {
    text.trim().is_empty()
}

impl ReportDefinition {
    /// The levels above facility it runs for, as a person reads them (`district and national`).
    fn above_facility(&self) -> Option<String> {
        let above: Vec<&str> = self
            .levels
            .iter()
            .filter(|l| **l != Level::Facility)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|l| l.code())
            .collect();
        match above.as_slice() {
            [] => None,
            [one] => Some(format!("{one} level")),
            [rest @ .., last] => Some(format!("{} and {last} levels", rest.join(", "))),
        }
    }

    /// Every reason this is not a usable Report, for Validation, given what it can pin.
    pub fn problems(&self, catalogue: &Catalogue) -> Vec<String> {
        let mut problems = Vec::new();
        if blank(&self.id) {
            problems.push("the Report has no id".to_owned());
        }
        if blank(&self.title) {
            problems.push("the Report has no title".to_owned());
        }
        if self.levels.is_empty() {
            problems.push(
                "the Report runs for no level: choose facility, district, region or national"
                    .to_owned(),
            );
        }
        if self.sections.is_empty() {
            problems.push("the Report has no sections".to_owned());
        }
        let above = self.above_facility();
        let mut ids = BTreeSet::new();
        for (n, section) in self.sections.iter().enumerate() {
            let name = if blank(section.title()) {
                format!("section {}", n + 1)
            } else {
                format!("section {} ({})", n + 1, section.title())
            };
            if blank(section.id()) {
                problems.push(format!("{name} has no id"));
            } else if !ids.insert(section.id()) {
                problems.push(format!(
                    "{name} has the id {}, which another section has",
                    section.id()
                ));
            }
            if blank(section.title()) {
                problems.push(format!("{name} has no title"));
            }
            match section {
                Section::Table(table) => {
                    problems.extend(table_problems(table, &name, above.as_deref(), catalogue));
                }
                Section::LineList(list) => {
                    if let Some(above) = &above {
                        problems.push(format!(
                            "{name} is a line list, which shows patient-level records, and they never leave the facility; the Report runs at {above}: run it at facility level only, or move the line list to a facility Report"
                        ));
                    }
                    problems.extend(list_problems(list, &name, catalogue));
                }
            }
        }
        problems
    }
}

/// A pinned indicator's problem, if `reference` is not pinned or not found.
fn indicator_problem(reference: &str, catalogue: &Catalogue) -> Option<String> {
    if !is_pinned(reference) {
        Some(format!(
            "the indicator {reference} is not pinned to a version (url|version)"
        ))
    } else if !catalogue.indicators.contains_key(reference) {
        Some(format!("no published indicator is {reference}"))
    } else {
        None
    }
}

fn table_problems(
    table: &TableSection,
    name: &str,
    above: Option<&str>,
    catalogue: &Catalogue,
) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for (n, d) in table.disaggregations.iter().enumerate() {
        let which = format!("{name}'s disaggregation {}", n + 1);
        match (&d.age_band_set, &d.code) {
            (Some(_), Some(_)) => problems.push(format!(
                "{which} is both by an Age Band Set and by a coded value"
            )),
            (None, None) => problems.push(format!(
                "{which} names neither an Age Band Set nor a coded value"
            )),
            (Some(set), None) if !is_pinned(set) => problems.push(format!(
                "{which}: the Age Band Set {set} is not pinned to a version (url|version)"
            )),
            (Some(set), None) if !catalogue.age_band_sets.contains(set) => {
                problems.push(format!("{which}: no published Age Band Set is {set}"));
            }
            (None, Some(code)) if blank(code) => {
                problems.push(format!("{which} has no code: the name it is shown by"));
            }
            (None, Some(code)) if d.expression.as_deref().is_none_or(blank) => problems.push(
                format!("{which} ({code}) has no expression giving a record's code"),
            ),
            _ => {}
        }
        if let Some(what) = d.name()
            && !seen.insert(what)
        {
            problems.push(format!("{name} is broken down by {what} twice"));
        }
    }
    let on = |axis| table.disaggregations.iter().any(|d| d.axis == axis);
    if table.column_totals && !on(Axis::Rows) {
        problems.push(format!(
            "{name} shows column totals, which need a disaggregation on rows"
        ));
    }
    if table.row_totals && !on(Axis::Columns) {
        problems.push(format!(
            "{name} shows row totals, which need a disaggregation on columns"
        ));
    }
    if table.rows.is_empty() {
        problems.push(format!("{name} has no indicator rows"));
    }
    let mut ids = BTreeSet::new();
    for (n, row) in table.rows.iter().enumerate() {
        let label = if blank(&row.label) {
            format!("{name}'s row {}", n + 1)
        } else {
            format!("{name}'s row {} ({})", n + 1, row.label)
        };
        if blank(&row.id) {
            problems.push(format!("{label} has no id"));
        } else if !ids.insert(row.id.as_str()) {
            problems.push(format!(
                "{label} has the id {}, which another row has",
                row.id
            ));
        }
        if blank(&row.label) {
            problems.push(format!("{label} has no label"));
        }
        if let Some(problem) = indicator_problem(&row.indicator, catalogue) {
            problems.push(format!("{label}: {problem}"));
            continue;
        }
        if let (Some(above), Some(what)) = (
            above,
            catalogue
                .indicators
                .get(&row.indicator)
                .copied()
                .and_then(facility_only),
        ) {
            problems.push(format!(
                "{label} is {what}, which cannot be rebuilt from facility summaries because patient-level data never leaves the facility; the Report runs at {above}: run it at facility level only, or move the row to a facility Report"
            ));
        }
    }
    problems
}

fn list_problems(list: &LineListSection, name: &str, catalogue: &Catalogue) -> Vec<String> {
    let mut problems = Vec::new();
    if blank(&list.program) {
        problems.push(format!("{name} names no program"));
    }
    match list.row_per {
        RowPer::Enrollment => {
            if list.enrollments.is_none() {
                problems.push(format!(
                    "{name} does not say which enrollments it lists: started in the period, or active during it"
                ));
            }
            if !list.visit_types.is_empty() {
                problems.push(format!(
                    "{name} lists enrollments, so it has no visit types"
                ));
            }
        }
        RowPer::Visit => {
            if list.enrollments.is_some() {
                problems.push(format!(
                    "{name} lists visits, so it does not choose enrollments"
                ));
            }
            for code in &list.visit_types {
                if blank(&code.system) || blank(&code.code) {
                    problems.push(format!(
                        "{name} has a visit type without its system and code"
                    ));
                }
            }
        }
    }
    if let Some(indicator) = &list.counted_by
        && let Some(problem) = indicator_problem(indicator, catalogue)
    {
        problems.push(format!(
            "{name} lists only those counted by {indicator}: {problem}"
        ));
    }
    if list.columns.is_empty() {
        problems.push(format!("{name} has no columns"));
    }
    let mut ids = BTreeSet::new();
    for (n, column) in list.columns.iter().enumerate() {
        let label = if blank(&column.label) {
            format!("{name}'s column {}", n + 1)
        } else {
            format!("{name}'s column {} ({})", n + 1, column.label)
        };
        if blank(&column.id) {
            problems.push(format!("{label} has no id"));
        } else if !ids.insert(column.id.as_str()) {
            problems.push(format!(
                "{label} has the id {}, which another column has",
                column.id
            ));
        }
        if blank(&column.label) {
            problems.push(format!("{label} has no label"));
        }
        match (&column.attribute, &column.concept) {
            (Some(_), Some(_)) => {
                problems.push(format!("{label} is both an attribute and a concept"));
            }
            (None, None) => problems.push(format!("{label} is neither an attribute nor a concept")),
            (Some(name), None) => {
                match attribute(name) {
                    None => problems.push(format!(
                        "{label}: {name} is not an attribute of the person, enrollment or visit"
                    )),
                    Some(AttributeOf::Visit) if list.row_per == RowPer::Enrollment => problems
                        .push(format!(
                            "{label}: {name} is a visit's, and a row is an enrollment, which has many visits"
                        )),
                    Some(_) => {}
                }
                if column.pick.is_some() {
                    problems.push(format!(
                        "{label} is an attribute, which has one value: it picks none"
                    ));
                }
            }
            (None, Some(concept)) => {
                if blank(&concept.system) || blank(&concept.code) {
                    problems.push(format!("{label}'s concept has no system and code"));
                }
                match (list.row_per, column.pick) {
                    (RowPer::Enrollment, None) => problems.push(format!(
                        "{label} does not say which of an enrollment's values it shows: the latest, the first, all, or how many times it was recorded"
                    )),
                    (RowPer::Visit, Some(_)) => problems.push(format!(
                        "{label} is on a visit list, where a concept has the visit's own value: it picks none"
                    )),
                    _ => {}
                }
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests;
