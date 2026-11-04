//! The workflow engine's records (migration `0028_workflow`): `health_workflow::WorkflowStore`
//! over the transaction.

use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;
use health_workflow::{HistoryEntry, InstanceRecord, TimerRecord, TimerStatus, WorkflowStore};
use serde::de::DeserializeOwned;

use crate::store::SurrealTx;

fn invalid() -> StoreError {
    StoreError::Unavailable("a workflow row has an invalid shape".into())
}

fn encode<T: serde::Serialize>(value: &T) -> Result<String, WriteError> {
    serde_json::to_string(value)
        .map_err(|e| WriteError::Store(StoreError::Unavailable(e.to_string())))
}

fn decode<T: DeserializeOwned>(rows: Vec<Vec<String>>) -> Result<Vec<T>, StoreError> {
    rows.into_iter()
        .filter_map(|r| r.into_iter().next())
        .map(|data| serde_json::from_str(&data).map_err(|_| invalid()))
        .collect()
}

/// `instance|seq`, the sequence zero-padded so ids sort in order.
fn history_id(instance: &str, seq: u64) -> String {
    format!("{instance}|{seq:020}")
}

fn status_code(status: TimerStatus) -> &'static str {
    match status {
        TimerStatus::Pending => "pending",
        TimerStatus::Fired => "fired",
        TimerStatus::Cancelled => "cancelled",
    }
}

impl WorkflowStore for SurrealTx {
    async fn create_instance(&mut self, instance: &InstanceRecord) -> Result<(), WriteError> {
        self.run(
            "CREATE type::record('workflow_instance', $id) CONTENT { data: $data, \
             version: <int>$version, definition: $definition, context: $context, \
             running: <bool>$running } RETURN VALUE record::id(id)",
            vec![
                ("id", instance.id.clone()),
                ("data", encode(instance)?),
                ("version", instance.version.to_string()),
                ("definition", instance.definition.clone()),
                ("context", instance.context.clone()),
                ("running", instance.state.running.to_string()),
            ],
        )
        .await?;
        Ok(())
    }

    async fn instance(&mut self, id: &str) -> Result<Option<InstanceRecord>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('workflow_instance', $id)",
                vec![("id", id.to_owned())],
            )
            .await?;
        Ok(decode(rows)?.into_iter().next())
    }

    async fn update_instance(
        &mut self,
        instance: &InstanceRecord,
        before: u64,
    ) -> Result<bool, WriteError> {
        let updated = self
            .run(
                "UPDATE type::record('workflow_instance', $id) SET data = $data, \
                 version = <int>$version, running = <bool>$running \
                 WHERE version = <int>$before RETURN VALUE record::id(id)",
                vec![
                    ("id", instance.id.clone()),
                    ("data", encode(instance)?),
                    ("version", instance.version.to_string()),
                    ("running", instance.state.running.to_string()),
                    ("before", before.to_string()),
                ],
            )
            .await?;
        Ok(!updated.is_empty())
    }

    async fn append_history(&mut self, entry: &HistoryEntry) -> Result<(), WriteError> {
        self.run(
            "CREATE type::record('workflow_history', $id) CONTENT { data: $data, \
             instance: $instance, seq: <int>$seq } RETURN VALUE record::id(id)",
            vec![
                ("id", history_id(&entry.instance, entry.seq)),
                ("data", encode(entry)?),
                ("instance", entry.instance.clone()),
                ("seq", entry.seq.to_string()),
            ],
        )
        .await?;
        Ok(())
    }

    async fn instance_history(&mut self, instance: &str) -> Result<Vec<HistoryEntry>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM workflow_history WHERE instance = $instance ORDER BY seq",
                vec![("instance", instance.to_owned())],
            )
            .await?;
        decode(rows)
    }

    async fn claim_event(&mut self, event_id: &str, at: Timestamp) -> Result<bool, WriteError> {
        let seen = self
            .rows(
                "SELECT VALUE [record::id(id)] FROM type::record('workflow_inbox', $id)",
                vec![("id", event_id.to_owned())],
            )
            .await?;
        if !seen.is_empty() {
            return Ok(false);
        }
        self.run(
            "CREATE type::record('workflow_inbox', $id) CONTENT { processed_at: <datetime>$at } \
             RETURN VALUE record::id(id)",
            vec![("id", event_id.to_owned()), ("at", at.to_string())],
        )
        .await?;
        Ok(true)
    }

    async fn put_timer(&mut self, timer: &TimerRecord) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('workflow_timer', $id) CONTENT { data: $data, \
             instance: $instance, status: $status, fire_at: <datetime>$fire_at } \
             RETURN VALUE record::id(id)",
            vec![
                ("id", format!("{}|{}", timer.instance, timer.id)),
                ("data", encode(timer)?),
                ("instance", timer.instance.clone()),
                ("status", status_code(timer.status).to_owned()),
                ("fire_at", timer.fire_at.to_string()),
            ],
        )
        .await?;
        Ok(())
    }

    async fn due_timers(
        &mut self,
        at: Timestamp,
        limit: u32,
    ) -> Result<Vec<TimerRecord>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM workflow_timer WHERE status = 'pending' \
                 AND fire_at <= <datetime>$at ORDER BY fire_at, id LIMIT <int>$limit",
                vec![("at", at.to_string()), ("limit", limit.to_string())],
            )
            .await?;
        decode(rows)
    }

    async fn timers(&mut self, instance: &str) -> Result<Vec<TimerRecord>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM workflow_timer WHERE instance = $instance ORDER BY id",
                vec![("instance", instance.to_owned())],
            )
            .await?;
        decode(rows)
    }
}
