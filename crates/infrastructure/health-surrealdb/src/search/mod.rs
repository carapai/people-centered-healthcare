//! FHIR search on SurrealDB (persistence.md § Search index tables; fhir-r5-model.md § Search
//! parameters and indexed projections).
//!
//! - Writes keep the `idx_*` tables in step with each head in the same transaction
//!   ([`crate::SurrealStore::with_registry`]).
//! - [`crate::SurrealTx::search`] runs a parsed [`SearchQuery`] as range scans, intersects the
//!   clauses smallest first, and pages by resource id.
//! - [`crate::SurrealStore::reindex_batch`] backfills new parameters, resumably.

mod compile;
mod reindex;
pub(crate) mod sql;
pub(crate) mod write;

use std::collections::BTreeSet;

use health_fhir_search::{SearchPolicy, SearchQuery};
use health_identifiers::ResourceId;
use health_persistence::{ResourceType, StoreError, StoredResource};

pub use compile::Unsupported;
pub use reindex::{ReindexJob, ReindexProgress};

use crate::SurrealTx;
use compile::{ClausePlan, compile};
use sql::Sql;

/// The page size when a search gives no `_count`.
pub const DEFAULT_COUNT: u32 = 50;

/// One page of search results, in resource-id order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPage {
    /// The matches on this page.
    pub resources: Vec<StoredResource>,
    /// How many resources match, when that is cheap: every indexed search knows it; the
    /// unfiltered listing of a type does not count.
    pub total: Option<usize>,
    /// The cursor for the next page: pass it as `after`.
    pub next: Option<ResourceId>,
}

/// Why a search did not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SearchFailure {
    /// The index cannot answer this search (400).
    #[error(transparent)]
    Unsupported(#[from] Unsupported),
    /// The database failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The next-page cursor: the last id on this page, when more follow.
fn cursor(more: bool, ids: &[String]) -> Option<ResourceId> {
    more.then(|| ids.last().and_then(|id| id.parse().ok()))
        .flatten()
}

fn unavailable(error: surrealdb::Error) -> StoreError {
    StoreError::Unavailable(error.to_string())
}

impl SurrealTx {
    /// Runs `query` with `policy`'s clauses ANDed in, returning the page after `after`.
    pub async fn search(
        &mut self,
        query: &SearchQuery,
        policy: &dyn SearchPolicy,
        after: Option<ResourceId>,
    ) -> Result<SearchPage, SearchFailure> {
        let resource_type: ResourceType = query
            .resource_type
            .parse()
            .map_err(|_| StoreError::UnknownResourceType(query.resource_type.clone()))?;
        let head_table = crate::store::table(&resource_type)?;
        let count = usize::try_from(query.count.unwrap_or(DEFAULT_COUNT)).unwrap_or(usize::MAX);
        let mut clauses = query.clauses.clone();
        clauses.extend(policy.clauses(&query.resource_type));

        if clauses.is_empty() {
            return self
                .list(&resource_type, head_table, count, after)
                .await
                .map_err(Into::into);
        }

        let mut sql = Sql::default();
        let plans = clauses
            .iter()
            .map(|clause| compile(&mut sql, &query.resource_type, clause, false))
            .collect::<Result<Vec<_>, _>>()?;
        let mut results = sql.run(self.tx()?).await.map_err(unavailable)?;
        let mut take = |index: usize| -> Result<BTreeSet<String>, StoreError> {
            Ok(results
                .take::<Vec<String>>(index)
                .map_err(unavailable)?
                .into_iter()
                .collect())
        };
        let mut sets = Vec::new();
        for plan in &plans {
            let union = |take: &mut dyn FnMut(usize) -> Result<BTreeSet<String>, StoreError>,
                         scans: &[usize]|
             -> Result<BTreeSet<String>, StoreError> {
                let mut all = BTreeSet::new();
                for scan in scans {
                    all.extend(take(*scan)?);
                }
                Ok(all)
            };
            sets.push(match plan {
                ClausePlan::Union(scans) => union(&mut take, scans)?,
                ClausePlan::Except { universe, minus } => {
                    let universe = union(&mut take, universe)?;
                    let minus = union(&mut take, minus)?;
                    universe.difference(&minus).cloned().collect()
                }
            });
        }
        // Intersect the most selective clause first.
        sets.sort_by_key(BTreeSet::len);
        let mut matched = sets.remove(0);
        for set in &sets {
            matched.retain(|id| set.contains(id));
        }
        let total = matched.len();
        let after = after.map(|a| a.to_string());
        let mut ids: Vec<String> = matched
            .into_iter()
            .filter(|id| after.as_ref().is_none_or(|a| id > a))
            .take(count + 1)
            .collect();
        let more = ids.len() > count;
        ids.truncate(count);
        let resources = self.fetch(&resource_type, head_table, &ids).await?;
        Ok(SearchPage {
            resources,
            total: Some(total),
            next: cursor(more, &ids),
        })
    }

    /// Returns the SurrealDB plan of every scan `query` compiles to, for tests and diagnostics.
    pub async fn explain(
        &mut self,
        query: &SearchQuery,
        policy: &dyn SearchPolicy,
    ) -> Result<Vec<String>, SearchFailure> {
        let mut clauses = query.clauses.clone();
        clauses.extend(policy.clauses(&query.resource_type));
        let mut sql = Sql::default();
        let mut scans = Vec::new();
        for clause in &clauses {
            match compile(&mut sql, &query.resource_type, clause, true)? {
                ClausePlan::Union(s) => scans.extend(s),
                ClausePlan::Except { universe, minus } => {
                    scans.extend(universe);
                    scans.extend(minus);
                }
            }
        }
        if sql.is_empty() {
            return Ok(Vec::new());
        }
        let mut results = sql.run(self.tx()?).await.map_err(unavailable)?;
        scans
            .into_iter()
            .map(|scan| {
                results
                    .take::<surrealdb::types::Value>(scan)
                    .map(|plan| format!("{plan:?}"))
                    .map_err(|e| unavailable(e).into())
            })
            .collect()
    }

    /// The unfiltered listing of a type, in id order: a record-id range over the head table.
    async fn list(
        &mut self,
        resource_type: &ResourceType,
        head_table: &str,
        count: usize,
        after: Option<ResourceId>,
    ) -> Result<SearchPage, StoreError> {
        // Resource ids are UUIDs, so the cursor is safe to place in the range literal; record-id
        // keys cannot be bound parameters.
        let range = after.map_or_else(|| "..".to_owned(), |a| format!("⟨{a}⟩>.."));
        let limit = count + 1;
        let mut ids: Vec<String> = self
            .tx()?
            .query(format!(
                "SELECT VALUE record::id(id) FROM {head_table}:{range} WHERE deleted = false LIMIT {limit}"
            ))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        let more = ids.len() > count;
        ids.truncate(count);
        let resources = self.fetch(resource_type, head_table, &ids).await?;
        Ok(SearchPage {
            resources,
            total: None,
            next: cursor(more, &ids),
        })
    }

    async fn fetch(
        &mut self,
        resource_type: &ResourceType,
        head_table: &str,
        ids: &[String],
    ) -> Result<Vec<StoredResource>, StoreError> {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let id: ResourceId = id.parse().map_err(|_| {
                StoreError::Unavailable(format!("{head_table} has an invalid id {id}"))
            })?;
            if let Some(head) = self.head(resource_type, id).await?
                && !head.version.deleted
            {
                out.push(head);
            }
        }
        Ok(out)
    }
}
