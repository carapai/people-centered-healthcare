//! The blocking index (`idx_block`) and block sizes (`block_count`), for patient matching.

use health_identifiers::ResourceId;
use health_persistence::{BlockingIndex, StoreError};

use crate::store::{SurrealTx, unavailable};

impl BlockingIndex for SurrealTx {
    async fn members(
        &mut self,
        kind: &str,
        value: &str,
        limit: usize,
    ) -> Result<Vec<ResourceId>, StoreError> {
        let ids: Vec<String> = self
            .tx()?
            .query(format!(
                "SELECT VALUE record::id(id)[2] FROM idx_block:[$kind, $value]..[$kind, $value, {{}}] LIMIT {limit}"
            ))
            .bind(("kind", kind.to_owned()))
            .bind(("value", value.to_owned()))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .map_err(unavailable)?
            .take(0)
            .map_err(unavailable)?;
        ids.iter()
            .map(|id| {
                id.parse()
                    .map_err(|_| StoreError::Unavailable("idx_block has an invalid id".into()))
            })
            .collect()
    }

    async fn counts(&mut self, keys: &[(String, String)]) -> Result<Vec<Option<u64>>, StoreError> {
        let mut counts = Vec::with_capacity(keys.len());
        for (kind, value) in keys {
            let found: Vec<i64> = self
                .tx()?
                .query("SELECT VALUE count FROM type::record('block_count', [$kind, $value])")
                .bind(("kind", kind.clone()))
                .bind(("value", value.clone()))
                .await
                .and_then(surrealdb::IndexedResults::check)
                .map_err(unavailable)?
                .take(0)
                .map_err(unavailable)?;
            counts.push(found.first().and_then(|n| u64::try_from(*n).ok()));
        }
        Ok(counts)
    }
}
