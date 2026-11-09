//! Package activation on this node (#89; configuration.md § Distribution and activation on
//! nodes; ADR 0025).
//!
//! After each sync round, the node works out the active set its activation records give it now
//! (`health_configuration::activation`), along its scope chain: national, its district
//! (`HEALTH_DISTRICT`), its facility (`HEALTH_FACILITY`) and its programs (`HEALTH_PROGRAMS`).
//!
//! - **Staging.** Each package in that set not yet staged here is fetched from central by its
//!   sha-256, verified (`health_packages::verify`: the sha-256 the activation names, the
//!   publication signature against the national root, a safe archive, the publication rules and
//!   the release's schemas), kept in the blob store, and recorded as staged with its
//!   `package.json`. A package that fails is reported, never staged.
//! - **Checks and the switch.** The set is checked (every package staged, dependencies at their
//!   pinned versions, `minSoftware`, `formatVersion`) and the node switches to it in one
//!   transaction, or keeps the previous set. Either way a report is kept, and a failure raises
//!   `package-activation-failed`.
//! - **Retention.** Tarballs stay in the blob store: every version ever activated here is kept.
//!
//! Loading the active set into what the node serves, the reindex when a SearchParameter changes,
//! and reporting failures to central come next (#89).

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use health_configuration::activation::{
    ActivationRecord, ActiveSet, Chain, Release, Switch, active_set, switch,
};
use health_configuration::package::{PackageJson, Version};
use health_files::FsBlobStore;
use health_packages::{Digest, FORMAT_VERSIONS, Limits, Schemas, verify};
use health_persistence::Store as _;
use health_pki::Trust;
use health_surrealdb::SurrealStore;
use health_time::Timestamp;
use serde::{Deserialize, Serialize};

/// What a node needs to stage and activate packages: the national root it verifies them against,
/// the release's schemas and format versions, and its place in the scope chain.
#[derive(Clone)]
pub struct Activator {
    trust: Arc<Trust>,
    schemas: Arc<Schemas>,
    release: Release,
    chain: Chain,
}

impl Activator {
    /// An activator trusting `trust`, for a node on `chain`, running this release.
    pub fn new(trust: Trust, chain: Chain) -> Result<Self, String> {
        Ok(Self {
            trust: Arc::new(trust),
            schemas: Arc::new(Schemas::platform()),
            release: Release {
                software: env!("CARGO_PKG_VERSION")
                    .parse()
                    .map_err(|e| format!("the package version: {e}"))?,
                formats: FORMAT_VERSIONS.iter().copied().collect(),
            },
            chain,
        })
    }

    /// An activator trusting the national root installed at provisioning (`root.pem` in
    /// `pki_dir`), if it is there.
    pub fn from_pki(pki_dir: &Path, chain: Chain) -> Result<Self, String> {
        let root = std::fs::read_to_string(pki_dir.join(crate::tls::ROOT))
            .map_err(|e| format!("the national root: {e}"))?;
        let trust = Trust::new(&root).map_err(|e| format!("the national root: {e}"))?;
        Self::new(trust, chain)
    }

    /// The same, for a node running `release` instead of this one.
    #[must_use]
    pub fn with_release(mut self, release: Release) -> Self {
        self.release = release;
        self
    }
}

/// How an attempt ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// The node switched to the wanted set.
    Activated,
    /// The wanted set could not be activated; the previous set stays.
    Failed,
}

/// A switch, or a failed attempt, as the node keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// When.
    pub at: Timestamp,
    /// How it ended.
    pub outcome: Outcome,
    /// The set the activations give.
    pub wanted: ActiveSet,
    /// The set active after it: the wanted set, or the previous one kept.
    pub active: ActiveSet,
    /// What changed, as `name from → to`.
    pub changes: Vec<String>,
    /// Why it failed: packages that would not stage, and the checks the set failed.
    pub problems: Vec<String>,
}

fn store_error(error: impl std::fmt::Display) -> String {
    format!("the store: {error}")
}

/// Stages what the activations want and switches, fetching a missing tarball with `fetch` (by
/// its sha-256). `None` when the active set is already the wanted one and nothing failed.
pub async fn activate<F, Fut>(
    store: &SurrealStore,
    blobs: Option<&FsBlobStore>,
    activator: &Activator,
    now: Timestamp,
    fetch: F,
) -> Result<Option<Report>, String>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<u8>, String>>,
{
    let mut tx = store.begin().await.map_err(store_error)?;
    let records: Vec<ActivationRecord> = tx
        .package_activations()
        .await
        .map_err(store_error)?
        .iter()
        .map(|json| serde_json::from_str(json).map_err(store_error))
        .collect::<Result<_, _>>()?;
    let mut staged: Vec<PackageJson> = tx
        .staged_packages()
        .await
        .map_err(store_error)?
        .iter()
        .map(|json| serde_json::from_str(json).map_err(store_error))
        .collect::<Result<_, _>>()?;
    let current: ActiveSet = match tx.active_packages().await.map_err(store_error)? {
        Some(json) => serde_json::from_str(&json).map_err(store_error)?,
        None => ActiveSet::new(),
    };
    drop(tx);
    let activations: Vec<_> = records.iter().map(|r| r.activation.clone()).collect();
    let wanted = active_set(&activations, &activator.chain, now);

    // Stage what is wanted and not here yet.
    let mut problems = Vec::new();
    let held: BTreeSet<(String, Option<Version>)> = staged
        .iter()
        .map(|p| (p.name.clone(), p.version.parse().ok()))
        .collect();
    for (name, version) in &wanted {
        if held.contains(&(name.clone(), Some(version.clone()))) {
            continue;
        }
        let Some(record) = records
            .iter()
            .rev()
            .find(|r| &r.activation.package == name && &r.activation.version == version)
        else {
            continue;
        };
        match stage(store, blobs, activator, record, now, &fetch).await {
            Ok(package) => staged.push(package),
            Err(why) => problems.push(format!("{name}@{version}: {why}")),
        }
    }

    let outcome = switch(
        &current,
        &activations,
        &activator.chain,
        now,
        &staged,
        &activator.release,
    );
    let report = match outcome {
        Switch::Unchanged if problems.is_empty() => return Ok(None),
        Switch::Unchanged => Report {
            at: now,
            outcome: Outcome::Failed,
            active: current.clone(),
            wanted,
            changes: Vec::new(),
            problems,
        },
        Switch::Failed {
            wanted,
            problems: failed,
        } => {
            problems.extend(failed.iter().map(ToString::to_string));
            Report {
                at: now,
                outcome: Outcome::Failed,
                active: current.clone(),
                wanted,
                changes: Vec::new(),
                problems,
            }
        }
        Switch::Activated { active, changes } => Report {
            at: now,
            outcome: Outcome::Activated,
            wanted: active.clone(),
            active,
            changes: changes
                .iter()
                .map(|c| {
                    let shown = |v: &Option<Version>| {
                        v.as_ref()
                            .map_or_else(|| "none".to_owned(), Version::to_string)
                    };
                    format!("{} {} → {}", c.package, shown(&c.from), shown(&c.to))
                })
                .collect(),
            problems,
        },
    };
    let mut tx = store.begin().await.map_err(store_error)?;
    if report.outcome == Outcome::Activated {
        tx.set_active_packages(&serde_json::to_string(&report.active).map_err(store_error)?)
            .await
            .map_err(store_error)?;
    }
    let (id, outcome) = (
        uuid::Uuid::now_v7().to_string(),
        match report.outcome {
            Outcome::Activated => "activated",
            Outcome::Failed => "failed",
        },
    );
    tx.append_activation_report(
        &id,
        &now.to_string(),
        outcome,
        &serde_json::to_string(&report).map_err(store_error)?,
    )
    .await
    .map_err(store_error)?;
    health_persistence::Transaction::commit(tx)
        .await
        .map_err(store_error)?;
    match report.outcome {
        Outcome::Activated => {
            tracing::info!(changes = ?report.changes, "switched to a new set of packages");
        }
        Outcome::Failed => tracing::warn!(
            alert = "package-activation-failed",
            problems = ?report.problems,
            "an activation failed: the previous packages stay active"
        ),
    }
    Ok(Some(report))
}

/// Fetches, verifies and keeps the package `record` activates; its `package.json`.
async fn stage<F, Fut>(
    store: &SurrealStore,
    blobs: Option<&FsBlobStore>,
    activator: &Activator,
    record: &ActivationRecord,
    now: Timestamp,
    fetch: &F,
) -> Result<PackageJson, String>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<u8>, String>>,
{
    let blobs = blobs.ok_or("this node keeps no blob store to stage packages in")?;
    let hash: health_files::BlobHash = record
        .digest
        .parse()
        .map_err(|_| format!("{} is not a sha-256", record.digest))?;
    let local = blobs.clone();
    let tgz = match tokio::task::spawn_blocking(move || local.read(&hash))
        .await
        .map_err(|e| e.to_string())?
    {
        Ok(tgz) => tgz,
        Err(_) => fetch(record.digest.clone()).await?,
    };
    let expected: Digest = record
        .digest
        .parse()
        .map_err(|_| format!("{} is not a sha-256", record.digest))?;
    let verified = verify(
        &tgz,
        &record.signature,
        Some(&expected),
        &activator.trust,
        &activator.schemas,
        Limits::default(),
        now,
    )
    .map_err(|e| e.to_string())?;
    let local = blobs.clone();
    tokio::task::spawn_blocking(move || local.put(tgz.as_slice()))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("keeping the tarball: {e}"))?;
    let package = verified.contents.package;
    let mut tx = store.begin().await.map_err(store_error)?;
    tx.put_staged_package(
        &format!("{}@{}", package.name, package.version),
        &record.digest,
        &serde_json::to_string(&package).map_err(store_error)?,
    )
    .await
    .map_err(store_error)?;
    health_persistence::Transaction::commit(tx)
        .await
        .map_err(store_error)?;
    Ok(package)
}
