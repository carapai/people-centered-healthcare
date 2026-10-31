//! Storage for the Inbound Dead Letter and the quarantine queue (migration `0005_inbound`), used
//! by the integration module inside its transactions.

use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;

use crate::store::{SurrealTx, unavailable, write_error};

/// Which queue an entry is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundKind {
    /// A tier 1 failure: the raw payload.
    DeadLetter,
    /// A tier 2–4 failure, or a write the store refused: the resource, for review.
    Quarantine,
}

impl InboundKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::DeadLetter => "dead_letter",
            Self::Quarantine => "quarantine",
        }
    }

    fn parse(text: &str) -> Result<Self, StoreError> {
        match text {
            "dead_letter" => Ok(Self::DeadLetter),
            "quarantine" => Ok(Self::Quarantine),
            _ => Err(StoreError::Unavailable(
                "inbound has an invalid kind".into(),
            )),
        }
    }
}

/// Where an entry stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundStatus {
    /// Waiting for review.
    Pending,
    /// Re-validated and written.
    Replayed,
    /// Rejected by a reviewer.
    Discarded,
}

impl InboundStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Replayed => "replayed",
            Self::Discarded => "discarded",
        }
    }

    fn parse(text: &str) -> Result<Self, StoreError> {
        match text {
            "pending" => Ok(Self::Pending),
            "replayed" => Ok(Self::Replayed),
            "discarded" => Ok(Self::Discarded),
            _ => Err(StoreError::Unavailable(
                "inbound has an invalid status".into(),
            )),
        }
    }
}

/// One kept payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundEntry {
    /// The entry id (a UUIDv7).
    pub id: String,
    /// Dead letter or quarantine.
    pub kind: InboundKind,
    /// The integration it came from.
    pub integration: String,
    /// When it arrived.
    pub received_at: Timestamp,
    /// When retention ends and it is purged.
    pub retain_until: Timestamp,
    /// The payload as received.
    pub payload: String,
    /// The problems found, without payload values.
    pub issues: Vec<String>,
    /// Pending, replayed or discarded.
    pub status: InboundStatus,
    /// When it was resolved.
    pub resolved_at: Option<Timestamp>,
    /// The reviewer's note.
    pub note: Option<String>,
}

fn timestamp(text: &str) -> Result<Timestamp, StoreError> {
    text.parse()
        .map_err(|_| StoreError::Unavailable("inbound has an invalid datetime".into()))
}

/// One row as strings: id, kind, integration, received_at, retain_until, payload, issues (one per
/// line), status, resolved_at ('' for none), note ('' for none).
fn entry(row: Vec<String>) -> Result<InboundEntry, StoreError> {
    let invalid = || StoreError::Unavailable("inbound row has an invalid shape".into());
    let [
        id,
        kind,
        integration,
        received_at,
        retain_until,
        payload,
        issues,
        status,
        resolved_at,
        note,
    ]: [String; 10] = row.try_into().map_err(|_| invalid())?;
    Ok(InboundEntry {
        id,
        kind: InboundKind::parse(&kind)?,
        integration,
        received_at: timestamp(&received_at)?,
        retain_until: timestamp(&retain_until)?,
        payload,
        issues: if issues.is_empty() {
            Vec::new()
        } else {
            issues.lines().map(str::to_owned).collect()
        },
        status: InboundStatus::parse(&status)?,
        resolved_at: (!resolved_at.is_empty())
            .then(|| timestamp(&resolved_at))
            .transpose()?,
        note: (!note.is_empty()).then_some(note),
    })
}

const COLUMNS: &str = "[record::id(id), kind, integration, <string>received_at, \
    <string>retain_until, payload, array::join(issues, '\\n'), status, \
    IF resolved_at THEN <string>resolved_at ELSE '' END, note ?? '']";

impl SurrealTx {
    /// Keeps a payload. The entry's id must be new.
    pub async fn put_inbound(&mut self, entry: &InboundEntry) -> Result<(), WriteError> {
        self.tx()?
            .query(
                "CREATE type::record('inbound', $id) CONTENT { \
                 kind: $kind, integration: $integration, received_at: <datetime>$received_at, \
                 retain_until: <datetime>$retain_until, payload: $payload, issues: $issues, \
                 status: $status, resolved_at: NONE, note: NONE };",
            )
            .bind(("id", entry.id.clone()))
            .bind(("kind", entry.kind.as_str()))
            .bind(("integration", entry.integration.clone()))
            .bind(("received_at", entry.received_at.to_string()))
            .bind(("retain_until", entry.retain_until.to_string()))
            .bind(("payload", entry.payload.clone()))
            .bind(("issues", entry.issues.clone()))
            .bind(("status", entry.status.as_str()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?;
        Ok(())
    }

    /// One entry.
    pub async fn inbound(&mut self, id: &str) -> Result<Option<InboundEntry>, StoreError> {
        let rows: Vec<Vec<String>> = self
            .tx()?
            .query(format!(
                "SELECT VALUE {COLUMNS} FROM type::record('inbound', $id)"
            ))
            .bind(("id", id.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        rows.into_iter().next().map(entry).transpose()
    }

    /// Pending entries of `kind`, oldest first, for review.
    pub async fn inbound_pending(
        &mut self,
        kind: InboundKind,
        limit: usize,
    ) -> Result<Vec<InboundEntry>, StoreError> {
        let rows: Vec<Vec<String>> = self
            .tx()?
            .query(format!(
                "SELECT VALUE {COLUMNS} FROM inbound WHERE kind = $kind AND status = 'pending' \
                 ORDER BY received_at LIMIT {limit}"
            ))
            .bind(("kind", kind.as_str()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        rows.into_iter().map(entry).collect()
    }

    /// Resolves a pending entry; false when it is not pending (already resolved, or unknown).
    pub async fn resolve_inbound(
        &mut self,
        id: &str,
        status: InboundStatus,
        at: Timestamp,
        note: Option<String>,
    ) -> Result<bool, WriteError> {
        let updated: Vec<String> = self
            .tx()?
            .query(
                "UPDATE type::record('inbound', $id) SET status = $status, \
                 resolved_at = <datetime>$at, note = $note WHERE status = 'pending' \
                 RETURN VALUE record::id(id)",
            )
            .bind(("id", id.to_owned()))
            .bind(("status", status.as_str()))
            .bind(("at", at.to_string()))
            .bind(("note", note))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?
            .take(0)
            .map_err(|e| WriteError::Store(unavailable(e)))?;
        Ok(!updated.is_empty())
    }

    /// Deletes every entry whose retention ended before `now`, returning their ids.
    pub async fn purge_inbound(&mut self, now: Timestamp) -> Result<Vec<String>, WriteError> {
        let ids: Vec<String> = self
            .tx()?
            .query(
                "SELECT VALUE record::id(id) FROM inbound WHERE retain_until < <datetime>$now; \
                 DELETE inbound WHERE retain_until < <datetime>$now;",
            )
            .bind(("now", now.to_string()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?
            .take(0)
            .map_err(|e| WriteError::Store(unavailable(e)))?;
        Ok(ids)
    }
}
