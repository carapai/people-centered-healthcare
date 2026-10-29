//! The resumable reindex job (persistence.md § Search index tables: activating a configuration
//! package that adds or changes a SearchParameter starts a background reindex; the parameter
//! becomes searchable only when the job completes).
//!
//! The new parameters are added to the registry as `Reindexing` first, so every write from then
//! on indexes them. The job then walks the head table in id order, one batch per transaction,
//! recording the last id it finished in `reindex_job`. A crash or restart loses at most the
//! uncommitted batch, and rerunning a batch is harmless because index writes are upserts.

use health_fhir_search::extract;
use health_persistence::{ResourceType, StoreError, WriteError};
use serde_json::Value;
use surrealdb::types::SurrealValue;

use super::sql::{Bind, Sql};
use super::write::replace_entries;
use crate::SurrealStore;
use crate::store::write_error;

/// A reindex of some parameters of one resource type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReindexJob {
    /// The job id, e.g. the activation's operation id.
    pub id: String,
    /// The resource type.
    pub resource_type: ResourceType,
    /// The parameter codes to backfill.
    pub params: Vec<String>,
}

/// Where a job stands after a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReindexProgress {
    /// Resources reindexed so far, over every run.
    pub processed: u64,
    /// Whether every resource has been reindexed; the parameters can now be activated.
    pub done: bool,
}

#[derive(Debug, SurrealValue)]
struct JobRow {
    rt: String,
    params: Vec<String>,
    after: Option<String>,
    processed: i64,
    done: bool,
}

#[derive(Debug, SurrealValue)]
struct HeadJson {
    rid: String,
    json: String,
    deleted: bool,
}

fn unavailable(error: surrealdb::Error) -> StoreError {
    StoreError::Unavailable(error.to_string())
}

impl SurrealStore {
    /// Records a job, unless one with this id exists already (a restart).
    pub async fn start_reindex(&self, job: &ReindexJob) -> Result<(), StoreError> {
        crate::store::table(&job.resource_type)?;
        self.connection
            .db()
            .query(
                "IF record::exists(type::record('reindex_job', $id)) THEN NONE ELSE \
                 (CREATE type::record('reindex_job', $id) SET rt = $rt, params = $params, after = NONE, processed = 0, done = false) END",
            )
            .bind(("id", job.id.clone()))
            .bind(("rt", job.resource_type.to_string()))
            .bind(("params", job.params.clone()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?;
        Ok(())
    }

    /// Reindexes the next `batch` resources of job `id` in one transaction and records the
    /// position. A [`WriteError::Conflict`] with a concurrent write means the batch did not
    /// commit; run it again.
    pub async fn reindex_batch(
        &self,
        id: &str,
        batch: usize,
    ) -> Result<ReindexProgress, WriteError> {
        use health_persistence::{Store, Transaction};

        let tx = self.begin().await?;
        let surreal = tx.tx()?;
        let job: Option<JobRow> = surreal
            .query("SELECT rt, params, after, processed, done FROM ONLY type::record('reindex_job', $id)")
            .bind(("id", id.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?
            .take(0)
            .map_err(write_error)?;
        let job = job.ok_or_else(|| StoreError::Unavailable(format!("no reindex job {id}")))?;
        let processed = u64::try_from(job.processed).unwrap_or(0);
        if job.done {
            return Ok(ReindexProgress {
                processed,
                done: true,
            });
        }
        let resource_type: ResourceType = job
            .rt
            .parse()
            .map_err(|_| StoreError::UnknownResourceType(job.rt.clone()))?;
        let head_table = crate::store::table(&resource_type)?;
        // Resource ids are UUIDs, so the cursor is safe to place in the range literal.
        let range = job
            .after
            .as_deref()
            .filter(|a| a.parse::<health_identifiers::ResourceId>().is_ok())
            .map_or_else(|| "..".to_owned(), |a| format!("⟨{a}⟩>.."));
        let heads: Vec<HeadJson> = surreal
            .query(format!(
                "SELECT record::id(id) AS rid, json, deleted FROM {head_table}:{range} LIMIT {batch}"
            ))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(write_error)?
            .take(0)
            .map_err(write_error)?;

        let registry = self.registry();
        let mut sql = Sql::default();
        for head in heads.iter().filter(|h| !h.deleted) {
            let resource: Value = serde_json::from_str(&head.json).map_err(|e| {
                StoreError::Unavailable(format!("stored {head_table} {}: {e}", head.rid))
            })?;
            let extraction = extract(&registry, &job.rt, &resource, Some(&job.params));
            let keys = replace_entries(
                &mut sql,
                head_table,
                &job.rt,
                &head.rid,
                &extraction,
                &[],
                Some(&job.params),
            );
            let table = sql.str(head_table);
            let rid = sql.str(head.rid.as_str());
            sql.push(format!(
                "UPDATE type::record({table}, {rid}) SET index_keys = {keys}"
            ));
        }
        let done = heads.len() < batch;
        let processed = processed + u64::try_from(heads.len()).unwrap_or(0);
        let job_id = sql.str(id);
        let after = match heads.last() {
            Some(last) => sql.value(Bind::Str(last.rid.clone())),
            None => job
                .after
                .as_deref()
                .map_or_else(|| "NONE".to_owned(), |a| sql.str(a)),
        };
        let processed_var = sql.value(Bind::Int(i64::try_from(processed).unwrap_or(i64::MAX)));
        sql.push(format!(
            "UPDATE type::record('reindex_job', {job_id}) SET after = {after}, processed = {processed_var}, done = {done}"
        ));
        sql.run(surreal).await.map_err(write_error)?;
        tx.commit().await?;
        Ok(ReindexProgress { processed, done })
    }
}
