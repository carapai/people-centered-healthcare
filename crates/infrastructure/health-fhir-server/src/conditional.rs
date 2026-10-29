//! Conditional update and delete routes (R5 http `#cond-update`, `#cdelete`); conditional
//! create is `POST` with `If-None-Exist` ([`crate::interactions::create`]). The matrices are in
//! [`crate::ops`]:
//!
//! | Interaction | 0 matches | 1 match | >1 matches |
//! |---|---|---|---|
//! | create (`If-None-Exist`) | create | 200 with the match | 412 |
//! | update (`PUT [type]?…`) | create (no id, or a new id); 409 if the id exists | update it (400 if the body id differs; `If-Match` required) | 412 |
//! | delete (`DELETE [type]?…`) | 204 | delete it | 412 (`conditionalDelete = single`) |
//!
//! A create under a condition locks it ([`health_surrealdb::SurrealTx::lock_condition`]), so two
//! concurrent creates under the same condition make one resource: the loser's rerun finds the
//! winner's. Matches are found with the caller's [`health_fhir_search::SearchPolicy`].

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::HeaderMap;
use axum::response::Response;
use health_identifiers::ResourceId;
use health_persistence::{Store, Transaction};

use crate::http::{if_match, prefer};
use crate::interactions::{Written, body, deleted};
use crate::ops::{self, Attempt, Condition};
use crate::outcome::FhirError;
use crate::references;
use crate::{FhirServer, resource_type};

/// `PUT /{type}?…`.
pub(crate) async fn update(
    State(server): State<FhirServer>,
    Path(type_name): Path<String>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<Response, FhirError> {
    let resource_type = resource_type(&type_name)?;
    let condition = Condition::parse(
        &server,
        &resource_type,
        query.as_deref().unwrap_or_default(),
    )?;
    let (json, body_id) = body(&server, &resource_type, &headers, &bytes).await?;
    let body_id: Option<ResourceId> = body_id
        .map(|id| {
            id.parse().map_err(|_| {
                FhirError::bad_request("resource ids are client-generated UUIDs (ADR 0013)")
            })
        })
        .transpose()?;
    let expected = if_match(&headers)?;
    let ret = prefer(&headers).0;
    retrying!(server, async {
        let mut tx = server.store.begin().await?;
        let plan = ops::plan_conditional_update(
            &server,
            &mut tx,
            &resource_type,
            &condition,
            body_id,
            expected,
        )
        .await?;
        let (version, stored) =
            ops::write(&mut tx, &resource_type, plan.id, &json, plan.precondition).await?;
        references::check_written(&server, &mut tx, &stored).await?;
        tx.commit().await?;
        let written = Written {
            created: plan.creates(),
            version,
            stored,
        };
        Ok::<_, Attempt>(server.written(&resource_type, plan.id, written, ret))
    })
}

/// `DELETE /{type}?…`.
pub(crate) async fn delete(
    State(server): State<FhirServer>,
    Path(type_name): Path<String>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, FhirError> {
    let resource_type = resource_type(&type_name)?;
    let condition = Condition::parse(
        &server,
        &resource_type,
        query.as_deref().unwrap_or_default(),
    )?;
    let expected = if_match(&headers)?;
    retrying!(server, async {
        let mut tx = server.store.begin().await?;
        let version =
            ops::conditional_delete(&server, &mut tx, &resource_type, &condition, expected).await?;
        if let Some((id, _)) = version {
            references::check_deletable(&mut tx, &resource_type, id).await?;
        }
        tx.commit().await?;
        Ok::<_, Attempt>(deleted(version.map(|(_, v)| v)))
    })
}
