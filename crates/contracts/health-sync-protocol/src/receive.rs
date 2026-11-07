use std::future::Future;

use health_identifiers::OpId;
use serde::{Deserialize, Serialize};

use crate::{ContentHash, Group};

/// Why the receiver rejected a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// An `op_id` the receiver holds a receipt for, with different content. Audited.
    OpIdReused,
    /// Some of the group's `op_id`s have receipts and some do not, so it is not the group that
    /// was sent before.
    InconsistentGroup,
    /// An item is malformed: its hash does not match its body, or its subject does not fit its
    /// kind.
    Malformed,
    /// It was validated against package versions the receiver no longer approves.
    MetadataIncompatible,
    /// Profile, references or invariants (ADR 0016).
    Invalid,
    /// The author was not authorized at action time.
    Unauthorized,
    /// An in-place edit of a signed version.
    SignedVersionEdited,
    /// The peer is quarantined; the group is held for review.
    Quarantined,
}

/// What became of a group, the same for each of its items.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    /// Fast-forwarded, resolved automatically or appended.
    Accepted,
    /// A Conflict Record was opened at the authority.
    Conflict {
        /// The Conflict Record.
        conflict: String,
    },
    /// Not applied.
    Rejected {
        /// Why.
        reason: Reason,
        /// Detail for the review queue.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        detail: String,
    },
}

/// The receiver's record of an item, kept with the write it caused so a retry is answered from
/// it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Receipt {
    /// The item.
    pub op_id: OpId,
    /// Its content when it was first received.
    pub content_hash: ContentHash,
    /// What became of it.
    pub outcome: Outcome,
}

/// The receiver's answer for one group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupResult {
    /// The group.
    pub tx_id: OpId,
    /// Its outcome.
    pub outcome: Outcome,
    /// Whether it was answered from receipts, without applying anything.
    pub replayed: bool,
}

/// The receiving side of a push: central for an edge, an edge for its desktops.
pub trait Authority: Send + Sync {
    /// Storage errors.
    type Error: Send;

    /// The receipts held for any of `op_ids`.
    fn receipts(
        &self,
        op_ids: &[OpId],
    ) -> impl Future<Output = Result<Vec<Receipt>, Self::Error>> + Send;

    /// Validates and applies a new group: fast-forward or conflict per version, append per
    /// operation. In one transaction it writes the heads, history, indexes, change log, the
    /// outbox toward its own upstream, and a [`Receipt`] for every item with the outcome it
    /// returns. A group it rejects leaves only those receipts.
    fn apply(&self, group: &Group) -> impl Future<Output = Result<Outcome, Self::Error>> + Send;
}

/// Receives one pushed group. A group sent before is answered from its receipts and never
/// applied twice, so a retry after a lost response is safe; an `op_id` reused for different
/// content is rejected with [`Reason::OpIdReused`] (synchronization.md § Push).
pub async fn receive<A: Authority>(authority: &A, group: &Group) -> Result<GroupResult, A::Error> {
    let answer = |outcome, replayed| GroupResult {
        tx_id: group.tx_id,
        outcome,
        replayed,
    };
    let rejected = |reason, detail: String| Outcome::Rejected { reason, detail };

    if group.items.is_empty() {
        return Ok(answer(
            rejected(Reason::InconsistentGroup, "an empty group".into()),
            false,
        ));
    }
    if let Some(error) = group.items.iter().find_map(|item| item.check().err()) {
        return Ok(answer(
            rejected(Reason::Malformed, error.to_string()),
            false,
        ));
    }
    if let Some(item) = group.items.iter().find(|item| item.tx_id != group.tx_id) {
        let detail = format!("item {} is in transaction {}", item.op_id, item.tx_id);
        return Ok(answer(rejected(Reason::InconsistentGroup, detail), false));
    }

    let op_ids: Vec<OpId> = group.items.iter().map(|item| item.op_id).collect();
    let receipts = authority.receipts(&op_ids).await?;
    let held = |item: &crate::Item| receipts.iter().find(|r| r.op_id == item.op_id);

    if let Some(item) = group
        .items
        .iter()
        .find(|item| held(item).is_some_and(|r| r.content_hash != item.content_hash))
    {
        let detail = format!(
            "op_id {} was received before with other content",
            item.op_id
        );
        return Ok(answer(rejected(Reason::OpIdReused, detail), false));
    }
    let known = group
        .items
        .iter()
        .filter(|item| held(item).is_some())
        .count();
    if known == group.items.len()
        && let Some(first) = receipts.first()
    {
        return Ok(answer(first.outcome.clone(), true));
    }
    if known > 0 {
        let detail = format!(
            "{known} of {} items were received before",
            group.items.len()
        );
        return Ok(answer(rejected(Reason::InconsistentGroup, detail), false));
    }
    Ok(answer(authority.apply(group).await?, false))
}
