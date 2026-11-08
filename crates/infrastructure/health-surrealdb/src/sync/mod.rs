//! Sync over the store (#81; synchronization.md), migration `0038_sync_apply`: the
//! `health-sync-protocol` ports implemented over the change log, history and heads.
//!
//! - **Sending.** [`SurrealStore::outbox`] reads whole transactions queued for an upstream and
//!   builds their [`Item`]s from history; [`SurrealStore::record_push`] acknowledges what the
//!   upstream answered and keeps its conflicts and rejections as unaccepted candidates.
//! - **Receiving a push.** [`SurrealAuthority`] is the [`Authority`]: per group it checks the Sync
//!   Scope (what the peer may send up), fast-forwards versions that build on the head, opens a Conflict Record for any that
//!   do not, and writes a receipt per item, all in one transaction.
//! - **Serving a pull.** [`SurrealStore::pull`] reads whole transactions after a cursor, in the
//!   peer's Sync Scope, leaving out what the peer sent itself.
//! - **Applying a pull.** [`SurrealReplica`] is the [`Replica`]: it applies a group and advances
//!   the checkpoint in one transaction, and never overwrites an unaccepted local candidate.
//!
//! A version received by sync keeps its `op_id` and the `lastUpdated` its origin stamped, is
//! logged with its origin node, and is never queued back toward the peer it came from.

use health_access_policy::{FACILITY_TAG_SYSTEM, is_facility_scope};
use health_identifiers::{NodeId, OpId, ResourceId};
use health_persistence::{RawJson, ResourceType, Store, StoreError, Transaction, WriteError};
use health_sync_protocol::{
    ApplyDecision, Authority, ContentHash, Cursor, Group, Hello, Hlc, Item, Kind, Local, Outcome,
    PullRequest, PullResponse, Pulled, Reason, Receipt, Replica, Verdict, group as group_items,
};
use health_time::Timestamp;
use serde_json::{Value, json};
use surrealdb::types::SurrealValue;

use crate::SurrealStore;
use crate::change_log::{Change, Source};
use crate::search::sql::{Bind, Sql};
use crate::store::{SurrealTx, table, unavailable, write_error};

/// Which records a peer may send and receive (synchronization.md § Pull and Sync Scope;
/// write-authority.md § Data authority matrix; ADR 0057).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncScope {
    /// A Facility Edge Node or standalone desktop. It sends up its facility's records and the
    /// global records a facility may create ([`UP_GLOBAL`]). It receives its facility's records
    /// and the global replicas ([`DOWN_GLOBAL`]). Nothing else crosses: another facility's records
    /// never, and central's own audit never.
    Facility(String),
    /// Every record, both ways.
    All,
}

/// The global (not facility-scope) types a facility creates and sends up, sorted: provisional
/// Patients and their relations, captured Consents, audit, provenance, its own Locations and
/// devices, and requests for central to act on.
pub const UP_GLOBAL: &[&str] = &[
    "AuditEvent",
    "CommunicationRequest",
    "Consent",
    "CoverageEligibilityRequest",
    "Device",
    "Location",
    "Patient",
    "Person",
    "Provenance",
    "RelatedPerson",
];

/// The global types central sends down as replicas, sorted: registries and golden records,
/// definitions and terminology, price masters, coverage and consent.
pub const DOWN_GLOBAL: &[&str] = &[
    "ActivityDefinition",
    "ChargeItemDefinition",
    "CodeSystem",
    "ConceptMap",
    "Consent",
    "Coverage",
    "Device",
    "HealthcareService",
    "InventoryItem",
    "Library",
    "Location",
    "Medication",
    "Organization",
    "Patient",
    "Person",
    "PlanDefinition",
    "Practitioner",
    "PractitionerRole",
    "Questionnaire",
    "RelatedPerson",
    "SearchParameter",
    "StructureDefinition",
    "ValueSet",
];

impl SyncScope {
    /// Whether the peer may send up a version of `resource_type` with this body.
    pub fn admits_up(&self, resource_type: &str, body: &Value) -> bool {
        self.admits(resource_type, body, UP_GLOBAL)
    }

    /// Whether the peer is sent a version of `resource_type` with this body.
    pub fn admits_down(&self, resource_type: &str, body: &Value) -> bool {
        self.admits(resource_type, body, DOWN_GLOBAL)
    }

    fn admits(&self, resource_type: &str, body: &Value, global: &[&str]) -> bool {
        match self {
            SyncScope::All => true,
            SyncScope::Facility(facility) if is_facility_scope(resource_type) => {
                facility_of(body).is_some_and(|tagged| tagged == facility)
            }
            SyncScope::Facility(_) => global.binary_search(&resource_type).is_ok(),
        }
    }
}

/// A record's facility tag (ADR 0057).
fn facility_of(body: &Value) -> Option<&str> {
    body["meta"]["tag"]
        .as_array()?
        .iter()
        .find(|tag| tag["system"].as_str() == Some(FACILITY_TAG_SYSTEM))?["code"]
        .as_str()
}

/// A queued transaction for an upstream, with the last `seq` it covers.
#[derive(Debug, Clone, PartialEq)]
pub struct Outgoing {
    /// The transaction's items.
    pub group: Group,
    /// Its last `seq` in this node's log; acknowledging it removes the group from the queue.
    pub through: u64,
}

#[derive(Debug, SurrealValue)]
struct VersionRow {
    json: String,
    version_id: String,
    last_updated: String,
}

#[derive(Debug, SurrealValue)]
struct ReceiptRow {
    op_id: String,
    content_hash: String,
    outcome: String,
}

fn corrupt(what: &str) -> StoreError {
    StoreError::Unavailable(format!("sync: invalid {what}"))
}

impl SurrealStore {
    /// One stored version of a resource, by its place in history.
    async fn stored_version(
        &self,
        resource_type: &ResourceType,
        id: &str,
        version_seq: u64,
    ) -> Result<Option<VersionRow>, StoreError> {
        let table = table(resource_type)?;
        let seq = i64::try_from(version_seq).map_err(|_| corrupt("version_seq"))?;
        let row: Option<VersionRow> = self
            .connection
            .db()
            .query(format!(
                "SELECT json, version_id, <string> last_updated AS last_updated \
                 FROM ONLY type::record('{table}_history', [$id, $seq])"
            ))
            .bind(("id", id.to_owned()))
            .bind(("seq", seq))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        Ok(row)
    }

    /// The envelope of a logged version: its body from history, its parent the version before
    /// it here, its origin `node` unless another node committed it first.
    async fn item(&self, change: &Change, node: NodeId) -> Result<Item, StoreError> {
        let id = change.resource_id.to_string();
        let row = self
            .stored_version(&change.resource_type, &id, change.version_seq)
            .await?
            .ok_or_else(|| corrupt("change log entry without its version"))?;
        let parent_version_id = if change.version_seq > 1 {
            let parent = self
                .stored_version(&change.resource_type, &id, change.version_seq - 1)
                .await?
                .ok_or_else(|| corrupt("history without its parent version"))?;
            Some(
                parent
                    .version_id
                    .parse()
                    .map_err(|_| corrupt("version_id"))?,
            )
        } else {
            None
        };
        let body: Value = serde_json::from_str(&row.json).map_err(|_| corrupt("json"))?;
        let last_updated: Timestamp = row
            .last_updated
            .parse()
            .map_err(|_| corrupt("last_updated"))?;
        let origin_node = match &change.origin {
            Some(origin) => origin.parse().map_err(|_| corrupt("origin"))?,
            None => node,
        };
        Ok(Item {
            kind: Kind::Version,
            op_id: change.op_id,
            tx_id: change.tx_id,
            resource_type: change.resource_type.to_string(),
            resource_id: id,
            parent_version_id,
            origin_node,
            device_id: None,
            author: None,
            hlc: Hlc {
                wall_ms: last_updated.as_millisecond(),
                logical: 0,
            },
            content_hash: ContentHash::of(&body),
            packages: Vec::new(),
            snapshot_version: 0,
            provisional: false,
            deleted: change.deleted,
            blobs: Vec::new(),
            body,
        })
    }

    /// Whole transactions queued for `peer`, at least `limit` items unless fewer are queued, as
    /// this node (`node`) sends them.
    pub async fn outbox(
        &self,
        peer: &str,
        node: NodeId,
        limit: usize,
    ) -> Result<Vec<Outgoing>, StoreError> {
        let mut want = limit.max(1);
        let changes = loop {
            let changes = self.pending_for(peer, want).await?;
            // The last transaction may continue past the limit: read on until it is whole, or
            // drop it when an earlier one makes a batch already.
            if changes.len() < want {
                break changes;
            }
            let last_tx = changes.last().map(|c| c.tx_id);
            let first_of_last = changes.iter().position(|c| Some(c.tx_id) == last_tx);
            match first_of_last {
                Some(0) | None => want = want.saturating_mul(2),
                Some(cut) => break changes[..cut].to_vec(),
            }
        };
        let mut seqs = Vec::with_capacity(changes.len());
        let mut items = Vec::with_capacity(changes.len());
        for change in &changes {
            seqs.push((change.tx_id, change.seq));
            items.push(self.item(change, node).await?);
        }
        let groups = group_items(items).map_err(|e| StoreError::Unavailable(e.to_string()))?;
        Ok(groups
            .into_iter()
            .map(|group| {
                let through = seqs
                    .iter()
                    .filter(|(tx, _)| *tx == group.tx_id)
                    .map(|(_, seq)| *seq)
                    .max()
                    .unwrap_or(0);
                Outgoing { group, through }
            })
            .collect())
    }

    /// Records the upstream's answers to a push: everything answered is acknowledged; a group
    /// in conflict or rejected stays an unaccepted candidate (synchronization.md § Item states).
    /// `answers` are in the order `sent` was pushed; groups after the last answer stay queued.
    pub async fn record_push(
        &self,
        peer: &str,
        sent: &[Outgoing],
        answers: &[Outcome],
    ) -> Result<(), StoreError> {
        let mut sql = Sql::default();
        let mut through = None;
        for (outgoing, outcome) in sent.iter().zip(answers) {
            through = Some(outgoing.through);
            let state = match outcome {
                Outcome::Accepted => continue,
                Outcome::Conflict { .. } => "conflict",
                Outcome::Rejected { .. } => "rejected",
            };
            let outcome = sql.str(serde_json::to_string(outcome).map_err(|_| corrupt("outcome"))?);
            for item in &outgoing.group.items {
                let op = sql.str(item.op_id.to_string());
                let resource_type = sql.str(item.resource_type.as_str());
                let resource_id = sql.str(item.resource_id.as_str());
                sql.push(format!(
                    "UPSERT type::record('sync_item_state', {op}) SET resource_type = \
                     {resource_type}, resource_id = {resource_id}, state = '{state}', \
                     outcome = {outcome}"
                ));
            }
        }
        let Some(through) = through else {
            return Ok(());
        };
        let peer_text = sql.str(peer);
        let through = sql.value(Bind::Int(
            i64::try_from(through).map_err(|_| corrupt("seq"))?,
        ));
        sql.push(format!(
            "DELETE sync_outbox:[{peer_text}]..=[{peer_text}, {through}]"
        ));
        let tx = self.begin().await?;
        sql.run(tx.tx()?).await.map_err(unavailable)?;
        tx.commit().await.map_err(|e| match e {
            WriteError::Store(e) => e,
            other => StoreError::Unavailable(other.to_string()),
        })
    }

    /// Whole transactions after `request.cursor` in `scope`, as this node (`node`) serves them to
    /// `peer`, leaving out what `peer` committed itself. A cursor from another node or epoch is
    /// answered with [`PullResponse::ReseedRequired`].
    pub async fn pull(
        &self,
        scope: &SyncScope,
        node: NodeId,
        peer: NodeId,
        request: &PullRequest,
    ) -> Result<PullResponse, StoreError> {
        let current = self.cursor().await?;
        let after = match &request.cursor {
            Some(cursor) if cursor.node != node || cursor.epoch != current.epoch => {
                return Ok(PullResponse::ReseedRequired);
            }
            Some(cursor) => cursor.seq,
            None => 0,
        };
        // Below the horizon the log has gaps: the peer re-seeds (synchronization.md § Tombstones,
        // retention and long outages).
        if after < self.pruned_through().await? {
            return Ok(PullResponse::ReseedRequired);
        }
        let limit = usize::try_from(request.limit.max(1)).unwrap_or(usize::MAX);
        let mut changes = self.changes_after(after, limit).await?;
        // Read on until the last transaction is whole.
        while changes.len() >= limit {
            let Some(last) = changes.last().cloned() else {
                break;
            };
            let next = self.changes_after(last.seq, 64).await?;
            let more: Vec<Change> = next
                .into_iter()
                .take_while(|c| c.tx_id == last.tx_id)
                .collect();
            let whole = more.len() < 64;
            changes.extend(more);
            if whole {
                break;
            }
        }
        let read_through = changes.last().map_or(after, |c| c.seq);
        let more = read_through < current.seq;
        let peer_text = peer.to_string();
        let mut pulled = Vec::with_capacity(changes.len());
        for change in &changes {
            if change.origin.as_deref() == Some(peer_text.as_str()) {
                continue;
            }
            let item = self.item(change, node).await?;
            if scope.admits_down(&item.resource_type, &item.body) {
                pulled.push(Pulled {
                    seq: change.seq,
                    item,
                });
            }
        }
        Ok(PullResponse::Items {
            items: pulled,
            cursor: Cursor {
                node,
                epoch: current.epoch,
                seq: read_through,
            },
            more,
        })
    }

    /// The cursor into `peer`'s log this node has applied, if any.
    pub async fn checkpoint(&self, peer: NodeId) -> Result<Option<Cursor>, StoreError> {
        #[derive(SurrealValue)]
        struct Row {
            epoch: String,
            seq: i64,
        }
        let row: Option<Row> = self
            .connection
            .db()
            .query("SELECT epoch, seq FROM ONLY type::record('sync_checkpoint', $peer)")
            .bind(("peer", peer.to_string()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        row.map(|row| {
            Ok(Cursor {
                node: peer,
                epoch: row.epoch,
                seq: u64::try_from(row.seq).map_err(|_| corrupt("checkpoint"))?,
            })
        })
        .transpose()
    }

    /// The highest `seq` pruned from the change log; 0 if none.
    pub async fn pruned_through(&self) -> Result<u64, StoreError> {
        let pruned: Option<i64> = self
            .connection
            .db()
            .query("SELECT VALUE pruned_through FROM ONLY sync_horizon:node")
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        Ok(pruned.and_then(|p| u64::try_from(p).ok()).unwrap_or(0))
    }

    /// Prunes the change log and receipts committed before `before` (the horizon: at least 180
    /// days back), except log entries an upstream's outbox still needs, and records how far the
    /// log was pruned. Returns the new horizon.
    pub async fn prune_log(&self, before: Timestamp) -> Result<u64, StoreError> {
        let at = format!("d'{before}'");
        let through: Option<i64> = self
            .connection
            .db()
            .query(format!(
                "BEGIN; \
                 LET $pinned = (SELECT VALUE record::id(id)[1] FROM sync_outbox); \
                 LET $through = (SELECT VALUE record::id(id) FROM change_log \
                                 WHERE committed_at < {at} ORDER BY id DESC LIMIT 1)[0]; \
                 DELETE change_log WHERE committed_at < {at} AND record::id(id) NOTINSIDE $pinned; \
                 DELETE sync_receipt WHERE received_at < {at}; \
                 IF $through != NONE AND $through > (sync_horizon:node.pruned_through ?? 0) {{ \
                   UPSERT sync_horizon:node SET pruned_through = $through; \
                 }}; \
                 RETURN sync_horizon:node.pruned_through ?? 0; \
                 COMMIT;"
            ))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(6)
            .map_err(unavailable)?;
        Ok(through.and_then(|t| u64::try_from(t).ok()).unwrap_or(0))
    }

    /// The heads in `scope`, tombstones included, with this node's cursor at that instant, read
    /// in one transaction: what a new, re-pointed or too-stale `peer` is seeded from
    /// (synchronization.md § Seeding). Search indexes are never shipped; the node rebuilds them as
    /// it applies the heads.
    pub async fn snapshot(
        &self,
        scope: &SyncScope,
        node: NodeId,
    ) -> Result<(Vec<Item>, Cursor), StoreError> {
        #[derive(SurrealValue)]
        struct HeadRow {
            rid: String,
            json: String,
            version_id: String,
            deleted: bool,
        }
        let resources = crate::ownership::resources();
        let mut sql = Sql::default();
        let mut statements = Vec::with_capacity(resources.len());
        for resource in resources {
            statements.push((
                resource.resource_type,
                sql.push(format!(
                    "SELECT record::id(id) AS rid, json, version_id, deleted FROM {}",
                    resource.table
                )),
            ));
        }
        let epoch = sql.push("SELECT VALUE epoch FROM ONLY sync_state:node");
        let last = sql.push(crate::change_log::LAST_SEQ);
        let tx = self.begin().await?;
        let mut results = sql.run(tx.tx()?).await.map_err(unavailable)?;
        drop(tx);
        let epoch: Option<String> = results.take(epoch).map_err(unavailable)?;
        let last: Option<i64> = results.take(last).map_err(unavailable)?;
        let mut items = Vec::new();
        for (resource_type, statement) in statements {
            let rows: Vec<HeadRow> = results.take(statement).map_err(unavailable)?;
            for row in rows {
                let body: Value = serde_json::from_str(&row.json).map_err(|_| corrupt("json"))?;
                if !scope.admits_down(resource_type, &body) {
                    continue;
                }
                let op_id: OpId = row.version_id.parse().map_err(|_| corrupt("version_id"))?;
                let wall_ms = body["meta"]["lastUpdated"]
                    .as_str()
                    .and_then(|at| at.parse::<Timestamp>().ok())
                    .map_or(0, |at| at.as_millisecond());
                items.push(Item {
                    kind: Kind::Version,
                    op_id,
                    tx_id: op_id,
                    resource_type: resource_type.to_owned(),
                    resource_id: row.rid,
                    parent_version_id: None,
                    origin_node: node,
                    device_id: None,
                    author: None,
                    hlc: Hlc {
                        wall_ms,
                        logical: 0,
                    },
                    content_hash: ContentHash::of(&body),
                    packages: Vec::new(),
                    snapshot_version: 0,
                    provisional: false,
                    deleted: row.deleted,
                    blobs: Vec::new(),
                    body,
                });
            }
        }
        Ok((
            items,
            Cursor {
                node,
                epoch: epoch.ok_or_else(|| corrupt("epoch"))?,
                seq: last.and_then(|l| u64::try_from(l).ok()).unwrap_or(0),
            },
        ))
    }

    /// Seeds this node from `upstream`'s snapshot: each head not already here becomes the local
    /// head, applied in batches without being queued back; then the checkpoint moves to the
    /// snapshot's cursor and this node takes a new epoch, so its own downstream peers re-seed in
    /// turn. Local candidates still in the outbox stay queued and are pushed against the new heads.
    pub async fn seed(
        &self,
        upstream: NodeId,
        items: &[Item],
        cursor: &Cursor,
    ) -> Result<usize, WriteError> {
        let replica = SurrealReplica::new(self, upstream);
        let mut applied = 0;
        for chunk in items.chunks(200) {
            let mut fresh = Vec::new();
            for item in chunk {
                if !replica.local(item).await?.has_item {
                    fresh.push(item);
                }
            }
            if fresh.is_empty() {
                continue;
            }
            let mut tx = self.begin().await?;
            tx.received_from(Source {
                peer: upstream.to_string(),
                origin: upstream.to_string(),
            });
            for item in &fresh {
                tx.apply_version(item).await?;
            }
            tx.commit().await?;
            applied += fresh.len();
        }
        replica.advance(cursor).await?;
        crate::change_log::new_epoch(&self.connection).await?;
        Ok(applied)
    }

    /// This node's own id on the sync protocol, for a node that has no provisioned one
    /// (central). Kept with the store, so a restore keeps it.
    pub async fn node_id(&self) -> Result<NodeId, StoreError> {
        let node: Option<String> = self
            .connection
            .db()
            .query("SELECT VALUE node FROM ONLY sync_state:node")
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        node.ok_or_else(|| corrupt("node id"))?
            .parse()
            .map_err(|_| corrupt("node id"))
    }

    /// Records `peer`'s handshake and the verdict on it.
    pub async fn record_peer(
        &self,
        peer: NodeId,
        hello: &Hello,
        verdict: &Verdict,
    ) -> Result<(), StoreError> {
        self.connection
            .db()
            .query(
                "UPSERT type::record('sync_peer', $peer) SET hello = $hello, verdict = $verdict, \
                 seen_at = time::now()",
            )
            .bind(("peer", peer.to_string()))
            .bind((
                "hello",
                serde_json::to_string(hello).map_err(|_| corrupt("hello"))?,
            ))
            .bind((
                "verdict",
                serde_json::to_string(verdict).map_err(|_| corrupt("verdict"))?,
            ))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?;
        Ok(())
    }

    /// The verdict on `peer`'s last handshake, if it has made one.
    pub async fn peer_verdict(&self, peer: NodeId) -> Result<Option<Verdict>, StoreError> {
        let verdict: Option<String> = self
            .connection
            .db()
            .query("SELECT VALUE verdict FROM ONLY type::record('sync_peer', $peer)")
            .bind(("peer", peer.to_string()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        verdict
            .map(|v| serde_json::from_str(&v).map_err(|_| corrupt("verdict")))
            .transpose()
    }

    /// Holds items an out-of-range `peer` pushed, for review (synchronization.md § Handshake and
    /// compatibility). A resent item replaces its earlier copy.
    pub async fn quarantine(&self, peer: NodeId, items: &[Item]) -> Result<(), StoreError> {
        let mut sql = Sql::default();
        let peer = sql.str(peer.to_string());
        for item in items {
            let op = sql.str(item.op_id.to_string());
            let text = sql.str(serde_json::to_string(item).map_err(|_| corrupt("item"))?);
            sql.push(format!(
                "UPSERT type::record('sync_quarantine', {op}) SET peer = {peer}, item = {text}, \
                 received_at = time::now()"
            ));
        }
        let tx = self.begin().await?;
        sql.run(tx.tx()?).await.map_err(unavailable)?;
        tx.commit().await.map_err(|e| match e {
            WriteError::Store(e) => e,
            other => StoreError::Unavailable(other.to_string()),
        })
    }

    /// The `op_id`s held in quarantine.
    pub async fn quarantined(&self) -> Result<Vec<String>, StoreError> {
        self.connection
            .db()
            .query("SELECT VALUE record::id(id) FROM sync_quarantine")
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)
    }

    /// The open Conflict Records, oldest first; with `opened_before`, only those opened before
    /// then (the review queue's ageing alert, synchronization.md § Conflict Record and review
    /// queue).
    pub async fn open_conflicts(
        &self,
        opened_before: Option<Timestamp>,
    ) -> Result<Vec<ConflictRecord>, StoreError> {
        let before = opened_before.map_or_else(
            || "d'9999-12-31T00:00:00Z'".to_owned(),
            |at| format!("d'{at}'"),
        );
        let rows: Vec<ConflictRow> = self
            .connection
            .db()
            .query(format!(
                "SELECT {CONFLICT_COLUMNS} FROM sync_conflict \
                 WHERE status = 'open' AND opened_at < {before} ORDER BY opened_at"
            ))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        rows.into_iter().map(ConflictRecord::from_row).collect()
    }

    /// One Conflict Record, by the `tx_id` of the group it was opened for.
    pub async fn conflict(&self, id: &str) -> Result<Option<ConflictRecord>, StoreError> {
        let row: Option<ConflictRow> = self
            .connection
            .db()
            .query(format!(
                "SELECT {CONFLICT_COLUMNS} FROM ONLY type::record('sync_conflict', $id)"
            ))
            .bind(("id", id.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        row.map(ConflictRecord::from_row).transpose()
    }

    /// Resolves an open Conflict Record as `resolver` decided, in one transaction with `audit`,
    /// the decision's AuditEvent: a new version written on top of the head, which then syncs like
    /// any other, or the head kept as it is. Returns the version the record is resolved to.
    pub async fn resolve_conflict(
        &self,
        id: &str,
        resolver: &str,
        resolution: Resolution,
        audit: Option<&Value>,
    ) -> Result<OpId, WriteError> {
        let record = self
            .conflict(id)
            .await?
            .filter(|record| record.status == "open")
            .ok_or_else(|| {
                WriteError::Store(StoreError::Unavailable(format!(
                    "no open Conflict Record {id}"
                )))
            })?;
        let mut tx = self.begin().await?;
        let version = match resolution {
            Resolution::KeepHead {
                resource_type,
                id: resource,
            } => tx
                .head(&resource_type, resource)
                .await?
                .map(|head| head.version.version_id)
                .ok_or_else(|| WriteError::Store(corrupt("a conflict without a head")))?,
            Resolution::Write {
                resource_type,
                id: resource,
                json,
            } => {
                let head = tx
                    .head(&resource_type, resource)
                    .await?
                    .map(|h| h.version.version_id);
                let op_id = OpId::new();
                tx.write(health_persistence::WriteRequest {
                    resource_type,
                    id: resource,
                    json,
                    op_id,
                    precondition: head.map_or(
                        health_persistence::Precondition::MustNotExist,
                        health_persistence::Precondition::IfMatch,
                    ),
                })
                .await?;
                op_id
            }
        };
        let mut sql = Sql::default();
        let key = sql.str(record.id.as_str());
        let by = sql.str(resolver);
        let resolved = sql.str(version.to_string());
        sql.push(format!(
            "UPDATE type::record('sync_conflict', {key}) SET status = 'resolved', \
             resolved_by = {by}, resolved_at = time::now(), resolution = {resolved} \
             WHERE status = 'open'"
        ));
        sql.run(tx.tx()?).await.map_err(write_error)?;
        if let Some(event) = audit {
            health_audit::append(&mut tx, event).await?;
        }
        tx.commit().await?;
        Ok(version)
    }

    /// The Conflict Records opened for pushes, as `(tx_id, status)`.
    pub async fn conflicts(&self) -> Result<Vec<(String, String)>, StoreError> {
        #[derive(SurrealValue)]
        struct Row {
            id: String,
            status: String,
        }
        let rows: Vec<Row> = self
            .connection
            .db()
            .query("SELECT record::id(id) AS id, status FROM sync_conflict")
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        Ok(rows.into_iter().map(|r| (r.id, r.status)).collect())
    }
}

impl SurrealTx {
    /// Writes a version received by sync as the resource's new head, keeping its `op_id` and
    /// the `lastUpdated` its origin stamped.
    async fn apply_version(&mut self, item: &Item) -> Result<(), WriteError> {
        let resource_type: ResourceType = item
            .resource_type
            .parse()
            .map_err(|_| WriteError::Store(corrupt("resource_type")))?;
        let id: ResourceId = item
            .resource_id
            .parse()
            .map_err(|_| WriteError::Store(corrupt("resource_id")))?;
        let head = self.head(&resource_type, id).await?;
        let previous = head.map_or(0, |h| h.version.version_seq);
        let last_updated = item.body["meta"]["lastUpdated"]
            .as_str()
            .and_then(|at| at.parse::<Timestamp>().ok());
        let body =
            serde_json::to_string(&item.body).map_err(|_| WriteError::Store(corrupt("body")))?;
        self.put(
            &resource_type,
            id,
            item.op_id,
            &RawJson::from_validated(body),
            previous,
            item.deleted,
            last_updated,
        )
        .await
        .map(|_| ())
    }
}

/// The receiving side of a push from one peer: central for an edge, an edge for its desktops.
pub struct SurrealAuthority<'a> {
    store: &'a SurrealStore,
    peer: NodeId,
    scope: SyncScope,
}

impl<'a> SurrealAuthority<'a> {
    /// Receives pushes from `peer`, whose Sync Scope is `scope`.
    pub fn new(store: &'a SurrealStore, peer: NodeId, scope: SyncScope) -> Self {
        Self { store, peer, scope }
    }
}

/// How a pushed version stands against the receiver's head.
enum Standing {
    /// Already here, or the same content as the head: nothing to write.
    Present,
    /// Builds on the head.
    FastForward,
    /// Builds on another version: `head` is the receiver's.
    Conflict { head: Option<OpId> },
}

impl Authority for SurrealAuthority<'_> {
    type Error = WriteError;

    async fn receipts(&self, op_ids: &[OpId]) -> Result<Vec<Receipt>, WriteError> {
        if op_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut sql = Sql::default();
        let targets: Vec<String> = op_ids
            .iter()
            .map(|op| {
                let op = sql.str(op.to_string());
                format!("type::record('sync_receipt', {op})")
            })
            .collect();
        let statement = sql.push(format!(
            "SELECT record::id(id) AS op_id, content_hash, outcome FROM {}",
            targets.join(", ")
        ));
        let tx = self.store.begin().await?;
        let rows: Vec<ReceiptRow> = sql
            .run(tx.tx()?)
            .await
            .map_err(write_error)?
            .take(statement)
            .map_err(write_error)?;
        drop(tx);
        rows.into_iter()
            .map(|row| {
                Ok(Receipt {
                    op_id: row.op_id.parse().map_err(|_| corrupt("receipt op_id"))?,
                    content_hash: row
                        .content_hash
                        .parse()
                        .map_err(|_| corrupt("receipt hash"))?,
                    outcome: serde_json::from_str(&row.outcome)
                        .map_err(|_| corrupt("receipt outcome"))?,
                })
            })
            .collect::<Result<_, StoreError>>()
            .map_err(WriteError::Store)
    }

    async fn apply(&self, group: &Group) -> Result<Outcome, WriteError> {
        let mut tx = self.store.begin().await?;
        let origin = group.items.first().map_or_else(
            || self.peer.to_string(),
            |item| item.origin_node.to_string(),
        );
        tx.received_from(Source {
            peer: self.peer.to_string(),
            origin,
        });
        let outcome = self.judge(&mut tx, group).await?;
        if outcome == Outcome::Accepted {
            for item in &group.items {
                if matches!(self.standing(&mut tx, item).await?, Standing::FastForward) {
                    tx.apply_version(item).await?;
                }
            }
        }
        let mut sql = Sql::default();
        let outcome_text = sql.str(
            serde_json::to_string(&outcome).map_err(|_| WriteError::Store(corrupt("outcome")))?,
        );
        for item in &group.items {
            let op = sql.str(item.op_id.to_string());
            let hash = sql.str(item.content_hash.to_string());
            sql.push(format!(
                "CREATE type::record('sync_receipt', {op}) SET content_hash = {hash}, \
                 outcome = {outcome_text}, received_at = time::now()"
            ));
        }
        sql.run(tx.tx()?).await.map_err(write_error)?;
        tx.commit().await?;
        Ok(outcome)
    }
}

impl SurrealAuthority<'_> {
    /// The group's outcome: rejected when out of scope or not a version, a conflict when any
    /// version does not build on the head (recorded as a Conflict Record), accepted otherwise.
    async fn judge(&self, tx: &mut SurrealTx, group: &Group) -> Result<Outcome, WriteError> {
        let rejected = |reason, detail: String| Outcome::Rejected { reason, detail };
        if let Some(item) = group.items.iter().find(|i| i.kind == Kind::Op) {
            return Ok(rejected(
                Reason::Invalid,
                format!("{}: ledger operations are not synced yet", item.op_id),
            ));
        }
        if let Some(item) = group
            .items
            .iter()
            .find(|i| !self.scope.admits_up(&i.resource_type, &i.body))
        {
            return Ok(rejected(
                Reason::Unauthorized,
                format!(
                    "{}/{} is outside the peer's Sync Scope",
                    item.resource_type, item.resource_id
                ),
            ));
        }
        for item in &group.items {
            if let Some(why) = signed_note_refusal(tx, item).await? {
                return Ok(rejected(Reason::SignedVersionEdited, why));
            }
        }
        let mut conflicts = Vec::new();
        for item in &group.items {
            if let Standing::Conflict { head } = self.standing(tx, item).await? {
                let (class, _) = class_of(&item.resource_type);
                conflicts.push(json!({
                    "op_id": item.op_id.to_string(),
                    "resource": format!("{}/{}", item.resource_type, item.resource_id),
                    "parent": item.parent_version_id.map(|p| p.to_string()),
                    "head": head.map(|h| h.to_string()),
                    "class": class,
                }));
            }
        }
        let Some(first) = conflicts.first() else {
            return Ok(Outcome::Accepted);
        };
        let resource_type = first["resource"]
            .as_str()
            .and_then(|r| r.split('/').next())
            .unwrap_or_default()
            .to_owned();
        let (class, policy) = class_of(&resource_type);
        let mut sql = Sql::default();
        let key = sql.str(group.tx_id.to_string());
        let peer = sql.str(self.peer.to_string());
        let items = sql.str(Value::Array(conflicts).to_string());
        let candidates = sql.str(
            serde_json::to_string(&group.items)
                .map_err(|_| WriteError::Store(corrupt("candidates")))?,
        );
        let class = sql.str(class);
        let policy = sql.str(policy);
        sql.push(format!(
            "UPSERT type::record('sync_conflict', {key}) SET peer = {peer}, items = {items}, \
             candidates = {candidates}, class = {class}, policy = {policy}, status = 'open', \
             opened_at = time::now()"
        ));
        sql.run(tx.tx()?).await.map_err(write_error)?;
        Ok(Outcome::Conflict {
            conflict: group.tx_id.to_string(),
        })
    }

    async fn standing(&self, tx: &mut SurrealTx, item: &Item) -> Result<Standing, WriteError> {
        let resource_type: ResourceType = item
            .resource_type
            .parse()
            .map_err(|_| WriteError::Store(corrupt("resource_type")))?;
        let id: ResourceId = item
            .resource_id
            .parse()
            .map_err(|_| WriteError::Store(corrupt("resource_id")))?;
        let head = tx.head(&resource_type, id).await?;
        let head_id = head.as_ref().map(|h| h.version.version_id);
        if head_id == Some(item.op_id) {
            return Ok(Standing::Present);
        }
        if let Some(head) = &head {
            let body: Value = serde_json::from_str(head.json.as_str())
                .map_err(|_| WriteError::Store(corrupt("head json")))?;
            if ContentHash::of(&body) == item.content_hash {
                return Ok(Standing::Present);
            }
            if tx.version(&resource_type, id, item.op_id).await?.is_some() {
                return Ok(Standing::Present);
            }
        }
        if item.parent_version_id == head_id {
            Ok(Standing::FastForward)
        } else {
            Ok(Standing::Conflict { head: head_id })
        }
    }
}

const CONFLICT_COLUMNS: &str = "record::id(id) AS id, peer, items, candidates, class, policy, \
     status, <string> opened_at AS opened_at, resolved_by, resolution";

#[derive(Debug, SurrealValue)]
struct ConflictRow {
    id: String,
    peer: String,
    items: String,
    candidates: Option<String>,
    class: Option<String>,
    policy: Option<String>,
    status: String,
    opened_at: String,
    resolved_by: Option<String>,
    resolution: Option<String>,
}

/// A Conflict Record: a pushed group whose versions did not build on the head, with both branches
/// (synchronization.md § Conflict Record and review queue; ADR 0020).
#[derive(Debug, Clone, PartialEq)]
pub struct ConflictRecord {
    /// The pushed group's `tx_id`.
    pub id: String,
    /// The node that pushed it.
    pub peer: String,
    /// Per conflicting version: its `op_id`, resource, parent, the head it met, and its class.
    pub items: Value,
    /// The group as it was pushed: the candidate branch.
    pub candidates: Vec<Item>,
    /// The records' class, e.g. `clinical` or `identity`.
    pub class: String,
    /// What the class's policy says to do.
    pub policy: String,
    /// `open`, `resolved` or `superseded`.
    pub status: String,
    /// When it was opened.
    pub opened_at: Timestamp,
    /// Who resolved it.
    pub resolved_by: Option<String>,
    /// The version it was resolved to.
    pub resolution: Option<String>,
}

impl ConflictRecord {
    fn from_row(row: ConflictRow) -> Result<Self, StoreError> {
        Ok(Self {
            id: row.id,
            peer: row.peer,
            items: serde_json::from_str(&row.items).map_err(|_| corrupt("conflict items"))?,
            candidates: row
                .candidates
                .map(|c| serde_json::from_str(&c).map_err(|_| corrupt("conflict candidates")))
                .transpose()?
                .unwrap_or_default(),
            class: row.class.unwrap_or_else(|| "clinical".into()),
            policy: row.policy.unwrap_or_default(),
            status: row.status,
            opened_at: row.opened_at.parse().map_err(|_| corrupt("opened_at"))?,
            resolved_by: row.resolved_by,
            resolution: row.resolution,
        })
    }
}

/// How a reviewer resolves a Conflict Record.
#[derive(Debug, Clone)]
pub enum Resolution {
    /// The head stands as it is; the candidate stays recorded in the Conflict Record.
    KeepHead {
        /// The resource's type.
        resource_type: ResourceType,
        /// Its id.
        id: ResourceId,
    },
    /// A new version, written on top of the head: the corrected record, or one branch marked
    /// entered-in-error.
    Write {
        /// The resource's type.
        resource_type: ResourceType,
        /// Its id.
        id: ResourceId,
        /// The version to write.
        json: RawJson,
    },
}

/// A conflict's class and its policy (synchronization.md § Per-class policy).
pub fn class_of(resource_type: &str) -> (&'static str, &'static str) {
    match resource_type {
        "Patient" | "Person" | "RelatedPerson" => (
            "identity",
            "resolved at central by registry staff; never auto-merged",
        ),
        "Composition" | "DocumentReference" => (
            "signed-note",
            "changes are amendments that relate to the original; a signed version is never edited",
        ),
        "Encounter" | "Task" | "RequestOrchestration" => (
            "workflow",
            "allowed, commuting transitions apply; anything else is reviewed",
        ),
        "QuestionnaireResponse" => (
            "questionnaire-response",
            "created once; later edits are handled like Observation",
        ),
        "Device" => ("device", "revocation wins"),
        _ => (
            "clinical",
            "version-safe correction: both branches are kept, and the resolver writes the \
             corrected version or marks one entered-in-error; never last-write-wins",
        ),
    }
}

/// Why a pushed version may not replace the note version it was written on, if it may not: a
/// signed note is never edited in place or deleted, only marked entered-in-error as it was
/// (synchronization.md § Per-class policy; clinical-documentation's rule).
async fn signed_note_refusal(
    tx: &mut SurrealTx,
    item: &Item,
) -> Result<Option<String>, WriteError> {
    if !matches!(
        item.resource_type.as_str(),
        "Composition" | "DocumentReference"
    ) {
        return Ok(None);
    }
    let Some(parent) = item.parent_version_id else {
        return Ok(None);
    };
    let resource_type: ResourceType = item
        .resource_type
        .parse()
        .map_err(|_| WriteError::Store(corrupt("resource_type")))?;
    let id: ResourceId = item
        .resource_id
        .parse()
        .map_err(|_| WriteError::Store(corrupt("resource_id")))?;
    let Some(base) = tx.version(&resource_type, id, parent).await? else {
        return Ok(None);
    };
    let base: Value = serde_json::from_str(base.json.as_str())
        .map_err(|_| WriteError::Store(corrupt("note json")))?;
    let refused = if item.deleted {
        health_clinical_documentation_app::notes::check_delete(&base)
    } else {
        health_clinical_documentation_app::notes::check_replace(&base, &item.body)
    };
    Ok(refused.err().map(|why| {
        format!(
            "{}/{}: {why}; amend it with a new note instead",
            item.resource_type, item.resource_id
        )
    }))
}

/// The pulling side's store: an edge pulling from central, a desktop from its edge.
pub struct SurrealReplica<'a> {
    store: &'a SurrealStore,
    upstream: NodeId,
}

impl<'a> SurrealReplica<'a> {
    /// Applies what `upstream` serves.
    pub fn new(store: &'a SurrealStore, upstream: NodeId) -> Self {
        Self { store, upstream }
    }
}

impl Replica for SurrealReplica<'_> {
    type Error = WriteError;

    async fn local(&self, item: &Item) -> Result<Local, WriteError> {
        let resource_type: ResourceType = item
            .resource_type
            .parse()
            .map_err(|_| WriteError::Store(corrupt("resource_type")))?;
        let table = table(&resource_type)?;
        #[derive(SurrealValue)]
        struct Row {
            stored: Option<String>,
            queued: Option<String>,
            held: Option<String>,
        }
        let row: Option<Row> = self
            .store
            .connection
            .db()
            .query(format!(
                "LET $queued = (SELECT VALUE record::id(id)[1] FROM sync_outbox \
                 WHERE resource = $resource AND record::id(id)[0] = $peer ORDER BY id DESC LIMIT 1)[0]; \
                 RETURN {{ \
                   stored: (SELECT VALUE version_id FROM {table}_history:[$id, 0]..[$id, {{}}] \
                            WHERE version_id = $op LIMIT 1)[0], \
                   queued: IF $queued != NONE THEN type::record('change_log', $queued).op_id END, \
                   held: (SELECT VALUE record::id(id) FROM sync_item_state \
                          WHERE resource_type = $type AND resource_id = $id LIMIT 1)[0] \
                 }}"
            ))
            .bind(("resource", format!("{}/{}", item.resource_type, item.resource_id)))
            .bind(("peer", self.upstream.to_string()))
            .bind(("id", item.resource_id.clone()))
            .bind(("type", item.resource_type.clone()))
            .bind(("op", item.op_id.to_string()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?
            .take(1)
            .map_err(write_error)?;
        let row = row.unwrap_or(Row {
            stored: None,
            queued: None,
            held: None,
        });
        let candidate = row
            .queued
            .or(row.held)
            .map(|op| {
                op.parse()
                    .map_err(|_| WriteError::Store(corrupt("candidate")))
            })
            .transpose()?;
        Ok(Local {
            has_item: row.stored.is_some(),
            unaccepted_candidate: candidate,
        })
    }

    async fn apply(
        &self,
        group: &Group,
        decisions: &[ApplyDecision],
        checkpoint: &Cursor,
    ) -> Result<(), WriteError> {
        let mut tx = self.store.begin().await?;
        let origin = group.items.first().map_or_else(
            || self.upstream.to_string(),
            |item| item.origin_node.to_string(),
        );
        tx.received_from(Source {
            peer: self.upstream.to_string(),
            origin,
        });
        for (item, decision) in group.items.iter().zip(decisions) {
            match decision {
                ApplyDecision::Skip => {}
                ApplyDecision::Head | ApplyDecision::HeadKeepingCandidate(_) => {
                    tx.apply_version(item).await?;
                }
                ApplyDecision::Append => {
                    return Err(WriteError::Store(StoreError::Unavailable(
                        "ledger operations are not synced yet".into(),
                    )));
                }
            }
        }
        let mut sql = Sql::default();
        let peer = sql.str(checkpoint.node.to_string());
        let epoch = sql.str(checkpoint.epoch.as_str());
        let seq = sql.value(Bind::Int(
            i64::try_from(checkpoint.seq).map_err(|_| WriteError::Store(corrupt("seq")))?,
        ));
        sql.push(format!(
            "UPSERT type::record('sync_checkpoint', {peer}) SET epoch = {epoch}, seq = {seq}"
        ));
        sql.run(tx.tx()?).await.map_err(write_error)?;
        tx.commit().await
    }

    async fn advance(&self, checkpoint: &Cursor) -> Result<(), WriteError> {
        self.apply(
            &Group {
                tx_id: OpId::new(),
                items: Vec::new(),
            },
            &[],
            checkpoint,
        )
        .await
    }
}
