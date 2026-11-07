use std::future::Future;

use health_identifiers::OpId;
use serde::{Deserialize, Serialize};

use crate::{Cursor, Group, GroupError, Item, Kind, group};

/// The order a slow link delivers in: what matters first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    /// Revocations and permission snapshots.
    Revocations,
    /// Metadata and configuration packages, before the data that depends on them.
    Packages,
    /// Clinical data and ledgers.
    Clinical,
    /// Audit.
    Audit,
    /// Attachments.
    Blobs,
}

/// A pull: what comes after `cursor` in the upstream's log, within the node's Sync Scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequest {
    /// Where the node has read to; `None` if it has not been seeded.
    pub cursor: Option<Cursor>,
    /// The lane to read.
    pub lane: Lane,
    /// At most this many items.
    pub limit: u32,
}

/// An item with its position in the upstream's log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pulled {
    /// Its `seq` in the upstream's change log.
    pub seq: u64,
    /// The item.
    pub item: Item,
}

/// The upstream's answer to a pull.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PullResponse {
    /// Items in `seq` order, whole groups only.
    Items {
        /// The items.
        items: Vec<Pulled>,
        /// The cursor after the last of them.
        cursor: Cursor,
        /// Whether more are waiting.
        more: bool,
    },
    /// The cursor is older than the upstream's retention horizon. The node keeps its outbox,
    /// re-seeds (a new epoch), then pushes the outbox again.
    ReseedRequired,
}

/// What the node holds locally for a pulled item's subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Local {
    /// The pulled `op_id` is already stored: a version in history, or an appended operation.
    pub has_item: bool,
    /// A local version the authority has not accepted yet (pending, sending, held, conflict or
    /// rejected).
    pub unaccepted_candidate: Option<OpId>,
}

/// How to apply one pulled item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyDecision {
    /// Already stored; nothing to do.
    Skip,
    /// Becomes the local head.
    Head,
    /// Becomes the local head, and the unaccepted local candidate is kept, still to be pushed;
    /// the authority judges it under the conflict rules.
    HeadKeepingCandidate(OpId),
    /// An operation, appended to its ledger.
    Append,
}

/// The rule for a pulled item: never apply twice, and never overwrite an unaccepted local
/// candidate (synchronization.md § Item states).
pub fn decide(local: Local, pulled: &Item) -> ApplyDecision {
    if local.has_item {
        return ApplyDecision::Skip;
    }
    match (pulled.kind, local.unaccepted_candidate) {
        (Kind::Op, _) => ApplyDecision::Append,
        (Kind::Version, Some(candidate)) => ApplyDecision::HeadKeepingCandidate(candidate),
        (Kind::Version, None) => ApplyDecision::Head,
    }
}

/// The pulling side's storage.
pub trait Replica: Send + Sync {
    /// Storage errors.
    type Error: Send;

    /// What is stored locally for `item`'s subject.
    fn local(&self, item: &Item) -> impl Future<Output = Result<Local, Self::Error>> + Send;

    /// Applies one group as decided, and advances the checkpoint to `checkpoint`, in one
    /// transaction.
    fn apply(
        &self,
        group: &Group,
        decisions: &[ApplyDecision],
        checkpoint: &Cursor,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// What a pull applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// The checkpoint after the last group.
    pub cursor: Cursor,
    /// Groups applied.
    pub groups: usize,
    /// Local candidates kept under a pulled head.
    pub candidates_kept: Vec<OpId>,
}

/// Why a pull could not be applied.
#[derive(Debug, thiserror::Error)]
pub enum PullError<E> {
    /// The upstream answered [`PullResponse::ReseedRequired`].
    #[error("the upstream requires a re-seed")]
    ReseedRequired,
    /// The items do not group by transaction.
    #[error(transparent)]
    Group(#[from] GroupError),
    /// Storage.
    #[error("storage")]
    Store(E),
}

/// Applies a pull response one group per transaction, each with its checkpoint, so a crash
/// mid-pull resumes from the last applied group.
pub async fn apply_pull<R: Replica>(
    replica: &R,
    response: PullResponse,
) -> Result<Applied, PullError<R::Error>> {
    let PullResponse::Items { items, cursor, .. } = response else {
        return Err(PullError::ReseedRequired);
    };
    let seqs: Vec<(OpId, u64)> = items.iter().map(|p| (p.item.op_id, p.seq)).collect();
    let groups = group(items.into_iter().map(|p| p.item).collect())?;
    let mut applied = Applied {
        cursor: cursor.clone(),
        groups: 0,
        candidates_kept: Vec::new(),
    };
    for group in &groups {
        let mut decisions = Vec::with_capacity(group.items.len());
        for item in &group.items {
            let local = replica.local(item).await.map_err(PullError::Store)?;
            let decision = decide(local, item);
            if let ApplyDecision::HeadKeepingCandidate(candidate) = decision {
                applied.candidates_kept.push(candidate);
            }
            decisions.push(decision);
        }
        let last = group.items.last().map(|item| item.op_id);
        let seq = seqs
            .iter()
            .find(|(op_id, _)| Some(*op_id) == last)
            .map_or(cursor.seq, |(_, seq)| *seq);
        let checkpoint = Cursor {
            seq,
            ..cursor.clone()
        };
        replica
            .apply(group, &decisions, &checkpoint)
            .await
            .map_err(PullError::Store)?;
        applied.groups += 1;
    }
    Ok(applied)
}
