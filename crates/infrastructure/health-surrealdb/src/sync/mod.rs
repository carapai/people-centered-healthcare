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

pub(crate) mod finance;
mod transfers;

pub use transfers::{TransferMatch, TransferRefused};

use health_access_policy::{FACILITY_TAG_SYSTEM, is_facility_scope};
use health_identifiers::{NodeId, OpId, ResourceId};
use health_persistence::{RawJson, ResourceType, Store, StoreError, Transaction, WriteError};
use health_sync_protocol::{
    ApplyDecision, Authority, BlobRef, ContentHash, Cursor, Group, Hello, Hlc, Item, Kind, Lane,
    Local, Outcome, PullRequest, PullResponse, Pulled, Reason, Receipt, Replica, Verdict,
    group as group_items,
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

/// The stock ledger's name on the change log: Stock Unit movements go up as operations
/// (write-authority.md § Data authority matrix).
pub const STOCK_LEDGER: &str = "stock";

/// Workflow candidates on the change log: transitions made on a replica go up to the instance's
/// authority as operations (#100; workflow-engine.md § Offline and sync).
pub const WORKFLOW_CANDIDATES: &str = "workflow-candidate";

/// The ledgers whose operations sync: up only, from the facility that keeps them.
pub const LEDGERS: &[&str] = &[
    STOCK_LEDGER,
    WORKFLOW_CANDIDATES,
    finance::CHARGE,
    finance::DEFERRAL,
    finance::SETTLEMENT,
    finance::WAIVER,
    finance::POSTING,
    finance::PAYMENT,
    finance::SHIFT,
];

/// Package activation records on the change log (#89; ADR 0025): global records central issues,
/// sent down to every node as operations in the packages lane, never up.
pub(crate) const ACTIVATIONS: &str = "activation";

/// How many rows of a table a snapshot reads at a time (#83).
pub const SNAPSHOT_PAGE: usize = 500;

/// A part of a snapshot, as [`SurrealStore::snapshot_stream`] produces it: the cursor first,
/// then the heads in pages.
#[derive(Debug)]
pub enum SnapshotPart {
    /// The node's cursor as of the snapshot.
    Cursor(Cursor),
    /// A page of heads in scope.
    Items(Vec<Item>),
}

/// A Conflict Record's resolution on the change log (#82; synchronization.md § Item states:
/// `conflict → accepted`, the resolution pulled down): sent to the node whose push opened it, so
/// it clears its candidates' `conflict` state, never up.
pub(crate) const RESOLUTIONS: &str = "resolution";

impl SyncScope {
    /// Whether the peer may send up a version of `resource_type` with this body, or an
    /// operation on that ledger. A facility sends its own ledgers' operations; which store a
    /// movement names is not yet checked against the facility's Locations.
    pub fn admits_up(&self, resource_type: &str, body: &Value) -> bool {
        resource_type != ACTIVATIONS
            && resource_type != RESOLUTIONS
            && (LEDGERS.contains(&resource_type) || self.admits(resource_type, body, UP_GLOBAL))
    }

    /// Whether the peer is sent a version of `resource_type` with this body. Ledger operations
    /// are never sent down: each ledger's authority is the facility that keeps it. Activation
    /// records go to every node, which applies those its scope chain covers.
    pub fn admits_down(&self, resource_type: &str, body: &Value) -> bool {
        resource_type == ACTIVATIONS
            || resource_type == RESOLUTIONS
            || (!LEDGERS.contains(&resource_type) && self.admits(resource_type, body, DOWN_GLOBAL))
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
pub(crate) fn facility_tag(body: &Value) -> Option<&str> {
    facility_of(body)
}

/// The patient a person record is about: the Patient itself, a RelatedPerson's patient, or a
/// Person's linked Patients (golden-record narrowing).
fn people_of(resource_type: &str, id: &str, body: &Value) -> Option<Vec<String>> {
    let patient = |reference: &Value| {
        reference
            .as_str()
            .and_then(|r| r.strip_prefix("Patient/"))
            .map(str::to_owned)
    };
    match resource_type {
        "Patient" => Some(vec![id.to_owned()]),
        "RelatedPerson" => Some(patient(&body["patient"]["reference"]).into_iter().collect()),
        "Person" => Some(
            body["link"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|l| patient(&l["target"]["reference"]))
                .collect(),
        ),
        _ => None,
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
        let origin_node = match &change.origin {
            Some(origin) => origin.parse().map_err(|_| corrupt("origin"))?,
            None => node,
        };
        if let Some(body) = &change.op {
            // A ledger operation: its body travels as written, appended at the receiver.
            let body: Value = serde_json::from_str(body).map_err(|_| corrupt("op"))?;
            return Ok(Item {
                kind: Kind::Op,
                op_id: change.op_id,
                tx_id: change.tx_id,
                resource_type: change.resource_type.clone(),
                resource_id: change.resource_id.clone(),
                parent_version_id: None,
                origin_node,
                device_id: None,
                author: None,
                hlc: Hlc::default(),
                content_hash: ContentHash::of(&body),
                packages: Vec::new(),
                snapshot_version: 0,
                provisional: false,
                deleted: false,
                blobs: Vec::new(),
                body,
            });
        }
        let resource_type: ResourceType = change
            .resource_type
            .parse()
            .map_err(|_| corrupt("resource_type"))?;
        let change_type = &resource_type;
        let id = change.resource_id.clone();
        let row = self
            .stored_version(change_type, &id, change.version_seq)
            .await?
            .ok_or_else(|| corrupt("change log entry without its version"))?;
        let parent_version_id = if change.version_seq > 1 {
            let parent = self
                .stored_version(change_type, &id, change.version_seq - 1)
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
            blobs: health_sync_protocol::blob_refs(&body),
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
        let peer_key = sql.str(peer);
        for (outgoing, outcome) in sent.iter().zip(answers) {
            through = Some(outgoing.through);
            if *outcome == Outcome::Accepted {
                // Accepted: its attachments follow it (synchronization.md § Attachments).
                for blob in outgoing.group.items.iter().flat_map(|item| &item.blobs) {
                    let hash = sql.str(blob.hash.to_string());
                    let size = sql.value(Bind::Int(
                        i64::try_from(blob.size).map_err(|_| corrupt("blob size"))?,
                    ));
                    let content_type = sql.str(blob.content_type.as_str());
                    sql.push(format!(
                        "UPSERT type::record('sync_blob_queue', [{peer_key}, {hash}]) \
                         SET size = {size}, content_type = {content_type}, queued_at = time::now()"
                    ));
                }
            }
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
            // A resolution goes only to the node whose push opened its Conflict Record.
            let resolution_for_another =
                item.resource_type == RESOLUTIONS && item.body["peer"] != peer_text.as_str();
            if Lane::of(&item.resource_type) == request.lane
                && !resolution_for_another
                && scope.admits_down(&item.resource_type, &item.body)
                && self
                    .person_admitted(scope, &item.resource_type, &item.resource_id, &item.body)
                    .await?
            {
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

    /// The cursor into `peer`'s log this node has applied in `lane`, if any.
    pub async fn checkpoint(&self, peer: NodeId, lane: Lane) -> Result<Option<Cursor>, StoreError> {
        #[derive(SurrealValue)]
        struct Row {
            epoch: String,
            seq: i64,
        }
        let row: Option<Row> = self
            .connection
            .db()
            .query("SELECT epoch, seq FROM ONLY type::record('sync_checkpoint', [$peer, $lane])")
            .bind(("peer", peer.to_string()))
            .bind(("lane", lane.key()))
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

    /// Whether a node with `scope` may be sent the blob `hash` (#84): it is the tarball an
    /// activation record names, or some record whose current version references it would be sent
    /// down to that scope. A blob no such record references, or one held only for other
    /// facilities, is not.
    pub async fn blob_admitted(&self, scope: &SyncScope, hash: &str) -> Result<bool, StoreError> {
        #[derive(SurrealValue)]
        struct Owner {
            resource_type: String,
            resource_id: String,
        }
        #[derive(SurrealValue)]
        struct Head {
            json: String,
            deleted: bool,
        }
        if matches!(scope, SyncScope::All) {
            return Ok(true);
        }
        // A package's tarball goes to the nodes its activation records reach (#89), as do the
        // records themselves: what a node fetches is the digest one of them names.
        let published: Option<String> = self
            .connection
            .db()
            .query(
                "SELECT VALUE package FROM package_activation \
                 WHERE string::contains(data, $hash) LIMIT 1",
            )
            .bind(("hash", hash.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        if published.is_some() {
            return Ok(true);
        }
        let owners: Vec<Owner> = self
            .connection
            .db()
            .query("SELECT resource_type, resource_id FROM blob_owner WHERE hash = $hash")
            .bind(("hash", hash.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        for owner in owners {
            let Ok(resource_type) = owner.resource_type.parse::<ResourceType>() else {
                continue;
            };
            let table = crate::store::table(&resource_type)?;
            let head: Option<Head> = self
                .connection
                .db()
                .query("SELECT json, deleted FROM ONLY type::record($table, $id)")
                .bind(("table", table))
                .bind(("id", owner.resource_id.clone()))
                .await
                .and_then(surrealdb::IndexedResults::check)
                .map_err(unavailable)?
                .take(0)
                .map_err(unavailable)?;
            let Some(head) = head.filter(|h| !h.deleted) else {
                continue;
            };
            let body: Value = serde_json::from_str(&head.json)
                .map_err(|_| corrupt("a stored version is not JSON"))?;
            // The current version must still reference it: an attachment replaced or removed
            // no longer opens its blob.
            let still = health_sync_protocol::blob_refs(&body)
                .iter()
                .any(|blob| blob.hash.to_string() == hash);
            if still
                && scope.admits_down(&owner.resource_type, &body)
                && self
                    .person_admitted(scope, &owner.resource_type, &owner.resource_id, &body)
                    .await?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Whether a person record (Patient, RelatedPerson, Person) goes down to `scope`: only to the
    /// facilities its patient has records at (synchronization.md § Pull and Sync Scope). Other
    /// records are not narrowed here.
    async fn person_admitted(
        &self,
        scope: &SyncScope,
        resource_type: &str,
        id: &str,
        body: &Value,
    ) -> Result<bool, StoreError> {
        let (SyncScope::Facility(facility), Some(patients)) =
            (scope, people_of(resource_type, id, body))
        else {
            return Ok(true);
        };
        for patient in patients {
            let held: Option<String> = self
                .connection
                .db()
                .query(
                    "SELECT VALUE <string>record::id(id)[0] FROM ONLY \
                     type::record('patient_facility', [$patient, $facility])",
                )
                .bind(("patient", patient))
                .bind(("facility", facility.clone()))
                .await
                .and_then(surrealdb::IndexedResults::check)
                .map_err(unavailable)?
                .take(0)
                .map_err(unavailable)?;
            if held.is_some() {
                return Ok(true);
            }
        }
        Ok(false)
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
    /// it applies the heads. Collected from [`Self::snapshot_stream`].
    pub async fn snapshot(
        &self,
        scope: &SyncScope,
        node: NodeId,
    ) -> Result<(Vec<Item>, Cursor), StoreError> {
        let mut parts = self.snapshot_stream(scope.clone(), node);
        let mut cursor = None;
        let mut items = Vec::new();
        while let Some(part) = parts.recv().await {
            match part? {
                SnapshotPart::Cursor(at) => cursor = Some(at),
                SnapshotPart::Items(page) => items.extend(page),
            }
        }
        Ok((items, cursor.ok_or_else(|| corrupt("snapshot cursor"))?))
    }

    /// The snapshot in parts (#83): the cursor first, then the heads in pages of
    /// [`SNAPSHOT_PAGE`] rows, read in one transaction so the whole is as of the cursor. Each table
    /// is read a page at a time, so the node holds one page, not every head of every facility,
    /// and a slow reader holds the producer back. The stream ends early if the reader goes away.
    pub fn snapshot_stream(
        &self,
        scope: SyncScope,
        node: NodeId,
    ) -> tokio::sync::mpsc::Receiver<Result<SnapshotPart, StoreError>> {
        self.snapshot_stream_paged(scope, node, SNAPSHOT_PAGE)
    }

    /// [`Self::snapshot_stream`] with pages of `page` rows.
    pub fn snapshot_stream_paged(
        &self,
        scope: SyncScope,
        node: NodeId,
        page: usize,
    ) -> tokio::sync::mpsc::Receiver<Result<SnapshotPart, StoreError>> {
        let (sender, receiver) = tokio::sync::mpsc::channel(2);
        let store = self.clone();
        let page = page.max(1);
        tokio::spawn(async move {
            if let Err(error) = store.snapshot_pages(&scope, node, page, &sender).await {
                let _ = sender.send(Err(error)).await;
            }
        });
        receiver
    }

    async fn snapshot_pages(
        &self,
        scope: &SyncScope,
        node: NodeId,
        page: usize,
        out: &tokio::sync::mpsc::Sender<Result<SnapshotPart, StoreError>>,
    ) -> Result<(), StoreError> {
        #[derive(SurrealValue)]
        struct HeadRow {
            rid: String,
            json: String,
            version_id: String,
            deleted: bool,
        }
        let tx = self.begin().await?;
        let mut sql = Sql::default();
        let activations =
            sql.push("SELECT VALUE data FROM package_activation ORDER BY issued_at, id");
        let epoch = sql.push("SELECT VALUE epoch FROM ONLY sync_state:node");
        let last = sql.push(crate::change_log::LAST_SEQ);
        let mut results = sql.run(tx.tx()?).await.map_err(unavailable)?;
        let epoch: Option<String> = results.take(epoch).map_err(unavailable)?;
        let last: Option<i64> = results.take(last).map_err(unavailable)?;
        let activations: Vec<String> = results.take(activations).map_err(unavailable)?;
        let cursor = Cursor {
            node,
            epoch: epoch.ok_or_else(|| corrupt("epoch"))?,
            seq: last.and_then(|l| u64::try_from(l).ok()).unwrap_or(0),
        };
        if out.send(Ok(SnapshotPart::Cursor(cursor))).await.is_err() {
            return Ok(());
        }
        // Activation records first, as in the packages lane: every node is sent them all.
        let mut items = Vec::new();
        for data in activations {
            let body: Value = serde_json::from_str(&data).map_err(|_| corrupt("activation"))?;
            let id = body["activation"]["id"]
                .as_str()
                .ok_or_else(|| corrupt("activation id"))?
                .to_owned();
            let op_id = OpId::new();
            items.push(Item {
                kind: Kind::Op,
                op_id,
                tx_id: op_id,
                resource_type: ACTIVATIONS.to_owned(),
                resource_id: id,
                parent_version_id: None,
                origin_node: node,
                device_id: None,
                author: None,
                hlc: Hlc::default(),
                content_hash: ContentHash::of(&body),
                packages: Vec::new(),
                snapshot_version: 0,
                provisional: false,
                deleted: false,
                blobs: Vec::new(),
                body,
            });
        }
        if !items.is_empty() && out.send(Ok(SnapshotPart::Items(items))).await.is_err() {
            return Ok(());
        }
        for resource in crate::ownership::resources() {
            let resource_type = resource.resource_type;
            let mut after: Option<String> = None;
            loop {
                let query = match &after {
                    None => format!(
                        "SELECT record::id(id) AS rid, json, version_id, deleted FROM {} \
                         ORDER BY id LIMIT {page}",
                        resource.table
                    ),
                    Some(_) => format!(
                        "SELECT record::id(id) AS rid, json, version_id, deleted FROM {table} \
                         WHERE id > type::record('{table}', $after) ORDER BY id LIMIT {page}",
                        table = resource.table
                    ),
                };
                let mut request = tx.tx()?.query(query);
                if let Some(after) = &after {
                    request = request.bind(("after", after.clone()));
                }
                let rows: Vec<HeadRow> = request
                    .await
                    .and_then(surrealdb::IndexedResults::check)
                    .map_err(unavailable)?
                    .take(0)
                    .map_err(unavailable)?;
                let full = rows.len() == page;
                after = rows.last().map(|row| row.rid.clone());
                let mut items = Vec::new();
                for row in rows {
                    let body: Value =
                        serde_json::from_str(&row.json).map_err(|_| corrupt("json"))?;
                    if !scope.admits_down(resource_type, &body)
                        || !self
                            .person_admitted(scope, resource_type, &row.rid, &body)
                            .await?
                    {
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
                        blobs: health_sync_protocol::blob_refs(&body),
                        body,
                    });
                }
                if !items.is_empty() && out.send(Ok(SnapshotPart::Items(items))).await.is_err() {
                    return Ok(());
                }
                if !full {
                    break;
                }
            }
        }
        drop(tx);
        Ok(())
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
        let applied = self.seed_items(upstream, items).await?;
        self.seed_complete(upstream, cursor).await?;
        Ok(applied)
    }

    /// Applies a batch of `upstream`'s snapshot (#83): each head not already here becomes the
    /// local head, without being queued back. A re-seed read as a stream applies each batch as it
    /// arrives, then calls [`Self::seed_complete`] once the whole snapshot has been read.
    pub async fn seed_items(&self, upstream: NodeId, items: &[Item]) -> Result<usize, WriteError> {
        let replica = SurrealReplica::new(self, upstream, Lane::Clinical);
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
                if item.kind == Kind::Op {
                    tx.receive_op(item).await?;
                } else {
                    tx.apply_version(item).await?;
                }
            }
            tx.commit().await?;
            applied += fresh.len();
        }
        Ok(applied)
    }

    /// Ends a re-seed from `upstream` whose every head has been applied: each lane's checkpoint
    /// moves to the snapshot's `cursor`, and this node takes a new epoch. Called only after the
    /// whole snapshot was read, so an interrupted re-seed leaves the node to re-seed again.
    pub async fn seed_complete(&self, upstream: NodeId, cursor: &Cursor) -> Result<(), WriteError> {
        // The snapshot covers every lane: each lane resumes from its cursor.
        for lane in Lane::PULLED {
            SurrealReplica::new(self, upstream, lane)
                .advance(cursor)
                .await?;
        }
        crate::change_log::new_epoch(&self.connection).await?;
        Ok(())
    }

    /// The break-glass grants queued for `peer` (#224).
    pub async fn queued_grants(
        &self,
        peer: &str,
    ) -> Result<Vec<health_sync_protocol::BreakGlassGrant>, StoreError> {
        let mut tx = self.begin().await?;
        let rows = tx.queued_grants(peer).await?;
        drop(tx);
        Ok(rows
            .into_iter()
            .map(|row| health_sync_protocol::BreakGlassGrant {
                id: row.id,
                session: row.session,
                account: row.account,
                facility: row.facility,
                patient: row.patient,
                reason: row.reason,
                justification: row.justification,
                at: row.at,
                expires_at: row.expires_at,
                acknowledged: row.acknowledged,
            })
            .collect())
    }

    /// Takes grants off `peer`'s queue once the peer has them.
    pub async fn grants_sent(&self, peer: &str, ids: &[String]) -> Result<(), StoreError> {
        self.connection
            .db()
            .query("FOR $id IN $ids { DELETE type::record('sync_grant_outbox', [$peer, $id]); };")
            .bind(("peer", peer.to_owned()))
            .bind(("ids", ids.to_vec()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?;
        Ok(())
    }

    /// Keeps a grant that arrived from a facility at `arrived`: new, its review is due 72 hours
    /// after arrival (offline-auth.md § Break-glass); known, an acknowledgement is taken, never
    /// undone. Returns whether it was new.
    pub async fn receive_grant(
        &self,
        grant: &health_sync_protocol::BreakGlassGrant,
        arrived: Timestamp,
    ) -> Result<bool, StoreError> {
        let due = arrived
            + health_time::SignedDuration::from_hours(health_sync_protocol::REVIEW_WITHIN_HOURS);
        let new: Option<bool> = self
            .connection
            .db()
            .query(
                "BEGIN; \
                 LET $new = array::len((SELECT id FROM type::record('break_glass', $id))) = 0; \
                 IF $new { \
                   CREATE type::record('break_glass', $id) CONTENT { session: $grant_session, \
                     account: $account, facility: $facility, patient: $patient, reason: $reason, \
                     justification: $justification, at: <datetime>$at, \
                     expires_at: <datetime>$expires_at, review_due: <datetime>$due, \
                     arrived_at: <datetime>$arrived, acknowledged: $acknowledged }; \
                 } ELSE IF $acknowledged { \
                   UPDATE type::record('break_glass', $id) SET acknowledged = true; \
                 }; \
                 RETURN $new; \
                 COMMIT;",
            )
            .bind(("id", grant.id.clone()))
            .bind(("grant_session", grant.session.clone()))
            .bind(("account", grant.account.clone()))
            .bind(("facility", grant.facility.clone()))
            .bind(("patient", grant.patient.clone()))
            .bind(("reason", grant.reason.clone()))
            .bind(("justification", grant.justification.clone()))
            .bind(("at", grant.at.to_string()))
            .bind(("expires_at", grant.expires_at.to_string()))
            .bind(("due", due.to_string()))
            .bind(("arrived", arrived.to_string()))
            .bind(("acknowledged", grant.acknowledged))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(3)
            .map_err(unavailable)?;
        Ok(new.unwrap_or(false))
    }

    /// The blobs queued for `peer`, oldest first.
    pub async fn queued_blobs(&self, peer: &str) -> Result<Vec<BlobRef>, StoreError> {
        #[derive(SurrealValue)]
        struct Row {
            hash: String,
            size: i64,
            content_type: String,
        }
        let rows: Vec<Row> = self
            .connection
            .db()
            .query(
                "SELECT record::id(id)[1] AS hash, size, content_type, queued_at \
                 FROM sync_blob_queue:[$peer]..[$peer, {}] ORDER BY queued_at",
            )
            .bind(("peer", peer.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        rows.into_iter()
            .map(|row| {
                Ok(BlobRef {
                    hash: row.hash.parse().map_err(|_| corrupt("blob hash"))?,
                    size: u64::try_from(row.size).map_err(|_| corrupt("blob size"))?,
                    content_type: row.content_type,
                })
            })
            .collect()
    }

    /// Takes a blob off `peer`'s queue once the peer holds all of it.
    pub async fn blob_sent(&self, peer: &str, hash: &ContentHash) -> Result<(), StoreError> {
        self.connection
            .db()
            .query("DELETE type::record('sync_blob_queue', [$peer, $hash])")
            .bind(("peer", peer.to_owned()))
            .bind(("hash", hash.to_string()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?;
        Ok(())
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
        // The node whose push opened it learns the outcome, and stops holding its candidates.
        let resolution = serde_json::json!({
            "conflict": record.id,
            "peer": record.peer,
            "candidates": record.candidates.iter().map(|i| i.op_id.to_string()).collect::<Vec<_>>(),
            "resolution": version.to_string(),
        });
        tx.log_op(RESOLUTIONS, &record.id, &resolution.to_string());
        if let Some(event) = audit {
            health_audit::append(&mut tx, event).await?;
        }
        tx.commit().await?;
        Ok(version)
    }

    /// This node's candidates the upstream did not accept, as `(op_id, state)`: `conflict` or
    /// `rejected` (synchronization.md § Item states), for the user to see as not accepted.
    pub async fn unaccepted(&self) -> Result<Vec<(String, String)>, StoreError> {
        #[derive(SurrealValue)]
        struct Row {
            op: String,
            state: String,
        }
        let rows: Vec<Row> = self
            .connection
            .db()
            .query("SELECT <string>record::id(id) AS op, state FROM sync_item_state ORDER BY op")
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        Ok(rows.into_iter().map(|r| (r.op, r.state)).collect())
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

/// A pushed transition merged onto the receiver's head (ADR 0020, case 2).
struct Merge {
    resource_type: ResourceType,
    id: ResourceId,
    /// The head it is written on.
    head: OpId,
    /// The head, with the candidate's transition applied.
    merged: Value,
}

/// The fields an Encounter's transition changes: its status, and the end of its period.
const TRANSITION_FIELDS: &[&str] = &["status", "actualPeriod"];

/// `value` without `meta` and `fields`.
fn without(value: &Value, fields: &[&str]) -> Value {
    let mut copy = value.clone();
    if let Some(object) = copy.as_object_mut() {
        object.remove("meta");
        for field in fields {
            object.remove(*field);
        }
    }
    copy
}

/// The head with `candidate`'s transition applied, when `candidate` is an Encounter that
/// changes only its status (and the end of its period) from its parent, the head kept the
/// parent's status, and the table allows that transition from it (ADR 0020, case 2). The two
/// edits commute: the merge keeps both, so nothing is lost.
async fn commute(
    tx: &mut SurrealTx,
    item: &Item,
    head: Option<OpId>,
) -> Result<Option<Merge>, WriteError> {
    use health_encounters::EncounterStatus;
    let (Some(head), Some(parent)) = (head, item.parent_version_id) else {
        return Ok(None);
    };
    if item.resource_type != "Encounter" || item.deleted {
        return Ok(None);
    }
    let resource_type: ResourceType = item
        .resource_type
        .parse()
        .map_err(|_| WriteError::Store(corrupt("resource_type")))?;
    let id: ResourceId = item
        .resource_id
        .parse()
        .map_err(|_| WriteError::Store(corrupt("resource_id")))?;
    let read = |entry: Option<crate::history::HistoryEntry>| {
        entry
            .filter(|e| !e.version.deleted)
            .and_then(|e| serde_json::from_str::<Value>(e.json.as_str()).ok())
    };
    let (Some(before), Some(now)) = (
        read(tx.version(&resource_type, id, parent).await?),
        read(tx.version(&resource_type, id, head).await?),
    ) else {
        return Ok(None);
    };
    let candidate = &item.body;
    let status = |v: &Value| serde_json::from_value::<EncounterStatus>(v["status"].clone()).ok();
    let (Some(from), Some(to), Some(at_head)) = (status(&before), status(candidate), status(&now))
    else {
        return Ok(None);
    };
    let commutes = from != to
        // The candidate changed nothing but the transition's fields.
        && without(candidate, TRANSITION_FIELDS) == without(&before, TRANSITION_FIELDS)
        // The head did not move the status, nor the period the candidate sets.
        && at_head == from
        && (candidate["actualPeriod"] == before["actualPeriod"]
            || now["actualPeriod"] == before["actualPeriod"])
        && at_head.may_become(to);
    if !commutes {
        return Ok(None);
    }
    let mut merged = now;
    merged["status"] = candidate["status"].clone();
    if candidate["actualPeriod"] != before["actualPeriod"] {
        merged["actualPeriod"] = candidate["actualPeriod"].clone();
    }
    Ok(Some(Merge {
        resource_type,
        id,
        head,
        merged,
    }))
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
        let (mut outcome, merges) = self.judge(&mut tx, group).await?;
        if outcome == Outcome::Accepted {
            for item in &group.items {
                if item.kind == Kind::Op {
                    if let Some(refused) = self.append_op(&mut tx, item).await? {
                        outcome = refused;
                        // A refused operation leaves nothing of the group applied: start over
                        // with only the receipts.
                        drop(tx);
                        tx = self.store.begin().await?;
                        break;
                    }
                } else if matches!(self.standing(&mut tx, item).await?, Standing::FastForward) {
                    tx.apply_version(item).await?;
                    // A Patient a facility registered is that facility's to receive back.
                    if item.resource_type == "Patient"
                        && let SyncScope::Facility(facility) = &self.scope
                    {
                        tx.tx()?
                            .query(
                                "UPSERT type::record('patient_facility', [$patient, $facility]) \
                                 SET at = time::now()",
                            )
                            .bind(("patient", item.resource_id.clone()))
                            .bind(("facility", facility.clone()))
                            .await
                            .and_then(surrealdb::IndexedResults::check)
                            .map_err(write_error)?;
                    }
                }
            }
        }
        if outcome == Outcome::Accepted && !merges.is_empty() {
            // Written as this node's own versions, so they go back down to the pushing node too.
            let source = tx.source.take();
            for merge in merges {
                tx.write(health_persistence::WriteRequest {
                    resource_type: merge.resource_type,
                    id: merge.id,
                    json: RawJson::from_validated(merge.merged.to_string()),
                    op_id: OpId::new(),
                    precondition: health_persistence::Precondition::IfMatch(merge.head),
                })
                .await?;
            }
            tx.source = source;
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
    async fn judge(
        &self,
        tx: &mut SurrealTx,
        group: &Group,
    ) -> Result<(Outcome, Vec<Merge>), WriteError> {
        let rejected = |reason, detail: String| (Outcome::Rejected { reason, detail }, Vec::new());
        if let Some(item) = group
            .items
            .iter()
            .find(|i| i.kind == Kind::Op && !LEDGERS.contains(&i.resource_type.as_str()))
        {
            return Ok(rejected(
                Reason::Invalid,
                format!("{}: no ledger {}", item.op_id, item.resource_type),
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
        let mut merges = Vec::new();
        // Ledger operations are appended, never in conflict (synchronization.md § Conflict
        // detection and resolution).
        for item in group.items.iter().filter(|i| i.kind == Kind::Version) {
            if let Standing::Conflict { head } = self.standing(tx, item).await? {
                // A transition the table allows, commuting with the head (ADR 0020, case 2).
                if let Some(merge) = commute(tx, item, head).await? {
                    merges.push(merge);
                    continue;
                }
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
            return Ok((Outcome::Accepted, merges));
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
        // A group with any true conflict is held whole: its commuting edits wait with it.
        Ok((
            Outcome::Conflict {
                conflict: group.tx_id.to_string(),
            },
            Vec::new(),
        ))
    }

    /// Appends a ledger operation, unless it is here already: a stock movement through the
    /// inventory's own rules as a late operation, so a balance it drives negative opens a
    /// Reconciliation Record (write-authority.md: ledger reconciliation). `Some` when the
    /// operation is refused.
    async fn append_op(
        &self,
        tx: &mut SurrealTx,
        item: &Item,
    ) -> Result<Option<Outcome>, WriteError> {
        let refused = |detail: String| {
            Some(Outcome::Rejected {
                reason: Reason::Invalid,
                detail,
            })
        };
        if let SyncScope::Facility(facility) = &self.scope
            && let Some(why) = self.claim(tx, item, facility).await?
        {
            return Ok(refused(format!("{}: {why}", item.op_id)));
        }
        // A workflow candidate waits for the engine to judge it (#100): kept once, as pushed.
        if item.resource_type == WORKFLOW_CANDIDATES {
            let pushed = serde_json::to_string(item)
                .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))?;
            tx.tx()?
                .query(
                    "IF (SELECT VALUE id FROM ONLY type::record('workflow_candidate', $id)) = NONE { \
                     CREATE type::record('workflow_candidate', $id) SET peer = $peer, \
                     instance = $instance, data = $data, item = $item, status = 'pending', \
                     received_at = time::now(); };",
                )
                .bind(("id", item.op_id.to_string()))
                .bind(("peer", self.peer.to_string()))
                .bind(("instance", item.resource_id.clone()))
                .bind(("data", item.body.to_string()))
                .bind(("item", pushed))
                .await
                .and_then(surrealdb::IndexedResults::check)
                .map_err(write_error)?;
            return Ok(None);
        }
        if finance::FINANCE.contains(&item.resource_type.as_str()) {
            let facility = match &self.scope {
                SyncScope::Facility(facility) => Some(facility.as_str()),
                _ => None,
            };
            return match tx
                .replay_finance(&item.resource_type, &item.body, facility)
                .await
            {
                Ok(()) => Ok(None),
                Err(Ok(reason)) => Ok(refused(format!("{}: {reason}", item.op_id))),
                Err(Err(error)) => Err(error),
            };
        }
        let Ok(movement) = serde_json::from_value::<health_inventory::Movement>(item.body.clone())
        else {
            return Ok(refused(format!("{}: not a stock movement", item.op_id)));
        };
        // A store leased to one desktop takes stock operations from that desktop alone (#135).
        if let Some(holder) = tx
            .leased_to(&movement.unit.store)
            .await
            .map_err(WriteError::Store)?
            && holder != self.peer.to_string()
        {
            return Ok(refused(format!(
                "{}: store {} is leased to {holder}",
                item.op_id, movement.unit.store
            )));
        }
        let held: Option<String> = tx
            .tx()?
            .query("SELECT VALUE record::id(id) FROM ONLY type::record('stock_movement', $id)")
            .bind(("id", movement.id.clone()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?
            .take(0)
            .map_err(write_error)?;
        if held.is_some() {
            return Ok(None);
        }
        let inventory = health_inventory_app::Inventory::new(
            self.store.clone(),
            health_inventory::Policy::default(),
        );
        match inventory
            .record_in(tx, &movement, health_inventory::Origin::Late)
            .await
        {
            Ok(_) => Ok(None),
            Err(health_inventory_app::InventoryError::Busy) => Err(WriteError::Conflict),
            Err(error) => Ok(refused(format!("{}: {error}", item.op_id))),
        }
    }

    /// Why `facility` may not send this operation, if it may not: its scope (a stock store, a
    /// billing account, a posting, payment or shift) belongs to another facility, claimed by the
    /// first to send an operation on it; or a charge names an encounter of another facility.
    async fn claim(
        &self,
        tx: &mut SurrealTx,
        item: &Item,
        facility: &str,
    ) -> Result<Option<String>, WriteError> {
        let scope = if item.resource_type == STOCK_LEDGER {
            item.body["unit"]["store"]
                .as_str()
                .unwrap_or(item.resource_id.as_str())
                .to_owned()
        } else {
            item.resource_id.clone()
        };
        let held: Option<String> = tx
            .tx()?
            .query(
                "LET $claim = type::record('ledger_claim', [$ledger, $scope]); \
                 LET $held = (SELECT VALUE facility FROM ONLY $claim); \
                 IF $held = NONE { CREATE $claim SET facility = $facility, claimed_at = time::now(); }; \
                 RETURN $held;",
            )
            .bind(("ledger", item.resource_type.clone()))
            .bind(("scope", scope.clone()))
            .bind(("facility", facility.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?
            .take(3)
            .map_err(write_error)?;
        if let Some(owner) = held.filter(|owner| owner != facility) {
            return Ok(Some(format!(
                "{} {scope} belongs to facility {owner}",
                item.resource_type
            )));
        }
        if item.resource_type == finance::CHARGE
            && let Some(encounter) = item.body["event"]
                .as_str()
                .and_then(|e| e.strip_prefix("Encounter/"))
                .and_then(|id| id.parse::<ResourceId>().ok())
        {
            let resource_type: ResourceType = "Encounter"
                .parse()
                .map_err(|_| WriteError::Store(corrupt("resource_type")))?;
            if let Some(head) = tx.head(&resource_type, encounter).await? {
                let body: Value = serde_json::from_str(head.json.as_str())
                    .map_err(|_| WriteError::Store(corrupt("encounter json")))?;
                if facility_of(&body).is_some_and(|tagged| tagged != facility) {
                    return Ok(Some(format!(
                        "the charge's encounter {encounter} is another facility's"
                    )));
                }
            }
        }
        Ok(None)
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
    lane: Lane,
}

impl<'a> SurrealReplica<'a> {
    /// Applies what `upstream` serves in `lane`, checkpointing that lane.
    pub fn new(store: &'a SurrealStore, upstream: NodeId, lane: Lane) -> Self {
        Self {
            store,
            upstream,
            lane,
        }
    }
}

impl Replica for SurrealReplica<'_> {
    type Error = WriteError;

    async fn local(&self, item: &Item) -> Result<Local, WriteError> {
        if item.kind == Kind::Op {
            // An operation is appended once, by its own id; it has no head to keep.
            return Ok(Local {
                has_item: false,
                unaccepted_candidate: None,
            });
        }
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
                ApplyDecision::Append => tx.receive_op(item).await?,
            }
        }
        let mut sql = Sql::default();
        let peer = sql.str(checkpoint.node.to_string());
        let lane = sql.str(self.lane.key());
        let epoch = sql.str(checkpoint.epoch.as_str());
        let seq = sql.value(Bind::Int(
            i64::try_from(checkpoint.seq).map_err(|_| WriteError::Store(corrupt("seq")))?,
        ));
        sql.push(format!(
            "UPSERT type::record('sync_checkpoint', [{peer}, {lane}]) SET epoch = {epoch}, \
             seq = {seq}"
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

impl SurrealTx {
    /// Appends an operation received from upstream: an activation record, kept once. Ledger
    /// operations only go up, so no other comes down.
    pub(crate) async fn receive_op(&mut self, item: &Item) -> Result<(), WriteError> {
        if item.resource_type == RESOLUTIONS {
            // The candidates the upstream resolved are no longer held: the resolution (a new
            // head, or the head kept) is the record now.
            for op in item.body["candidates"].as_array().into_iter().flatten() {
                if let Some(op) = op.as_str() {
                    self.run(
                        "DELETE type::record('sync_item_state', $op) RETURN NONE",
                        vec![("op", op.to_owned())],
                    )
                    .await?;
                }
            }
            return Ok(());
        }
        if item.resource_type != ACTIVATIONS {
            return Err(WriteError::Store(corrupt("a ledger operation came down")));
        }
        let activation = &item.body["activation"];
        let (Some(id), Some(package), Some(issued_at)) = (
            activation["id"].as_str(),
            activation["package"].as_str(),
            activation["issuedAt"].as_str(),
        ) else {
            return Err(WriteError::Store(corrupt("activation record")));
        };
        self.put_package_activation(id, package, issued_at, &item.body.to_string())
            .await
            .map(|_| ())
    }
}
