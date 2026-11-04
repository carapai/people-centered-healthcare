//! The FHIR projections of workflow state (#99; workflow-engine.md § Definitions vs FHIR; ADR
//! 0002): every projected Task as a FHIR **Task**, and each instance as a
//! **RequestOrchestration** whose actions are its Tasks. Both are owned by Workflow and written
//! through the unit of work, in the transaction that changed the instance, so `/Task` and
//! `/RequestOrchestration` read like any other resource.
//!
//! - A version is written only when the projection changed, so a step that leaves a Task as it
//!   was adds no version.
//! - Workflow state never reaches a clinical resource: these are the only resources written
//!   here. CarePlan belongs to Care Coordination, so program-style CarePlans come through its
//!   typed actions, not from here.
//! - A claim that lapses while nobody touches the instance stays `in-progress` in the stored
//!   Task until the next step; the worklists apply the lapse as they read.

use health_identifiers::{OpId, ResourceId};
use health_persistence::{Precondition, RawJson, ResourceType, Transaction, WriteError};
use health_workflow::InstanceRecord;
use health_workflow::worklists::TaskView;
use serde_json::{Map, Value, json};

use crate::WorkflowError;

/// The identifier system of workflow instances, under the platform's canonical base.
pub const INSTANCE_SYSTEM: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/sid/workflow-instance";

fn reference(who: &str) -> Value {
    if who.contains('/') {
        json!({ "reference": who })
    } else {
        json!({ "display": who })
    }
}

/// The FHIR Task of `task`.
pub fn task(task: &TaskView) -> Value {
    let mut resource = Map::new();
    resource.insert("resourceType".into(), json!("Task"));
    resource.insert("id".into(), json!(task.id));
    resource.insert(
        "identifier".into(),
        json!([{ "system": INSTANCE_SYSTEM, "value": task.instance }]),
    );
    resource.insert(
        "partOf".into(),
        json!([{ "reference": format!("RequestOrchestration/{}", task.instance) }]),
    );
    resource.insert("status".into(), json!(task.status.code()));
    resource.insert("businessStatus".into(), json!({ "text": task.state }));
    resource.insert("intent".into(), json!("order"));
    resource.insert("priority".into(), json!(task.priority.code()));
    resource.insert("focus".into(), json!({ "reference": task.context }));
    resource.insert("authoredOn".into(), json!(task.authored_on.to_string()));
    resource.insert("lastModified".into(), json!(task.last_modified.to_string()));
    resource.insert(
        "requestedPerformer".into(),
        json!([{ "concept": { "text": task.role } }]),
    );
    if let Some(owner) = &task.owner {
        resource.insert("owner".into(), reference(owner));
    }
    if let Some(form) = &task.form {
        resource.insert(
            "input".into(),
            json!([{ "type": { "text": "form" }, "valueCanonical": form }]),
        );
    }
    Value::Object(resource)
}

/// The FHIR RequestOrchestration of `record`, with an action per Task.
pub fn orchestration(record: &InstanceRecord, tasks: &[TaskView]) -> Value {
    // A cancelled instance ends with no active state; a completed one ends in a final state.
    let status = if record.state.running {
        "active"
    } else if record.state.configuration.is_empty() {
        "revoked"
    } else {
        "completed"
    };
    let mut resource = Map::new();
    resource.insert("resourceType".into(), json!("RequestOrchestration"));
    resource.insert("id".into(), json!(record.id));
    resource.insert(
        "identifier".into(),
        json!([{ "system": INSTANCE_SYSTEM, "value": record.id }]),
    );
    resource.insert("instantiatesUri".into(), json!([record.definition]));
    resource.insert("status".into(), json!(status));
    resource.insert("intent".into(), json!("order"));
    if record.context.starts_with("Encounter/") {
        resource.insert("encounter".into(), json!({ "reference": record.context }));
    }
    let actions: Vec<Value> = tasks
        .iter()
        .map(|t| {
            json!({
                "title": t.state,
                "priority": t.priority.code(),
                "resource": { "reference": format!("Task/{}", t.id) },
            })
        })
        .collect();
    if !actions.is_empty() {
        resource.insert("action".into(), Value::Array(actions));
    }
    Value::Object(resource)
}

fn without_meta(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.remove("meta");
    }
    value
}

fn store_error(error: impl ToString) -> WorkflowError {
    WorkflowError::Store(error.to_string())
}

/// Writes `resource` as `kind/id`, unless its head already says the same.
pub(crate) async fn put<T: Transaction>(
    tx: &mut T,
    kind: &str,
    id: &str,
    resource: Value,
) -> Result<(), WorkflowError> {
    let resource_type: ResourceType = kind.parse().map_err(store_error)?;
    let id = ResourceId::from_uuid(id.parse().map_err(store_error)?).map_err(store_error)?;
    let head = tx.read(&resource_type, id).await?;
    let precondition = match &head {
        Some(head) => {
            let stored: Value = serde_json::from_str(head.json.as_str()).map_err(store_error)?;
            if !head.version.deleted && without_meta(stored) == resource {
                return Ok(());
            }
            Precondition::IfMatch(head.version.version_id)
        }
        None => Precondition::MustNotExist,
    };
    tx.write(health_persistence::WriteRequest {
        resource_type,
        id,
        json: RawJson::from_validated(resource.to_string()),
        op_id: OpId::new(),
        precondition,
    })
    .await
    .map_err(|error: WriteError| store_error(error))?;
    Ok(())
}
