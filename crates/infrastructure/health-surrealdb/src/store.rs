//! The `health-persistence` unit of work on SurrealDB interactive transactions
//! (persistence.md § The write transaction).
//!
//! Every read and write runs inside one SurrealDB transaction, which gives snapshot isolation:
//! a concurrent commit to the same head, guard or counter record makes this commit fail, and
//! that failure is [`WriteError::Conflict`]. Preconditions are checked against the head read
//! inside the transaction, so a rerun after a conflict sees the winner and returns 412.

use std::sync::Arc;

use health_identifiers::{NumberFormat, OpId, ResourceId, SequenceState};
use health_persistence::{
    CounterKey, GuardKey, OutboxEntry, OutboxPayload, Precondition, RawJson, ResourceType, Store,
    StoreError, StoredResource, Transaction, VersionMeta, WriteError, WriteRequest, stamp_meta,
};
use health_time::{Clock, Timestamp};
use surrealdb::engine::any::Any;
use surrealdb::method::Transaction as SurrealTransaction;
use surrealdb::types::SurrealValue;

use crate::{Connection, ownership};

/// The SurrealDB-backed [`Store`].
#[derive(Clone)]
pub struct SurrealStore {
    connection: Connection,
    clock: Arc<dyn Clock>,
}

impl SurrealStore {
    /// A store over a migrated connection, stamping `meta.lastUpdated` from `clock`.
    pub fn new(connection: Connection, clock: Arc<dyn Clock>) -> Self {
        Self { connection, clock }
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
        })
    }
}

/// A unit of work over one SurrealDB transaction. Dropping it without committing cancels it.
pub struct SurrealTx {
    tx: Option<SurrealTransaction<Any>>,
    clock: Arc<dyn Clock>,
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

fn unavailable(error: surrealdb::Error) -> StoreError {
    StoreError::Unavailable(error.to_string())
}

/// SurrealDB reports a lost snapshot-isolation race as a retryable transaction error.
fn is_conflict(error: &surrealdb::Error) -> bool {
    let message = error.to_string().to_lowercase();
    message.contains("conflict")
        || message.contains("can be retried")
        || message.contains("retryable")
}

fn write_error(error: surrealdb::Error) -> WriteError {
    if is_conflict(&error) {
        WriteError::Conflict
    } else {
        WriteError::Store(unavailable(error))
    }
}

fn table(resource_type: &ResourceType) -> Result<&'static str, StoreError> {
    ownership::resource(resource_type.as_str())
        .map(|r| r.table)
        .ok_or_else(|| StoreError::UnknownResourceType(resource_type.to_string()))
}

impl SurrealTx {
    fn tx(&self) -> Result<&SurrealTransaction<Any>, StoreError> {
        self.tx
            .as_ref()
            .ok_or_else(|| StoreError::Unavailable("transaction already finished".into()))
    }

    async fn head(
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
        let version_seq = previous_seq + 1;
        let seq = i64::try_from(version_seq)
            .map_err(|_| StoreError::Unavailable("version_seq overflow".into()))?;
        self.tx()?
            .query(
                "LET $row = { json: $json, version_id: $version_id, version_seq: $seq, last_updated: <datetime> $at, deleted: $deleted };
                 UPSERT type::record($table, $id) CONTENT $row;
                 CREATE type::record($history, [$id, $seq]) CONTENT $row;",
            )
            .bind(("table", table))
            .bind(("history", format!("{table}_history")))
            .bind(("id", id.to_string()))
            .bind(("seq", seq))
            .bind(("json", stamped.as_str().to_owned()))
            .bind(("version_id", op_id.to_string()))
            .bind(("at", format!("{last_updated:.3}")))
            .bind(("deleted", deleted))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?;
        Ok(VersionMeta {
            version_id: op_id,
            version_seq,
            last_updated,
            deleted,
        })
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
