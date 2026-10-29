//! An in-memory [`Store`] for domain and application tests.
//!
//! It models what the SurrealDB adapter guarantees: each transaction reads a snapshot taken at
//! [`Store::begin`], and at commit the first committer wins. A transaction that touched a head,
//! guard or counter changed by a later commit fails with [`WriteError::Conflict`].

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use health_identifiers::{NumberFormat, OpId, ResourceId, SequenceState};
use health_persistence::{
    CounterKey, GuardKey, OutboxEntry, Precondition, ResourceType, Store, StoreError,
    StoredResource, Transaction, VersionMeta, WriteError, WriteRequest, stamp_meta,
};
use health_time::Clock;

type HeadKey = (ResourceType, ResourceId);

#[derive(Debug, Default, Clone)]
struct State {
    /// Commit counter; every change records the commit that made it.
    commits: u64,
    heads: BTreeMap<HeadKey, (StoredResource, u64)>,
    history: Vec<StoredResource>,
    guards: BTreeMap<GuardKey, (bool, u64)>,
    counters: BTreeMap<CounterKey, (SequenceState, u64)>,
    outbox: Vec<OutboxEntry>,
}

/// The shared in-memory store. Cloning shares the data.
#[derive(Clone)]
pub struct MemoryStore {
    state: Arc<Mutex<State>>,
    clock: Arc<dyn Clock>,
}

impl MemoryStore {
    /// An empty store stamping `meta.lastUpdated` from `clock`.
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            state: Arc::default(),
            clock,
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Every committed outbox row, in commit order.
    pub fn outbox(&self) -> Vec<OutboxEntry> {
        self.lock().outbox.clone()
    }

    /// Every committed version of one resource, oldest first.
    pub fn history(&self, resource_type: &ResourceType, id: ResourceId) -> Vec<StoredResource> {
        self.lock()
            .history
            .iter()
            .filter(|v| &v.resource_type == resource_type && v.id == id)
            .cloned()
            .collect()
    }
}

impl Store for MemoryStore {
    type Tx = MemoryTx;

    async fn begin(&self) -> Result<MemoryTx, StoreError> {
        let snapshot = self.lock().clone();
        Ok(MemoryTx {
            store: self.clone(),
            base: snapshot.commits,
            snapshot,
            heads: BTreeMap::new(),
            history: Vec::new(),
            guards: BTreeMap::new(),
            counters: BTreeMap::new(),
            outbox: Vec::new(),
        })
    }
}

/// A transaction over [`MemoryStore`].
pub struct MemoryTx {
    store: MemoryStore,
    base: u64,
    snapshot: State,
    heads: BTreeMap<HeadKey, StoredResource>,
    history: Vec<StoredResource>,
    guards: BTreeMap<GuardKey, bool>,
    counters: BTreeMap<CounterKey, SequenceState>,
    outbox: Vec<OutboxEntry>,
}

impl MemoryTx {
    fn head(&self, key: &HeadKey) -> Option<&StoredResource> {
        self.heads
            .get(key)
            .or_else(|| self.snapshot.heads.get(key).map(|(head, _)| head))
    }

    fn guard_held(&self, key: &GuardKey) -> bool {
        self.guards
            .get(key)
            .copied()
            .or_else(|| self.snapshot.guards.get(key).map(|(held, _)| *held))
            .unwrap_or(false)
    }

    fn put(
        &mut self,
        key: HeadKey,
        op_id: OpId,
        json: &health_persistence::RawJson,
        deleted: bool,
    ) -> Result<VersionMeta, WriteError> {
        let version_seq = self.head(&key).map_or(1, |h| h.version.version_seq + 1);
        let last_updated = self.store.clock.now();
        let stamped = stamp_meta(json, &key.0, key.1, op_id, last_updated)?;
        let version = VersionMeta {
            version_id: op_id,
            version_seq,
            last_updated,
            deleted,
        };
        let stored = StoredResource {
            resource_type: key.0.clone(),
            id: key.1,
            version,
            json: stamped,
        };
        self.history.push(stored.clone());
        self.heads.insert(key, stored);
        Ok(version)
    }
}

impl Transaction for MemoryTx {
    async fn read(
        &mut self,
        resource_type: &ResourceType,
        id: ResourceId,
    ) -> Result<Option<StoredResource>, StoreError> {
        Ok(self.head(&(resource_type.clone(), id)).cloned())
    }

    async fn write(&mut self, request: WriteRequest) -> Result<VersionMeta, WriteError> {
        let key = (request.resource_type.clone(), request.id);
        match (self.head(&key), request.precondition) {
            (Some(head), _) if head.version.deleted => return Err(WriteError::Gone),
            (Some(head), Precondition::MustNotExist) => {
                return Err(WriteError::PreconditionFailed {
                    current: Some(head.version.version_id),
                });
            }
            (Some(head), Precondition::IfMatch(expected))
                if head.version.version_id != expected =>
            {
                return Err(WriteError::PreconditionFailed {
                    current: Some(head.version.version_id),
                });
            }
            (None, Precondition::IfMatch(_)) => {
                return Err(WriteError::PreconditionFailed { current: None });
            }
            _ => {}
        }
        self.put(key, request.op_id, &request.json, false)
    }

    async fn delete(
        &mut self,
        resource_type: &ResourceType,
        id: ResourceId,
        op_id: OpId,
        if_match: Option<OpId>,
    ) -> Result<VersionMeta, WriteError> {
        let key = (resource_type.clone(), id);
        let head = self
            .head(&key)
            .cloned()
            .ok_or(WriteError::PreconditionFailed { current: None })?;
        if head.version.deleted {
            return Err(WriteError::Gone);
        }
        if if_match.is_some_and(|expected| expected != head.version.version_id) {
            return Err(WriteError::PreconditionFailed {
                current: Some(head.version.version_id),
            });
        }
        self.put(key, op_id, &head.json, true)
    }

    async fn claim_guard(&mut self, key: GuardKey) -> Result<(), WriteError> {
        if self.guard_held(&key) {
            return Err(WriteError::GuardHeld(key));
        }
        self.guards.insert(key, true);
        Ok(())
    }

    async fn release_guard(&mut self, key: GuardKey) -> Result<(), WriteError> {
        self.guards.insert(key, false);
        Ok(())
    }

    async fn next_sequence(
        &mut self,
        counter: &CounterKey,
        format: &NumberFormat,
        year: i32,
    ) -> Result<u64, WriteError> {
        let current = self
            .counters
            .get(counter)
            .copied()
            .or_else(|| self.snapshot.counters.get(counter).map(|(state, _)| *state))
            .unwrap_or_default();
        let (next, value) = current.next(format, year)?;
        self.counters.insert(counter.clone(), next);
        Ok(value)
    }

    async fn append_outbox(&mut self, entry: OutboxEntry) -> Result<(), WriteError> {
        self.outbox.push(entry);
        Ok(())
    }

    async fn commit(self) -> Result<(), WriteError> {
        let mut state = self.store.lock();
        let changed_since = |at: u64| at > self.base;
        let conflict = self
            .heads
            .keys()
            .any(|k| state.heads.get(k).is_some_and(|(_, at)| changed_since(*at)))
            || self.guards.keys().any(|k| {
                state
                    .guards
                    .get(k)
                    .is_some_and(|(_, at)| changed_since(*at))
            })
            || self.counters.keys().any(|k| {
                state
                    .counters
                    .get(k)
                    .is_some_and(|(_, at)| changed_since(*at))
            });
        if conflict {
            return Err(WriteError::Conflict);
        }
        state.commits += 1;
        let at = state.commits;
        for (key, head) in self.heads {
            state.heads.insert(key, (head, at));
        }
        state.history.extend(self.history);
        for (key, held) in self.guards {
            state.guards.insert(key, (held, at));
        }
        for (key, counter) in self.counters {
            state.counters.insert(key, (counter, at));
        }
        state.outbox.extend(self.outbox);
        Ok(())
    }
}
