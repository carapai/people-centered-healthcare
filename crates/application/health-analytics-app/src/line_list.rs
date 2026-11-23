//! A line list section's rows (analytics.md § Reports; ADR 0072; #867): one row per enrollment
//! in a program or per visit, over a patient's records however they were read (a package's test
//! patients in the Report designer's preview, a facility's records on its edge).
//!
//! - **The program's enrollments** are the EpisodeOfCares of its type ([`ProgramType`]): one
//!   naming the program in its `episode-program` extension, or whose `type` has the program's
//!   episode type (`program-episode-type`: its system and code, or its text when it has no code).
//!   One entered in error is none.
//! - **Enrollment rows:** those *started* in the period (`period.start` within it) or *active*
//!   during it (started by its end and not ended before its start). An enrollment's visits are
//!   the Encounters whose `episodeOfCare` (or `workflow-episodeOfCare`) is it, dated by their
//!   `actualPeriod.start` up to the period's end.
//! - **Visit rows:** the program's enrollments' Encounters dated in the period, of the visit
//!   types chosen (any, when none is), neither cancelled nor entered in error.
//! - **Only those counted by** an indicator: the references its records counted under
//!   ([`counted`]): an enrollment row is kept when it or one of its visits was counted, a visit
//!   row when it or its enrollment was.
//! - **Attribute columns** ([`health_analytics::ATTRIBUTES`]) read the Patient, the EpisodeOfCare
//!   or the Encounter; a person's age is in whole years at the visit, or at the period's end on
//!   an enrollment row. **Concept columns** read the Observations of that code recorded at the
//!   enrollment's visits (the visit's own, on a visit row), oldest first: the latest, the first,
//!   all of them with their dates, or how many; a visit row shows its latest.
//!
//! Rows come in the order the patients are given, then by start or visit date.

use std::collections::BTreeSet;

use health_analytics::{
    Code, Enrollments, Evaluation, LineListSection, ListColumn, ListPick, Period, Reason, RowPer,
    local_date,
};
use jiff::civil::Date;
use serde_json::Value;

use crate::{PatientRecords, enrollment, extension, text};

/// On an EpisodeOfCare: the program it is an enrollment in (`valueCanonical`).
pub const EPISODE_PROGRAM: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/episode-program";

/// What makes an EpisodeOfCare an enrollment in a program: its canonical URL and its episode
/// type (`program-episode-type`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramType {
    /// The program's canonical URL, without a version.
    pub url: String,
    /// The episode type's code system, if coded.
    pub system: Option<String>,
    /// The episode type's code, if coded.
    pub code: Option<String>,
    /// What a person reads.
    pub text: String,
}

impl ProgramType {
    /// Whether `episode` is an enrollment in the program.
    pub fn enrolls(&self, episode: &Value) -> bool {
        if episode["resourceType"] != "EpisodeOfCare" || episode["status"] == "entered-in-error" {
            return false;
        }
        let named = extension(episode, EPISODE_PROGRAM)
            .and_then(|e| e["valueCanonical"].as_str())
            .is_some_and(|c| c.split('|').next() == Some(self.url.as_str()));
        named
            || episode["type"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|concept| match (&self.system, &self.code) {
                    (Some(system), Some(code)) => concept["coding"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|c| c["system"] == system.as_str() && c["code"] == code.as_str()),
                    _ => concept["text"] == self.text.as_str(),
                })
    }
}

/// One value of a cell: what a person reads, and the date it was recorded, if it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellValue {
    /// The value.
    pub value: String,
    /// When it was recorded.
    pub date: Option<Date>,
}

impl CellValue {
    fn plain(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            date: None,
        }
    }
}

/// One row of a line list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRow {
    /// The patient (`Patient/…`).
    pub patient: String,
    /// What the row is: the enrollment (`EpisodeOfCare/…`) or the visit (`Encounter/…`).
    pub record: String,
    /// Each column's values, in the section's order: none when it has no value, several for a
    /// concept's *all*.
    pub cells: Vec<Vec<CellValue>>,
}

/// The references an indicator's evaluation counted: each record that meets its criteria.
pub fn counted(evaluation: &Evaluation) -> BTreeSet<String> {
    evaluation
        .members
        .iter()
        .flat_map(|m| &m.records)
        .filter(|r| r.reason == Reason::Meets)
        .map(|r| r.reference.clone())
        .collect()
}

fn reference(resource: &Value) -> String {
    format!(
        "{}/{}",
        resource["resourceType"].as_str().unwrap_or_default(),
        resource["id"].as_str().unwrap_or_default()
    )
}

fn date(written: &Value) -> Option<Date> {
    written.as_str().and_then(local_date)
}

/// When a visit took place: its `actualPeriod.start` (R5), else its `period.start`.
fn visit_date(encounter: &Value) -> Option<Date> {
    date(&encounter["actualPeriod"]["start"]).or_else(|| date(&encounter["period"]["start"]))
}

/// What a person reads for a concept: its first coding's display, its text, or its code.
fn concept_text(concept: &Value) -> Option<String> {
    let coding = &concept["coding"][0];
    text(&coding["display"])
        .or_else(|| text(&concept["text"]))
        .or_else(|| text(&coding["code"]))
}

fn has_code(concept: &Value, code: &Code) -> bool {
    concept["coding"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|c| c["system"] == code.system.as_str() && c["code"] == code.code.as_str())
}

/// An Observation's value as a person reads it.
fn observed(observation: &Value) -> Option<String> {
    let number = |v: &Value| v.as_number().map(ToString::to_string);
    if let Some(quantity) = observation.get("valueQuantity") {
        let value = number(&quantity["value"])?;
        return Some(match quantity["unit"].as_str() {
            Some(unit) if !unit.is_empty() => format!("{value} {unit}"),
            _ => value,
        });
    }
    if let Some(concept) = observation.get("valueCodeableConcept") {
        return concept_text(concept);
    }
    if let Some(b) = observation["valueBoolean"].as_bool() {
        return Some(if b { "yes" } else { "no" }.to_owned());
    }
    number(&observation["valueInteger"])
        .or_else(|| text(&observation["valueString"]))
        .or_else(|| text(&observation["valueDateTime"]))
        .or_else(|| text(&observation["valueDate"]))
}

/// When an Observation was made.
fn observed_on(observation: &Value) -> Option<Date> {
    date(&observation["effectiveDateTime"])
        .or_else(|| date(&observation["effectivePeriod"]["start"]))
        .or_else(|| date(&observation["effectiveInstant"]))
        .or_else(|| date(&observation["issued"]))
}

/// Whole years from `birth` to `at`.
fn years(birth: Date, at: Date) -> Option<i16> {
    if at < birth {
        return None;
    }
    let mut age = at.year() - birth.year();
    if (at.month(), at.day()) < (birth.month(), birth.day()) {
        age -= 1;
    }
    Some(age)
}

/// The patient's first phone number.
fn phone(patient: &Value) -> Option<String> {
    patient["telecom"]
        .as_array()?
        .iter()
        .find(|t| t["system"] == "phone")
        .and_then(|t| text(&t["value"]))
}

/// The patient's first address as a person reads it.
fn address(patient: &Value) -> Option<String> {
    let address = &patient["address"][0];
    text(&address["text"]).or_else(|| {
        let parts: Vec<&str> = address["line"]
            .as_array()
            .into_iter()
            .flatten()
            .chain([&address["city"], &address["district"], &address["state"]])
            .filter_map(Value::as_str)
            .filter(|p| !p.is_empty())
            .collect();
        (!parts.is_empty()).then(|| parts.join(", "))
    })
}

fn facility(encounter: &Value) -> Option<String> {
    encounter["meta"]["tag"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["system"] == crate::report::FACILITY_TAG)
        .and_then(|t| text(&t["code"]))
        .or_else(|| text(&encounter["serviceProvider"]["display"]))
        .or_else(|| text(&encounter["serviceProvider"]["reference"]))
}

/// A row being read: the patient, the enrollment, the visit (on a visit row), and the visits
/// whose observations the row reads.
struct Row<'a> {
    patient: &'a PatientRecords,
    episode: &'a Value,
    visit: Option<&'a Value>,
    /// The enrollment's visits up to the period's end, oldest first.
    visits: Vec<&'a Value>,
    at: Date,
}

impl Row<'_> {
    fn attribute(&self, name: &str) -> Option<String> {
        let patient = &self.patient.patient;
        let episode = self.episode;
        let visit = self.visit;
        match name {
            "person.name" => self.patient.name(),
            "person.sex" => text(&patient["gender"]),
            "person.birthDate" => text(&patient["birthDate"]),
            "person.age" => {
                years(date(&patient["birthDate"])?, self.at).map(|years| years.to_string())
            }
            "person.phone" => phone(patient),
            "person.address" => address(patient),
            "person.identifier" => text(&patient["identifier"][0]["value"]),
            "enrollment.start" => date(&episode["period"]["start"]).map(|d| d.to_string()),
            "enrollment.end" => date(&episode["period"]["end"]).map(|d| d.to_string()),
            "enrollment.status" => text(&episode["status"]),
            "enrollment.visits" => Some(self.visits.len().to_string()),
            "enrollment.lastVisit" => self
                .visits
                .iter()
                .filter_map(|v| visit_date(v))
                .max()
                .map(|d| d.to_string()),
            "visit.date" => visit.and_then(visit_date).map(|d| d.to_string()),
            "visit.type" => visit.and_then(|v| concept_text(&v["type"][0])),
            "visit.facility" => visit.and_then(facility),
            "visit.recorded" => visit
                .and_then(|v| date(&v["meta"]["lastUpdated"]))
                .map(|d| d.to_string()),
            _ => None,
        }
    }

    /// The Observations of `code` at the row's visits, oldest first.
    fn observations(&self, code: &Code) -> Vec<CellValue> {
        let visits: Vec<String> = match self.visit {
            Some(visit) => vec![reference(visit)],
            None => self.visits.iter().map(|v| reference(v)).collect(),
        };
        let mut values: Vec<CellValue> = self
            .patient
            .records
            .iter()
            .filter(|r| r["resourceType"] == "Observation")
            .filter(|r| !matches!(r["status"].as_str(), Some("entered-in-error" | "cancelled")))
            .filter(|r| has_code(&r["code"], code))
            .filter(|r| {
                r["encounter"]["reference"]
                    .as_str()
                    .is_some_and(|e| visits.iter().any(|v| v == e))
            })
            .filter_map(|r| {
                Some(CellValue {
                    value: observed(r)?,
                    date: observed_on(r),
                })
            })
            .collect();
        values.sort_by_key(|v| v.date);
        values
    }

    fn cell(&self, column: &ListColumn, per: RowPer) -> Vec<CellValue> {
        if let Some(name) = &column.attribute {
            return self
                .attribute(name)
                .map(CellValue::plain)
                .into_iter()
                .collect();
        }
        let Some(code) = &column.concept else {
            return Vec::new();
        };
        let mut values = self.observations(code);
        let pick = match per {
            RowPer::Visit => ListPick::Latest,
            RowPer::Enrollment => column.pick.unwrap_or(ListPick::Latest),
        };
        match pick {
            ListPick::Latest => values.pop().into_iter().collect(),
            ListPick::First => values.into_iter().take(1).collect(),
            ListPick::All => values,
            ListPick::Count => vec![CellValue::plain(values.len().to_string())],
        }
    }
}

/// The rows of `list` over `patients` for `period`, of the program `program`; only those an
/// indicator counted, when `counted` is given ([`counted`]).
pub fn rows(
    list: &LineListSection,
    program: &ProgramType,
    patients: &[PatientRecords],
    period: &Period,
    counted: Option<&BTreeSet<String>>,
) -> Vec<ListRow> {
    let mut out = Vec::new();
    for patient in patients {
        let encounters: Vec<&Value> = patient
            .records
            .iter()
            .filter(|r| r["resourceType"] == "Encounter")
            .filter(|r| !matches!(r["status"].as_str(), Some("entered-in-error" | "cancelled")))
            .collect();
        let mut found: Vec<(Option<Date>, ListRow)> = Vec::new();
        for episode in patient.records.iter().filter(|r| program.enrolls(r)) {
            let enrolled = reference(episode);
            let mut visits: Vec<&Value> = encounters
                .iter()
                .copied()
                .filter(|e| enrollment(e).as_deref() == Some(enrolled.as_str()))
                .filter(|e| visit_date(e).is_some_and(|d| d <= period.end))
                .collect();
            visits.sort_by_key(|v| visit_date(v));
            let is_counted = |reference: &str| counted.is_none_or(|c| c.contains(reference));
            match list.row_per {
                RowPer::Enrollment => {
                    let start = date(&episode["period"]["start"]);
                    let end = date(&episode["period"]["end"]);
                    let shown = match list.enrollments.unwrap_or(Enrollments::Active) {
                        Enrollments::Started => {
                            start.is_some_and(|s| s >= period.start && s <= period.end)
                        }
                        Enrollments::Active => {
                            start.is_some_and(|s| s <= period.end)
                                && end.is_none_or(|e| e >= period.start)
                        }
                    };
                    let kept =
                        is_counted(&enrolled) || visits.iter().any(|v| is_counted(&reference(v)));
                    if !shown || !kept {
                        continue;
                    }
                    let row = Row {
                        patient,
                        episode,
                        visit: None,
                        visits,
                        at: period.end,
                    };
                    found.push((start, read(&row, list, &enrolled)));
                }
                RowPer::Visit => {
                    for visit in &visits {
                        let Some(on) = visit_date(visit).filter(|d| *d >= period.start) else {
                            continue;
                        };
                        let typed = list.visit_types.is_empty()
                            || visit["type"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .any(|t| list.visit_types.iter().any(|c| has_code(t, c)));
                        let visited = reference(visit);
                        if !typed || !(is_counted(&visited) || is_counted(&enrolled)) {
                            continue;
                        }
                        let row = Row {
                            patient,
                            episode,
                            visit: Some(visit),
                            visits: vec![visit],
                            at: on,
                        };
                        found.push((Some(on), read(&row, list, &visited)));
                    }
                }
            }
        }
        found.sort_by_key(|(on, _)| *on);
        out.extend(found.into_iter().map(|(_, row)| row));
    }
    out
}

fn read(row: &Row<'_>, list: &LineListSection, record: &str) -> ListRow {
    ListRow {
        patient: row.patient.reference(),
        record: record.to_owned(),
        cells: list
            .columns
            .iter()
            .map(|column| row.cell(column, list.row_per))
            .collect(),
    }
}

#[cfg(test)]
mod tests;
