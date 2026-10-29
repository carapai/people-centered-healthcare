//! Conditional create, update and delete (R5 http `#ccreate`, `#cond-update`, `#cdelete`;
//! persistence.md: resolved through the search index inside the same transaction).
//!
//! | Interaction | 0 matches | 1 match | >1 matches |
//! |---|---|---|---|
//! | create (`If-None-Exist`) | create | 200 with the match | 412 |
//! | update (`PUT [type]?…`) | create (no id, or a new id); 409 if the id exists | update it (400 if the body id differs; `If-Match` required) | 412 |
//! | delete (`DELETE [type]?…`) | 204 | delete it | 412 (`conditionalDelete = single`) |
//!
//! A create under a condition locks it ([`health_surrealdb::SurrealTx::lock_condition`]), so two
//! concurrent creates under the same condition make one resource: the loser's rerun finds the
//! winner's. The match is found with the caller's [`health_fhir_search::SearchPolicy`] applied.

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use health_fhir_search::{Handling, SearchQuery, parse_query};
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store, StoredResource, Transaction, VersionMeta,
    WriteError, WriteRequest,
};
use health_surrealdb::SurrealTx;

use crate::http::{Return, fhir_json, if_match, prefer, version_headers};
use crate::interactions::{Attempt, Written, body};
use crate::outcome::FhirError;
use crate::{FhirServer, pairs, resource_type};

/// The parsed condition and its pairs (the lock key).
struct Condition {
    query: SearchQuery,
    pairs: Vec<(String, String)>,
}

fn condition(
    server: &FhirServer,
    resource_type: &ResourceType,
    text: &str,
) -> Result<Condition, FhirError> {
    let text = text.trim().trim_start_matches('?');
    let text = text.split_once('?').map_or(
        text,
        |(path, query)| if path.contains('=') { text } else { query },
    );
    let pairs = pairs(Some(text));
    if pairs.is_empty() {
        return Err(FhirError::bad_request(
            "a conditional interaction needs search parameters",
        ));
    }
    let mut query = parse_query(
        &server.store.registry(),
        resource_type.as_str(),
        &pairs,
        Handling::Strict,
    )?;
    if let Some((name, _)) = query.control.first() {
        return Err(FhirError::bad_request(format!(
            "{name} cannot be part of a condition"
        )));
    }
    if query.count.is_some() {
        return Err(FhirError::bad_request(
            "_count cannot be part of a condition",
        ));
    }
    // Two matches are enough to tell 0, 1 and many apart.
    query.count = Some(2);
    Ok(Condition { query, pairs })
}

async fn matches(
    server: &FhirServer,
    tx: &mut SurrealTx,
    condition: &Condition,
) -> Result<Vec<StoredResource>, Attempt> {
    let page = tx
        .search(&condition.query, server.policy.as_ref(), None)
        .await
        .map_err(|e| Attempt::Fail(e.into()))?;
    let total = page.total.unwrap_or(page.resources.len());
    if total > 1 {
        return Err(FhirError::new(
            StatusCode::PRECONDITION_FAILED,
            "multiple-matches",
            format!("the condition matches {total} resources"),
        )
        .into());
    }
    Ok(page.resources)
}

async fn write(
    tx: &mut SurrealTx,
    resource_type: &ResourceType,
    id: ResourceId,
    json: &RawJson,
    precondition: Precondition,
) -> Result<(VersionMeta, RawJson), Attempt> {
    let version = tx
        .write(WriteRequest {
            resource_type: resource_type.clone(),
            id,
            json: json.clone(),
            op_id: OpId::new(),
            precondition,
        })
        .await?;
    let stored = tx.read(resource_type, id).await?.ok_or_else(|| {
        Attempt::Fail(
            health_persistence::StoreError::Unavailable("written head is missing".into()).into(),
        )
    })?;
    Ok((version, stored.json))
}

/// What a conditional create or update did.
enum Outcome {
    Written(ResourceId, Written),
    /// Conditional create found one match and wrote nothing.
    Existing(StoredResource),
}

/// Reruns `attempt` on write-write conflicts, as `RetryPolicy` allows.
macro_rules! retrying {
    ($server:expr, $attempt:expr) => {{
        let mut round = 1;
        loop {
            match $attempt.await {
                Ok(value) => break Ok(value),
                Err(Attempt::Conflict)
                    if $server.retry.should_retry(round, &WriteError::Conflict) =>
                {
                    round += 1;
                    FhirServer::back_off(round).await;
                }
                Err(Attempt::Conflict) => break Err(FhirError::from(WriteError::Conflict)),
                Err(Attempt::Fail(error)) => break Err(error),
            }
        }
    }};
}

fn respond(
    server: &FhirServer,
    resource_type: &ResourceType,
    outcome: Outcome,
    ret: Return,
) -> Response {
    match outcome {
        Outcome::Written(id, written) => server.written(resource_type, id, written, ret),
        Outcome::Existing(existing) => {
            let body = match ret {
                Return::Minimal => String::new(),
                _ => existing.json.as_str().to_owned(),
            };
            let mut response = fhir_json(StatusCode::OK, body);
            version_headers(&mut response, &existing.version);
            response
        }
    }
}

/// `POST /{type}` with `If-None-Exist`.
pub(crate) async fn create(
    server: &FhirServer,
    resource_type: &ResourceType,
    condition_text: &str,
    json: &RawJson,
    headers: &HeaderMap,
) -> Result<Response, FhirError> {
    let condition = condition(server, resource_type, condition_text)?;
    let outcome = retrying!(server, async {
        let mut tx = server.store.begin().await?;
        if let Some(existing) = matches(server, &mut tx, &condition)
            .await?
            .into_iter()
            .next()
        {
            return Ok(Outcome::Existing(existing));
        }
        tx.lock_condition(resource_type, &condition.pairs).await?;
        let id = ResourceId::new();
        let (version, stored) =
            write(&mut tx, resource_type, id, json, Precondition::MustNotExist).await?;
        tx.commit().await?;
        Ok::<_, Attempt>(Outcome::Written(
            id,
            Written {
                created: true,
                version,
                stored,
            },
        ))
    })?;
    Ok(respond(server, resource_type, outcome, prefer(headers).0))
}

/// `PUT /{type}?…`.
pub(crate) async fn update(
    State(server): State<FhirServer>,
    Path(type_name): Path<String>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<Response, FhirError> {
    let resource_type = resource_type(&type_name)?;
    let condition = condition(
        &server,
        &resource_type,
        query.as_deref().unwrap_or_default(),
    )?;
    let (json, body_id) = body(&resource_type, &headers, &bytes)?;
    let body_id: Option<ResourceId> = body_id
        .map(|id| {
            id.parse().map_err(|_| {
                FhirError::bad_request("resource ids are client-generated UUIDs (ADR 0013)")
            })
        })
        .transpose()?;
    let expected = if_match(&headers)?;
    let outcome = retrying!(server, async {
        let mut tx = server.store.begin().await?;
        let found = matches(&server, &mut tx, &condition).await?;
        let (id, precondition) = match found.first() {
            Some(matched) => {
                if body_id.is_some_and(|id| id != matched.id) {
                    return Err(FhirError::bad_request(
                        "the body id does not match the resource the condition found",
                    )
                    .into());
                }
                let expected = expected.ok_or_else(|| {
                    FhirError::new(
                        StatusCode::PRECONDITION_REQUIRED,
                        "business-rule",
                        "updates require If-Match with the current ETag",
                    )
                })?;
                (matched.id, Precondition::IfMatch(expected))
            }
            None => {
                let id = match body_id {
                    Some(id) => {
                        if tx.read(&resource_type, id).await?.is_some() {
                            return Err(FhirError::new(
                                StatusCode::CONFLICT,
                                "conflict",
                                "the body id names an existing resource the condition does not match",
                            )
                            .into());
                        }
                        id
                    }
                    None => ResourceId::new(),
                };
                tx.lock_condition(&resource_type, &condition.pairs).await?;
                (id, Precondition::MustNotExist)
            }
        };
        let created = precondition == Precondition::MustNotExist;
        let (version, stored) = write(&mut tx, &resource_type, id, &json, precondition).await?;
        tx.commit().await?;
        Ok::<_, Attempt>(Outcome::Written(
            id,
            Written {
                created,
                version,
                stored,
            },
        ))
    })?;
    Ok(respond(
        &server,
        &resource_type,
        outcome,
        prefer(&headers).0,
    ))
}

/// `DELETE /{type}?…`.
pub(crate) async fn delete(
    State(server): State<FhirServer>,
    Path(type_name): Path<String>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, FhirError> {
    let resource_type = resource_type(&type_name)?;
    let condition = condition(
        &server,
        &resource_type,
        query.as_deref().unwrap_or_default(),
    )?;
    let expected = if_match(&headers)?;
    let version = retrying!(server, async {
        let mut tx = server.store.begin().await?;
        let Some(matched) = matches(&server, &mut tx, &condition)
            .await?
            .into_iter()
            .next()
        else {
            return Ok(None);
        };
        let version = tx
            .delete(&resource_type, matched.id, OpId::new(), expected)
            .await?;
        tx.commit().await?;
        Ok::<_, Attempt>(Some(version))
    })?;
    let mut response = Response::new(axum::body::Body::empty());
    *response.status_mut() = StatusCode::NO_CONTENT;
    if let Some(version) = version {
        version_headers(&mut response, &version);
    }
    Ok(response)
}
