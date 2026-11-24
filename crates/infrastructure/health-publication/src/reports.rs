//! Reports at validation (analytics.md § Reports; ADR 0070, ADR 0072; #556): a Report is
//! configuration in a Reporting package, `configuration/reports/<id>.json`, written to the
//! `report/1` schema. The schema checks its shape when the package is checked; this checks the
//! rest ([`ReportDefinition::problems`]): its indicators and Age Band Sets are pinned to
//! `url|version` and published (or in the package itself), and a Report that runs above facility
//! level holds no median, no distinct count and no line list.
//!
//! The indicators and sets are looked up in every published package version, so a Report keeps
//! the version it pinned whatever is published after it.

use std::collections::BTreeMap;

use health_analytics::{
    Catalogue, CellMapping, DataSetsExport, Dhis2Metadata, REPORTS_FOLDER, ReportDefinition,
    Section,
};
use health_analytics_app::measure;
use health_configuration::lifecycle::{Lifecycle, Stage};
use health_configuration::package::{INDEX, PackageKind};
use health_packages::{Files, Packed, REPORT};
use health_persistence::Transaction as _;
use health_surrealdb::SurrealTx;
use serde_json::{Value, json};

use crate::age_bands::age_band_sets_in;
use crate::{Error, Publications, failed, lifecycle_id};

/// The package's Reports, each with its file, as written or why it cannot be read.
pub fn reports_in(packed: &Packed) -> Vec<(String, Result<ReportDefinition, String>)> {
    reports_in_files(&packed.files)
}

/// The Reports in a package's `files`, as [`reports_in`] reads them: what a Facility Edge Node
/// runs from its active packages (#557).
pub fn reports_in_files(files: &Files) -> Vec<(String, Result<ReportDefinition, String>)> {
    files
        .iter()
        .filter(|(path, _)| path.starts_with("configuration/") && path.ends_with(".json"))
        .filter_map(|(path, bytes)| {
            let value = serde_json::from_slice::<Value>(bytes).ok()?;
            (value["$schema"] == REPORT).then(|| {
                let read = serde_json::from_value::<ReportDefinition>(value)
                    .map_err(|e| format!("is not a Report: {e}"));
                (path.clone(), read)
            })
        })
        .collect()
}

/// A Report as published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedReport {
    /// Its package's name.
    pub package: String,
    /// The package version it is in.
    pub version: String,
    /// The Report.
    pub report: ReportDefinition,
}

impl Publications {
    /// The Reports of the newest published version of every package, by package and then file
    /// (#557): what district and national users run at central. A Report that does not read is
    /// left out (Validation refused it).
    pub async fn published_reports(&self) -> Result<Vec<PublishedReport>, Error> {
        let mut found = Vec::new();
        for (package, version, packed) in self.newest_published().await? {
            for (_, read) in reports_in(&packed) {
                if let Ok(report) = read {
                    found.push(PublishedReport {
                        package: package.clone(),
                        version: version.to_string(),
                        report,
                    });
                }
            }
        }
        Ok(found)
    }
}

/// Adds the indicators (with their disaggregations) and Age Band Sets `packed` holds to
/// `catalogue`.
fn add(catalogue: &mut Catalogue, packed: &Packed) {
    for (path, bytes) in &packed.files {
        if path.contains('/') {
            continue;
        }
        let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        if value["resourceType"] == "Measure"
            && let Ok(indicator) = measure(&value).indicator()
        {
            catalogue.disaggregations.insert(
                indicator.canonical(),
                indicator
                    .disaggregations
                    .iter()
                    .map(|d| d.name.clone())
                    .collect(),
            );
            catalogue
                .indicators
                .insert(indicator.canonical(), indicator.calculation);
        }
    }
    for set in age_band_sets_in(packed) {
        catalogue.age_band_sets.insert(set.canonical());
        catalogue.age_band_codes.insert(
            set.canonical(),
            set.bands.iter().map(|band| band.code()).collect(),
        );
    }
}

/// Central's recorded copy of the DHIS2 instance's metadata, if one is recorded (#560).
async fn recorded_dhis2(tx: &mut SurrealTx) -> Result<Option<Dhis2Metadata>, Error> {
    tx.dhis2_metadata()
        .await
        .map_err(failed)?
        .map(|export| {
            serde_json::from_str::<DataSetsExport>(&export)
                .map(|e| e.metadata())
                .map_err(failed)
        })
        .transpose()
}

/// What a recorded copy of the DHIS2 instance's metadata holds: its data sets, each with how
/// many data elements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedDhis2 {
    /// Each data set's ID, name and number of data elements.
    pub data_sets: Vec<(String, String, usize)>,
}

impl Publications {
    /// Records `export`, the DHIS2 instance's `dataSets` export (#560; analytics.md § Reporting
    /// to DHIS2), as central's copy of its metadata, by `by`, audited: what Validation checks
    /// Report mappings against from now on. A file that is not such an export is refused.
    pub async fn record_dhis2_metadata(
        &self,
        export: &str,
        by: &str,
    ) -> Result<RecordedDhis2, Error> {
        let read: DataSetsExport = serde_json::from_str(export).map_err(|e| {
            Error::Conflict(format!("the file is not a DHIS2 dataSets export: {e}"))
        })?;
        if read.data_sets.is_empty() {
            return Err(Error::Conflict(
                "the file holds no data sets: export them with their data elements and category option combos".to_owned(),
            ));
        }
        let metadata = read.metadata();
        let now = self.clock.now();
        let mut tx = self.begin().await?;
        tx.put_dhis2_metadata(export, by).await.map_err(failed)?;
        let mut agents = vec![json!({ "who": { "display": by }, "requestor": true })];
        if self.cli {
            agents.push(json!({ "who": { "display": crate::CLI_AGENT }, "requestor": false }));
        }
        let audit = json!({
            "resourceType": "AuditEvent",
            "category": [{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/audit-event-type", "code": "rest", "display": "Restful Operation" }] }],
            "code": { "coding": [{ "system": "urn:health:audit-event", "code": "dhis2-metadata" }] },
            "action": "U",
            "severity": "notice",
            "recorded": now.to_string(),
            "outcome": { "code": { "system": "http://terminology.hl7.org/CodeSystem/audit-event-outcome", "code": "0" } },
            "agent": agents,
            "source": { "observer": { "display": self.observer } },
            "entity": [{
                "what": { "display": "DHIS2 instance metadata" },
                "detail": [
                    { "type": { "text": "data sets" }, "valueString": metadata.data_sets.keys().cloned().collect::<Vec<_>>().join(", ") },
                ],
            }],
        });
        health_audit::append(&mut tx, &audit)
            .await
            .map_err(failed)?;
        tx.commit().await.map_err(failed)?;
        Ok(RecordedDhis2 {
            data_sets: metadata
                .data_sets
                .iter()
                .map(|(id, set)| (id.clone(), set.name.clone(), set.data_elements.len()))
                .collect(),
        })
    }
}

/// Every problem with the package's Reports, each prefixed with its file, given what they can
/// pin.
pub(crate) fn problems(packed: &Packed, catalogue: &Catalogue) -> Vec<String> {
    let mut problems = Vec::new();
    let kind = packed.contents.package.kind;
    for (path, read) in reports_in(packed) {
        if kind != PackageKind::Reporting {
            problems.push(format!(
                "{path}: a Report belongs in a Reporting package, not a {} package",
                serde_json::to_value(kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default()
            ));
        }
        let report = match read {
            Ok(report) => report,
            Err(why) => {
                problems.push(format!("{path}: {why}"));
                continue;
            }
        };
        let expected = format!("{REPORTS_FOLDER}{}.json", report.id);
        if !report.id.trim().is_empty() && path != expected {
            problems.push(format!(
                "{path}: the Report {} belongs at {expected}",
                report.id
            ));
        }
        problems.extend(
            report
                .problems(catalogue)
                .into_iter()
                .map(|problem| format!("{path}: {problem}")),
        );
    }
    problems
}

impl Publications {
    /// What the Reports and standard Dashboards in `packed` can pin: the indicators and Age Band
    /// Sets of every published package version, and of `packed` itself.
    pub(crate) async fn catalogue(
        &self,
        tx: &mut SurrealTx,
        packed: &Packed,
    ) -> Result<Catalogue, Error> {
        let mut catalogue = Catalogue {
            dhis2: recorded_dhis2(tx).await?,
            ..Catalogue::default()
        };
        add(&mut catalogue, packed);
        for json in tx.package_lifecycles().await.map_err(failed)? {
            let lifecycle: Lifecycle = serde_json::from_str(&json).map_err(failed)?;
            if lifecycle.stage == Stage::Published {
                add(&mut catalogue, &self.packed(&lifecycle.content).await?);
            }
        }
        Ok(catalogue)
    }
}

/// A package version the Report designer's preview reads indicators and test patients from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePackage {
    /// `name@version`.
    pub id: String,
    /// Whether it is published; otherwise it is an open draft.
    pub published: bool,
    /// Its files.
    pub files: Files,
}

/// What the Report designer's preview reads (#559; studio.md § Reports): the Reporting
/// package's draft, what Validation lets its Reports pin, and the other package versions in the
/// Studio, published or in an open draft, whose indicators and test patients it computes
/// with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportSources {
    /// The draft's files, without the generated index.
    pub draft: Files,
    /// What its Reports can pin: every published version's indicators and Age Band Sets, and
    /// the draft's own.
    pub catalogue: Catalogue,
    /// Every other package version that is published or an open draft, published first.
    pub packages: Vec<SourcePackage>,
}

impl Publications {
    /// What the Report designer's preview of draft `id` reads.
    pub async fn report_sources(&self, id: &str) -> Result<ReportSources, Error> {
        let mut tx = self.begin().await?;
        let (lifecycle, _) = self.load(&mut tx, id).await?;
        let packed = self.packed(&lifecycle.content).await?;
        let mut catalogue = Catalogue {
            dhis2: recorded_dhis2(&mut tx).await?,
            ..Catalogue::default()
        };
        add(&mut catalogue, &packed);
        let lifecycles = tx.package_lifecycles().await.map_err(failed)?;
        drop(tx);
        let mut packages = Vec::new();
        for json in lifecycles {
            let other: Lifecycle = serde_json::from_str(&json).map_err(failed)?;
            let other_id = lifecycle_id(&other);
            if other_id == id || other.stage == Stage::Withdrawn {
                continue;
            }
            let published = other.stage == Stage::Published;
            let other_packed = self.packed(&other.content).await?;
            if published {
                add(&mut catalogue, &other_packed);
            }
            packages.push(SourcePackage {
                id: other_id,
                published,
                files: other_packed.files,
            });
        }
        packages.sort_by_key(|p| !p.published);
        let mut draft = packed.files;
        draft.remove(INDEX);
        Ok(ReportSources {
            draft,
            catalogue,
            packages,
        })
    }
}

/// Where a Report lays out an indicator (#864; studio.md § Indicators, *HMIS mapping*): one row
/// of one of its table sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndicatorMapping {
    /// The package version the Report is in, `name@version`.
    pub package: String,
    /// Whether that version is published; otherwise it is the package's open draft.
    pub published: bool,
    /// The Report's id.
    pub report: String,
    /// The Report's title (e.g. *HMIS 105 · 2 Antenatal care*).
    pub title: String,
    /// The section's title.
    pub section: String,
    /// The row's label (e.g. *ANC 1st visit for women*).
    pub label: String,
    /// The version of the indicator the row pins, `url|version`.
    pub indicator: String,
    /// Whether the section's cells are sent to DHIS2.
    pub sent_to_dhis2: bool,
    /// The row's cells' DHIS2 mappings, in the section's order (#560).
    pub mappings: Vec<CellMapping>,
}

impl Publications {
    /// Every row of a Report that lays out the indicator `url`, whichever version it pins, in
    /// each package's newest published version and its open draft (#864): what the indicator
    /// editor shows, read-only, as the indicator's HMIS mapping. By package version, then the
    /// Reports' files, sections and rows in order.
    pub async fn indicator_mappings(&self, url: &str) -> Result<Vec<IndicatorMapping>, Error> {
        let mut tx = self.begin().await?;
        let mut by_package: BTreeMap<String, Vec<Lifecycle>> = BTreeMap::new();
        for json in tx.package_lifecycles().await.map_err(failed)? {
            let lifecycle: Lifecycle = serde_json::from_str(&json).map_err(failed)?;
            by_package
                .entry(lifecycle.package.clone())
                .or_default()
                .push(lifecycle);
        }
        drop(tx);
        let mut found = Vec::new();
        for lifecycles in by_package.values() {
            let newest = |wanted: &dyn Fn(Stage) -> bool| {
                lifecycles
                    .iter()
                    .filter(|l| wanted(l.stage))
                    .max_by(|a, b| a.version.cmp(&b.version))
            };
            let published = newest(&|stage| stage == Stage::Published);
            let draft = newest(&|stage| !matches!(stage, Stage::Published | Stage::Withdrawn));
            for lifecycle in published.into_iter().chain(draft) {
                let packed = self.packed(&lifecycle.content).await?;
                for (_, read) in reports_in(&packed) {
                    let Ok(report) = read else { continue };
                    for section in &report.sections {
                        let Section::Table(table) = section else {
                            continue;
                        };
                        for row in &table.rows {
                            let pinned = row.indicator.split('|').next().unwrap_or_default();
                            if pinned != url {
                                continue;
                            }
                            found.push(IndicatorMapping {
                                package: lifecycle_id(lifecycle),
                                published: lifecycle.stage == Stage::Published,
                                report: report.id.clone(),
                                title: report.title.clone(),
                                section: table.title.clone(),
                                label: row.label.clone(),
                                indicator: row.indicator.clone(),
                                sent_to_dhis2: table.send_to_dhis2,
                                mappings: table
                                    .mappings
                                    .iter()
                                    .filter(|m| m.row == row.id)
                                    .cloned()
                                    .collect(),
                            });
                        }
                    }
                }
            }
        }
        Ok(found)
    }
}
