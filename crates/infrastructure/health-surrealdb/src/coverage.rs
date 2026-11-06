//! Coverage (migration `0036_coverage`, #125): `health_coverage::CoverageStore` over the
//! transaction. `coverage_accumulator` is keyed by coverage and year, the guard every settlement
//! writes; `coverage_eligibility` holds each coverage's latest eligibility.

use health_coverage::{Accumulator, CoverageStore, Eligibility};
use health_persistence::{StoreError, WriteError};

use crate::store::SurrealTx;

fn invalid() -> StoreError {
    StoreError::Unavailable("coverage row has an invalid shape".into())
}

fn unavailable(e: &serde_json::Error) -> WriteError {
    WriteError::Store(StoreError::Unavailable(e.to_string()))
}

impl SurrealTx {
    async fn one(&mut self, sql: &str, key: String) -> Result<Option<String>, StoreError> {
        let rows = self.rows(sql, vec![("key", key)]).await?;
        Ok(rows.into_iter().next().and_then(|r| r.into_iter().next()))
    }
}

impl CoverageStore for SurrealTx {
    async fn accumulator(
        &mut self,
        coverage: &str,
        year: i16,
    ) -> Result<Option<Accumulator>, StoreError> {
        self.one(
            "SELECT VALUE [data] FROM type::record('coverage_accumulator', $key)",
            format!("{coverage}|{year}"),
        )
        .await?
        .map(|data| serde_json::from_str(&data).map_err(|_| invalid()))
        .transpose()
    }

    async fn put_accumulator(&mut self, accumulator: &Accumulator) -> Result<(), WriteError> {
        let data = serde_json::to_string(accumulator).map_err(|e| unavailable(&e))?;
        self.run(
            "UPSERT type::record('coverage_accumulator', $key) CONTENT { data: $data } \
             RETURN VALUE record::id(id)",
            vec![
                (
                    "key",
                    format!("{}|{}", accumulator.coverage, accumulator.year),
                ),
                ("data", data),
            ],
        )
        .await?;
        Ok(())
    }

    async fn eligibility(&mut self, coverage: &str) -> Result<Option<Eligibility>, StoreError> {
        self.one(
            "SELECT VALUE [data] FROM type::record('coverage_eligibility', $key)",
            coverage.to_owned(),
        )
        .await?
        .map(|data| serde_json::from_str(&data).map_err(|_| invalid()))
        .transpose()
    }

    async fn put_eligibility(&mut self, eligibility: &Eligibility) -> Result<(), WriteError> {
        let data = serde_json::to_string(eligibility).map_err(|e| unavailable(&e))?;
        self.run(
            "UPSERT type::record('coverage_eligibility', $key) CONTENT { data: $data } \
             RETURN VALUE record::id(id)",
            vec![("key", eligibility.coverage.clone()), ("data", data)],
        )
        .await?;
        Ok(())
    }
}
