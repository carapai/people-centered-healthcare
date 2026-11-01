//! The `health-persistence` unit of work on SurrealDB interactive transactions
//! (persistence.md § The write transaction).
//!
//! Every read and write runs inside one SurrealDB transaction, which gives snapshot isolation:
//! a concurrent commit to the same head, guard or counter record makes this commit fail, and
//! that failure is [`WriteError::Conflict`]. Preconditions are checked against the head read
//! inside the transaction, so a rerun after a conflict sees the winner and returns 412.
//!
//! Each version write also replaces the resource's search index entries (see [`crate::search`]).

use std::sync::{Arc, PoisonError, RwLock};

use health_fhir_search::{Registry, extract};
use jiff::tz::Offset;
use serde_json::Value;

use health_identifiers::{NumberFormat, OpId, ResourceId, SequenceState};
use health_persistence::{
    CounterKey, DerivedKeys, GuardKey, OutboxEntry, OutboxPayload, Precondition, RawJson,
    ResourceType, Store, StoreError, StoredResource, Transaction, VersionMeta, WriteError,
    WriteRequest, stamp_meta,
};
use health_time::{Clock, Timestamp};
use surrealdb::engine::any::Any;
use surrealdb::method::Transaction as SurrealTransaction;
use surrealdb::types::SurrealValue;

use crate::search::sql::Sql;
use crate::search::write::replace_entries;
use crate::{Connection, ownership};

/// The SurrealDB-backed [`Store`].
#[derive(Clone)]
pub struct SurrealStore {
    pub(crate) connection: Connection,
    clock: Arc<dyn Clock>,
    registry: Arc<RwLock<Arc<Registry>>>,
    blocking: Option<Arc<dyn DerivedKeys>>,
}

impl SurrealStore {
    /// A store over a migrated connection, stamping `meta.lastUpdated` from `clock`. It indexes
    /// nothing until a registry is set.
    pub fn new(connection: Connection, clock: Arc<dyn Clock>) -> Self {
        Self {
            connection,
            clock,
            registry: Arc::new(RwLock::new(Arc::new(Registry::new(Offset::UTC)))),
            blocking: None,
        }
    }

    /// This store, writing `keys`' blocking keys with every live version (`idx_block`).
    #[must_use]
    pub fn with_blocking(mut self, keys: Arc<dyn DerivedKeys>) -> Self {
        self.blocking = Some(keys);
        self
    }

    /// Recomputes every block's size into `block_count` (ADR 0055: hourly, at central and on
    /// each edge), in one transaction. Returns the number of blocks.
    pub async fn recount_blocks(&self) -> Result<usize, StoreError> {
        let db = self.connection.db();
        db.query(
            "BEGIN; \
             DELETE block_count; \
             LET $groups = SELECT array::slice(record::id(id), 0, 2) AS key, count() AS n \
                 FROM idx_block GROUP BY key; \
             FOR $g IN $groups { \
                 CREATE type::record('block_count', $g.key) \
                     SET count = $g.n, counted_at = time::now(); \
             }; \
             COMMIT;",
        )
        .await
        .and_then(surrealdb::IndexedResults::check)
        .map_err(unavailable)?;
        let blocks: Option<i64> = db
            .query("RETURN count(SELECT VALUE id FROM block_count)")
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        Ok(usize::try_from(blocks.unwrap_or(0)).unwrap_or(0))
    }

    /// This store, indexing the parameters `registry` supports.
    #[must_use]
    pub fn with_registry(self, registry: Registry) -> Self {
        self.set_registry(registry);
        self
    }

    /// Replaces the registry, e.g. when a configuration package adds parameters (as
    /// `Reindexing`) or a reindex completes (`Active`). Transactions already begun keep the
    /// registry they started with.
    pub fn set_registry(&self, registry: Registry) {
        *self
            .registry
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Arc::new(registry);
    }

    /// The current registry.
    pub fn registry(&self) -> Arc<Registry> {
        Arc::clone(&self.registry.read().unwrap_or_else(PoisonError::into_inner))
    }
}

impl Store for SurrealStore {
    type Tx = SurrealTx;

    async fn begin(&self) -> Result<SurrealTx, StoreError> {
        let tx = self
            .connection
            .db()
            .clone()
            .begin()
            .await
            .map_err(unavailable)?;
        Ok(SurrealTx {
            tx: Some(tx),
            clock: Arc::clone(&self.clock),
            registry: self.registry(),
            blocking: self.blocking.clone(),
        })
    }
}

/// A unit of work over one SurrealDB transaction. Dropping it without committing cancels it.
pub struct SurrealTx {
    tx: Option<SurrealTransaction<Any>>,
    clock: Arc<dyn Clock>,
    registry: Arc<Registry>,
    blocking: Option<Arc<dyn DerivedKeys>>,
}

impl Drop for SurrealTx {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take()
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            runtime.spawn(async move {
                let _ = tx.cancel().await;
            });
        }
    }
}

#[derive(Debug, SurrealValue)]
struct HeadRow {
    json: String,
    version_id: String,
    version_seq: i64,
    last_updated: String,
    deleted: bool,
}

#[derive(Debug, SurrealValue)]
struct CounterRow {
    year: Option<i64>,
    last: i64,
}

pub(crate) fn unavailable(error: surrealdb::Error) -> StoreError {
    StoreError::Unavailable(error.to_string())
}

/// SurrealDB reports a lost snapshot-isolation race as a retryable transaction error.
fn is_conflict(error: &surrealdb::Error) -> bool {
    let message = error.to_string().to_lowercase();
    message.contains("conflict")
        || message.contains("can be retried")
        || message.contains("retryable")
}

pub(crate) fn write_error(error: surrealdb::Error) -> WriteError {
    if is_conflict(&error) {
        WriteError::Conflict
    } else {
        WriteError::Store(unavailable(error))
    }
}

pub(crate) fn table(resource_type: &ResourceType) -> Result<&'static str, StoreError> {
    ownership::resource(resource_type.as_str())
        .map(|r| r.table)
        .ok_or_else(|| StoreError::UnknownResourceType(resource_type.to_string()))
}

impl SurrealTx {
    /// The ids of the Provenance owned by `owner` (its targets' Owner), for sync to carry with
    /// their targets (#196).
    pub async fn provenance_of_owner(&mut self, owner: &str) -> Result<Vec<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [<string>record::id(id)] FROM provenance WHERE owner = $owner AND deleted = false ORDER BY id",
                vec![("owner", owner.to_owned())],
            )
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.into_iter().next())
            .collect())
    }

    /// The Owner of the Provenance `json`: its target's, read in this transaction. A Provenance
    /// whose target the platform does not store, or that is missing, is refused.
    async fn provenance_owner(&self, json: &str) -> Result<String, WriteError> {
        let refused =
            |why: &str| WriteError::Store(StoreError::Unavailable(format!("a Provenance {why}")));
        let record: Value = serde_json::from_str(json).map_err(|_| refused("is not JSON"))?;
        let (target_type, target_id) = crate::ownership::provenance_target(&record)
            .ok_or_else(|| refused("needs a target reference"))?;
        let resource_type: ResourceType = target_type
            .parse()
            .map_err(|_| refused("targets an unknown resource type"))?;
        let id: ResourceId = target_id
            .parse()
            .map_err(|_| refused("targets an invalid id"))?;
        let target = self
            .head(&resource_type, id)
            .await?
            .ok_or_else(|| refused("targets a record that is not stored"))?;
        let target_json: Value = serde_json::from_str(target.json.as_str())
            .map_err(|_| refused("targets a record that is not JSON"))?;
        crate::ownership::provenance_owner(target_type, &target_json)
            .map(str::to_owned)
            .ok_or_else(|| refused("targets a type no Owner holds"))
    }

    pub(crate) fn tx(&self) -> Result<&SurrealTransaction<Any>, StoreError> {
        self.tx
            .as_ref()
            .ok_or_else(|| StoreError::Unavailable("transaction already finished".into()))
    }

    pub(crate) async fn head(
        &self,
        resource_type: &ResourceType,
        id: ResourceId,
    ) -> Result<Option<StoredResource>, StoreError> {
        let table = table(resource_type)?;
        let row: Option<HeadRow> = self
            .tx()?
            .query("SELECT json, version_id, version_seq, <string> last_updated AS last_updated, deleted FROM ONLY type::record($table, $id)")
            .bind(("table", table))
            .bind(("id", id.to_string()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        row.map(|row| {
            let corrupt = |what: &str| {
                StoreError::Unavailable(format!("stored {table} {id} has an invalid {what}"))
            };
            Ok(StoredResource {
                resource_type: resource_type.clone(),
                id,
                version: VersionMeta {
                    version_id: row.version_id.parse().map_err(|_| corrupt("version_id"))?,
                    version_seq: u64::try_from(row.version_seq)
                        .map_err(|_| corrupt("version_seq"))?,
                    last_updated: row
                        .last_updated
                        .parse::<Timestamp>()
                        .map_err(|_| corrupt("last_updated"))?,
                    deleted: row.deleted,
                },
                json: RawJson::from_validated(row.json),
            })
        })
        .transpose()
    }

    async fn put(
        &mut self,
        resource_type: &ResourceType,
        id: ResourceId,
        op_id: OpId,
        json: &RawJson,
        previous_seq: u64,
        deleted: bool,
    ) -> Result<VersionMeta, WriteError> {
        let table = table(resource_type)?;
        let last_updated = self.clock.now();
        let stamped = stamp_meta(json, resource_type, id, op_id, last_updated)?;
        // A Provenance belongs to its target's Owner (resource-ownership.md rule 7), read in
        // this transaction so the target's category decides it.
        let owner = if resource_type.as_str() == "Provenance" {
            Some(self.provenance_owner(stamped.as_str()).await?)
        } else {
            None
        };
        let version_seq = previous_seq + 1;
        let seq = i64::try_from(version_seq)
            .map_err(|_| StoreError::Unavailable("version_seq overflow".into()))?;
        let (extraction, references, blocks) = if deleted {
            // A tombstone keeps its body but is never found by search, references nothing and
            // sits in no block.
            (
                health_fhir_search::Extraction::default(),
                Vec::new(),
                Vec::new(),
            )
        } else {
            let resource: Value = serde_json::from_str(stamped.as_str())
                .map_err(|e| StoreError::Unavailable(format!("stamped JSON: {e}")))?;
            let blocks = self
                .blocking
                .as_ref()
                .map(|b| b.keys(resource_type.as_str(), &resource))
                .unwrap_or_default();
            (
                extract(&self.registry, resource_type.as_str(), &resource, None),
                crate::search::write::references(&resource),
                blocks,
            )
        };
        let mut sql = Sql::default();
        let id_text = id.to_string();
        let keys = replace_entries(
            &mut sql,
            table,
            resource_type.as_str(),
            &id_text,
            &extraction,
            crate::search::write::Derived {
                references: &references,
                blocks: &blocks,
            },
            None,
        );
        let head = sql.str(table);
        let history = sql.str(format!("{table}_history"));
        let rid = sql.str(id_text.as_str());
        let seq = sql.value(crate::search::sql::Bind::Int(seq));
        let json = sql.str(stamped.as_str());
        let version_id = sql.str(op_id.to_string());
        let at = sql.value(crate::search::sql::Bind::Datetime(format!(
            "{last_updated:.3}"
        )));
        let deleted_text = if deleted { "true" } else { "false" };
        let (owner_set, owner_field) = match owner {
            Some(owner) => {
                let owner = sql.str(owner);
                (format!(", owner = {owner}"), format!(", owner: {owner}"))
            }
            None => (String::new(), String::new()),
        };
        sql.push(format!(
            "UPSERT type::record({head}, {rid}) SET json = {json}, version_id = {version_id}, \
             version_seq = {seq}, last_updated = {at}, deleted = {deleted_text}, index_keys = {keys}{owner_set}"
        ));
        sql.push(format!(
            "CREATE type::record({history}, [{rid}, {seq}]) CONTENT {{ json: {json}, \
             version_id: {version_id}, version_seq: {seq}, last_updated: {at}, deleted: {deleted_text}{owner_field} }}"
        ));
        sql.run(self.tx()?).await.map_err(write_error)?;
        Ok(VersionMeta {
            version_id: op_id,
            version_seq,
            last_updated,
            deleted,
        })
    }
}

impl SurrealTx {
    /// Up to `limit` live resources whose current version references `resource_type/id`, not
    /// counting the resource itself or AuditEvents (persistence.md § Reference integrity). A
    /// record-id range over `idx_ref_in`.
    pub async fn referrers(
        &mut self,
        resource_type: &ResourceType,
        id: ResourceId,
        limit: usize,
    ) -> Result<Vec<(ResourceType, ResourceId)>, StoreError> {
        let rows: Vec<Vec<String>> = self
            .tx()?
            .query(format!(
                "SELECT VALUE array::slice(record::id(id), 2) FROM idx_ref_in:[$t, $i]..[$t, $i, {{}}] \
                 WHERE !(record::id(id)[2] = $t AND record::id(id)[3] = $i) \
                 AND record::id(id)[2] != 'AuditEvent' LIMIT {limit}"
            ))
            .bind(("t", resource_type.to_string()))
            .bind(("i", id.to_string()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        rows.into_iter()
            .map(|row| match row.as_slice() {
                [t, i] => Ok((
                    t.parse().map_err(|_| {
                        StoreError::Unavailable("idx_ref_in has an invalid type".into())
                    })?,
                    i.parse().map_err(|_| {
                        StoreError::Unavailable("idx_ref_in has an invalid id".into())
                    })?,
                )),
                _ => Err(StoreError::Unavailable(
                    "idx_ref_in has an invalid key".into(),
                )),
            })
            .collect()
    }

    /// Serializes conditional writes on the same condition (a resource type and its search
    /// parameters, in any order). Call it before a write the condition allows; a concurrent
    /// transaction that locked the same condition makes one of the two commits fail with
    /// [`WriteError::Conflict`]. Nothing is left behind: the lock record is written and removed
    /// in this transaction.
    pub async fn lock_condition(
        &mut self,
        resource_type: &ResourceType,
        condition: &[(String, String)],
    ) -> Result<(), WriteError> {
        use sha2::{Digest, Sha256};
        let mut sorted = condition.to_vec();
        sorted.sort();
        let mut hasher = Sha256::new();
        hasher.update(resource_type.as_str().as_bytes());
        for (name, value) in &sorted {
            hasher.update([0]);
            hasher.update(name.as_bytes());
            hasher.update([1]);
            hasher.update(value.as_bytes());
        }
        let key: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        self.tx()?
            .query(
                "UPSERT type::record('condition_lock', $key) SET at = time::now(); \
                 DELETE type::record('condition_lock', $key);",
            )
            .bind(("key", key))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?;
        Ok(())
    }
}

impl Transaction for SurrealTx {
    async fn read(
        &mut self,
        resource_type: &ResourceType,
        id: ResourceId,
    ) -> Result<Option<StoredResource>, StoreError> {
        self.head(resource_type, id).await
    }

    async fn write(&mut self, request: WriteRequest) -> Result<VersionMeta, WriteError> {
        let head = self.head(&request.resource_type, request.id).await?;
        match (&head, request.precondition) {
            (Some(h), _) if h.version.deleted => return Err(WriteError::Gone),
            (Some(h), Precondition::MustNotExist) => {
                return Err(WriteError::PreconditionFailed {
                    current: Some(h.version.version_id),
                });
            }
            (Some(h), Precondition::IfMatch(expected)) if h.version.version_id != expected => {
                return Err(WriteError::PreconditionFailed {
                    current: Some(h.version.version_id),
                });
            }
            (None, Precondition::IfMatch(_)) => {
                return Err(WriteError::PreconditionFailed { current: None });
            }
            _ => {}
        }
        let previous = head.map_or(0, |h| h.version.version_seq);
        self.put(
            &request.resource_type,
            request.id,
            request.op_id,
            &request.json,
            previous,
            false,
        )
        .await
    }

    async fn delete(
        &mut self,
        resource_type: &ResourceType,
        id: ResourceId,
        op_id: OpId,
        if_match: Option<OpId>,
    ) -> Result<VersionMeta, WriteError> {
        let head = self
            .head(resource_type, id)
            .await?
            .ok_or(WriteError::PreconditionFailed { current: None })?;
        if head.version.deleted {
            return Err(WriteError::Gone);
        }
        if if_match.is_some_and(|expected| expected != head.version.version_id) {
            return Err(WriteError::PreconditionFailed {
                current: Some(head.version.version_id),
            });
        }
        self.put(
            resource_type,
            id,
            op_id,
            &head.json,
            head.version.version_seq,
            true,
        )
        .await
    }

    async fn claim_guard(&mut self, key: GuardKey) -> Result<(), WriteError> {
        let held: Option<bool> = self
            .tx()?
            .query("SELECT VALUE held FROM ONLY type::record('guard', $key); UPSERT type::record('guard', $key) SET held = true;")
            .bind(("key", key.0.clone()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?
            .take(0)
            .map_err(write_error)?;
        if held == Some(true) {
            Err(WriteError::GuardHeld(key))
        } else {
            Ok(())
        }
    }

    async fn release_guard(&mut self, key: GuardKey) -> Result<(), WriteError> {
        self.tx()?
            .query("UPSERT type::record('guard', $key) SET held = false;")
            .bind(("key", key.0))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?;
        Ok(())
    }

    async fn next_sequence(
        &mut self,
        counter: &CounterKey,
        format: &NumberFormat,
        year: i32,
    ) -> Result<u64, WriteError> {
        let row: Option<CounterRow> = self
            .tx()?
            .query("SELECT year, last FROM ONLY type::record('counter', $key)")
            .bind(("key", counter.0.clone()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?
            .take(0)
            .map_err(write_error)?;
        let current = row.map_or_else(SequenceState::default, |r| SequenceState {
            year: r.year.and_then(|y| i32::try_from(y).ok()),
            last: u64::try_from(r.last).unwrap_or(0),
        });
        let (next, value) = current.next(format, year)?;
        let last = i64::try_from(next.last)
            .map_err(|_| WriteError::Counter(health_identifiers::NumberError::Exhausted))?;
        self.tx()?
            .query("UPSERT type::record('counter', $key) SET year = $year, last = $last;")
            .bind(("key", counter.0.clone()))
            .bind(("year", next.year.map(i64::from)))
            .bind(("last", last))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?;
        Ok(value)
    }

    async fn append_outbox(&mut self, entry: OutboxEntry) -> Result<(), WriteError> {
        let (kind, payload) = match &entry.payload {
            OutboxPayload::ResourceVersion {
                resource_type,
                id,
                version_id,
            } => (
                "resource-version".to_owned(),
                format!(
                    r#"{{"resourceType":"{resource_type}","id":"{id}","versionId":"{version_id}"}}"#
                ),
            ),
            OutboxPayload::DomainEvent { name, body } => {
                (format!("event:{name}"), body.as_str().to_owned())
            }
        };
        self.tx()?
            .query("CREATE outbox SET op_id = $op, kind = $kind, payload = $payload, created_at = time::now();")
            .bind(("op", entry.op_id.to_string()))
            .bind(("kind", kind))
            .bind(("payload", payload))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?;
        Ok(())
    }

    async fn commit(mut self) -> Result<(), WriteError> {
        let tx = self
            .tx
            .take()
            .ok_or_else(|| StoreError::Unavailable("transaction already finished".into()))?;
        tx.commit().await.map(|_| ()).map_err(write_error)
    }
}
