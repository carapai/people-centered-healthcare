//! Attachments on the wire (synchronization.md § Attachments; ADR 0021).
//!
//! A resource references a blob in an `Attachment` whose `url` is [`BLOB_URL_PREFIX`] and the
//! blob's sha-256 in hex, with `size` and `contentType`. The blob travels after the resource that
//! references it, in [`CHUNK_SIZE`] chunks the receiver appends at the offset it reports, so an
//! interrupted upload resumes where it stopped and a blob the receiver holds is never resent.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{BlobRef, ContentHash};

/// The `Attachment.url` prefix of a blob; the sha-256 in lowercase hex follows.
pub const BLOB_URL_PREFIX: &str = "urn:health:blob:sha256:";

/// The size of an upload chunk: 1 MiB.
pub const CHUNK_SIZE: u64 = 1024 * 1024;

/// The largest attachment: 50 MiB.
pub const MAX_BLOB_SIZE: u64 = 50 * 1024 * 1024;

/// The `Attachment.url` of a blob.
pub fn blob_url(hash: &ContentHash) -> String {
    format!("{BLOB_URL_PREFIX}{hash}")
}

/// Every blob a resource references, in the order they appear, each once: any object, at any
/// depth, whose `url` names a blob.
pub fn blob_refs(body: &Value) -> Vec<BlobRef> {
    let mut found: Vec<BlobRef> = Vec::new();
    collect(body, &mut found);
    found
}

fn collect(value: &Value, found: &mut Vec<BlobRef>) {
    match value {
        Value::Object(map) => {
            let blob = map
                .get("url")
                .and_then(Value::as_str)
                .and_then(|url| url.strip_prefix(BLOB_URL_PREFIX))
                .and_then(|hex| hex.parse::<ContentHash>().ok());
            if let Some(hash) = blob
                && !found.iter().any(|b| b.hash == hash)
            {
                found.push(BlobRef {
                    hash,
                    size: map.get("size").and_then(Value::as_u64).unwrap_or(0),
                    content_type: map
                        .get("contentType")
                        .and_then(Value::as_str)
                        .unwrap_or("application/octet-stream")
                        .to_owned(),
                });
            }
            map.values().for_each(|v| collect(v, found));
        }
        Value::Array(values) => values.iter().for_each(|v| collect(v, found)),
        _ => {}
    }
}

/// How much of a blob the receiver holds: `received == size` once it is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobStatus {
    /// Bytes received so far; where the next chunk starts.
    pub received: u64,
    /// The blob's size.
    pub size: u64,
}

impl BlobStatus {
    /// Whether the receiver holds the whole blob.
    pub fn complete(self) -> bool {
        self.received >= self.size
    }

    /// The next chunk to send, as `(offset, length)`, if any is left.
    pub fn next_chunk(self) -> Option<(u64, u64)> {
        (!self.complete()).then(|| (self.received, CHUNK_SIZE.min(self.size - self.received)))
    }
}
