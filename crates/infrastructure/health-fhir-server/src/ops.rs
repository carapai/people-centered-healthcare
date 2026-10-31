//! The write interactions as steps inside an open transaction, shared by the single-request
//! handlers and by transaction and batch Bundles.
//!
//! Each write is split into a **plan** (which id it targets and with what precondition, read
//! inside the transaction) and the **write** itself. A transaction Bundle plans every create
//! first, so references to `urn:uuid:` entries can be rewritten before anything is written.

use axum::http::StatusCode;
use health_fhir_search::{Handling, SearchQuery, parse_query};
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    Precondition, RawJson, ResourceType, StoreError, StoredResource, Transaction, VersionMeta,
    WriteError, WriteRequest,
};
use health_surrealdb::SurrealTx;

use crate::outcome::FhirError;
use crate::{FhirServer, pairs};
use crate::{access, audit};
use health_access_policy::Interaction as Decide;
use health_audit::Interaction;

/// One attempt of a use case: a write-write conflict reruns it, anything else ends it.
pub(crate) enum Attempt {
    Conflict,
    Fail(FhirError),
}

impl From<WriteError> for Attempt {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::Conflict => Self::Conflict,
            other => Self::Fail(other.into()),
        }
    }
}

impl From<StoreError> for Attempt {
    fn from(error: StoreError) -> Self {
        Self::Fail(error.into())
    }
}

impl From<FhirError> for Attempt {
    fn from(error: FhirError) -> Self {
        Self::Fail(error)
    }
}

/// A parsed condition and its pairs, which key the condition lock.
pub(crate) struct Condition {
    query: SearchQuery,
    pairs: Vec<(String, String)>,
}

impl Condition {
    /// Parses `text`: search parameters, optionally as `[type]?…`.
    pub(crate) fn parse(
        server: &FhirServer,
        resource_type: &ResourceType,
        text: &str,
    ) -> Result<Self, FhirError> {
        let text = text.trim().trim_start_matches('?');
        let text = match text.split_once('?') {
            Some((path, query)) if !path.contains('=') => query,
            _ => text,
        };
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
        Ok(Self { query, pairs })
    }
}

/// The resources a condition matches: none or one, or 412 for several.
async fn matches(
    server: &FhirServer,
    tx: &mut SurrealTx,
    condition: &Condition,
) -> Result<Option<StoredResource>, Attempt> {
    let policy = access::search_policy(
        server,
        condition.query.resource_type.as_str(),
        access::search_patient(&condition.pairs),
    )?;
    let page = tx
        .search(&condition.query, policy.as_ref(), None)
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
    Ok(page.resources.into_iter().next())
}

/// Where a create lands.
pub(crate) enum CreateTarget {
    /// A new resource with this id.
    New(ResourceId),
    /// Conditional create found one match: nothing is written.
    Existing(StoredResource),
}

/// Plans a create: a fresh UUIDv7, or the one match of `If-None-Exist`. A create under a
/// condition locks it, so concurrent creates under the same condition make one resource.
pub(crate) async fn plan_create(
    server: &FhirServer,
    tx: &mut SurrealTx,
    resource_type: &ResourceType,
    condition: Option<&Condition>,
) -> Result<CreateTarget, Attempt> {
    if let Some(condition) = condition {
        if let Some(existing) = matches(server, tx, condition).await? {
            return Ok(CreateTarget::Existing(existing));
        }
        tx.lock_condition(resource_type, &condition.pairs).await?;
    }
    Ok(CreateTarget::New(ResourceId::new()))
}

/// The id and precondition an update writes with.
#[derive(Debug, Clone, Copy)]
pub(crate) struct UpdatePlan {
    pub(crate) id: ResourceId,
    pub(crate) precondition: Precondition,
}

impl UpdatePlan {
    pub(crate) fn creates(&self) -> bool {
        self.precondition == Precondition::MustNotExist
    }
}

fn if_match_required() -> FhirError {
    FhirError::new(
        StatusCode::PRECONDITION_REQUIRED,
        "business-rule",
        "updates require If-Match with the current ETag",
    )
}

/// Plans `PUT [type]/[id]`: update-as-create for a new id; an existing resource needs
/// `If-Match` (428 without it); a deleted one is 410.
pub(crate) async fn plan_update(
    tx: &mut SurrealTx,
    resource_type: &ResourceType,
    id: ResourceId,
    expected: Option<OpId>,
) -> Result<UpdatePlan, Attempt> {
    let head = tx.read(resource_type, id).await?;
    if head.as_ref().is_some_and(|h| h.version.deleted) {
        return Err(FhirError::gone().into());
    }
    let precondition = match (head, expected) {
        (_, Some(version)) => Precondition::IfMatch(version),
        (None, None) => Precondition::MustNotExist,
        (Some(_), None) => return Err(if_match_required().into()),
    };
    Ok(UpdatePlan { id, precondition })
}

/// Plans `PUT [type]?…` by the R5 conditional-update matrix.
pub(crate) async fn plan_conditional_update(
    server: &FhirServer,
    tx: &mut SurrealTx,
    resource_type: &ResourceType,
    condition: &Condition,
    body_id: Option<ResourceId>,
    expected: Option<OpId>,
) -> Result<UpdatePlan, Attempt> {
    if let Some(matched) = matches(server, tx, condition).await? {
        if body_id.is_some_and(|id| id != matched.id) {
            return Err(FhirError::bad_request(
                "the body id does not match the resource the condition found",
            )
            .into());
        }
        let expected = expected.ok_or_else(if_match_required)?;
        return Ok(UpdatePlan {
            id: matched.id,
            precondition: Precondition::IfMatch(expected),
        });
    }
    let id = match body_id {
        Some(id) => {
            if tx.read(resource_type, id).await?.is_some() {
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
    tx.lock_condition(resource_type, &condition.pairs).await?;
    Ok(UpdatePlan {
        id,
        precondition: Precondition::MustNotExist,
    })
}

/// Writes a version and returns it with the stored bytes.
pub(crate) async fn write(
    tx: &mut SurrealTx,
    resource_type: &ResourceType,
    id: ResourceId,
    json: &RawJson,
    precondition: Precondition,
) -> Result<(VersionMeta, RawJson), Attempt> {
    audit::writable(resource_type.as_str())?;
    let interaction = if precondition == Precondition::MustNotExist {
        Interaction::Create
    } else {
        Interaction::Update
    };
    let replaced = match interaction {
        Interaction::Update => tx
            .read(resource_type, id)
            .await?
            .filter(|head| !head.version.deleted),
        _ => None,
    };
    if let Some(head) = &replaced {
        // The version replaced must be updatable too: another facility's record stays theirs.
        access::permitted(Decide::Update, &head.json)?;
    }
    // A facility-scope record names its facility, which never changes (ADR 0057).
    let json = &access::stamp(
        resource_type.as_str(),
        json,
        replaced.as_ref().map(|head| &head.json),
    )?;
    access::permitted(
        if interaction == Interaction::Create {
            Decide::Create
        } else {
            Decide::Update
        },
        json,
    )?;
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
        Attempt::Fail(StoreError::Unavailable("written head is missing".into()).into())
    })?;
    audit::written(tx, interaction, &stored.json).await?;
    Ok((version, stored.json))
}

/// `DELETE [type]/[id]`: the tombstone version, `None` if already deleted, 404 if unknown.
pub(crate) async fn delete(
    tx: &mut SurrealTx,
    resource_type: &ResourceType,
    id: ResourceId,
    expected: Option<OpId>,
) -> Result<Option<VersionMeta>, Attempt> {
    audit::writable(resource_type.as_str())?;
    let head = tx
        .read(resource_type, id)
        .await?
        .ok_or_else(|| FhirError::not_found(format!("{resource_type}/{id} is not known")))?;
    if head.version.deleted {
        return Ok(None);
    }
    access::permitted(Decide::Delete, &head.json)?;
    let version = tx.delete(resource_type, id, OpId::new(), expected).await?;
    audit::written(tx, Interaction::Delete, &head.json).await?;
    Ok(Some(version))
}

/// `DELETE [type]?…`: deletes the one match; `None` when nothing matches.
pub(crate) async fn conditional_delete(
    server: &FhirServer,
    tx: &mut SurrealTx,
    resource_type: &ResourceType,
    condition: &Condition,
    expected: Option<OpId>,
) -> Result<Option<(ResourceId, VersionMeta)>, Attempt> {
    audit::writable(resource_type.as_str())?;
    match matches(server, tx, condition).await? {
        Some(matched) => {
            access::permitted(Decide::Delete, &matched.json)?;
            let version = tx
                .delete(resource_type, matched.id, OpId::new(), expected)
                .await?;
            audit::written(tx, Interaction::Delete, &matched.json).await?;
            Ok(Some((matched.id, version)))
        }
        None => Ok(None),
    }
}
