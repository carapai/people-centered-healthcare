//! Seeds on removable media (#83; synchronization.md § Seeding): a facility's snapshot written to
//! a file, for a large first install where the link cannot carry it. The file is the snapshot
//! answer with a `seed` header naming the facility, zstd-compressed:
//! `{"seed":{"facility":…,"central":…,"createdAt":…},"cursor":…,"items":[…]}`. It is signed
//! detached, over its sha-256, by the publication signer (`health_packages::SEED_TYP`), and the
//! edge applies it only for its own facility.

use std::io::Write;

use health_surrealdb::{SnapshotPart, SurrealStore, SyncScope};
use health_sync_protocol::StreamEncoder;
use health_time::Timestamp;
use sha2::{Digest as _, Sha256};

/// Writes `facility`'s snapshot from `store` to `out` as a seed, read a page at a time; returns
/// the sha-256 of the bytes written, for the signature, and how many heads it holds.
pub async fn write_seed(
    store: &SurrealStore,
    facility: &str,
    at: Timestamp,
    out: &mut impl Write,
) -> Result<([u8; 32], usize), String> {
    let node = store.node_id().await.map_err(|e| e.to_string())?;
    let mut parts = store.snapshot_stream(SyncScope::Facility(facility.to_owned()), node);
    let cursor = match parts.recv().await {
        Some(Ok(SnapshotPart::Cursor(cursor))) => cursor,
        Some(Err(error)) => return Err(error.to_string()),
        _ => return Err("the snapshot ended before its cursor".into()),
    };
    let header = serde_json::json!({
        "facility": facility,
        "central": node.to_string(),
        "createdAt": at.to_string(),
    });
    let mut sink = Sink {
        out,
        hash: Sha256::new(),
        encoder: StreamEncoder::new(),
    };
    sink.write(
        format!(
            r#"{{"seed":{header},"cursor":{},"items":["#,
            serde_json::to_string(&cursor).map_err(|e| e.to_string())?
        )
        .as_bytes(),
    )?;
    let mut heads = 0;
    while let Some(part) = parts.recv().await {
        let SnapshotPart::Items(page) = part.map_err(|e| e.to_string())? else {
            continue;
        };
        let mut raw = Vec::new();
        for item in &page {
            if heads > 0 {
                raw.push(b',');
            }
            heads += 1;
            serde_json::to_writer(&mut raw, item).map_err(|e| e.to_string())?;
        }
        sink.write(&raw)?;
    }
    sink.write(b"]}")?;
    let tail = sink.encoder.finish();
    sink.hash.update(&tail);
    sink.out.write_all(&tail).map_err(|e| e.to_string())?;
    sink.out.flush().map_err(|e| e.to_string())?;
    Ok((sink.hash.finalize().into(), heads))
}

/// The compressed bytes going out, hashed as they go.
struct Sink<'w, W: Write> {
    out: &'w mut W,
    hash: Sha256,
    encoder: StreamEncoder,
}

impl<W: Write> Sink<'_, W> {
    fn write(&mut self, raw: &[u8]) -> Result<(), String> {
        let compressed = self.encoder.push(raw);
        self.hash.update(&compressed);
        self.out.write_all(&compressed).map_err(|e| e.to_string())
    }
}
