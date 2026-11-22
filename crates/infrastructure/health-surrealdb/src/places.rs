//! The facility registry's places (ADR 0094; #869): `health_facility_registry::PlaceStore` over
//! the transaction, reading every Organization in the `organization` table.

use health_facility_registry::PlaceStore;
use health_persistence::StoreError;

use crate::store::SurrealTx;

impl PlaceStore for SurrealTx {
    async fn organizations(&mut self) -> Result<Vec<String>, StoreError> {
        Ok(self
            .rows(
                "SELECT VALUE [json] FROM organization WHERE deleted = false ORDER BY id",
                vec![],
            )
            .await?
            .into_iter()
            .filter_map(|row| row.into_iter().next())
            .collect())
    }
}
