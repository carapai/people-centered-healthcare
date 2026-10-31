//! Derived lookup keys written with each version, and the blocking index they form (ADR 0017,
//! ADR 0055: patient-matching blocks in `idx_block`).

use std::future::Future;

use health_identifiers::ResourceId;
use serde_json::Value;

use crate::StoreError;

/// Computes a resource's blocking keys, `(kind, value)`, written in its version's transaction
/// and replaced by every later version (none for a tombstone). The registry supplies it for
/// Patients; the store knows nothing about matching.
pub trait DerivedKeys: Send + Sync {
    /// The keys of `resource` (of type `resource_type`); empty for types it does not index.
    fn keys(&self, resource_type: &str, resource: &Value) -> Vec<(String, String)>;
}

/// Reads the blocking index inside a transaction.
pub trait BlockingIndex {
    /// Up to `limit` live resources in block `(kind, value)`, in id order.
    fn members(
        &mut self,
        kind: &str,
        value: &str,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<ResourceId>, StoreError>> + Send;
    /// Each block's size at the last recount; `None` for a block not counted yet. Counts are
    /// recomputed in the background, never in the write transaction (ADR 0055).
    fn counts(
        &mut self,
        keys: &[(String, String)],
    ) -> impl Future<Output = Result<Vec<Option<u64>>, StoreError>> + Send;
}
