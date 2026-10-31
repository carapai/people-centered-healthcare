//! The client registry's review queue (migration `0007_registry_review`).

use health_client_registry::{
    Claim, Decided, Decision, Field, ReviewEntry, ReviewKind, ReviewQueue, Trust,
};
use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;
use serde_json::{Value, json};

use crate::store::{SurrealTx, unavailable, write_error};

fn claim_json(c: &Claim) -> Value {
    json!({ "value": c.value, "trust": c.trust.level(), "recorded": c.recorded.to_string(), "source": c.source })
}

fn claim(v: &Value) -> Option<Claim> {
    Some(Claim {
        value: v["value"].as_str()?.to_owned(),
        trust: Trust::from_level(u8::try_from(v["trust"].as_u64()?).ok()?)?,
        recorded: v["recorded"].as_str()?.parse().ok()?,
        source: v["source"].as_str()?.to_owned(),
    })
}

fn details(kind: &ReviewKind) -> Value {
    match kind {
        ReviewKind::PotentialMatch {
            source,
            candidate,
            golden,
            score,
        } => {
            json!({ "source": source, "candidate": candidate, "golden": golden, "score": score.to_string() })
        }
        ReviewKind::Survivorship {
            golden,
            field,
            current,
            incoming,
        } => {
            json!({ "golden": golden, "field": field.code(), "current": claim_json(current), "incoming": claim_json(incoming) })
        }
    }
}

fn kind(code: &str, d: &Value) -> Option<ReviewKind> {
    let text = |k: &str| d[k].as_str().map(str::to_owned);
    match code {
        "potential-match" => Some(ReviewKind::PotentialMatch {
            source: text("source")?,
            candidate: text("candidate")?,
            golden: text("golden")?,
            score: d["score"].as_str()?.parse().ok()?,
        }),
        "survivorship" => Some(ReviewKind::Survivorship {
            golden: text("golden")?,
            field: Field::from_code(d["field"].as_str()?)?,
            current: claim(&d["current"])?,
            incoming: claim(&d["incoming"])?,
        }),
        _ => None,
    }
}

/// id, kind, details, created_at, decision ('' for none), decided_by, decided_at.
const COLUMNS: &str = "[record::id(id), kind, details, <string>created_at, decision ?? '', \
    decided_by ?? '', IF decided_at THEN <string>decided_at ELSE '' END]";

fn entry(row: Vec<String>) -> Result<ReviewEntry, StoreError> {
    let invalid = || StoreError::Unavailable("registry_review row has an invalid shape".into());
    let [id, code, details, created, decision, by, at]: [String; 7] =
        row.try_into().map_err(|_| invalid())?;
    let parsed: Value = serde_json::from_str(&details).map_err(|_| invalid())?;
    let created_at: Timestamp = created.parse().map_err(|_| invalid())?;
    let decided = match Decision::from_code(&decision) {
        Some(decision) => Some(Decided {
            decision,
            by,
            at: at.parse().map_err(|_| invalid())?,
        }),
        None => None,
    };
    Ok(ReviewEntry {
        id,
        kind: kind(&code, &parsed).ok_or_else(invalid)?,
        created_at,
        decided,
    })
}

impl ReviewQueue for SurrealTx {
    async fn put_review(&mut self, entry: &ReviewEntry) -> Result<(), WriteError> {
        self.tx()?
            .query(
                "CREATE type::record('registry_review', $id) CONTENT { kind: $kind, details: $details, \
                 created_at: <datetime>$created_at, decision: NONE, decided_by: NONE, decided_at: NONE };",
            )
            .bind(("id", entry.id.clone()))
            .bind(("kind", entry.kind.code()))
            .bind(("details", details(&entry.kind).to_string()))
            .bind(("created_at", entry.created_at.to_string()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?;
        Ok(())
    }

    async fn review(&mut self, id: &str) -> Result<Option<ReviewEntry>, StoreError> {
        let rows: Vec<Vec<String>> = self
            .tx()?
            .query(format!(
                "SELECT VALUE {COLUMNS} FROM type::record('registry_review', $id)"
            ))
            .bind(("id", id.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        rows.into_iter().next().map(entry).transpose()
    }

    async fn pending_reviews(&mut self, limit: usize) -> Result<Vec<ReviewEntry>, StoreError> {
        let rows: Vec<Vec<String>> = self
            .tx()?
            .query(format!(
                "SELECT VALUE {COLUMNS} FROM registry_review WHERE decision = NONE ORDER BY created_at LIMIT {limit}"
            ))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        rows.into_iter().map(entry).collect()
    }

    async fn record_decision(&mut self, entry: &ReviewEntry) -> Result<bool, WriteError> {
        let Some(decided) = &entry.decided else {
            return Ok(false);
        };
        let updated: Vec<String> = self
            .tx()?
            .query(
                "UPDATE type::record('registry_review', $id) SET decision = $decision, \
                 decided_by = $by, decided_at = <datetime>$at WHERE decision = NONE \
                 RETURN VALUE record::id(id)",
            )
            .bind(("id", entry.id.clone()))
            .bind(("decision", decided.decision.code()))
            .bind(("by", decided.by.clone()))
            .bind(("at", decided.at.to_string()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?
            .take(0)
            .map_err(|e| WriteError::Store(unavailable(e)))?;
        Ok(!updated.is_empty())
    }
}
