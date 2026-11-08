//! The edge's sync with central (#81; synchronization.md § Session, § Wake-up; ADR 0062).
//!
//! A round:
//! 1. **Handshake.** The node says hello and learns central's node id and verdict. Central is
//!    added as the node's upstream: the first time, everything already on the log is queued for
//!    it.
//! 2. **Push.** The outbox goes up in batches of whole transactions (at most 500 items or 4 MiB).
//!    Each answered group is acknowledged, and a conflict or rejection is kept as an unaccepted
//!    candidate. A quarantined node still pushes; central holds its items for review.
//! 3. **Attachments.** Each blob an accepted record references is uploaded in 1 MiB chunks from
//!    where central says it stopped, so an interrupted upload resumes.
//! 4. **Break-glass grants** made or acknowledged here go up, so their review starts when they
//!    reach central.
//! 5. **Pull.** While the node is compatible, it pulls until caught up. Each group is applied
//!    with its checkpoint.
//!
//! Between rounds the task waits for central's "cursor advanced" hint on `/sync/v1/events`, or
//! for the polling interval. While central cannot be reached, the interval doubles, up to 30
//! minutes, with jitter. Every request is signed with the facility intermediate's key (ADR 0062)
//! and goes over HTTPS only, verified against central's CA when provisioning named one.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::StreamExt as _;
use health_identifiers::NodeId;
use health_provisioning::{SYNC_SCHEME, sync_jws};
use health_surrealdb::{Outgoing, SurrealReplica, SurrealStore};
use health_sync_protocol::{
    BlobStatus, GroupResult, Hello, Lane, PROTOCOL, PullRequest, PullResponse, SoftwareVersion,
    Verdict, Welcome, apply_pull, batches, from_ndjson, to_ndjson,
};
use health_time::Timestamp;
use http_body_util::{BodyExt as _, BodyStream, Full};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use p256::ecdsa::SigningKey;
use rustls::{ClientConfig, RootCertStore};
use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject as _;
use tokio::sync::Notify;

/// How long one request may take.
const TIMEOUT: Duration = Duration::from_secs(120);
/// The longest wait between rounds while central cannot be reached.
const MAX_BACKOFF: Duration = Duration::from_secs(30 * 60);
/// Items per push or pull request.
const BATCH: usize = 500;

/// Where the node syncs to, from its provisioning record (`node.json`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upstream {
    /// Central's origin, e.g. `https://central.example.org`.
    pub origin: String,
    /// Central's CA, PEM, when provisioning named one; otherwise the OS trust store.
    pub ca_pem: Option<String>,
    /// This node's id, as central's registry knows it.
    pub node: NodeId,
}

impl Upstream {
    /// The upstream named in `pki_dir/node.json`, once the node is provisioned.
    pub fn from_identity(pki_dir: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(pki_dir.join("node.json")).ok()?;
        let value: serde_json::Value = serde_json::from_str(&text).ok()?;
        let ca_pem = match value["central_ca"].as_str() {
            Some(path) => Some(std::fs::read_to_string(path).ok()?),
            None => None,
        };
        Some(Self {
            origin: value["central"].as_str()?.trim_end_matches('/').to_owned(),
            ca_pem,
            node: value["node_id"].as_str()?.parse().ok()?,
        })
    }
}

/// What a round moved.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Round {
    /// Items pushed.
    pub pushed: usize,
    /// Items pulled.
    pub pulled: usize,
    /// Heads applied from a re-seed, when central said the node was beyond its horizon.
    pub seeded: usize,
    /// Attachments whose upload completed.
    pub blobs: usize,
    /// Break-glass grants, and acknowledgements of them, sent for review (#224).
    pub grants: usize,
    /// The verdict central gave.
    pub verdict: Option<Verdict>,
}

type Https = hyper_rustls::HttpsConnector<HttpConnector>;

/// The node's sync with central.
#[derive(Clone)]
pub struct Sync {
    store: SurrealStore,
    upstream: Arc<Upstream>,
    key: Arc<SigningKey>,
    client: Client<Https, Full<VecDeque<u8>>>,
    software: SoftwareVersion,
    blobs: Option<health_files::FsBlobStore>,
}

fn connector(ca_pem: Option<&str>) -> Result<Https, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = hyper_rustls::HttpsConnectorBuilder::new();
    let builder = match ca_pem {
        Some(pem) => {
            let mut roots = RootCertStore::empty();
            for der in CertificateDer::pem_slice_iter(pem.as_bytes()) {
                roots
                    .add(der.map_err(|e| format!("central's CA: {e}"))?)
                    .map_err(|e| format!("central's CA: {e}"))?;
            }
            if roots.is_empty() {
                return Err("central's CA file holds no certificate".into());
            }
            let config = ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .map_err(|e| e.to_string())?
                .with_root_certificates(roots)
                .with_no_client_auth();
            builder.with_tls_config(config)
        }
        None => builder
            .with_provider_and_native_roots(provider)
            .map_err(|e| format!("the system trust store: {e}"))?,
    };
    Ok(builder.https_only().enable_http1().build())
}

impl Sync {
    /// Syncs `store` with `upstream`, signing with the facility intermediate's `key`.
    pub fn new(store: SurrealStore, upstream: Upstream, key: SigningKey) -> Result<Self, String> {
        let client =
            Client::builder(TokioExecutor::new()).build(connector(upstream.ca_pem.as_deref())?);
        Ok(Self {
            store,
            upstream: Arc::new(upstream),
            key: Arc::new(key),
            client,
            software: env!("CARGO_PKG_VERSION")
                .parse()
                .map_err(|_| "the package version is not major.minor.patch".to_owned())?,
            blobs: None,
        })
    }

    /// Sends the attachments its records reference from `blobs`, and keeps those it fetches there
    /// (synchronization.md § Attachments).
    #[must_use]
    pub fn with_blobs(mut self, blobs: health_files::FsBlobStore) -> Self {
        self.blobs = Some(blobs);
        self
    }

    /// Sends `method` to `path` with `body`; the status and the answer's body.
    async fn send(
        &self,
        method: &str,
        path: &str,
        body: Vec<u8>,
    ) -> Result<(u16, Vec<u8>), String> {
        let request = self.request(method, path, body)?;
        let response = tokio::time::timeout(TIMEOUT, self.client.request(request))
            .await
            .map_err(|_| format!("{path}: central did not answer in time"))?
            .map_err(|e| format!("{path}: cannot reach central: {e}"))?;
        let status = response.status().as_u16();
        let bytes = tokio::time::timeout(TIMEOUT, response.into_body().collect())
            .await
            .map_err(|_| format!("{path}: central did not answer in time"))?
            .map_err(|e| format!("{path}: {e}"))?
            .to_bytes()
            .to_vec();
        Ok((status, bytes))
    }

    /// Uploads the attachments queued for central, each from where central says it stopped, so
    /// an interrupted upload resumes rather than restarts. A blob not yet held here (its record
    /// came from a desktop whose upload has not finished) stays queued. Returns blobs completed.
    pub async fn upload_blobs(&self, central: NodeId) -> Result<usize, String> {
        let Some(blobs) = self.blobs.clone() else {
            return Ok(0);
        };
        let peer = central.to_string();
        let mut completed = 0;
        for blob in self
            .store
            .queued_blobs(&peer)
            .await
            .map_err(|e| e.to_string())?
        {
            let hash: health_files::BlobHash = blob
                .hash
                .to_string()
                .parse()
                .map_err(|_| "a queued blob has an invalid hash".to_owned())?;
            let local = blobs.clone();
            let Ok(Ok(bytes)) = tokio::task::spawn_blocking(move || local.read(&hash)).await else {
                continue;
            };
            let base = format!("/sync/v1/blobs/{}/{}", blob.hash, blob.size);
            let (status, answer) = self.send("GET", &base, Vec::new()).await?;
            if status != 200 {
                return Err(format!("{base}: central answered {status}"));
            }
            let mut at: BlobStatus =
                serde_json::from_slice(&answer).map_err(|e| format!("{base}: {e}"))?;
            while let Some((offset, length)) = at.next_chunk() {
                let start = usize::try_from(offset).map_err(|e| e.to_string())?;
                let end = usize::try_from(offset + length).map_err(|e| e.to_string())?;
                let chunk = bytes
                    .get(start..end)
                    .ok_or_else(|| "a blob is shorter than its record says".to_owned())?
                    .to_vec();
                let path = format!("{base}/{offset}");
                let (status, answer) = self.send("PUT", &path, chunk).await?;
                if status != 200 && status != 409 {
                    return Err(format!("{path}: central answered {status}"));
                }
                at = serde_json::from_slice(&answer).map_err(|e| format!("{path}: {e}"))?;
            }
            self.store
                .blob_sent(&peer, &blob.hash)
                .await
                .map_err(|e| e.to_string())?;
            completed += 1;
        }
        Ok(completed)
    }

    /// Sends the break-glass grants queued for central, so the privacy officer's review starts
    /// when they arrive (#224; offline-auth.md § Break-glass).
    pub async fn send_grants(&self, central: NodeId) -> Result<usize, String> {
        let peer = central.to_string();
        let grants = self
            .store
            .queued_grants(&peer)
            .await
            .map_err(|e| e.to_string())?;
        if grants.is_empty() {
            return Ok(0);
        }
        let body = serde_json::to_vec(&grants).map_err(|e| e.to_string())?;
        self.post("/sync/v1/break-glass", body).await?;
        let ids: Vec<String> = grants.into_iter().map(|g| g.id).collect();
        self.store
            .grants_sent(&peer, &ids)
            .await
            .map_err(|e| e.to_string())?;
        Ok(ids.len())
    }

    /// Fetches an attachment from central on demand and keeps it, verified, in this node's store.
    pub async fn fetch_blob(&self, blob: &health_sync_protocol::BlobRef) -> Result<(), String> {
        let blobs = self
            .blobs
            .clone()
            .ok_or_else(|| "this node keeps no attachments".to_owned())?;
        let path = format!("/sync/v1/blobs/{}/content", blob.hash);
        let (status, bytes) = self.send("GET", &path, Vec::new()).await?;
        if status != 200 {
            return Err(format!("{path}: central answered {status}"));
        }
        let expected = health_files::BlobRef {
            hash: blob
                .hash
                .to_string()
                .parse()
                .map_err(|_| "an invalid hash".to_owned())?,
            size: blob.size,
        };
        tokio::task::spawn_blocking(move || blobs.put_expecting(expected, bytes.as_slice()))
            .await
            .map_err(|e| e.to_string())?
            .map(|_| ())
            .map_err(|e| format!("{path}: {e}"))
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Vec<u8>,
    ) -> Result<hyper::Request<Full<VecDeque<u8>>>, String> {
        let token = sync_jws(
            &self.upstream.node.to_string(),
            method,
            path,
            &body,
            &self.key,
            Timestamp::now(),
        );
        hyper::Request::builder()
            .method(method)
            .uri(format!("{}{path}", self.upstream.origin))
            .header("authorization", format!("{SYNC_SCHEME} {token}"))
            .header(
                "content-type",
                if path.ends_with("/push") {
                    "application/x-ndjson"
                } else {
                    "application/json"
                },
            )
            .body(Full::new(VecDeque::from(body)))
            .map_err(|e| format!("{path}: {e}"))
    }

    /// POSTs `body` to `path`; the answer's body, or why there is none.
    async fn post(&self, path: &str, body: Vec<u8>) -> Result<Vec<u8>, String> {
        let request = self.request("POST", path, body)?;
        let response = tokio::time::timeout(TIMEOUT, self.client.request(request))
            .await
            .map_err(|_| format!("{path}: central did not answer in time"))?
            .map_err(|e| format!("{path}: cannot reach central: {e}"))?;
        let status = response.status();
        let bytes = tokio::time::timeout(TIMEOUT, response.into_body().collect())
            .await
            .map_err(|_| format!("{path}: central did not answer in time"))?
            .map_err(|e| format!("{path}: {e}"))?
            .to_bytes()
            .to_vec();
        if status.is_success() {
            Ok(bytes)
        } else {
            let detail = serde_json::from_slice::<serde_json::Value>(&bytes)
                .ok()
                .and_then(|v| v["detail"].as_str().map(str::to_owned))
                .unwrap_or_default();
            Err(format!("{path}: central answered {status}: {detail}"))
        }
    }

    /// One round: handshake, push the outbox, then pull until caught up.
    pub async fn round(&self) -> Result<Round, String> {
        let mut round = Round::default();
        let cursor = self.store.cursor().await.map_err(|e| e.to_string())?;
        let hello = Hello {
            protocol: PROTOCOL,
            software: self.software,
            node: self.upstream.node,
            epoch: cursor.epoch,
            packages: Vec::new(),
            snapshot_version: 0,
            cursor: None,
        };
        let body = serde_json::to_vec(&hello).map_err(|e| e.to_string())?;
        let welcome: Welcome = serde_json::from_slice(&self.post("/sync/v1/hello", body).await?)
            .map_err(|e| format!("/sync/v1/hello: {e}"))?;
        round.verdict = Some(welcome.verdict.clone());
        if let Verdict::Refused { reason } = &welcome.verdict {
            return Err(format!("central refuses this node's protocol: {reason:?}"));
        }
        let central = welcome.node;
        let peer = central.to_string();
        self.store
            .add_upstream(&peer)
            .await
            .map_err(|e| e.to_string())?;

        loop {
            let pending = self
                .store
                .outbox(&peer, self.upstream.node, BATCH)
                .await
                .map_err(|e| e.to_string())?;
            if pending.is_empty() {
                break;
            }
            let groups = pending.iter().map(|o| o.group.clone()).collect();
            let mut start = 0;
            for batch in batches(groups) {
                let sent: &[Outgoing] = &pending[start..start + batch.len()];
                start += batch.len();
                let items: Vec<_> = batch.into_iter().flat_map(|g| g.items).collect();
                let count = items.len();
                let body = to_ndjson(&items).map_err(|e| e.to_string())?;
                let answer = self.post("/sync/v1/push", body).await?;
                let results: Vec<GroupResult> =
                    from_ndjson(&answer).map_err(|e| format!("/sync/v1/push: {e}"))?;
                let outcomes: Vec<_> = results.into_iter().map(|r| r.outcome).collect();
                self.store
                    .record_push(&peer, sent, &outcomes)
                    .await
                    .map_err(|e| e.to_string())?;
                if outcomes.len() < sent.len() {
                    return Err("central applied part of a push; the rest is resent".into());
                }
                round.pushed += count;
            }
        }

        round.blobs = self.upload_blobs(central).await?;
        round.grants = self.send_grants(central).await?;

        if welcome.verdict != Verdict::Compatible {
            tracing::warn!(
                alert = "sync-quarantined",
                verdict = ?welcome.verdict,
                "central holds this node's pushes for review and sends it nothing until it upgrades"
            );
            return Ok(round);
        }
        let replica = SurrealReplica::new(&self.store, central);
        loop {
            let request = PullRequest {
                cursor: self
                    .store
                    .checkpoint(central)
                    .await
                    .map_err(|e| e.to_string())?,
                lane: Lane::Clinical,
                limit: u32::try_from(BATCH).unwrap_or(u32::MAX),
            };
            let body = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
            let response: PullResponse =
                serde_json::from_slice(&self.post("/sync/v1/pull", body).await?)
                    .map_err(|e| format!("/sync/v1/pull: {e}"))?;
            if response == PullResponse::ReseedRequired {
                // Beyond central's horizon (or central was restored): re-seed from its snapshot.
                // The outbox, already pushed above, is kept; anything left goes up next round.
                round.seeded += self.reseed(central).await?;
                continue;
            }
            let more = matches!(response, PullResponse::Items { more: true, .. });
            if let PullResponse::Items { items, .. } = &response {
                round.pulled += items.len();
            }
            apply_pull(&replica, response)
                .await
                .map_err(|e| format!("applying the pull: {e}"))?;
            if !more {
                break;
            }
        }
        Ok(round)
    }

    /// Re-seeds the node from central's snapshot; the number of heads applied.
    async fn reseed(&self, central: NodeId) -> Result<usize, String> {
        #[derive(serde::Deserialize)]
        struct Snapshot {
            cursor: health_sync_protocol::Cursor,
            items: Vec<health_sync_protocol::Item>,
        }
        tracing::warn!(
            alert = "sync-reseed",
            "central asked this node to re-seed: it was offline beyond central's horizon, or central was restored"
        );
        let snapshot: Snapshot =
            serde_json::from_slice(&self.post("/sync/v1/snapshot", b"{}".to_vec()).await?)
                .map_err(|e| format!("/sync/v1/snapshot: {e}"))?;
        self.store
            .seed(central, &snapshot.items, &snapshot.cursor)
            .await
            .map_err(|e| format!("re-seeding: {e}"))
    }

    /// Wakes `hint` whenever central says its log advanced, reconnecting after a pause when the
    /// stream drops. A node that cannot hold the stream open still polls.
    async fn listen(self, hint: Arc<Notify>) {
        loop {
            if let Ok(request) = self.request("GET", "/sync/v1/events", Vec::new())
                && let Ok(response) = self.client.request(request).await
                && response.status().is_success()
            {
                let mut frames = BodyStream::new(response.into_body());
                while let Some(Ok(frame)) = frames.next().await {
                    if frame
                        .data_ref()
                        .is_some_and(|data| data.windows(13).any(|w| w == b"event: cursor"))
                    {
                        hint.notify_one();
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }

    /// Syncs until the node stops: a round, then a wait for central's hint or `every`, doubled
    /// with jitter while central cannot be reached.
    pub async fn run(self, every: Duration) -> Result<(), String> {
        let hint = Arc::new(Notify::new());
        let listener = tokio::spawn(self.clone().listen(Arc::clone(&hint)));
        let mut failures: u32 = 0;
        loop {
            match self.round().await {
                Ok(round) => {
                    if failures > 0 {
                        tracing::info!(
                            alert = "sync-failing",
                            outcome = "cleared",
                            "syncing with central again"
                        );
                    }
                    failures = 0;
                    if round.pushed + round.pulled > 0 {
                        tracing::info!(
                            pushed = round.pushed,
                            pulled = round.pulled,
                            "synced with central"
                        );
                    }
                }
                Err(error) => {
                    if failures == 0 {
                        tracing::warn!(alert = "sync-failing", %error, "cannot sync with central");
                    }
                    failures = failures.saturating_add(1);
                }
            }
            let wait = backoff(every, failures);
            tokio::select! {
                () = hint.notified(), if failures == 0 => {}
                () = tokio::time::sleep(wait) => {}
            }
            if listener.is_finished() {
                return Err("the hint listener stopped".into());
            }
        }
    }
}

/// The wait after `failures` failed rounds in a row: `every`, doubled per failure up to
/// 30 minutes, plus up to a tenth more as jitter so nodes do not retry in step.
pub fn backoff(every: Duration, failures: u32) -> Duration {
    let base = every
        .saturating_mul(2u32.saturating_pow(failures.min(16)))
        .min(MAX_BACKOFF.max(every));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let jitter = base / 10 * (nanos % 1000) / 1000;
    base + jitter
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_up_to_half_an_hour_with_jitter() {
        let every = Duration::from_secs(60);
        let first = backoff(every, 0);
        assert!(first >= every && first <= every + every / 10);
        let third = backoff(every, 2);
        assert!(third >= every * 4 && third <= every * 4 + every * 4 / 10);
        let many = backoff(every, 40);
        assert!(many >= MAX_BACKOFF && many <= MAX_BACKOFF + MAX_BACKOFF / 10);
    }
}
