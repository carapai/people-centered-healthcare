//! Reports at validation (analytics.md § Reports; ADR 0070, ADR 0072; #556): a Report is
//! configuration in a Reporting package, `configuration/reports/<id>.json`, written to the
//! `report/1` schema. The schema checks its shape when the package is checked; this checks the
//! rest ([`ReportDefinition::problems`]): its indicators and Age Band Sets are pinned to
//! `url|version` and published (or in the package itself), and a Report that runs above facility
//! level holds no median, no distinct count and no line list.
//!
//! The indicators and sets are looked up in every published package version, so a Report keeps
//! the version it pinned whatever is published after it.

use health_analytics::{Catalogue, REPORTS_FOLDER, ReportDefinition};
use health_analytics_app::measure;
use health_configuration::lifecycle::{Lifecycle, Stage};
use health_configuration::package::{INDEX, PackageKind};
use health_packages::{Files, Packed, REPORT};
use health_surrealdb::SurrealTx;
use serde_json::Value;

use crate::age_bands::age_band_sets_in;
use crate::{Error, Publications, failed, lifecycle_id};

/// The package's Reports, each with its file, as written or why it cannot be read.
pub fn reports_in(packed: &Packed) -> Vec<(String, Result<ReportDefinition, String>)> {
    packed
        .files
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
    catalogue
        .age_band_sets
        .extend(age_band_sets_in(packed).iter().map(|set| set.canonical()));
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
        let mut catalogue = Catalogue::default();
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
        let mut catalogue = Catalogue::default();
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
