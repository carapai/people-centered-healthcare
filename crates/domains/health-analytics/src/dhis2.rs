//! DHIS2 mappings on Report cells (ADR 0069, ADR 0070; analytics.md § Reporting to DHIS2; #560).
//!
//! - **A cell** of a table section is an indicator row at one band of each of the section's
//!   Disaggregations (a row band × a column band). A section sent to DHIS2 maps each of its
//!   cells ([`CellMapping`]) to a DHIS2 data set, data element and category option combo, in
//!   effect from a date and optionally up to one, because the forms are revised and their IDs
//!   change. A cell may have several mappings, one after the other.
//! - **Which cells must be mapped:** every cell of a count row (a count or a sum: DHIS2's
//!   aggregate data elements on these forms hold counts) at a band of each Disaggregation. A
//!   coded Disaggregation's bands are only known from the data, so its bands are the codes the
//!   section's mappings name. Totals and the *not recorded* band are sent only if mapped.
//! - **The instance's metadata** ([`Dhis2Metadata`]) is central's recorded copy of the target
//!   instance's data sets, their data elements and each one's category option combos, read
//!   from the instance's `dataSets` export ([`DataSetsExport`]). Validation checks every
//!   mapping's IDs against it.

use std::collections::{BTreeMap, BTreeSet};

use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use crate::disaggregation::NOT_RECORDED;
use crate::indicator::{Aggregate, Calculation};
use crate::period::PeriodType;
use crate::report::{Axis, Catalogue, TableSection};

/// One mapping of one cell to DHIS2, in effect over a range of dates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellMapping {
    /// The indicator row's id in the section.
    pub row: String,
    /// The cell's band of each Disaggregation, by the Disaggregation's name (an Age Band Set's
    /// `url|version`, or a code); a side left out is that side's Total.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bands: BTreeMap<String, String>,
    /// The DHIS2 data set's ID.
    pub data_set: String,
    /// The DHIS2 data element's ID.
    pub data_element: String,
    /// The DHIS2 category option combo's ID.
    pub category_option_combo: String,
    /// The first day it is in effect, `YYYY-MM-DD`: for periods starting on or after it.
    pub from: String,
    /// The last day it is in effect, `YYYY-MM-DD`; none is until further notice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Whether a zero is sent as a zero; otherwise a value corrected to zero is sent as a
    /// deletion (ADR 0069).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub zero_is_significant: bool,
}

/// A DHIS2 data element of a data set, with the category option combos it takes there.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataElement {
    /// What a person reads.
    pub name: String,
    /// Its category option combos' names, by ID.
    pub category_option_combos: BTreeMap<String, String>,
}

/// A DHIS2 data set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataSet {
    /// What a person reads.
    pub name: String,
    /// Its DHIS2 period type (`Monthly`, `Quarterly`, …).
    pub period_type: String,
    /// Its data elements, by ID.
    pub data_elements: BTreeMap<String, DataElement>,
}

/// The target instance's metadata a mapping is checked against: its data sets, by ID.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dhis2Metadata {
    /// Each data set, by ID.
    pub data_sets: BTreeMap<String, DataSet>,
}

/// A name and ID, as DHIS2 exports them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Named {
    /// Its ID.
    pub id: String,
    /// Its name.
    #[serde(default)]
    pub name: String,
}

/// A category combo, with its option combos.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryComboExport {
    /// Its option combos.
    #[serde(default)]
    pub category_option_combos: Vec<Named>,
}

/// A data element, with its category combo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataElementExport {
    /// Its ID.
    pub id: String,
    /// Its name.
    #[serde(default)]
    pub name: String,
    /// Its category combo.
    #[serde(default)]
    pub category_combo: Option<CategoryComboExport>,
}

/// A data element of a data set, optionally with the data set's own category combo for it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSetElementExport {
    /// The data element.
    pub data_element: DataElementExport,
    /// The data set's override of its category combo.
    #[serde(default)]
    pub category_combo: Option<CategoryComboExport>,
}

/// A data set, as exported.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSetExport {
    /// Its ID.
    pub id: String,
    /// Its name.
    #[serde(default)]
    pub name: String,
    /// Its period type.
    #[serde(default)]
    pub period_type: String,
    /// Its data elements.
    #[serde(default)]
    pub data_set_elements: Vec<DataSetElementExport>,
}

/// The instance's `dataSets` export, as `GET /api/dataSets?paging=false&fields=id,name,
/// periodType,dataSetElements[categoryCombo[categoryOptionCombos[id,name]],dataElement[id,name,
/// categoryCombo[categoryOptionCombos[id,name]]]]` answers it (or `/api/metadata` with
/// `dataSets`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSetsExport {
    /// The data sets.
    #[serde(default)]
    pub data_sets: Vec<DataSetExport>,
}

impl DataSetsExport {
    /// What Validation checks mappings against.
    pub fn metadata(&self) -> Dhis2Metadata {
        let combos = |combo: &CategoryComboExport| {
            combo
                .category_option_combos
                .iter()
                .map(|c| (c.id.clone(), c.name.clone()))
                .collect()
        };
        Dhis2Metadata {
            data_sets: self
                .data_sets
                .iter()
                .map(|set| {
                    let elements = set
                        .data_set_elements
                        .iter()
                        .map(|e| {
                            let combo = e
                                .category_combo
                                .as_ref()
                                .or(e.data_element.category_combo.as_ref());
                            (
                                e.data_element.id.clone(),
                                DataElement {
                                    name: e.data_element.name.clone(),
                                    category_option_combos: combo.map(combos).unwrap_or_default(),
                                },
                            )
                        })
                        .collect();
                    (
                        set.id.clone(),
                        DataSet {
                            name: set.name.clone(),
                            period_type: set.period_type.clone(),
                            data_elements: elements,
                        },
                    )
                })
                .collect(),
        }
    }
}

/// DHIS2's name for a period type.
pub const fn dhis2_period_type(kind: PeriodType) -> &'static str {
    match kind {
        PeriodType::Weekly => "Weekly",
        PeriodType::Monthly => "Monthly",
        PeriodType::Quarterly => "Quarterly",
        PeriodType::SemiAnnual => "SixMonthly",
        PeriodType::Annual => "Yearly",
        PeriodType::FinancialJuly => "FinancialJuly",
    }
}

/// Whether a calculation's figure is a count DHIS2's data elements on these forms take.
pub const fn is_sent(calculation: Calculation) -> bool {
    matches!(
        calculation,
        Calculation::Count | Calculation::Value(Aggregate::Sum)
    )
}

fn blank(text: &str) -> bool {
    text.trim().is_empty()
}

/// A cell as a person reads it: `row 1 (ANC 1st visit for women) at 15-19`, or `… Total`.
fn cell_name(row: &str, bands: &BTreeMap<String, String>, order: &[String]) -> String {
    let at: Vec<&str> = order
        .iter()
        .filter_map(|name| bands.get(name).map(String::as_str))
        .collect();
    if at.is_empty() {
        row.to_owned()
    } else {
        format!("{row} at {}", at.join(" × "))
    }
}

/// The days a mapping is in effect, if both read: from, and to (none is open).
fn days(mapping: &CellMapping) -> Option<(Date, Option<Date>)> {
    let from = mapping.from.parse().ok()?;
    let to = match &mapping.to {
        Some(to) => Some(to.parse().ok()?),
        None => None,
    };
    Some((from, to))
}

/// Whether two ranges of days share a day.
fn overlap(a: (Date, Option<Date>), b: (Date, Option<Date>)) -> bool {
    a.1.is_none_or(|end| b.0 <= end) && b.1.is_none_or(|end| a.0 <= end)
}

/// Every problem with a table section's DHIS2 mappings, `name` being how a problem names the
/// section and `period_type` the Report's.
pub(crate) fn section_problems(
    table: &TableSection,
    name: &str,
    period_type: PeriodType,
    catalogue: &Catalogue,
) -> Vec<String> {
    let mut problems = Vec::new();
    if !table.send_to_dhis2 {
        if !table.mappings.is_empty() {
            problems.push(format!(
                "{name} maps cells to DHIS2 but is not sent to DHIS2: send it, or remove its mappings"
            ));
        }
        return problems;
    }
    let order: Vec<String> = table
        .disaggregations
        .iter()
        .filter_map(|d| d.age_band_set.clone().or_else(|| d.code.clone()))
        .collect();
    let side = |axis: Axis| -> BTreeSet<&str> {
        table
            .disaggregations
            .iter()
            .filter(|d| d.axis == axis)
            .filter_map(|d| d.age_band_set.as_deref().or(d.code.as_deref()))
            .collect()
    };
    let (rows_side, columns_side) = (side(Axis::Rows), side(Axis::Columns));
    let row_label = |id: &str| {
        table
            .rows
            .iter()
            .position(|r| r.id == id)
            .map(|n| {
                let row = &table.rows[n];
                if blank(&row.label) {
                    format!("row {}", n + 1)
                } else {
                    format!("row {} ({})", n + 1, row.label)
                }
            })
            .unwrap_or_else(|| format!("row {id}"))
    };
    let calculation = |id: &str| {
        table
            .rows
            .iter()
            .find(|r| r.id == id)
            .and_then(|r| catalogue.indicators.get(&r.indicator).copied())
    };
    let mut metadata_missing = false;
    for (n, mapping) in table.mappings.iter().enumerate() {
        let which = format!(
            "{name}'s DHIS2 mapping {} ({})",
            n + 1,
            cell_name(&row_label(&mapping.row), &mapping.bands, &order)
        );
        if !table.rows.iter().any(|r| r.id == mapping.row) {
            problems.push(format!(
                "{which} is of the row {}, which the section does not have",
                mapping.row
            ));
        } else if let Some(calculation) = calculation(&mapping.row)
            && !is_sent(calculation)
        {
            problems.push(format!(
                "{which} maps a row that is not a count, and DHIS2's data elements on these forms hold counts: send the counts it is built from instead"
            ));
        }
        for (disaggregation, code) in &mapping.bands {
            let Some(d) = table.disaggregations.iter().find(|d| {
                d.age_band_set.as_deref() == Some(disaggregation.as_str())
                    || d.code.as_deref() == Some(disaggregation.as_str())
            }) else {
                problems.push(format!(
                    "{which} names a band of {disaggregation}, which the section is not broken down by"
                ));
                continue;
            };
            if blank(code) {
                problems.push(format!("{which} names no band of {disaggregation}"));
            } else if let Some(set) = &d.age_band_set
                && code != NOT_RECORDED
                && let Some(codes) = catalogue.age_band_codes.get(set)
                && !codes.contains(code)
            {
                problems.push(format!(
                    "{which}: {code} is not a band of {set} (its bands are {})",
                    codes.join(", ")
                ));
            }
        }
        for (axis_side, totals, what) in [
            (&rows_side, table.column_totals, "row"),
            (&columns_side, table.row_totals, "column"),
        ] {
            let named = axis_side
                .iter()
                .filter(|d| mapping.bands.contains_key(**d))
                .count();
            if named != axis_side.len() && !(named == 0 && totals) {
                problems.push(format!(
                    "{which} is not a cell of the section: name a band of every {what} disaggregation{}",
                    if totals { ", or none for the Total" } else { "" }
                ));
            }
        }
        for (field, value) in [
            ("data set", &mapping.data_set),
            ("data element", &mapping.data_element),
            ("category option combo", &mapping.category_option_combo),
        ] {
            if blank(value) {
                problems.push(format!("{which} has no {field}"));
            }
        }
        match (mapping.from.parse::<Date>(), mapping.to.as_deref()) {
            (Err(_), _) => problems.push(format!(
                "{which} has no date it is in effect from (YYYY-MM-DD)"
            )),
            (Ok(_), Some(to)) if to.parse::<Date>().is_err() => problems.push(format!(
                "{which}: {to} is not a date it is in effect to (YYYY-MM-DD)"
            )),
            (Ok(from), Some(to)) if to.parse::<Date>().is_ok_and(|to| to < from) => {
                problems.push(format!(
                    "{which} is in effect to {to}, before it is in effect from {}",
                    mapping.from
                ))
            }
            _ => {}
        }
        let Some(metadata) = &catalogue.dhis2 else {
            metadata_missing = true;
            continue;
        };
        if blank(&mapping.data_set) || blank(&mapping.data_element) {
            continue;
        }
        let Some(set) = metadata.data_sets.get(&mapping.data_set) else {
            problems.push(format!(
                "{which}: the data set {} is not in the DHIS2 instance's metadata",
                mapping.data_set
            ));
            continue;
        };
        let wanted = dhis2_period_type(period_type);
        if set.period_type != wanted {
            problems.push(format!(
                "{which}: the data set {} ({}) is {}, and the Report is {wanted}",
                mapping.data_set, set.name, set.period_type
            ));
        }
        let Some(element) = set.data_elements.get(&mapping.data_element) else {
            let elsewhere = metadata
                .data_sets
                .values()
                .any(|s| s.data_elements.contains_key(&mapping.data_element));
            problems.push(if elsewhere {
                format!(
                    "{which}: the data element {} is not in the data set {} ({})",
                    mapping.data_element, mapping.data_set, set.name
                )
            } else {
                format!(
                    "{which}: the data element {} is not in the DHIS2 instance's metadata",
                    mapping.data_element
                )
            });
            continue;
        };
        if !blank(&mapping.category_option_combo)
            && !element
                .category_option_combos
                .contains_key(&mapping.category_option_combo)
        {
            problems.push(format!(
                "{which}: the category option combo {} is not one of the data element {} ({})",
                mapping.category_option_combo, mapping.data_element, element.name
            ));
        }
    }
    if metadata_missing {
        problems.push(format!(
            "{name}'s DHIS2 mappings cannot be checked: no copy of the DHIS2 instance's metadata is recorded at central"
        ));
    }
    // Two mappings of one cell in effect on the same dates.
    let mut by_cell: BTreeMap<(&str, &BTreeMap<String, String>), Vec<&CellMapping>> =
        BTreeMap::new();
    for mapping in &table.mappings {
        by_cell
            .entry((mapping.row.as_str(), &mapping.bands))
            .or_default()
            .push(mapping);
    }
    for ((row, bands), mappings) in &by_cell {
        for (n, a) in mappings.iter().enumerate() {
            for b in &mappings[n + 1..] {
                if let (Some(x), Some(y)) = (days(a), days(b))
                    && overlap(x, y)
                {
                    let range = |m: &CellMapping| {
                        format!(
                            "{} to {}",
                            m.from,
                            m.to.as_deref().unwrap_or("further notice")
                        )
                    };
                    problems.push(format!(
                        "{name}: two DHIS2 mappings of {} are in effect on the same dates ({} and {})",
                        cell_name(&row_label(row), bands, &order),
                        range(a),
                        range(b)
                    ));
                }
            }
        }
    }
    // The cells that must be mapped: each count row at a band of every Disaggregation.
    let mut bands: Vec<(String, Vec<String>)> = Vec::new();
    for d in &table.disaggregations {
        if let Some(set) = &d.age_band_set {
            let Some(codes) = catalogue.age_band_codes.get(set) else {
                // An unknown set is already a problem.
                return problems;
            };
            bands.push((set.clone(), codes.clone()));
        } else if let Some(code) = &d.code {
            let named: BTreeSet<String> = table
                .mappings
                .iter()
                .filter_map(|m| m.bands.get(code))
                .filter(|c| !blank(c) && c.as_str() != NOT_RECORDED)
                .cloned()
                .collect();
            if named.is_empty() {
                problems.push(format!(
                    "{name} is sent to DHIS2 by {code}, but no mapping names one of its codes: map the codes DHIS2 takes"
                ));
                return problems;
            }
            bands.push((code.clone(), named.into_iter().collect()));
        }
    }
    let mut cells: Vec<BTreeMap<String, String>> = vec![BTreeMap::new()];
    for (disaggregation, codes) in &bands {
        cells = cells
            .iter()
            .flat_map(|cell| {
                codes.iter().map(move |code| {
                    let mut next = cell.clone();
                    next.insert(disaggregation.clone(), code.clone());
                    next
                })
            })
            .collect();
    }
    for row in &table.rows {
        if !catalogue
            .indicators
            .get(&row.indicator)
            .copied()
            .is_some_and(is_sent)
        {
            continue;
        }
        for cell in &cells {
            let mapped: Vec<&&CellMapping> = by_cell
                .get(&(row.id.as_str(), cell))
                .map(|m| m.iter().collect())
                .unwrap_or_default();
            let what = cell_name(&row_label(&row.id), cell, &order);
            if mapped.is_empty() {
                problems.push(format!("{name}: {what} is not mapped to DHIS2"));
            } else if mapped.iter().all(|m| m.to.is_some()) {
                let last = mapped
                    .iter()
                    .filter_map(|m| m.to.as_deref())
                    .max()
                    .unwrap_or_default();
                problems.push(format!(
                    "{name}: {what} has no DHIS2 mapping after {last}: map it until further notice"
                ));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests;
