//! Seeding from removable media (#83; synchronization.md § Seeding): a facility's snapshot that
//! central wrote to a file (`central-server seed export`), applied here without the link.
//!
//! In order: the file's sha-256 is taken as it is read, and its detached signature must be the
//! publication signer's under the national root, as a seed (`SEED_TYP`, never a package's).
//! Only then is it read again, decompressed and parsed as it goes, like a re-seed over the
//! network: its `seed` header must name this facility before any head is applied, the heads are
//! applied a batch at a time, and the checkpoints move to its cursor only once all of it has been
//! read. A seed that fails part way leaves the node to be seeded again.

use std::io::Read;
use std::path::Path;

use health_identifiers::NodeId;
use health_packages::{Digest, SEED_TYP, verify_detached};
use health_pki::Trust;
use health_surrealdb::SurrealStore;
use health_sync_protocol::{SnapshotEvent, decoder, read_snapshot};
use health_time::Timestamp;
use sha2::{Digest as _, Sha256};

/// What a seed brought.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    /// The heads applied that were not here already.
    pub applied: usize,
    /// The central node it came from, now this node's upstream checkpoint.
    pub central: NodeId,
    /// Central's change log position as of the seed: sync resumes after it.
    pub seq: u64,
}

/// Applies the seed at `file`, signed by `jws`, to `store` for `facility`, trusting `trust`'s
/// root at `now`.
pub async fn import(
    store: &SurrealStore,
    file: &Path,
    jws: &str,
    trust: &Trust,
    facility: &str,
    now: Timestamp,
) -> Result<Imported, String> {
    let path = file.to_path_buf();
    let digest = tokio::task::spawn_blocking(move || hash(&path))
        .await
        .map_err(|e| e.to_string())??;
    verify_detached(jws, &Digest(digest), trust, now, SEED_TYP)
        .map_err(|e| format!("the seed's signature: {e}"))?;

    let (events, mut parsed) = tokio::sync::mpsc::channel::<SnapshotEvent>(2);
    let path = file.to_path_buf();
    let parser = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        let reader = decoder(std::io::BufReader::new(file))?;
        read_snapshot(reader, 200, |event| {
            events
                .blocking_send(event)
                .map_err(|_| "the seed stopped".to_owned())
        })
    });
    let applied = async {
        let mut header = false;
        let mut cursor = None;
        let mut applied = 0;
        while let Some(event) = parsed.recv().await {
            match event {
                SnapshotEvent::Seed(seed) => {
                    let named = seed["facility"].as_str().unwrap_or_default();
                    if named != facility {
                        return Err(format!(
                            "the seed is for facility {named:?}, not this edge's {facility:?}"
                        ));
                    }
                    header = true;
                }
                SnapshotEvent::Cursor(at) if header => cursor = Some(at),
                SnapshotEvent::Items(batch) if header => {
                    let Some(at) = &cursor else {
                        return Err("the seed's heads come before its cursor".into());
                    };
                    applied += store
                        .seed_items(at.node, &batch)
                        .await
                        .map_err(|e| format!("seeding: {e}"))?;
                }
                _ => return Err("the file is not a seed: it names no facility first".into()),
            }
        }
        Ok((cursor, applied))
    };
    let applied = applied.await;
    // Dropping the receiver stops a parser still sending.
    drop(parsed);
    let read = parser.await.map_err(|e| e.to_string())?;
    let (cursor, applied) = applied?;
    read?;
    let cursor = cursor.ok_or("the seed has no cursor")?;
    store
        .seed_complete(cursor.node, &cursor)
        .await
        .map_err(|e| format!("seeding: {e}"))?;
    Ok(Imported {
        applied,
        central: cursor.node,
        seq: cursor.seq,
    })
}

/// The sha-256 of the file at `path`, read in pieces.
fn hash(path: &Path) -> Result<[u8; 32], String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hash = Sha256::new();
    let mut buf = vec![0; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(hash.finalize().into());
        }
        hash.update(&buf[..n]);
    }
}
