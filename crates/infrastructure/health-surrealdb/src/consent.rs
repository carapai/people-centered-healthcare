//! Caregiver grants (migration `0014_caregiver_grant`): `health_consent::GrantStore` over the
//! transaction.

use health_consent::{CaregiverGrant, GrantStore};
use health_persistence::{StoreError, WriteError};

use crate::store::SurrealTx;

fn parse(rows: Vec<Vec<String>>) -> Result<Vec<CaregiverGrant>, StoreError> {
    rows.into_iter()
        .filter_map(|r| r.into_iter().next())
        .map(|data| {
            serde_json::from_str(&data).map_err(|_| {
                StoreError::Unavailable("caregiver_grant row has an invalid shape".into())
            })
        })
        .collect()
}

impl GrantStore for SurrealTx {
    async fn put_grant(&mut self, grant: &CaregiverGrant) -> Result<(), WriteError> {
        let data = serde_json::to_string(grant)
            .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))?;
        self.run(
            "UPSERT type::record('caregiver_grant', $id) CONTENT { caregiver: $caregiver, \
             patient: $patient, data: $data } RETURN VALUE record::id(id)",
            vec![
                ("id", grant.id.clone()),
                ("caregiver", grant.caregiver.clone()),
                ("patient", grant.patient.clone()),
                ("data", data),
            ],
        )
        .await?;
        Ok(())
    }

    async fn grant(&mut self, id: &str) -> Result<Option<CaregiverGrant>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('caregiver_grant', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        Ok(parse(rows)?.into_iter().next())
    }

    async fn grants_of_patient(
        &mut self,
        patient: &str,
    ) -> Result<Vec<CaregiverGrant>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM caregiver_grant WHERE patient = $patient",
                vec![("patient", patient.to_owned())],
            )
            .await?;
        parse(rows)
    }

    async fn grants_of_caregiver(
        &mut self,
        caregiver: &str,
    ) -> Result<Vec<CaregiverGrant>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM caregiver_grant WHERE caregiver = $caregiver",
                vec![("caregiver", caregiver.to_owned())],
            )
            .await?;
        parse(rows)
    }
}
