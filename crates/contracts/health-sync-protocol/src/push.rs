use health_identifiers::OpId;

use crate::Item;

/// At most this many items in one push batch.
pub const MAX_BATCH_ITEMS: usize = 500;

/// At most this many body bytes in one push batch. The protocol caps the compressed size; a
/// batch is measured uncompressed here, which never exceeds it.
pub const MAX_BATCH_BYTES: usize = 4 * 1024 * 1024;

/// The items of one transaction, applied all-or-nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    /// The transaction.
    pub tx_id: OpId,
    /// Its items, in `seq` order.
    pub items: Vec<Item>,
}

/// Items that cannot be grouped.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GroupError {
    /// A transaction's items are not contiguous, so the log was not read in `seq` order.
    #[error("transaction {0} is split by another transaction's items")]
    Split(OpId),
}

/// Groups items read in `seq` order by `tx_id`. A transaction commits atomically under the
/// commit lock (ADR 0061), so its items are contiguous in the log.
pub fn group(items: Vec<Item>) -> Result<Vec<Group>, GroupError> {
    let mut groups: Vec<Group> = Vec::new();
    for item in items {
        match groups.last_mut() {
            Some(last) if last.tx_id == item.tx_id => last.items.push(item),
            _ => {
                if groups.iter().any(|g| g.tx_id == item.tx_id) {
                    return Err(GroupError::Split(item.tx_id));
                }
                groups.push(Group {
                    tx_id: item.tx_id,
                    items: vec![item],
                });
            }
        }
    }
    Ok(groups)
}

/// Splits groups into push batches of at most [`MAX_BATCH_ITEMS`] items and [`MAX_BATCH_BYTES`]
/// bytes, keeping order. A group is never split: one larger than a batch goes alone.
pub fn batches(groups: Vec<Group>) -> Vec<Vec<Group>> {
    let mut batches: Vec<Vec<Group>> = Vec::new();
    let (mut items, mut bytes) = (0, 0);
    for group in groups {
        let (len, size) = (group.items.len(), group_bytes(&group));
        let fits = items + len <= MAX_BATCH_ITEMS && bytes + size <= MAX_BATCH_BYTES;
        match batches.last_mut() {
            Some(batch) if fits => batch.push(group),
            _ => {
                (items, bytes) = (0, 0);
                batches.push(vec![group]);
            }
        }
        items += len;
        bytes += size;
    }
    batches
}

fn group_bytes(group: &Group) -> usize {
    group
        .items
        .iter()
        .map(|item| serde_json::to_vec(item).map_or(0, |line| line.len() + 1))
        .sum()
}
