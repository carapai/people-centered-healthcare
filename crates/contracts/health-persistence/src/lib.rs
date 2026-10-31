//! Persistence contracts (dependency-rules.md § Ports, adapters, transactions and events;
//! persistence.md § The write transaction).
//!
//! - A use case opens a [`Transaction`] from a [`Store`], does all its writes through it — the
//!   resource versions of every Owner involved (ADR 0004), guard records, counters and outbox
//!   rows — and commits once. Dropping a transaction without committing rolls it back.
//! - The store keeps the **validated input JSON** with the server-set `id` and `meta` fields
//!   ([`stamp_meta`]), never a typed re-serialization, and reads return the stored bytes
//!   (ADR 0051).
//! - Stores give snapshot isolation only: a concurrent write to the same record fails the commit
//!   with [`WriteError::Conflict`], which [`RetryPolicy`] retries. Multi-record invariants use
//!   [`Transaction::claim_guard`], never read-then-check.

mod blocking;
mod json;
mod model;
mod store;

pub use blocking::{BlockingIndex, DerivedKeys};
pub use json::{RawJson, RawJsonError, stamp_meta};
pub use model::{
    CounterKey, GuardKey, OutboxEntry, OutboxPayload, Precondition, ResourceType,
    ResourceTypeError, StoredResource, VersionMeta, WriteRequest,
};
pub use store::{RetryPolicy, Store, StoreError, Transaction, WriteError};
