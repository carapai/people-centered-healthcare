//! The sync protocol, `sync/v1` (synchronization.md; ADRs 0019, 0020, 0061).
//!
//! One implementation for every hop: central serves edges and standalone desktops, an edge serves
//! its desktops and is a client of central. This crate holds what both sides share, with no I/O:
//!
//! - **The envelope** ([`Item`]): every version or ledger operation on the wire, with its
//!   [`ContentHash`] (sha-256 of canonical JSON) and [`Hlc`].
//! - **The handshake** ([`Hello`], [`Compatibility`], [`Verdict`]): the protocol's major version must
//!   match, the peer's software must be within N-1, its packages must be approved; and the
//!   [`expected_cursor`] after an epoch change.
//! - **Push** ([`group`], [`batches`]): items in `seq` order, grouped by `tx_id`, at most 500 items or
//!   4 MiB per batch, a group never split.
//! - **The receiver** ([`receive`] over the [`Authority`] port): a retried group is answered from
//!   its [`Receipt`]s and never applied twice; an `op_id` reused with a different hash is rejected.
//! - **Pull** ([`PullRequest`], [`PullResponse`], [`decide`], [`apply_pull`] over the [`Replica`]
//!   port): one transaction per group, with its checkpoint; pulled data never overwrites an
//!   unaccepted local candidate.
//! - **Item states** ([`ItemState`]): the outbox item's life cycle.
//! - **Attachments** ([`blob_refs`], [`BlobStatus`]): blobs referenced by `Attachment.url`, sent
//!   after their resource in resumable 1 MiB chunks.
//! - **NDJSON bodies** ([`to_ndjson`], [`from_ndjson`]). Compression (zstd) belongs to the transport.

mod blobs;
mod envelope;
mod grants;
mod handshake;
mod hash;
mod hlc;
mod ndjson;
mod pull;
mod push;
mod receive;
mod state;

pub use blobs::{BLOB_URL_PREFIX, BlobStatus, CHUNK_SIZE, MAX_BLOB_SIZE, blob_refs, blob_url};
pub use envelope::{BlobRef, EnvelopeError, Item, Kind, PackageRef};
pub use grants::{BreakGlassGrant, REVIEW_WITHIN_HOURS};
pub use handshake::{
    Compatibility, Cursor, Hello, Incompatibility, PROTOCOL, ProtocolVersion, SoftwareVersion,
    SoftwareVersionError, Verdict, Welcome, expected_cursor,
};
pub use hash::{ContentHash, ContentHashError};
pub use hlc::Hlc;
pub use ndjson::{NdjsonError, from_ndjson, to_ndjson};
pub use pull::{
    Applied, ApplyDecision, Lane, Local, PullError, PullRequest, PullResponse, Pulled, Replica,
    apply_pull, decide,
};
pub use push::{Group, GroupError, MAX_BATCH_BYTES, MAX_BATCH_ITEMS, batches, group};
pub use receive::{Authority, GroupResult, Outcome, Reason, Receipt, receive};
pub use state::{Answer, Event, InvalidTransition, ItemState};
