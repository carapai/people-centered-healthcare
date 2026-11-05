//! Laboratory tests (migration `0034_lab_tests`): `health_laboratory::LabStore` over the
//! transaction.

use health_laboratory::{LabStore, LabTest};
use health_persistence::{StoreError, WriteError};

use crate::store::SurrealTx;

impl LabStore for SurrealTx {
    async fn put_test(&mut self, test: &LabTest) -> Result<(), WriteError> {
        let data = serde_json::to_string(test)
            .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))?;
        self.run(
            "UPSERT type::record('lab_test', $id) CONTENT { data: $data, patient: $patient, \
             stage: $stage } RETURN VALUE record::id(id)",
            vec![
                ("id", test.id.clone()),
                ("data", data),
                ("patient", test.patient.clone()),
                ("stage", test.stage.code().to_owned()),
            ],
        )
        .await?;
        Ok(())
    }

    async fn test(&mut self, id: &str) -> Result<Option<LabTest>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('lab_test', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        rows.into_iter()
            .filter_map(|r| r.into_iter().next())
            .map(|data| {
                serde_json::from_str(&data).map_err(|_| {
                    StoreError::Unavailable("a lab_test row has an invalid shape".into())
                })
            })
            .next()
            .transpose()
    }
}
