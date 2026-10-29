//! Version reads: `vread` and `_history` at instance, type and system level (persistence.md
//! § REST semantics implemented here).
//!
//! History rows are keyed `[resource_id, version_seq]`, so one resource's versions are a
//! record-id range, and each history table has an index on `(last_updated, version_seq)` for
//! type- and system-level history.

use health_identifiers::{OpId, ResourceId};
use health_persistence::{RawJson, ResourceType, StoreError, VersionMeta};
use health_time::Timestamp;
use surrealdb::types::SurrealValue;

use crate::search::sql::{Bind, Sql};
use crate::{SurrealTx, ownership};

/// One stored version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// The resource type.
    pub resource_type: ResourceType,
    /// The resource id.
    pub id: ResourceId,
    /// The version.
    pub version: VersionMeta,
    /// The version's stored bytes. A tombstone keeps the last body, which is never served.
    pub json: RawJson,
}

/// Which versions a history request covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryScope {
    /// `[type]/[id]/_history`.
    Instance(ResourceType, ResourceId),
    /// `[type]/_history`.
    Type(ResourceType),
    /// `/_history`, across every resource type.
    System,
}

#[derive(Debug, SurrealValue)]
struct Row {
    rid: String,
    json: String,
    version_id: String,
    version_seq: i64,
    last_updated: String,
    deleted: bool,
}

const COLUMNS: &str = "array::first(record::id(id)) AS rid, json, version_id, version_seq, \
                       <string> last_updated AS last_updated, deleted";

fn unavailable(error: surrealdb::Error) -> StoreError {
    StoreError::Unavailable(error.to_string())
}

fn entry(resource_type: &ResourceType, row: Row) -> Result<HistoryEntry, StoreError> {
    let corrupt = |what: &str| {
        StoreError::Unavailable(format!("{resource_type} history row has an invalid {what}"))
    };
    Ok(HistoryEntry {
        resource_type: resource_type.clone(),
        id: row.rid.parse().map_err(|_| corrupt("id"))?,
        version: VersionMeta {
            version_id: row.version_id.parse().map_err(|_| corrupt("version_id"))?,
            version_seq: u64::try_from(row.version_seq).map_err(|_| corrupt("version_seq"))?,
            last_updated: row
                .last_updated
                .parse()
                .map_err(|_| corrupt("last_updated"))?,
            deleted: row.deleted,
        },
        json: RawJson::from_validated(row.json),
    })
}

impl SurrealTx {
    /// The version `version_id` of a resource (`vread`), tombstones included.
    pub async fn version(
        &mut self,
        resource_type: &ResourceType,
        id: ResourceId,
        version_id: OpId,
    ) -> Result<Option<HistoryEntry>, StoreError> {
        let table = crate::store::table(resource_type)?;
        let mut sql = Sql::default();
        let rid = sql.str(id.to_string());
        let version = sql.str(version_id.to_string());
        let statement = sql.push(format!(
            "SELECT {COLUMNS} FROM {table}_history:[{rid}, 0]..[{rid}, {{}}] WHERE version_id = {version} LIMIT 1"
        ));
        let rows: Vec<Row> = sql
            .run(self.tx()?)
            .await
            .map_err(unavailable)?
            .take(statement)
            .map_err(unavailable)?;
        rows.into_iter()
            .next()
            .map(|row| entry(resource_type, row))
            .transpose()
    }

    /// The newest `count` versions in `scope` updated at or after `since`, newest first.
    pub async fn history(
        &mut self,
        scope: &HistoryScope,
        since: Option<Timestamp>,
        count: usize,
    ) -> Result<Vec<HistoryEntry>, StoreError> {
        let mut sql = Sql::default();
        let since_filter = since.map(|since| {
            let at = sql.value(Bind::Datetime(format!("{since:.3}")));
            format!(" WHERE last_updated >= {at}")
        });
        let since_filter = since_filter.unwrap_or_default();
        let tables: Vec<(ResourceType, &'static str)> = match scope {
            HistoryScope::Instance(resource_type, _) | HistoryScope::Type(resource_type) => {
                vec![(resource_type.clone(), crate::store::table(resource_type)?)]
            }
            HistoryScope::System => ownership::resources()
                .iter()
                .filter_map(|r| Some((r.resource_type.parse().ok()?, r.table)))
                .collect(),
        };
        let mut statements = Vec::with_capacity(tables.len());
        for (resource_type, table) in &tables {
            let from = match scope {
                HistoryScope::Instance(_, id) => {
                    let rid = sql.str(id.to_string());
                    format!("{table}_history:[{rid}, 0]..[{rid}, {{}}]")
                }
                HistoryScope::Type(_) | HistoryScope::System => format!("{table}_history"),
            };
            statements.push((
                resource_type.clone(),
                sql.push(format!(
                    "SELECT {COLUMNS} FROM {from}{since_filter} ORDER BY last_updated DESC, version_seq DESC LIMIT {count}"
                )),
            ));
        }
        let mut results = sql.run(self.tx()?).await.map_err(unavailable)?;
        let mut entries = Vec::new();
        for (resource_type, statement) in statements {
            let rows: Vec<Row> = results.take(statement).map_err(unavailable)?;
            for row in rows {
                entries.push(entry(&resource_type, row)?);
            }
        }
        // Newest first across tables; ties keep a stable, per-resource order.
        entries.sort_by(|a, b| {
            b.version
                .last_updated
                .cmp(&a.version.last_updated)
                .then(b.version.version_seq.cmp(&a.version.version_seq))
                .then(a.id.cmp(&b.id))
        });
        entries.truncate(count);
        Ok(entries)
    }
}
