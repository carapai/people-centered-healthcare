//! The sync change log, commit counter and outbox (#79; synchronization.md § Change log, outbox
//! and cursors; ADR 0019), migration `0037_change_log`.
//!
//! - **In the write transaction.** Every version a unit of work writes is recorded as it goes.
//!   On commit the unit of work appends one `change_log` entry per version and one `sync_outbox`
//!   row per upstream peer and entry, in the same transaction.
//! - **`seq` is commit order** (ADR 0019 amendment). The unit of work takes the node's commit
//!   lock, reads the last committed `seq` outside its snapshot, numbers its entries after it and
//!   commits before releasing the lock. A reader that has seen `seq` n therefore never later
//!   finds an entry below n commit, and writers queue on the lock instead of conflicting on a
//!   counter record. One process writes each database (edge, desktop and central alike).
//! - **Epoch.** [`new_epoch`] replaces the node's epoch, after a restore from backup or a
//!   re-seed, so a peer holding a cursor from before never misreads a `seq` that went back.

use health_identifiers::{OpId, ResourceId};
use health_persistence::{ResourceType, StoreError, WriteError};
use surrealdb::engine::any::Any;
use surrealdb::method::Transaction as SurrealTransaction;
use surrealdb::types::SurrealValue;

use crate::search::sql::{Bind, Sql};
use crate::store::{unavailable, write_error};
use crate::{Connection, SurrealStore};

/// Where a node's change log stands: its epoch and the last `seq` committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    /// Changes when the node is restored or re-seeded.
    pub epoch: String,
    /// The last `seq` taken; 0 before the first write.
    pub seq: u64,
}

/// One accepted version on the change log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Its place in commit order.
    pub seq: u64,
    /// The local transaction it was committed in; entries sharing it apply together.
    pub tx_id: OpId,
    /// The version's `op_id` (its `meta.versionId`).
    pub op_id: OpId,
    /// The resource's type.
    pub resource_type: ResourceType,
    /// The resource's id.
    pub resource_id: ResourceId,
    /// The version's place in the resource's history.
    pub version_seq: u64,
    /// Whether the version is a tombstone.
    pub deleted: bool,
}

/// A version written in this transaction, waiting for its `seq` at commit.
#[derive(Debug, Clone)]
pub(crate) struct Pending {
    pub(crate) op_id: OpId,
    pub(crate) resource_type: String,
    pub(crate) resource_id: String,
    pub(crate) version_seq: i64,
    pub(crate) deleted: bool,
}

#[derive(Debug, SurrealValue)]
struct ChangeRow {
    seq: i64,
    tx_id: String,
    op_id: String,
    resource_type: String,
    resource_id: String,
    version_seq: i64,
    deleted: bool,
}

/// The last committed `seq`, read outside any transaction's snapshot; 0 before the first write.
/// The unit of work calls it holding the commit lock, so nothing commits in between.
pub(crate) async fn last_seq(connection: &Connection) -> Result<i64, StoreError> {
    let last: Option<i64> = connection
        .db()
        .query("SELECT VALUE record::id(id) FROM change_log ORDER BY id DESC LIMIT 1")
        .await
        .and_then(surrealdb::IndexedResults::check)
        .map_err(unavailable)?
        .take(0)
        .map_err(unavailable)?;
    Ok(last.unwrap_or(0))
}

/// Appends `pending` to the change log and the upstream outboxes inside `tx`, numbered after
/// `last`. Called by the unit of work holding the commit lock, just before it commits.
pub(crate) async fn append(
    tx: &SurrealTransaction<Any>,
    tx_id: OpId,
    last: i64,
    pending: &[Pending],
) -> Result<(), WriteError> {
    let peers: Vec<String> = tx
        .query("SELECT VALUE record::id(id) FROM sync_upstream")
        .await
        .and_then(surrealdb::IndexedResults::check)
        .map_err(write_error)?
        .take(0)
        .map_err(write_error)?;
    let mut sql = Sql::default();
    let tx_text = sql.str(tx_id.to_string());
    for (seq, change) in (last + 1..).zip(pending) {
        let seq = sql.value(Bind::Int(seq));
        let op = sql.str(change.op_id.to_string());
        let resource_type = sql.str(change.resource_type.as_str());
        let resource_id = sql.str(change.resource_id.as_str());
        let version_seq = sql.value(Bind::Int(change.version_seq));
        let deleted = if change.deleted { "true" } else { "false" };
        sql.push(format!(
            "CREATE type::record('change_log', {seq}) CONTENT {{ tx_id: {tx_text}, op_id: {op}, \
             resource_type: {resource_type}, resource_id: {resource_id}, \
             version_seq: {version_seq}, deleted: {deleted}, committed_at: time::now() }}"
        ));
        for peer in &peers {
            let peer = sql.str(peer.as_str());
            sql.push(format!(
                "CREATE type::record('sync_outbox', [{peer}, {seq}]) SET tx_id = {tx_text}"
            ));
        }
    }
    sql.run(tx).await.map_err(write_error)?;
    Ok(())
}

impl SurrealStore {
    /// The node's epoch and last committed `seq`.
    pub async fn cursor(&self) -> Result<Cursor, StoreError> {
        cursor(&self.connection).await
    }

    /// Up to `limit` change log entries after `after`, in `seq` order. `seq`s have no gaps, so
    /// this is the bounded range `(after, after + limit]`, a key-range scan whatever the log's
    /// length.
    pub async fn changes_after(&self, after: u64, limit: usize) -> Result<Vec<Change>, StoreError> {
        let through = after.saturating_add(u64::try_from(limit).unwrap_or(u64::MAX));
        let rows: Vec<ChangeRow> = self
            .connection
            .db()
            .query(format!(
                "SELECT record::id(id) AS seq, tx_id, op_id, resource_type, resource_id, \
                 version_seq, deleted FROM change_log:{after}>..={through}"
            ))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        let mut changes = rows
            .into_iter()
            .map(change)
            .collect::<Result<Vec<_>, _>>()?;
        changes.sort_by_key(|c| c.seq);
        Ok(changes)
    }

    /// Adds an upstream peer: every version committed from now on is queued for it.
    pub async fn add_upstream(&self, peer: &str) -> Result<(), StoreError> {
        self.connection
            .db()
            .query("UPSERT type::record('sync_upstream', $peer) SET added_at = time::now()")
            .bind(("peer", peer.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?;
        Ok(())
    }

    /// Removes an upstream peer and its queue (re-pointing a drained node).
    pub async fn remove_upstream(&self, peer: &str) -> Result<(), StoreError> {
        self.connection
            .db()
            .query(
                "BEGIN; DELETE type::record('sync_upstream', $peer); \
                 DELETE sync_outbox:[$peer]..[$peer, {}]; COMMIT;",
            )
            .bind(("peer", peer.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?;
        Ok(())
    }

    /// Up to `limit` entries queued for `peer` and not yet acknowledged, in `seq` order.
    pub async fn pending_for(&self, peer: &str, limit: usize) -> Result<Vec<Change>, StoreError> {
        let rows: Vec<ChangeRow> = self
            .connection
            .db()
            .query(format!(
                "LET $entries = (SELECT VALUE type::record('change_log', record::id(id)[1]) \
                 FROM sync_outbox:[$peer]..[$peer, {{}}] LIMIT {limit}); \
                 SELECT record::id(id) AS seq, tx_id, op_id, resource_type, resource_id, \
                 version_seq, deleted FROM $entries ORDER BY seq"
            ))
            .bind(("peer", peer.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(1)
            .map_err(unavailable)?;
        rows.into_iter().map(change).collect()
    }

    /// Removes `peer`'s queue up to and including `through`, once the peer has acknowledged it.
    pub async fn acknowledge(&self, peer: &str, through: u64) -> Result<(), StoreError> {
        let through = i64::try_from(through)
            .map_err(|_| StoreError::Unavailable("seq out of range".into()))?;
        self.connection
            .db()
            .query("DELETE sync_outbox:[$peer]..=[$peer, $through]")
            .bind(("peer", peer.to_owned()))
            .bind(("through", through))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?;
        Ok(())
    }
}

async fn cursor(connection: &Connection) -> Result<Cursor, StoreError> {
    let epoch: Option<String> = connection
        .db()
        .query("SELECT VALUE epoch FROM ONLY sync_state:node")
        .await
        .and_then(surrealdb::IndexedResults::check)
        .map_err(unavailable)?
        .take(0)
        .map_err(unavailable)?;
    let epoch =
        epoch.ok_or_else(|| StoreError::Unavailable("sync_state:node is missing".into()))?;
    let seq = last_seq(connection).await?;
    Ok(Cursor {
        epoch,
        seq: u64::try_from(seq)
            .map_err(|_| StoreError::Unavailable("change_log has a negative seq".into()))?,
    })
}

/// Gives the node a new epoch, keeping its `seq`, and returns the new cursor. Run after a
/// restore from backup and after a re-seed (synchronization.md § Change log, outbox and
/// cursors).
pub async fn new_epoch(connection: &Connection) -> Result<Cursor, StoreError> {
    connection
        .db()
        .query("UPDATE sync_state:node SET epoch = <string> rand::uuid::v7()")
        .await
        .and_then(surrealdb::IndexedResults::check)
        .map_err(unavailable)?;
    cursor(connection).await
}

fn change(row: ChangeRow) -> Result<Change, StoreError> {
    let corrupt = |what: &str| StoreError::Unavailable(format!("change_log has an invalid {what}"));
    Ok(Change {
        seq: u64::try_from(row.seq).map_err(|_| corrupt("seq"))?,
        tx_id: row.tx_id.parse().map_err(|_| corrupt("tx_id"))?,
        op_id: row.op_id.parse().map_err(|_| corrupt("op_id"))?,
        resource_type: row
            .resource_type
            .parse()
            .map_err(|_| corrupt("resource_type"))?,
        resource_id: row
            .resource_id
            .parse()
            .map_err(|_| corrupt("resource_id"))?,
        version_seq: u64::try_from(row.version_seq).map_err(|_| corrupt("version_seq"))?,
        deleted: row.deleted,
    })
}
