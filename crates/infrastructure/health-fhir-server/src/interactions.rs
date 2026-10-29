//! read, vread, create, update, delete and capabilities.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::header::LOCATION;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store, StoredResource, Transaction, VersionMeta,
    WriteError, WriteRequest,
};
use serde_json::Value;

use crate::http::{Return, fhir_json, if_match, prefer, require_json_body, version_headers};
use crate::outcome::{FhirError, operation_outcome};
use crate::{FhirServer, capability_statement, resource_id, resource_type};

/// `GET /metadata`.
pub(crate) async fn metadata(State(server): State<FhirServer>) -> Response {
    let statement = capability_statement(&server.store.registry(), &server.base, server.started);
    fhir_json(StatusCode::OK, statement.to_string())
}

fn ok_resource(stored: &StoredResource) -> Response {
    let mut response = fhir_json(StatusCode::OK, stored.json.as_str().to_owned());
    version_headers(&mut response, &stored.version);
    response
}

/// `GET /{type}/{id}`: 200, 404 when unknown, 410 when deleted.
pub(crate) async fn read(
    State(server): State<FhirServer>,
    Path((type_name, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, FhirError> {
    let resource_type = resource_type(&type_name)?;
    let not_found = || FhirError::not_found(format!("{type_name}/{id} is not known"));
    let id = resource_id(&id).ok_or_else(not_found)?;
    let mut tx = server.store.begin().await?;
    let stored = tx.read(&resource_type, id).await?.ok_or_else(not_found)?;
    if stored.version.deleted {
        return Err(FhirError::gone());
    }
    if headers
        .get("if-none-match")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim() == crate::etag(stored.version.version_id))
    {
        let mut response = Response::new(axum::body::Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        version_headers(&mut response, &stored.version);
        return Ok(response);
    }
    Ok(ok_resource(&stored))
}

/// `GET /{type}/{id}/_history/{vid}`: 200, 404 when unknown, 410 for a delete version.
pub(crate) async fn vread(
    State(server): State<FhirServer>,
    Path((type_name, id, vid)): Path<(String, String, String)>,
) -> Result<Response, FhirError> {
    let resource_type = resource_type(&type_name)?;
    let not_found =
        || FhirError::not_found(format!("{type_name}/{id}/_history/{vid} is not known"));
    let id = resource_id(&id).ok_or_else(not_found)?;
    let vid: OpId = vid.parse().map_err(|_| not_found())?;
    let mut tx = server.store.begin().await?;
    let entry = tx
        .version(&resource_type, id, vid)
        .await?
        .ok_or_else(not_found)?;
    if entry.version.deleted {
        return Err(FhirError::gone());
    }
    let mut response = fhir_json(StatusCode::OK, entry.json.as_str().to_owned());
    version_headers(&mut response, &entry.version);
    Ok(response)
}

/// The request body as a resource of `resource_type`, with the id it carries. Validation tiers
/// 1–2 replace this shape check (#59).
pub(crate) fn body(
    resource_type: &ResourceType,
    headers: &HeaderMap,
    bytes: &Bytes,
) -> Result<(RawJson, Option<String>), FhirError> {
    require_json_body(headers)?;
    let text =
        std::str::from_utf8(bytes).map_err(|_| FhirError::bad_request("the body is not UTF-8"))?;
    let value: Value = serde_json::from_str(text)
        .map_err(|e| FhirError::bad_request(format!("the body is not JSON: {e}")))?;
    let object = value
        .as_object()
        .ok_or_else(|| FhirError::bad_request("the body is not a resource"))?;
    if object.get("resourceType").and_then(Value::as_str) != Some(resource_type.as_str()) {
        return Err(FhirError::bad_request(format!(
            "the body is not a {resource_type} resource"
        )));
    }
    let id = object.get("id").and_then(Value::as_str).map(str::to_owned);
    Ok((RawJson::from_validated(text), id))
}

/// One attempt of a write use case.
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

impl From<health_persistence::StoreError> for Attempt {
    fn from(error: health_persistence::StoreError) -> Self {
        Self::Fail(error.into())
    }
}

impl From<FhirError> for Attempt {
    fn from(error: FhirError) -> Self {
        Self::Fail(error)
    }
}

/// What a write did.
pub(crate) struct Written {
    pub(crate) created: bool,
    pub(crate) version: VersionMeta,
    pub(crate) stored: RawJson,
}

impl FhirServer {
    /// Writes a version in one transaction, choosing the precondition from the head read inside
    /// it, and reruns the whole use case on a write-write conflict (persistence.md).
    async fn put_version(
        &self,
        resource_type: &ResourceType,
        id: ResourceId,
        json: &RawJson,
        decide: impl Fn(Option<&StoredResource>) -> Result<Precondition, FhirError>,
    ) -> Result<Written, FhirError> {
        let mut attempt = 1;
        loop {
            let result: Result<Written, Attempt> = async {
                let mut tx = self.store.begin().await?;
                let head = tx.read(resource_type, id).await?;
                if head.as_ref().is_some_and(|h| h.version.deleted) {
                    return Err(FhirError::gone().into());
                }
                let precondition = decide(head.as_ref())?;
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
                    FhirError::from(health_persistence::StoreError::Unavailable(
                        "written head is missing".into(),
                    ))
                })?;
                tx.commit().await?;
                Ok(Written {
                    created: head.is_none(),
                    version,
                    stored: stored.json,
                })
            }
            .await;
            match result {
                Ok(written) => return Ok(written),
                Err(Attempt::Conflict)
                    if self.retry.should_retry(attempt, &WriteError::Conflict) =>
                {
                    attempt += 1;
                    Self::back_off(attempt).await;
                }
                Err(Attempt::Conflict) => return Err(WriteError::Conflict.into()),
                Err(Attempt::Fail(error)) => return Err(error),
            }
        }
    }

    pub(crate) fn written(
        &self,
        resource_type: &ResourceType,
        id: ResourceId,
        written: Written,
        ret: Return,
    ) -> Response {
        let status = if written.created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        };
        let body = match ret {
            Return::Representation => written.stored.as_str().to_owned(),
            Return::Minimal => String::new(),
            Return::OperationOutcome => operation_outcome(
                "information",
                "informational",
                if written.created {
                    "created"
                } else {
                    "updated"
                },
            )
            .to_string(),
        };
        let mut response = fhir_json(status, body);
        version_headers(&mut response, &written.version);
        if written.created {
            let location = self.url(&format!(
                "{resource_type}/{id}/_history/{}",
                written.version.version_id
            ));
            if let Ok(value) = HeaderValue::from_str(&location) {
                response.headers_mut().insert(LOCATION, value);
            }
        }
        response
    }
}

/// `POST /{type}`: the server mints a UUIDv7 and ignores any id in the body (R5 http). With
/// `If-None-Exist` it is a conditional create.
pub(crate) async fn create(
    State(server): State<FhirServer>,
    Path(type_name): Path<String>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<Response, FhirError> {
    let resource_type = resource_type(&type_name)?;
    let (json, _ignored_id) = body(&resource_type, &headers, &bytes)?;
    if let Some(condition) = headers.get("if-none-exist") {
        let condition = condition
            .to_str()
            .map_err(|_| FhirError::bad_request("If-None-Exist is not ASCII"))?;
        return crate::conditional::create(&server, &resource_type, condition, &json, &headers)
            .await;
    }
    let id = ResourceId::new();
    let written = server
        .put_version(&resource_type, id, &json, |_| {
            Ok(Precondition::MustNotExist)
        })
        .await?;
    Ok(server.written(&resource_type, id, written, prefer(&headers).0))
}

/// `PUT /{type}/{id}`: update, or update-as-create with a client UUID. Updating an existing
/// resource requires `If-Match` (persistence.md); without it the server answers 428.
pub(crate) async fn update(
    State(server): State<FhirServer>,
    Path((type_name, id)): Path<(String, String)>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<Response, FhirError> {
    let resource_type = resource_type(&type_name)?;
    let id_value: ResourceId = id.parse().map_err(|_| {
        FhirError::bad_request("resource ids are client-generated UUIDs (ADR 0013)")
    })?;
    let (json, body_id) = body(&resource_type, &headers, &bytes)?;
    if body_id.as_deref() != Some(id.as_str()) {
        return Err(FhirError::bad_request(
            "the resource id must be present and match the URL",
        ));
    }
    let expected = if_match(&headers)?;
    let written = server
        .put_version(&resource_type, id_value, &json, |head| {
            Ok(match (head, expected) {
                (None, None) => Precondition::MustNotExist,
                (_, Some(version)) => Precondition::IfMatch(version),
                (Some(_), None) => {
                    return Err(FhirError::new(
                        StatusCode::PRECONDITION_REQUIRED,
                        "business-rule",
                        "updates require If-Match with the current ETag",
                    ));
                }
            })
        })
        .await?;
    Ok(server.written(&resource_type, id_value, written, prefer(&headers).0))
}

/// `DELETE /{type}/{id}`: writes a tombstone (204). Deleting a deleted resource is 204 again;
/// an unknown one is 404. `If-Match` is honoured when sent.
pub(crate) async fn delete(
    State(server): State<FhirServer>,
    Path((type_name, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, FhirError> {
    let resource_type = resource_type(&type_name)?;
    let not_found = || FhirError::not_found(format!("{type_name}/{id} is not known"));
    let id = resource_id(&id).ok_or_else(not_found)?;
    let expected = if_match(&headers)?;
    let mut attempt = 1;
    let version = loop {
        let result: Result<Option<VersionMeta>, Attempt> = async {
            let mut tx = server.store.begin().await?;
            let head = tx.read(&resource_type, id).await?.ok_or_else(not_found)?;
            if head.version.deleted {
                return Ok(None);
            }
            let version = tx.delete(&resource_type, id, OpId::new(), expected).await?;
            tx.commit().await?;
            Ok(Some(version))
        }
        .await;
        match result {
            Ok(version) => break version,
            Err(Attempt::Conflict) if server.retry.should_retry(attempt, &WriteError::Conflict) => {
                attempt += 1;
                FhirServer::back_off(attempt).await;
            }
            Err(Attempt::Conflict) => return Err(WriteError::Conflict.into()),
            Err(Attempt::Fail(error)) => return Err(error),
        }
    };
    let mut response = Response::new(axum::body::Body::empty());
    *response.status_mut() = StatusCode::NO_CONTENT;
    if let Some(version) = version {
        version_headers(&mut response, &version);
    }
    Ok(response)
}
