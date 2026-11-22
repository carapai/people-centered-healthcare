//! Indicators computed on the node (migration `0062_indicator_summaries`; #554):
//! `health_analytics::IndicatorStore` over the transaction. Membership is keyed
//! `[measure, period_type, period, patient]` and summaries `[measure, period_type, period,
//! facility]`, so a period's membership is one record-id range.

use health_analytics::{
    IndicatorStore, MemberKey, PatientOutcome, Period, PeriodType, Position, StoredSummary,
};
use health_persistence::{ResourceType, StoreError, WriteError};

use crate::store::{SurrealTx, table};

/// Patients with more records than this are read up to it.
const MOST_RECORDS: usize = 100_000;

fn invalid() -> StoreError {
    StoreError::Unavailable("an indicator row has an invalid shape".into())
}

fn kind_name(kind: PeriodType) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn kind_of(name: &str) -> Result<PeriodType, StoreError> {
    serde_json::from_value(serde_json::Value::String(name.to_owned())).map_err(|_| invalid())
}

fn store_error(error: impl std::fmt::Display) -> WriteError {
    WriteError::Store(StoreError::Unavailable(error.to_string()))
}

fn patient_type() -> Result<ResourceType, StoreError> {
    "Patient".parse().map_err(|_| invalid())
}

impl IndicatorStore for SurrealTx {
    async fn members(
        &mut self,
        measure: &str,
        period: &Period,
    ) -> Result<Vec<PatientOutcome>, StoreError> {
        self.rows(
            "SELECT VALUE [data] FROM indicator_member:[$m, $k, $p]..[$m, $k, $p, {}] ORDER BY id",
            vec![
                ("m", measure.to_owned()),
                ("k", kind_name(period.kind)),
                ("p", period.id.clone()),
            ],
        )
        .await?
        .into_iter()
        .filter_map(|r| r.into_iter().next())
        .map(|data| serde_json::from_str(&data).map_err(|_| invalid()))
        .collect()
    }

    async fn memberships_of(&mut self, patient: &str) -> Result<Vec<MemberKey>, StoreError> {
        self.rows(
            "SELECT VALUE array::slice(record::id(id), 0, 3) FROM indicator_member \
             WHERE patient = $patient",
            vec![("patient", patient.to_owned())],
        )
        .await?
        .into_iter()
        .map(|row| match row.as_slice() {
            [measure, kind, period] => Ok(MemberKey {
                measure: measure.clone(),
                kind: kind_of(kind)?,
                period: period.clone(),
            }),
            _ => Err(invalid()),
        })
        .collect()
    }

    async fn put_member(
        &mut self,
        measure: &str,
        period: &Period,
        outcome: &PatientOutcome,
    ) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('indicator_member', [$m, $k, $p, $patient]) CONTENT { \
             patient: $patient, data: $data } RETURN NONE",
            vec![
                ("m", measure.to_owned()),
                ("k", kind_name(period.kind)),
                ("p", period.id.clone()),
                ("patient", outcome.patient.clone()),
                ("data", serde_json::to_string(outcome).map_err(store_error)?),
            ],
        )
        .await?;
        Ok(())
    }

    async fn remove_member(
        &mut self,
        measure: &str,
        period: &Period,
        patient: &str,
    ) -> Result<(), WriteError> {
        self.run(
            "DELETE type::record('indicator_member', [$m, $k, $p, $patient]) RETURN NONE",
            vec![
                ("m", measure.to_owned()),
                ("k", kind_name(period.kind)),
                ("p", period.id.clone()),
                ("patient", patient.to_owned()),
            ],
        )
        .await?;
        Ok(())
    }

    async fn put_summary(&mut self, summary: &StoredSummary) -> Result<(), WriteError> {
        let kind = kind_name(summary.period.kind);
        self.run(
            "UPSERT type::record('indicator_summary', [$m, $k, $p, $f]) CONTENT { \
             measure: $m, period_type: $k, period: $p, facility: $f, start: $start, end: $end, \
             report: $report, computed_at: time::now() } RETURN NONE",
            vec![
                ("m", summary.measure.clone()),
                ("k", kind),
                ("p", summary.period.id.clone()),
                ("f", summary.facility.clone()),
                ("start", summary.period.start.to_string()),
                ("end", summary.period.end.to_string()),
                ("report", summary.report.clone()),
            ],
        )
        .await?;
        Ok(())
    }

    async fn remove_summary(
        &mut self,
        measure: &str,
        period: &Period,
        facility: &str,
    ) -> Result<(), WriteError> {
        self.run(
            "DELETE type::record('indicator_summary', [$m, $k, $p, $f]) RETURN NONE",
            vec![
                ("m", measure.to_owned()),
                ("k", kind_name(period.kind)),
                ("p", period.id.clone()),
                ("f", facility.to_owned()),
            ],
        )
        .await?;
        Ok(())
    }

    async fn summaries(
        &mut self,
        kind: PeriodType,
        period: &str,
    ) -> Result<Vec<StoredSummary>, StoreError> {
        let parsed = Period::parse(kind, period).map_err(|_| invalid())?;
        self.rows(
            "SELECT VALUE [measure, facility, report] FROM indicator_summary \
             WHERE period_type = $k AND period = $p ORDER BY measure, facility",
            vec![("k", kind_name(kind)), ("p", period.to_owned())],
        )
        .await?
        .into_iter()
        .map(|row| match <[String; 3]>::try_from(row) {
            Ok([measure, facility, report]) => Ok(StoredSummary {
                measure,
                period: parsed.clone(),
                facility,
                report,
            }),
            Err(_) => Err(invalid()),
        })
        .collect()
    }

    async fn clear_indicators(&mut self) -> Result<(), WriteError> {
        self.run(
            "DELETE indicator_member RETURN NONE; DELETE indicator_summary RETURN NONE",
            vec![],
        )
        .await?;
        Ok(())
    }

    async fn indicator_position(&mut self) -> Result<Option<Position>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [epoch, <string>seq] FROM indicator_state:node",
                vec![],
            )
            .await?;
        rows.into_iter()
            .next()
            .map(|row| match row.as_slice() {
                [epoch, seq] => Ok(Position {
                    epoch: epoch.clone(),
                    seq: seq.parse().map_err(|_| invalid())?,
                }),
                _ => Err(invalid()),
            })
            .transpose()
    }

    async fn set_indicator_position(&mut self, position: &Position) -> Result<(), WriteError> {
        self.run(
            "UPSERT indicator_state:node CONTENT { epoch: $epoch, seq: <int>$seq } RETURN NONE",
            vec![
                ("epoch", position.epoch.clone()),
                ("seq", position.seq.to_string()),
            ],
        )
        .await?;
        Ok(())
    }

    async fn patient_ids(&mut self) -> Result<Vec<String>, StoreError> {
        let table = table(&patient_type()?)?;
        Ok(self
            .rows(
                &format!(
                    "SELECT VALUE [<string>record::id(id)] FROM {table} WHERE deleted = false \
                     ORDER BY id"
                ),
                vec![],
            )
            .await?
            .into_iter()
            .filter_map(|r| r.into_iter().next())
            .collect())
    }

    async fn patient_records(&mut self, id: &str) -> Result<Option<Vec<String>>, StoreError> {
        let Ok(id) = id.parse() else {
            return Ok(None);
        };
        let patient = patient_type()?;
        let Some(head) = self.head(&patient, id).await? else {
            return Ok(None);
        };
        if head.version.deleted {
            return Ok(None);
        }
        let mut found = vec![head.json.as_str().to_owned()];
        for (resource_type, record) in self.referrers(&patient, id, MOST_RECORDS).await? {
            if let Some(head) = self.head(&resource_type, record).await?
                && !head.version.deleted
            {
                found.push(head.json.as_str().to_owned());
            }
        }
        Ok(Some(found))
    }

    async fn changed_versions(
        &mut self,
        resource_type: &str,
        id: &str,
        version_seq: u64,
    ) -> Result<Vec<String>, StoreError> {
        let (Ok(resource_type), Ok(id)) = (
            resource_type.parse::<ResourceType>(),
            id.parse::<health_identifiers::ResourceId>(),
        ) else {
            return Ok(Vec::new());
        };
        let Ok(table) = table(&resource_type) else {
            return Ok(Vec::new());
        };
        let from = version_seq.saturating_sub(1);
        Ok(self
            .rows(
                &format!(
                    "SELECT VALUE [json] FROM {table}_history:[$id, {from}]..=[$id, {version_seq}]"
                ),
                vec![("id", id.to_string())],
            )
            .await?
            .into_iter()
            .filter_map(|r| r.into_iter().next())
            .collect())
    }
}
