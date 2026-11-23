//! Standard Dashboards at validation and once published (analytics.md § Dashboards; ADR 0075;
//! #603): a standard Dashboard is configuration in a Reporting package,
//! `configuration/dashboards/<id>.json`, written to the `dashboard/1` schema. The schema checks
//! its shape when the package is checked; this checks the rest
//! ([`StandardDashboard::problems`]): its indicators are pinned to `url|version` and published
//! (or in the package itself), and split only by disaggregations they offer.
//!
//! Once published, every user's Dashboards tab lists the Dashboards of the newest published
//! version of each package ([`Publications::published_dashboards`]): changing one is a new
//! package version.

use std::collections::BTreeMap;

use health_analytics::{Catalogue, DASHBOARDS_FOLDER, DashboardProblem, StandardDashboard};
use health_configuration::lifecycle::{Lifecycle, Stage};
use health_configuration::package::{PackageKind, Version};
use health_packages::{DASHBOARD, Packed};
use serde_json::Value;

use crate::{Error, Publications, failed};

/// The package's standard Dashboards, each with its file, as written or why it cannot be read.
pub fn dashboards_in(packed: &Packed) -> Vec<(String, Result<StandardDashboard, String>)> {
    packed
        .files
        .iter()
        .filter(|(path, _)| path.starts_with("configuration/") && path.ends_with(".json"))
        .filter_map(|(path, bytes)| {
            let value = serde_json::from_slice::<Value>(bytes).ok()?;
            (value["$schema"] == DASHBOARD).then(|| {
                let read = serde_json::from_value::<StandardDashboard>(value)
                    .map_err(|e| format!("is not a Dashboard: {e}"));
                (path.clone(), read)
            })
        })
        .collect()
}

/// Every problem with the package's standard Dashboards, each prefixed with its file, given the
/// indicators they can pin.
pub(crate) fn problems(packed: &Packed, catalogue: &Catalogue) -> Vec<String> {
    let mut problems = Vec::new();
    let kind = packed.contents.package.kind;
    for (path, read) in dashboards_in(packed) {
        if kind != PackageKind::Reporting {
            problems.push(format!(
                "{path}: a standard Dashboard belongs in a Reporting package, not a {} package",
                serde_json::to_value(kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default()
            ));
        }
        let dashboard = match read {
            Ok(dashboard) => dashboard,
            Err(why) => {
                problems.push(format!("{path}: {why}"));
                continue;
            }
        };
        problems.extend(
            located_problems(&path, &dashboard, catalogue)
                .into_iter()
                .map(|problem| format!("{path}: {}", problem.message)),
        );
    }
    problems
}

/// Every problem publication finds with the Dashboard at `path`, beyond its schema, each with
/// the Visualization it is in: the file it belongs at and [`StandardDashboard::located_problems`].
/// The Studio's Dashboard designer shows these as the author edits (#879), so it refuses what
/// publication refuses.
pub fn located_problems(
    path: &str,
    dashboard: &StandardDashboard,
    catalogue: &Catalogue,
) -> Vec<DashboardProblem> {
    let mut problems = Vec::new();
    let expected = format!("{DASHBOARDS_FOLDER}{}.json", dashboard.id);
    if !dashboard.id.trim().is_empty() && path != expected {
        problems.push(DashboardProblem {
            item: None,
            message: format!("the Dashboard {} belongs at {expected}", dashboard.id),
        });
    }
    problems.extend(dashboard.located_problems(catalogue));
    problems
}

/// A standard Dashboard as published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedDashboard {
    /// Its package's name.
    pub package: String,
    /// The package version it is in.
    pub version: String,
    /// The Dashboard.
    pub dashboard: StandardDashboard,
}

impl Publications {
    /// The standard Dashboards of the newest published version of every package, by package
    /// and then file. A Dashboard that does not read is left out (Validation refused it).
    pub async fn published_dashboards(&self) -> Result<Vec<PublishedDashboard>, Error> {
        let mut tx = self.begin().await?;
        let lifecycles = tx.package_lifecycles().await.map_err(failed)?;
        drop(tx);
        let mut newest: BTreeMap<String, (Version, String)> = BTreeMap::new();
        for json in lifecycles {
            let lifecycle: Lifecycle = serde_json::from_str(&json).map_err(failed)?;
            if lifecycle.stage != Stage::Published {
                continue;
            }
            if newest
                .get(&lifecycle.package)
                .is_none_or(|(version, _)| *version < lifecycle.version)
            {
                newest.insert(
                    lifecycle.package.clone(),
                    (lifecycle.version.clone(), lifecycle.content.clone()),
                );
            }
        }
        let mut found = Vec::new();
        for (package, (version, content)) in newest {
            let packed = self.packed(&content).await?;
            for (_, read) in dashboards_in(&packed) {
                if let Ok(dashboard) = read {
                    found.push(PublishedDashboard {
                        package: package.clone(),
                        version: version.to_string(),
                        dashboard,
                    });
                }
            }
        }
        Ok(found)
    }
}
