//! A node's software roll-out, after each sync round (#136; deployment.md § Environments and
//! rollout): the release its roll-out records allow along its scope chain, its own artifact
//! fetched and handed to its [`Stager`] when that release is newer than the one it runs, and, on
//! an edge, its desktops' artifacts kept so they update over the LAN.
//!
//! Artifacts are fetched from upstream in ranges (`/sync/v1/releases/{hash}/{offset}`), resuming
//! where the last attempt stopped, so a slow or dropping link still gets there over a few
//! rounds. The blob store checks the whole file against its sha-256; the [`Stager`] checks its
//! signature against the key the node pins before anything is staged.

use std::sync::Arc;

use health_configuration::activation::Chain;
use health_configuration::package::Version;
use health_configuration::rollout::{Artifact, Offer, RolloutRecord, Target, allowed, offer};
use health_files::{BlobRef, MAX_RELEASE_SIZE};
use health_time::Timestamp;

use crate::Sync;

/// Stages the node's own update: on an edge, verifies the package against the pinned
/// release-signing key and unpacks it for an admin to apply in a maintenance window.
pub trait Stager: Send + std::marker::Sync + 'static {
    /// The release staged or running here that a newer offer would replace, if one is staged:
    /// so a staged update is not staged again every round.
    fn staged(&self) -> Option<Version>;
    /// Verifies and stages `bytes`, the artifact of `version`.
    fn stage(&self, version: &Version, artifact: &Artifact, bytes: Vec<u8>) -> Result<(), String>;
}

/// What a node fetches and stages.
#[derive(Clone)]
pub struct Releases {
    /// Its place in the scope chain.
    pub chain: Chain,
    /// Its own artifact's target.
    pub own: Target,
    /// The release it runs.
    pub running: Version,
    /// The targets it keeps for the nodes below it: an edge's desktops.
    pub downstream: Vec<Target>,
    /// What stages its own update; without one, it is only fetched.
    pub stager: Option<Arc<dyn Stager>>,
}

/// What the roll-out meant for this node in a round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseStep {
    /// The allowed, newer release was fetched, verified and staged.
    Staged(Version),
    /// The allowed release is older than the one running: it stays, and someone should correct
    /// the roll-out.
    Held(Version),
    /// The allowed, newer release has no artifact for this node.
    Missing(Version),
}

impl Sync {
    /// Fetches artifact `artifact` from upstream into the blob store, in ranges from where an
    /// earlier attempt stopped. Done when the store holds it, verified against its sha-256.
    pub async fn fetch_release(&self, artifact: &Artifact) -> Result<(), String> {
        let blobs = self
            .blobs
            .clone()
            .ok_or_else(|| "this node keeps no files".to_owned())?;
        let blob = BlobRef {
            hash: artifact
                .sha256
                .parse()
                .map_err(|_| format!("{}: not a sha-256", artifact.file))?,
            size: artifact.size,
        };
        loop {
            let at = blobs.clone();
            let offset = tokio::task::spawn_blocking(move || at.received(&blob))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            if offset >= blob.size && blobs.contains(&blob.hash) {
                return Ok(());
            }
            let path = format!("/sync/v1/releases/{}/{offset}", artifact.sha256);
            let (status, bytes) = self.send("GET", &path, Vec::new()).await?;
            if status != 200 {
                return Err(format!("{path}: upstream answered {status}"));
            }
            if bytes.is_empty() {
                return Err(format!(
                    "{}: upstream sent nothing at {offset}",
                    artifact.file
                ));
            }
            let at = blobs.clone();
            tokio::task::spawn_blocking(move || {
                at.write_chunk_limited(&blob, offset, &bytes, MAX_RELEASE_SIZE)
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("{}: {e}", artifact.file))?;
        }
    }

    /// The roll-out step of a round: see the module docs. `None` when there is nothing to do.
    pub async fn releases(&self, releases: &Releases) -> Result<Option<ReleaseStep>, String> {
        let mut tx = health_persistence::Store::begin(&self.store)
            .await
            .map_err(|e| e.to_string())?;
        let records: Vec<RolloutRecord> = tx
            .release_rollouts()
            .await
            .map_err(|e| e.to_string())?
            .iter()
            .map(|json| serde_json::from_str(json).map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        drop(tx);
        let Some(record) = allowed(&records, &releases.chain, Timestamp::now()) else {
            return Ok(None);
        };
        // The desktops' first: they need no staging here, and keep updating over the LAN
        // while the link is down.
        for target in &releases.downstream {
            if let Some(artifact) = record.artifact(*target)
                && let Err(error) = self.fetch_release(artifact).await
            {
                tracing::info!(%error, file = %artifact.file, "a release artifact is still downloading");
            }
        }
        match offer(Some(record), releases.own, &releases.running) {
            Offer::Current => Ok(None),
            Offer::Held { allowed } => Ok(Some(ReleaseStep::Held(allowed.clone()))),
            Offer::Missing { version } => Ok(Some(ReleaseStep::Missing(version.clone()))),
            Offer::Update { version, artifact } => {
                let Some(stager) = &releases.stager else {
                    self.fetch_release(artifact).await?;
                    return Ok(None);
                };
                if stager.staged().as_ref() == Some(version) {
                    return Ok(None);
                }
                self.fetch_release(artifact).await?;
                let blobs = self.blobs.clone().ok_or("this node keeps no files")?;
                let hash = artifact
                    .sha256
                    .parse()
                    .map_err(|_| "not a sha-256".to_owned())?;
                let bytes = tokio::task::spawn_blocking(move || blobs.read(&hash))
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?;
                stager.stage(version, artifact, bytes)?;
                Ok(Some(ReleaseStep::Staged(version.clone())))
            }
        }
    }
}
