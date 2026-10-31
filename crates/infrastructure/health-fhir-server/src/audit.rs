//! Auditing every write and every sensitive read (#66; security.md: AuditEvent is written only
//! by the audit module, `health-audit`).
//!
//! The router's [`scope`] middleware puts the request's [`Agent`] (inserted as a request
//! extension by the authentication layer, or `unauthenticated`) and the server's [`Auditor`] in a
//! task-local for the request. The write and read steps then append their AuditEvent in the same
//! transaction as the access, so neither can commit without the other.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use health_audit::{Agent, Auditor, Entity, Interaction};
use health_persistence::{RawJson, Transaction};
use health_time::Timestamp;
use serde_json::Value;

use crate::FhirServer;
use crate::ops::Attempt;

#[derive(Clone)]
struct Scope {
    auditor: Arc<Auditor>,
    agent: Agent,
}

tokio::task_local! {
    static SCOPE: Scope;
}

/// Middleware: runs the request inside its audit scope, when the server audits.
pub(crate) async fn scope(
    State(server): State<FhirServer>,
    request: Request,
    next: Next,
) -> Response {
    let Some(auditor) = server.audit.clone() else {
        return next.run(request).await;
    };
    let agent = request
        .extensions()
        .get::<Agent>()
        .cloned()
        .unwrap_or_else(Agent::unauthenticated);
    SCOPE
        .scope(Scope { auditor, agent }, next.run(request))
        .await
}

fn current() -> Option<Scope> {
    SCOPE.try_with(Clone::clone).ok()
}

/// The patient a resource is about: itself, or its `subject` / `patient` when that is a Patient.
fn patient_of(json: &Value) -> Option<String> {
    if json["resourceType"] == "Patient" {
        return json["id"].as_str().map(|id| format!("Patient/{id}"));
    }
    ["subject", "patient"]
        .iter()
        .filter_map(|field| json[*field]["reference"].as_str())
        .find(|r| r.starts_with("Patient/"))
        .map(str::to_owned)
}

async fn append<T: Transaction>(
    tx: &mut T,
    scope: &Scope,
    interaction: Interaction,
    records: &[Value],
) -> Result<(), Attempt> {
    let entities: Vec<Entity> = records.iter().filter_map(Entity::of).collect();
    let patient = records.iter().find_map(patient_of);
    let event = scope.auditor.event(
        &scope.agent,
        interaction,
        &entities,
        patient.as_deref(),
        Timestamp::now(),
        None,
    );
    scope.auditor.append(tx, &event).await?;
    Ok(())
}

/// After a create, update or delete in `tx`: its AuditEvent, always. `stored` is the version
/// written (for a delete, the head that was deleted).
pub(crate) async fn written<T: Transaction>(
    tx: &mut T,
    interaction: Interaction,
    stored: &RawJson,
) -> Result<(), Attempt> {
    let Some(scope) = current() else {
        return Ok(());
    };
    let json: Value = serde_json::from_str(stored.as_str()).unwrap_or(Value::Null);
    append(tx, &scope, interaction, &[json]).await
}

/// After a read, vread, search or history in `tx`: an AuditEvent when a record is sensitive or
/// the access relied on break-glass, naming the sensitive records (all of them under
/// break-glass). Returns whether one was appended, so the caller commits.
pub(crate) async fn read<'a, T: Transaction>(
    tx: &mut T,
    interaction: Interaction,
    records: impl IntoIterator<Item = &'a RawJson>,
) -> Result<bool, Attempt> {
    let Some(scope) = current() else {
        return Ok(false);
    };
    let parsed: Vec<Value> = records
        .into_iter()
        .filter_map(|r| serde_json::from_str(r.as_str()).ok())
        .collect();
    if !scope.auditor.must_audit_read(&scope.agent, &parsed) {
        return Ok(false);
    }
    let named: Vec<Value> = if scope.agent.break_glass {
        parsed
    } else {
        parsed
            .into_iter()
            .filter(|r| scope.auditor.is_sensitive(r))
            .collect()
    };
    append(tx, &scope, interaction, &named).await?;
    Ok(true)
}

/// A read handler's failure: the audit append is a write, so it can conflict.
pub(crate) fn failed(attempt: Attempt) -> crate::FhirError {
    match attempt {
        Attempt::Conflict => health_persistence::WriteError::Conflict.into(),
        Attempt::Fail(error) => error,
    }
}

/// The audit module is the only writer of AuditEvent: REST writes to it are refused.
pub(crate) fn writable(resource_type: &str) -> Result<(), crate::FhirError> {
    if resource_type == health_audit::AUDIT_EVENT {
        return Err(crate::FhirError::new(
            axum::http::StatusCode::METHOD_NOT_ALLOWED,
            "not-supported",
            "AuditEvent is append-only and written only by the audit module",
        ));
    }
    Ok(())
}
