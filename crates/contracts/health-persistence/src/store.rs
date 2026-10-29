use std::future::Future;

use health_identifiers::{NumberFormat, OpId, ResourceId};

use crate::{
    CounterKey, GuardKey, OutboxEntry, ResourceType, StoredResource, VersionMeta, WriteRequest,
};

/// A failure of the store itself.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// The store is unreachable or failed; the message never contains resource bodies.
    #[error("store unavailable: {0}")]
    Unavailable(String),
}

/// Why a write or commit was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WriteError {
    /// The head does not match the [`crate::Precondition`] (HTTP 412). `current` is the head's
    /// `versionId`, if there is a head.
    #[error("precondition failed; current version is {current:?}")]
    PreconditionFailed {
        /// The current head's version, if the resource exists.
        current: Option<OpId>,
    },
    /// The resource is deleted (HTTP 410), and the write did not expect that.
    #[error("resource is deleted")]
    Gone,
    /// Another transaction committed a conflicting change first; safe to retry the whole use case.
    #[error("write-write conflict")]
    Conflict,
    /// A guard key is already held by another record.
    #[error("guard `{0:?}` is already held")]
    GuardHeld(GuardKey),
    /// A counter cannot advance.
    #[error("counter: {0}")]
    Counter(#[from] health_identifiers::NumberError),
    /// The request JSON could not be stamped.
    #[error("invalid resource JSON: {0}")]
    Json(#[from] crate::RawJsonError),
    /// The store failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// A source of transactions: the unit of work.
pub trait Store: Send + Sync {
    /// The transaction type.
    type Tx: Transaction;

    /// Begins a transaction with snapshot isolation.
    fn begin(&self) -> impl Future<Output = Result<Self::Tx, StoreError>> + Send;
}

/// One unit of work. Reads see the snapshot plus this transaction's own writes. Nothing is visible
/// to others until [`Transaction::commit`]; dropping the transaction rolls it back.
pub trait Transaction: Send {
    /// The current head of a resource, including tombstones.
    fn read(
        &mut self,
        resource_type: &ResourceType,
        id: ResourceId,
    ) -> impl Future<Output = Result<Option<StoredResource>, StoreError>> + Send;

    /// Writes a new version: checks the precondition, stamps `id` and `meta`, and assigns the
    /// next `version_seq`. The head, history row and index entries move together.
    fn write(
        &mut self,
        request: WriteRequest,
    ) -> impl Future<Output = Result<VersionMeta, WriteError>> + Send;

    /// Writes a tombstone version. The resource keeps its id (ids are never recycled).
    fn delete(
        &mut self,
        resource_type: &ResourceType,
        id: ResourceId,
        op_id: OpId,
        if_match: Option<OpId>,
    ) -> impl Future<Output = Result<VersionMeta, WriteError>> + Send;

    /// Claims a guard key for the rest of time, or until released by a later write; fails with
    /// [`WriteError::GuardHeld`] if another record holds it, or at commit if a concurrent
    /// transaction claimed it first.
    fn claim_guard(&mut self, key: GuardKey)
    -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Releases a guard key held by this node's records, e.g. when an Encounter closes.
    fn release_guard(
        &mut self,
        key: GuardKey,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Advances a node-local counter and returns the value to mint, per
    /// [`health_identifiers::SequenceState::next`]. Concurrent transactions advancing the same
    /// counter conflict, so a value is never handed out twice.
    fn next_sequence(
        &mut self,
        counter: &CounterKey,
        format: &NumberFormat,
        year: i32,
    ) -> impl Future<Output = Result<u64, WriteError>> + Send;

    /// Appends an outbox row, committed with everything else.
    fn append_outbox(
        &mut self,
        entry: OutboxEntry,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Commits atomically, or fails with [`WriteError::Conflict`] and applies nothing.
    fn commit(self) -> impl Future<Output = Result<(), WriteError>> + Send;
}

/// When to rerun a use case whose commit conflicted (persistence.md: up to 3 retries with jitter).
/// The adapter supplies the backoff, because this crate has no runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total attempts, including the first.
    pub max_attempts: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: 4 }
    }
}

impl RetryPolicy {
    /// Whether attempt number `attempt` (1-based), which failed with `error`, should be retried.
    /// Only conflicts are retried; on the rerun, `If-Match` turns a lost race into a 412.
    pub fn should_retry(&self, attempt: u32, error: &WriteError) -> bool {
        matches!(error, WriteError::Conflict) && attempt < self.max_attempts
    }
}
