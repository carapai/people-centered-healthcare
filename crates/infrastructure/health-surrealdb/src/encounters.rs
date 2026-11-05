//! Encounters and patient-flow queues (migration `0033_encounters_and_queues`):
//! `health_encounters::EncounterStore` and `health_patient_flow::QueueStore` over the
//! transaction.

use health_encounters::{Encounter, EncounterStore};
use health_patient_flow::{QueueEntry, QueueStore};
use health_persistence::{StoreError, WriteError};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::store::SurrealTx;

fn encode<T: Serialize>(value: &T) -> Result<String, WriteError> {
    serde_json::to_string(value)
        .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))
}

fn decode<T: DeserializeOwned>(rows: Vec<Vec<String>>) -> Result<Vec<T>, StoreError> {
    rows.into_iter()
        .filter_map(|r| r.into_iter().next())
        .map(|data| {
            serde_json::from_str(&data).map_err(|_| {
                StoreError::Unavailable("an encounter or queue row has an invalid shape".into())
            })
        })
        .collect()
}

impl EncounterStore for SurrealTx {
    async fn put_encounter(&mut self, encounter: &Encounter) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('encounter_state', $id) CONTENT { data: $data } \
             RETURN VALUE record::id(id)",
            vec![("id", encounter.id.clone()), ("data", encode(encounter)?)],
        )
        .await?;
        Ok(())
    }

    async fn encounter(&mut self, id: &str) -> Result<Option<Encounter>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('encounter_state', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        Ok(decode(rows)?.into_iter().next())
    }
}

fn entry_binds(entry: &QueueEntry) -> Result<Vec<(&'static str, String)>, WriteError> {
    Ok(vec![
        ("id", entry.id.clone()),
        ("data", encode(entry)?),
        ("encounter", entry.encounter.clone()),
        ("service_point", entry.service_point.clone()),
        ("open", entry.status.is_open().to_string()),
    ])
}

impl QueueStore for SurrealTx {
    async fn add_entry(&mut self, entry: &QueueEntry) -> Result<bool, WriteError> {
        let kept = self
            .rows(
                "SELECT VALUE [record::id(id)] FROM type::record('queue_entry', $id)",
                vec![("id", entry.id.clone())],
            )
            .await?;
        if !kept.is_empty() {
            return Ok(false);
        }
        self.run(
            "CREATE type::record('queue_entry', $id) CONTENT { data: $data, \
             encounter: $encounter, service_point: $service_point, open: <bool>$open } \
             RETURN VALUE record::id(id)",
            entry_binds(entry)?,
        )
        .await?;
        Ok(true)
    }

    async fn put_entry(&mut self, entry: &QueueEntry) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('queue_entry', $id) CONTENT { data: $data, \
             encounter: $encounter, service_point: $service_point, open: <bool>$open } \
             RETURN VALUE record::id(id)",
            entry_binds(entry)?,
        )
        .await?;
        Ok(())
    }

    async fn entry(&mut self, id: &str) -> Result<Option<QueueEntry>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('queue_entry', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        Ok(decode(rows)?.into_iter().next())
    }

    async fn open_entries(&mut self, service_point: &str) -> Result<Vec<QueueEntry>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM queue_entry WHERE service_point = $service_point \
                 AND open = true ORDER BY id",
                vec![("service_point", service_point.to_owned())],
            )
            .await?;
        decode(rows)
    }

    async fn entries_of_encounter(
        &mut self,
        encounter: &str,
    ) -> Result<Vec<QueueEntry>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM queue_entry WHERE encounter = $encounter ORDER BY id",
                vec![("encounter", encounter.to_owned())],
            )
            .await?;
        decode(rows)
    }
}
