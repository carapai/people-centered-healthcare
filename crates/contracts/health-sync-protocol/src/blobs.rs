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

/// The resource types whose attachments a node fetches as soon as it receives the record,
/// rather than when first opened (ADR 0021): signed consent scans, which are needed at the
/// bedside without waiting on the uplink. Every other type's attachments come on demand.
pub const PREFETCH: &[&str] = &["Consent"];

/// Whether a record of `resource_type` has its attachments fetched ahead of need.
pub fn prefetched(resource_type: &str) -> bool {
    PREFETCH.contains(&resource_type)
}

/// The extension a node adds, when it serves a resource, to an `Attachment` whose blob it does
/// not hold yet (ADR 0021): the record was accepted before its blob arrived. It is never stored;
/// the record reads without it once the blob is here.
pub const PENDING_EXTENSION: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/attachment-pending";

/// Marks, in `body`, each `Attachment` naming a blob for which `held` is false with
/// [`PENDING_EXTENSION`] (`valueBoolean: true`). Whether any was marked.
pub fn mark_pending(body: &mut Value, held: &impl Fn(&ContentHash) -> bool) -> bool {
    match body {
        Value::Object(map) => {
            let blob = map
                .get("url")
                .and_then(Value::as_str)
                .and_then(|url| url.strip_prefix(BLOB_URL_PREFIX))
                .and_then(|hex| hex.parse::<ContentHash>().ok());
            let mut marked = false;
            if let Some(hash) = blob
                && !held(&hash)
            {
                let pending = serde_json::json!({"url": PENDING_EXTENSION, "valueBoolean": true});
                match map.get_mut("extension").and_then(Value::as_array_mut) {
                    Some(extensions) => extensions.push(pending),
                    None => {
                        map.insert("extension".to_owned(), Value::Array(vec![pending]));
                    }
                }
                marked = true;
            }
            for value in map.values_mut() {
                marked |= mark_pending(value, held);
            }
            marked
        }
        Value::Array(values) => values
            .iter_mut()
            .fold(false, |marked, v| mark_pending(v, held) | marked),
        _ => false,
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
