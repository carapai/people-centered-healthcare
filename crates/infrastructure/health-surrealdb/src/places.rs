//! The facility registry's places (ADR 0094; #869): `health_facility_registry::PlaceStore` over
//! the transaction, reading every Organization in the `organization` table.
//!
//! Each facility's HMIS reporting settings at central (#560; migration
//! `0066_facility_reporting`): `health_facility_registry::FacilityReportingStore`, one record
//! per facility keyed by its code, the settings kept as JSON text.

use health_facility_registry::{FacilityReporting, FacilityReportingStore, PlaceStore};
use health_persistence::{StoreError, WriteError};

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

impl FacilityReportingStore for SurrealTx {
    async fn put_facility_reporting(
        &mut self,
        setting: &FacilityReporting,
        by: &str,
    ) -> Result<(), WriteError> {
        let json = serde_json::to_string(setting)
            .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))?;
        self.run(
            "UPSERT type::record('facility_reporting', $facility) CONTENT { setting: $setting, \
             set_by: $by, set_at: time::now() } RETURN NONE",
            vec![
                ("facility", setting.facility.clone()),
                ("setting", json),
                ("by", by.to_owned()),
            ],
        )
        .await?;
        Ok(())
    }

    async fn facility_reporting(&mut self) -> Result<Vec<FacilityReporting>, StoreError> {
        self.rows(
            "SELECT VALUE [setting] FROM facility_reporting ORDER BY id",
            vec![],
        )
        .await?
        .into_iter()
        .filter_map(|row| row.into_iter().next())
        .map(|json| serde_json::from_str(&json).map_err(|e| StoreError::Unavailable(e.to_string())))
        .collect()
    }
}
