//! The Conflict Record review queue over HTTP (#82; synchronization.md § Conflict Record and
//! review queue), for staff signed in with [`CONFLICT_PERMISSION`]:
//!
//! - `GET /` lists the open records, oldest first. Each is flagged `overdue` after 7 days.
//! - `GET /{id}` is one record, both branches: the candidate items as pushed, and the current
//!   head of each conflicting resource here.
//! - `POST /{id}` takes the decision, audited:
//!   - `{"decision": "keep-head", "resource": "Type/id"}` keeps the head;
//!   - `{"decision": "write", "resource": "Type/id", "json": {…}}` writes the corrected version, or
//!     one branch marked entered-in-error, on top of the head.
//!
//!   Either way the record is resolved, and a written version syncs like any other.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_audit::{Agent, Auditor, Entity, Interaction};
use health_persistence::{RawJson, ResourceType, Store as _, Transaction as _};
use health_surrealdb::{ConflictRecord, Resolution, SurrealStore};
use health_time::{SignedDuration, Timestamp};
use serde_json::{Value, json};

/// The permission a reviewer's roles must grant.
pub const CONFLICT_PERMISSION: &str = "sync.resolve-conflicts";

/// How long a record may stay open before it is overdue.
const OVERDUE_HOURS: i64 = 24 * 7;

#[derive(Clone)]
struct Review {
    store: SurrealStore,
    auditor: Auditor,
}

/// The review queue's routes. They expect the subject layer's `Subject`, and its `Agent` for the
/// audit.
pub fn review_router(store: SurrealStore, auditor: Auditor) -> Router {
    Router::new()
        .route("/", get(queue))
        .route("/{id}", get(record).post(decide))
        .with_state(Review { store, auditor })
}

fn problem(status: StatusCode, detail: impl Into<String>) -> Response {
    (
        status,
        Json(json!({ "error": status.canonical_reason(), "detail": detail.into() })),
    )
        .into_response()
}

fn reviewer(subject: Option<Extension<Subject>>) -> Result<Subject, Box<Response>> {
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(problem(
            StatusCode::UNAUTHORIZED,
            "the review queue needs a signed-in reviewer",
        )));
    };
    if !subject.permissions.has(CONFLICT_PERMISSION) {
        return Err(Box::new(problem(
            StatusCode::FORBIDDEN,
            format!("the review queue needs the {CONFLICT_PERMISSION} permission"),
        )));
    }
    Ok(subject)
}

fn summary(record: &ConflictRecord, now: Timestamp) -> Value {
    json!({
        "id": record.id,
        "peer": record.peer,
        "class": record.class,
        "policy": record.policy,
        "status": record.status,
        "openedAt": record.opened_at.to_string(),
        "overdue": now - SignedDuration::from_hours(OVERDUE_HOURS) > record.opened_at,
        "items": record.items,
    })
}

async fn queue(State(review): State<Review>, subject: Option<Extension<Subject>>) -> Response {
    if let Err(response) = reviewer(subject) {
        return *response;
    }
    match review.store.open_conflicts(None).await {
        Ok(records) => {
            let now = Timestamp::now();
            Json(json!({
                "conflicts": records.iter().map(|r| summary(r, now)).collect::<Vec<_>>()
            }))
            .into_response()
        }
        Err(error) => problem(StatusCode::SERVICE_UNAVAILABLE, error.to_string()),
    }
}

/// `Type/id` as a resource type and id.
fn resource(text: &str) -> Option<(ResourceType, health_identifiers::ResourceId)> {
    let (resource_type, id) = text.split_once('/')?;
    Some((resource_type.parse().ok()?, id.parse().ok()?))
}

async fn record(
    State(review): State<Review>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
) -> Response {
    if let Err(response) = reviewer(subject) {
        return *response;
    }
    let record = match review.store.conflict(&id).await {
        Ok(Some(record)) => record,
        Ok(None) => return problem(StatusCode::NOT_FOUND, "no such Conflict Record"),
        Err(error) => return problem(StatusCode::SERVICE_UNAVAILABLE, error.to_string()),
    };
    let mut heads = serde_json::Map::new();
    let Ok(mut tx) = review.store.begin().await else {
        return problem(StatusCode::SERVICE_UNAVAILABLE, "the store is unavailable");
    };
    for item in record.items.as_array().into_iter().flatten() {
        let Some(text) = item["resource"].as_str() else {
            continue;
        };
        if let Some((resource_type, resource_id)) = resource(text)
            && let Ok(Some(head)) = tx.read(&resource_type, resource_id).await
        {
            heads.insert(
                text.to_owned(),
                serde_json::from_str(head.json.as_str()).unwrap_or(Value::Null),
            );
        }
    }
    drop(tx);
    let mut body = summary(&record, Timestamp::now());
    body["candidates"] = serde_json::to_value(&record.candidates).unwrap_or(Value::Null);
    body["heads"] = Value::Object(heads);
    body["resolvedBy"] = json!(record.resolved_by);
    body["resolution"] = json!(record.resolution);
    Json(body).into_response()
}

async fn decide(
    State(review): State<Review>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let subject = match reviewer(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let Some((resource_type, resource_id)) = body["resource"].as_str().and_then(resource) else {
        return problem(
            StatusCode::BAD_REQUEST,
            "`resource` names the conflicting record as Type/id",
        );
    };
    let named = body["resource"].as_str().unwrap_or_default().to_owned();
    match review.store.conflict(&id).await {
        Ok(Some(record))
            if record
                .items
                .as_array()
                .into_iter()
                .flatten()
                .any(|i| i["resource"] == named) => {}
        Ok(Some(_)) => {
            return problem(
                StatusCode::BAD_REQUEST,
                "that record is not in this Conflict Record",
            );
        }
        Ok(None) => return problem(StatusCode::NOT_FOUND, "no such Conflict Record"),
        Err(error) => return problem(StatusCode::SERVICE_UNAVAILABLE, error.to_string()),
    }
    let resolution = match body["decision"].as_str() {
        Some("keep-head") => Resolution::KeepHead {
            resource_type,
            id: resource_id,
        },
        Some("write") if body["json"]["resourceType"].as_str() == Some(resource_type.as_str()) => {
            Resolution::Write {
                resource_type,
                id: resource_id,
                json: RawJson::from_validated(body["json"].to_string()),
            }
        }
        _ => {
            return problem(
                StatusCode::BAD_REQUEST,
                "`decision` is keep-head, or write with the resource's `json`",
            );
        }
    };
    let agent = agent.map_or_else(
        || Agent {
            display: format!("account {}", subject.account),
            ..Agent::unauthenticated()
        },
        |Extension(agent)| agent,
    );
    let event = review.auditor.event(
        &agent,
        Interaction::Update,
        &[Entity {
            what: named,
            labels: Vec::new(),
        }],
        None,
        Timestamp::now(),
        None,
    );
    match review
        .store
        .resolve_conflict(
            &id,
            &format!("account {}", subject.account),
            resolution,
            Some(&event),
        )
        .await
    {
        Ok(version) => {
            Json(json!({ "id": id, "status": "resolved", "resolution": version.to_string() }))
                .into_response()
        }
        Err(health_persistence::WriteError::PreconditionFailed { .. }) => problem(
            StatusCode::CONFLICT,
            "the head moved while you were deciding; reload the record",
        ),
        Err(error) => problem(StatusCode::CONFLICT, error.to_string()),
    }
}
