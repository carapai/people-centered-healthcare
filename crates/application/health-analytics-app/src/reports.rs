//! Running a Report (analytics.md § Reports; ADR 0070, ADR 0072; #557): a Report's table
//! sections laid out over stored Indicator Reports, never patient-level records, so it is
//! instant.
//!
//! - **Where.** At central for any place in the session's area at a level the Report runs for,
//!   over the summaries the facilities' edges synced; on a Facility Edge Node for its own
//!   facility, offline, over the summaries it computed itself (`explore::OwnSummaries`). Each
//!   row's figure is the data explorer's ([`explore`]): rolled up over the place's facilities
//!   from summed parts, never by averaging percentages, so a Total (the whole, over every band
//!   of a side) is computed from summed parts like any other cell.
//! - **Bands.** Each row carries the whole and its band of each of the section's
//!   Disaggregations, under the section's name for it (the Age Band Set's `url|version`, or the
//!   code), with what a person reads for each band. A cell crossing several Disaggregations is
//!   read from the summaries' crossings (each combination of bands' own parts, rolled up from
//!   summed parts like a band), for the crossings the section lays out: its row Disaggregations,
//!   its column ones, and all of them. When a facility's summary predates crossings, its crossed
//!   cells are left out rather than summed in part.
//! - **Estimates** (#553). An indicator whose denominator is a Population Estimate type takes
//!   its place's estimate for the period, as the explorer does; a place with no estimate has no
//!   figure.
//! - **Completeness.** The facilities that reported out of those expected, and the missing ones
//!   with their last sync, never counted as zero; a median or a distinct count over more than
//!   one facility has no figure, and says why.
//! - **Line lists** are patient-level, so they run on the facility's edge only (ADR 0072).

use std::collections::{BTreeMap, BTreeSet};

use health_analytics::{
    AgeBandSet, Axis, DisaggregationSource, FacilitySummaries, Indicator, Level, NOT_RECORDED,
    Period, PopulationEstimates, ReportDefinition, Section, SectionDisaggregation, Summary,
    TableSection, pinned,
};
use jiff::Timestamp;
use rust_decimal::RoundingStrategy;
use serde_json::{Value, json};

use crate::explore::{Cell, ExplorePlace, Figure, calculation_code, explore, periods_json};

/// The place a Report runs for: its code, what a person reads, its level, and the facilities
/// its figures roll up over (a facility's is itself).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPlace {
    /// The place, with its facilities.
    pub place: ExplorePlace,
    /// What a person reads.
    pub name: String,
    /// Its level.
    pub level: Level,
}

/// What a Report runs with: the indicators it may pin, the Age Band Sets that name their bands,
/// the Population Estimates over the places, and each facility's name.
pub struct RunWith<'a> {
    /// The indicators offered, by `url|version`.
    pub indicators: &'a [Indicator],
    /// The Age Band Sets, to label their bands.
    pub sets: &'a [AgeBandSet],
    /// The Population Estimates (#553).
    pub estimates: &'a PopulationEstimates,
    /// Each facility's name, by code, to name the missing ones.
    pub names: &'a BTreeMap<String, String>,
}

/// The level a place of the facility registry is at, as a Report names it.
pub fn level_of(code: &str) -> Option<Level> {
    [
        Level::Facility,
        Level::District,
        Level::Region,
        Level::National,
    ]
    .into_iter()
    .find(|level| level.code() == code)
}

/// The section's name for one of its Disaggregations: the Age Band Set's `url|version`, or the
/// code.
fn section_name(d: &SectionDisaggregation) -> Option<&str> {
    d.age_band_set.as_deref().or(d.code.as_deref())
}

/// Which of `indicator`'s disaggregations the section's `d` is: by name, or an Age Band Set by
/// the set it pins.
fn matching(indicator: &Indicator, d: &SectionDisaggregation) -> Option<usize> {
    let name = section_name(d)?;
    indicator.disaggregations.iter().position(|own| {
        own.name == name
            || matches!(
                (&own.source, d.age_band_set.as_deref()),
                (DisaggregationSource::AgeBands(set), Some(wanted)) if set == wanted
            )
    })
}

fn exact(value: rust_decimal::Decimal) -> String {
    value
        .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
        .normalize()
        .to_string()
}

/// What a person reads for band `code` of the section's `d`.
fn band_label(d: &SectionDisaggregation, code: &str, sets: &[AgeBandSet]) -> String {
    if code == NOT_RECORDED {
        return "Not recorded".to_owned();
    }
    d.age_band_set
        .as_deref()
        .and_then(|reference| pinned(reference, sets).ok())
        .and_then(|set| {
            set.bands
                .iter()
                .find(|b| b.code() == code)
                .map(|b| b.label(set.unit))
        })
        .unwrap_or_else(|| code.to_owned())
}

/// `summary`'s figure as a Report reads it: the whole, and its bands of each of the section's
/// Disaggregations the indicator is broken down by, under the section's names.
fn figures_json(
    indicator: &Indicator,
    table: &TableSection,
    summary: &Summary,
    sets: &[AgeBandSet],
) -> Value {
    let disaggregations: Vec<Value> = table
        .disaggregations
        .iter()
        .filter_map(|d| {
            let at = matching(indicator, d)?;
            let by = summary.disaggregations.get(at)?;
            Some(json!({
                "name": section_name(d),
                "bands": by.bands.iter().map(|band| json!({
                    "code": band.code,
                    "label": band_label(d, &band.code, sets),
                    "value": band.summary.value.map(exact),
                    "count": band.summary.count,
                    "denominator": band.summary.denominator,
                })).collect::<Vec<_>>(),
            }))
        })
        .collect();
    json!({
        "value": summary.value.map(exact),
        "count": summary.count,
        "denominator": summary.denominator,
        "disaggregations": disaggregations,
    })
}

/// Which of a table section's Disaggregations its cells cross: those on rows, those on columns,
/// and all of them, each when it is two or more (as the designer lays cells out).
fn crossings(table: &TableSection) -> Vec<Vec<&SectionDisaggregation>> {
    let side = |axis: Option<Axis>| -> Vec<&SectionDisaggregation> {
        table
            .disaggregations
            .iter()
            .filter(|d| axis.is_none_or(|a| d.axis == a) && section_name(d).is_some())
            .collect()
    };
    let mut out: Vec<Vec<&SectionDisaggregation>> = Vec::new();
    for ds in [
        side(Some(Axis::Rows)),
        side(Some(Axis::Columns)),
        side(None),
    ] {
        if ds.len() > 1 && !out.contains(&ds) {
            out.push(ds);
        }
    }
    out
}

/// `summary`'s figure at every cell crossing each of the section's `crossings`, over the bands
/// the section lays out (`bands`, as [`section_bands`] gives them), under the section's names. A
/// crossing with a Disaggregation the indicator is not broken down by, or that the summary cannot
/// tell, is left out (the page says why).
fn crossed_json(
    indicator: &Indicator,
    summary: &Summary,
    crossings: &[Vec<&SectionDisaggregation>],
    bands: &[Value],
) -> Vec<Value> {
    let mut cells = Vec::new();
    'crossing: for ds in crossings {
        let Some(sides) = ds
            .iter()
            .map(|d| {
                let at = matching(indicator, d)?;
                let name = section_name(d)?;
                let codes: Vec<&str> = bands.iter().find(|b| b["name"] == name)?["bands"]
                    .as_array()?
                    .iter()
                    .filter_map(|b| b["code"].as_str())
                    .collect();
                Some((name, at, codes))
            })
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        // The indicator keeps its crossings in its own order of disaggregations.
        let mut order: Vec<usize> = (0..sides.len()).collect();
        order.sort_by_key(|i| sides[*i].1);
        let names: Vec<&str> = order
            .iter()
            .map(|i| indicator.disaggregations[sides[*i].1].name.as_str())
            .collect();
        let combos = sides
            .iter()
            .fold(vec![Vec::new()], |combos, (_, _, codes)| {
                combos
                    .iter()
                    .flat_map(|combo: &Vec<&str>| {
                        codes.iter().map(move |code| {
                            let mut next = combo.clone();
                            next.push(*code);
                            next
                        })
                    })
                    .collect()
            });
        let mut found = Vec::with_capacity(combos.len());
        for combo in combos {
            let codes: Vec<&str> = order.iter().map(|i| combo[*i]).collect();
            let Some(cell) = summary.crossed_at(&names, &codes) else {
                continue 'crossing;
            };
            found.push(json!({
                "at": sides.iter().zip(&combo).map(|((name, _, _), code)| json!({
                    "name": name,
                    "code": code,
                })).collect::<Vec<_>>(),
                "value": cell.value.map(exact),
                "count": cell.count,
                "denominator": cell.denominator,
            }));
        }
        cells.extend(found);
    }
    cells
}

/// A row's indicator, with its summary for the period run and, for % change, the one before.
type RowSummaries<'a> = (&'a Indicator, Option<Summary>, Option<Summary>);

/// The facilities expected, those missing (with their last sync) and the newest summary taken,
/// over every figure of the period run.
#[derive(Default)]
struct Completeness {
    expected: BTreeSet<String>,
    missing: BTreeMap<String, Option<Timestamp>>,
    computed_at: Option<Timestamp>,
}

impl Completeness {
    fn add(&mut self, cell: &Cell) {
        let Figure::Rolled(rolled) = &cell.figure else {
            return;
        };
        self.expected
            .extend(rolled.reported.iter().map(|r| r.facility.clone()));
        for missing in &rolled.missing {
            self.expected.insert(missing.facility.clone());
            self.missing
                .insert(missing.facility.clone(), missing.last_sync);
        }
        self.computed_at = self.computed_at.max(rolled.computed_at);
    }

    fn json(&self, names: &BTreeMap<String, String>) -> Value {
        json!({
            "reported": self.expected.len() - self.missing.len(),
            "expected": self.expected.len(),
            "computedAt": self.computed_at.map(|t| t.to_string()),
            "missing": self.missing.iter().map(|(facility, last_sync)| json!({
                "facility": facility,
                "name": names.get(facility).cloned().unwrap_or_else(|| facility.clone()),
                "lastSync": last_sync.map(|t| t.to_string()),
            })).collect::<Vec<_>>(),
        })
    }
}

/// One period's summary of a row, if it has one.
fn summary_of(cell: Option<&Cell>) -> Option<&Summary> {
    match cell.map(|c| &c.figure) {
        Some(Figure::Rolled(rolled)) => rolled.summary.as_ref(),
        _ => None,
    }
}

/// One period's figure of a row, as a Report reads it, or none when there is none.
fn period_json(
    summary: Option<&Summary>,
    indicator: &Indicator,
    table: &TableSection,
    sets: &[AgeBandSet],
) -> Value {
    summary.map_or(Value::Null, |s| figures_json(indicator, table, s, sets))
}

/// The bands of the section's `d`, in order: an Age Band Set's from the set, a coded value's from
/// what the rows found; then *not recorded*.
fn section_bands(d: &SectionDisaggregation, sets: &[AgeBandSet], rows: &[Value]) -> Value {
    let name = section_name(d).unwrap_or_default();
    let mut bands: Vec<Value> = Vec::new();
    let mut codes: Vec<String> = Vec::new();
    let mut title = name.to_owned();
    if let Some(set) = d
        .age_band_set
        .as_deref()
        .and_then(|reference| pinned(reference, sets).ok())
    {
        title.clone_from(&set.title);
        codes.extend(set.bands.iter().map(|b| b.code()));
    }
    for row in rows {
        for figures in [&row["current"], &row["previous"]] {
            for by in figures["disaggregations"].as_array().into_iter().flatten() {
                if by["name"] != name {
                    continue;
                }
                for band in by["bands"].as_array().into_iter().flatten() {
                    if let Some(code) = band["code"].as_str()
                        && code != NOT_RECORDED
                        && !codes.iter().any(|c| c == code)
                    {
                        codes.push(code.to_owned());
                    }
                }
            }
        }
    }
    codes.push(NOT_RECORDED.to_owned());
    for code in codes {
        bands.push(json!({ "label": band_label(d, &code, sets), "code": code }));
    }
    json!({ "name": name, "title": title, "bands": bands })
}

/// `report` run for `place` over `period`, from the summaries `store` keeps; `here` says where
/// (`central`, or `facility` on an edge). Its rows' figures for the period, and the period
/// before for a section with % change; each section's bands; and the completeness of the
/// period's figures.
pub async fn run_report<S: FacilitySummaries>(
    store: &mut S,
    report: &ReportDefinition,
    place: &RunPlace,
    period: &Period,
    with: &RunWith<'_>,
    here: &str,
) -> Result<Value, String> {
    let previous = period.previous();
    let mut completeness = Completeness::default();
    let mut sections = Vec::with_capacity(report.sections.len());
    for section in &report.sections {
        let table = match section {
            Section::Table(table) => table,
            Section::LineList(_) => {
                // Patient-level: filled in on the facility's edge ([`crate::line_list`]).
                sections.push(json!({ "kind": "lineList", "list": Value::Null }));
                continue;
            }
        };
        let mut rows = Vec::with_capacity(table.rows.len());
        // Each row's indicator and summaries for the period and the one before, to cross.
        let mut summaries: Vec<Option<RowSummaries<'_>>> = Vec::with_capacity(table.rows.len());
        for row in &table.rows {
            let Some(indicator) = with
                .indicators
                .iter()
                .find(|i| i.canonical() == row.indicator)
            else {
                rows.push(json!({
                    "row": row.id,
                    "indicator": Value::Null,
                    "current": Value::Null,
                    "previous": Value::Null,
                    "error": format!("no indicator is {}", row.indicator),
                }));
                summaries.push(None);
                continue;
            };
            let mut periods = vec![period.clone()];
            if let (true, Some(before)) = (table.change, &previous) {
                periods.push(before.clone());
            }
            let cells = explore(
                store,
                std::slice::from_ref(indicator),
                &periods,
                std::slice::from_ref(&place.place),
                with.estimates,
            )
            .await?;
            let now = cells.iter().find(|c| c.period == period.id);
            let before = previous
                .as_ref()
                .and_then(|p| cells.iter().find(|c| c.period == p.id));
            if let Some(now) = now {
                completeness.add(now);
            }
            let facility_only = match now.map(|c| &c.figure) {
                Some(Figure::FacilityOnly(aggregate)) => Some(aggregate.code()),
                _ => None,
            };
            let estimate = now.and_then(|c| c.estimate.as_ref()).map(|e| {
                json!({
                    "type": e.kind,
                    "title": e.title,
                    "value": e.value.map(exact),
                })
            });
            rows.push(json!({
                "row": row.id,
                "indicator": {
                    "title": indicator.title,
                    "calculation": calculation_code(indicator.calculation),
                },
                "current": period_json(summary_of(now), indicator, table, with.sets),
                "previous": if table.change {
                    period_json(summary_of(before), indicator, table, with.sets)
                } else {
                    Value::Null
                },
                "facilityOnly": facility_only,
                "estimate": estimate,
                "error": Value::Null,
            }));
            summaries.push(Some((
                indicator,
                summary_of(now).cloned(),
                summary_of(before).filter(|_| table.change).cloned(),
            )));
        }
        let disaggregations: Vec<Value> = table
            .disaggregations
            .iter()
            .filter(|d| section_name(d).is_some())
            .map(|d| section_bands(d, with.sets, &rows))
            .collect();
        let crossings = crossings(table);
        if !crossings.is_empty() {
            for (row, kept) in rows.iter_mut().zip(&summaries) {
                let Some((indicator, now, before)) = kept else {
                    continue;
                };
                for (side, summary) in [("current", now), ("previous", before)] {
                    if let Some(summary) = summary
                        && row[side].is_object()
                    {
                        row[side]["crossed"] = json!(crossed_json(
                            indicator,
                            summary,
                            &crossings,
                            &disaggregations
                        ));
                    }
                }
            }
        }
        sections.push(json!({
            "kind": "table",
            "disaggregations": disaggregations,
            "rows": rows,
        }));
    }
    let periods = periods_json(std::slice::from_ref(period));
    Ok(json!({
        "where": here,
        "place": {
            "code": place.place.code,
            "name": place.name,
            "level": place.level.code(),
        },
        "period": periods[0],
        "previous": previous.map(|p| periods_json(&[p])[0].clone()),
        "completeness": completeness.json(with.names),
        "sections": sections,
    }))
}

/// A Report as listed to run: its id (`package:report`), title, package version, period type,
/// levels and definition (`report/1`), and the places it may be run for here.
pub fn listed_json(
    package: &str,
    version: &str,
    report: &ReportDefinition,
    places: &[Value],
) -> Value {
    json!({
        "id": format!("{package}:{}", report.id),
        "title": report.title,
        "package": format!("{package}@{version}"),
        "periodType": report.period_type,
        "levels": report.levels,
        "definition": report,
        "places": places,
    })
}

#[cfg(test)]
mod tests;
