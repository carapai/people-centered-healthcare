//! Indicators on a node (#554; analytics.md § Compute; ADR 0068): the edge computes its own
//! facility's indicators from its change log, offline, with the active packages' Measures.
//!
//! [`spawn_indicators`] runs every [`INTERVAL`]:
//! - **Definitions:** when the active package set has changed, its Measures (and the Age Band
//!   Sets they pin) are read ([`load`]) and everything is computed again from scratch.
//! - **Changes:** each change committed since the last run re-evaluates the patients it concerns
//!   and the periods they have care in ([`catch_up`]), so a recorded visit shows in its month's
//!   summary within seconds.
//! - **Every day,** and whenever the change log's epoch changed (a restore or a re-seed) or it
//!   was pruned past where the indicators stand, everything is computed again: indicators that
//!   depend on the date rather than on new records (cumulative ones, *within N days*) move on.
//!
//! Each indicator is computed for the period types of the active Reports and the standard
//! periods dashboards offer (ADR 0075): weeks, months, quarters and financial years.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use health_analytics::{IndicatorStore as _, PeriodType, Position};
use health_analytics_app::{Indicators, Library, age_band_set, is_age_band_set, measure};
use health_configuration::activation::ActiveSet;
use health_fhir_validation::OctofhirEngine;
use health_files::FsBlobStore;
use health_persistence::{Store as _, Transaction as _};
use health_surrealdb::SurrealStore;
use serde_json::Value;

/// How often the node takes its new changes into account.
pub const INTERVAL: Duration = Duration::from_secs(5);

/// How many changes are read at a time.
const PAGE: usize = 500;

/// The period types dashboards offer (ADR 0075), computed whatever the Reports.
pub const STANDARD_PERIODS: [PeriodType; 4] = [
    PeriodType::Weekly,
    PeriodType::Monthly,
    PeriodType::Quarterly,
    PeriodType::FinancialJuly,
];

/// The indicators in `packages` (each package's files by path), for `facility`: every top-level
/// `Measure` that reads as an indicator, the Age Band Sets they may pin, and the period types of
/// the Reports (`configuration/reports/`) besides [`STANDARD_PERIODS`]. Measures that do not
/// read as an indicator are named in the second part.
pub fn load<'a>(
    packages: impl IntoIterator<Item = (&'a str, &'a health_packages::Files)>,
    facility: &str,
) -> (Indicators, Vec<String>) {
    let mut indicators = Vec::new();
    let mut sets = Vec::new();
    let mut kinds: BTreeSet<PeriodType> = STANDARD_PERIODS.into_iter().collect();
    let mut problems = Vec::new();
    for (package, files) in packages {
        for (path, bytes) in files {
            if !path.ends_with(".json") {
                continue;
            }
            let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
                continue;
            };
            if path.starts_with(health_analytics::REPORTS_FOLDER) {
                if let Ok(kind) = serde_json::from_value(value["periodType"].clone()) {
                    kinds.insert(kind);
                }
                continue;
            }
            if path.contains('/') {
                continue;
            }
            if value["resourceType"] == "Measure" {
                match measure(&value).indicator() {
                    Ok(indicator) => indicators.push(indicator),
                    Err(_) => problems.push(format!("{package}: {path} is not a usable indicator")),
                }
            } else if is_age_band_set(&value)
                && let Ok(set) = age_band_set(&value).age_band_set()
            {
                sets.push(set);
            }
        }
    }
    let library = Library::new(indicators.clone()).with_age_band_sets(sets);
    let engine = Arc::new(OctofhirEngine::new());
    (
        Indicators::new(
            engine,
            library,
            indicators,
            kinds.into_iter().collect(),
            facility,
        ),
        problems,
    )
}

/// What [`catch_up`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Caught {
    /// Whether everything was computed again.
    pub recomputed: bool,
    /// How many changes were read.
    pub changes: usize,
    /// What the refreshes did.
    pub refreshed: health_analytics_app::Refreshed,
}

fn failed(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Takes every change committed on `store` since the indicators last stood into account,
/// computing everything again when the log's epoch changed, it was pruned past them, they have
/// never been computed, or `recompute` asks for it. `now` is the facility's local time.
pub async fn catch_up(
    indicators: &Indicators,
    store: &SurrealStore,
    now: &jiff::Zoned,
    recompute: bool,
) -> Result<Caught, String> {
    let mut caught = Caught::default();
    let standing = {
        let mut tx = store.begin().await.map_err(failed)?;
        tx.indicator_position().await.map_err(failed)?
    };
    let cursor = store.cursor().await.map_err(failed)?;
    let pruned = store.pruned_through().await.map_err(failed)?;
    let mut seq = match standing {
        Some(at) if !recompute && at.epoch == cursor.epoch && at.seq >= pruned => at.seq,
        _ => {
            // Changes after the cursor are read again below: evaluating a patient twice is
            // harmless.
            caught.refreshed = indicators.recompute(store, now).await?;
            caught.recomputed = true;
            cursor.seq
        }
    };
    loop {
        let changes = store.changes_after(seq, PAGE).await.map_err(failed)?;
        let Some(last) = changes.last().map(|c| c.seq) else {
            break;
        };
        caught.changes += changes.len();
        let versions: Vec<(String, String, u64)> = changes
            .into_iter()
            .filter(|c| c.op.is_none())
            .map(|c| (c.resource_type, c.resource_id, c.version_seq))
            .collect();
        let patients = Indicators::patients_changed(store, &versions).await?;
        if !patients.is_empty() {
            let done = indicators.refresh(store, &patients, now).await?;
            caught.refreshed.patients += done.patients;
            caught.refreshed.summaries += done.summaries;
            caught.refreshed.problems.extend(done.problems);
        }
        seq = last;
    }
    let mut tx = store.begin().await.map_err(failed)?;
    tx.set_indicator_position(&Position {
        epoch: cursor.epoch,
        seq,
    })
    .await
    .map_err(failed)?;
    tx.commit().await.map_err(failed)?;
    Ok(caught)
}

/// The active packages' indicators, when the active set differs from `last`; `last` becomes
/// the set read.
async fn reload(
    store: &SurrealStore,
    blobs: &FsBlobStore,
    facility: &str,
    last: &mut Option<Option<String>>,
) -> Result<Option<Indicators>, String> {
    let json = {
        let mut tx = store.begin().await.map_err(failed)?;
        tx.active_packages().await.map_err(failed)?
    };
    if last.as_ref() == Some(&json) {
        return Ok(None);
    }
    let active: ActiveSet = match &json {
        Some(json) => serde_json::from_str(json).map_err(failed)?,
        None => ActiveSet::new(),
    };
    let packages = health_activation::definitions::active_files(store, blobs, &active).await?;
    let (indicators, problems) = load(
        packages.iter().map(|(id, files)| (id.as_str(), files)),
        facility,
    );
    for problem in problems {
        tracing::warn!(%problem, "an active Measure is not computed");
    }
    *last = Some(json);
    Ok(Some(indicators))
}

/// Computes `facility`'s indicators over `store` until the process ends, with the active
/// packages' Measures (their tarballs in `blobs`).
pub fn spawn_indicators(store: SurrealStore, blobs: FsBlobStore, facility: String) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(INTERVAL);
        let mut last = None;
        let mut current: Option<Indicators> = None;
        let mut day = None;
        loop {
            interval.tick().await;
            let mut recompute = false;
            match reload(&store, &blobs, &facility, &mut last).await {
                Ok(Some(indicators)) => {
                    tracing::info!(
                        indicators = indicators.indicators().len(),
                        "computing the active packages' indicators"
                    );
                    current = Some(indicators);
                    recompute = true;
                }
                Ok(None) => {}
                Err(error) => tracing::error!(
                    alert = "indicators-failed",
                    %error,
                    "the active indicators could not be read"
                ),
            }
            let Some(indicators) = current.as_ref().filter(|i| !i.indicators().is_empty()) else {
                continue;
            };
            let now = jiff::Zoned::now();
            if day != Some(now.date()) {
                recompute = true;
            }
            match catch_up(indicators, &store, &now, recompute).await {
                Ok(caught) => {
                    day = Some(now.date());
                    for problem in &caught.refreshed.problems {
                        tracing::warn!(%problem, "an indicator could not be computed");
                    }
                    if caught.refreshed.summaries > 0 {
                        tracing::info!(
                            recomputed = caught.recomputed,
                            changes = caught.changes,
                            patients = caught.refreshed.patients,
                            summaries = caught.refreshed.summaries,
                            "indicator summaries updated"
                        );
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "computing indicators failed; it runs again");
                }
            }
        }
    });
}
