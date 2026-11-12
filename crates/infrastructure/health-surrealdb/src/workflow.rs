//! The workflow engine's records (migration `0028_workflow`): `health_workflow::WorkflowStore`
//! over the transaction.

use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;
use health_workflow::worklists::TaskView;
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

    async fn replace_tasks(
        &mut self,
        instance: &str,
        tasks: &[TaskView],
    ) -> Result<(), WriteError> {
        self.run(
            "DELETE workflow_task WHERE instance = $instance RETURN NONE",
            vec![("instance", instance.to_owned())],
        )
        .await?;
        for task in tasks {
            self.run(
                "CREATE type::record('workflow_task', $id) CONTENT { data: $data, \
                 instance: $instance, worklist: $worklist, open: <bool>$open, rank: <int>$rank, \
                 authored_on: <datetime>$authored_on } RETURN VALUE record::id(id)",
                vec![
                    ("id", task.id.clone()),
                    ("data", encode(task)?),
                    ("instance", instance.to_owned()),
                    ("worklist", task.worklist.clone()),
                    ("open", task.status.is_open().to_string()),
                    ("rank", (task.priority as u8).to_string()),
                    ("authored_on", task.authored_on.to_string()),
                ],
            )
            .await?;
        }
        Ok(())
    }

    async fn open_tasks(&mut self, worklist: &str) -> Result<Vec<TaskView>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM workflow_task WHERE worklist = $worklist AND open = true \
                 ORDER BY rank DESC, authored_on, id",
                vec![("worklist", worklist.to_owned())],
            )
            .await?;
        decode(rows)
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

    async fn send_candidate(&mut self, instance: &str, candidate: &str) -> Result<(), WriteError> {
        // Candidates go up to the authority as operations (#100), as the ledgers' do.
        self.log_op(crate::sync::WORKFLOW_CANDIDATES, instance, candidate);
        Ok(())
    }

    async fn pending_candidates(
        &mut self,
        limit: u32,
    ) -> Result<Vec<(String, String)>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [record::id(id), data] FROM workflow_candidate \
                 WHERE status = 'pending' ORDER BY received_at LIMIT <int>$limit",
                vec![("limit", limit.to_string())],
            )
            .await?;
        rows.into_iter()
            .map(|row| {
                let [id, data]: [String; 2] = row.try_into().map_err(|_| invalid())?;
                Ok((id, data))
            })
            .collect()
    }

    async fn settle_candidate(
        &mut self,
        op_id: &str,
        conflict: Option<&str>,
    ) -> Result<(), WriteError> {
        let Some(reason) = conflict else {
            self.run(
                "UPDATE type::record('workflow_candidate', $id) SET status = 'applied' \
                 RETURN VALUE record::id(id)",
                vec![("id", op_id.to_owned())],
            )
            .await?;
            return Ok(());
        };
        // The Conflict Record (ADR 0020): the candidate as it was pushed is its candidate branch,
        // reviewed in the queue like any other; keeping the head discards it.
        let (class, policy) = crate::sync::class_of("RequestOrchestration");
        let [peer, item]: [String; 2] = self
            .rows(
                "SELECT VALUE [peer, item] FROM type::record('workflow_candidate', $id)",
                vec![("id", op_id.to_owned())],
            )
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| WriteError::Store(invalid()))?
            .try_into()
            .map_err(|_| WriteError::Store(invalid()))?;
        self.run(
            "UPDATE type::record('workflow_candidate', $id) SET status = 'conflict', \
             reason = $reason RETURN VALUE record::id(id)",
            vec![("id", op_id.to_owned()), ("reason", reason.to_owned())],
        )
        .await?;
        self.run(
            "UPSERT type::record('sync_conflict', $id) SET peer = $peer, items = $items, \
             candidates = $candidates, class = $class, policy = $policy, status = 'open', \
             opened_at = time::now() RETURN VALUE record::id(id)",
            vec![
                ("id", op_id.to_owned()),
                ("peer", peer),
                (
                    "items",
                    serde_json::json!([{ "op_id": op_id, "class": class, "reason": reason }])
                        .to_string(),
                ),
                ("candidates", format!("[{item}]")),
                ("class", class.to_owned()),
                ("policy", policy.to_owned()),
            ],
        )
        .await?;
        Ok(())
    }
}
