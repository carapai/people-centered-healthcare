//! read, vread, create, update, delete and capabilities.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::header::LOCATION;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store, StoredResource, Transaction, VersionMeta,
};
use serde_json::Value;

use crate::http::{Return, fhir_json, if_match, prefer, require_json_body, version_headers};
use crate::ops::{self, Attempt, Condition, CreateTarget};
use crate::outcome::{FhirError, operation_outcome};
use crate::references;
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

/// What a write did.
pub(crate) struct Written {
    pub(crate) created: bool,
    pub(crate) version: VersionMeta,
    pub(crate) stored: RawJson,
}

impl FhirServer {
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

    /// The response to a conditional create that found its one match.
    pub(crate) fn existing(existing: &StoredResource, ret: Return) -> Response {
        let body = match ret {
            Return::Minimal => String::new(),
            _ => existing.json.as_str().to_owned(),
        };
        let mut response = fhir_json(StatusCode::OK, body);
        version_headers(&mut response, &existing.version);
        response
    }
}

/// 204, with the tombstone's ETag when this request deleted the resource.
pub(crate) fn deleted(version: Option<VersionMeta>) -> Response {
    let mut response = Response::new(axum::body::Body::empty());
    *response.status_mut() = StatusCode::NO_CONTENT;
    if let Some(version) = version {
        version_headers(&mut response, &version);
    }
    response
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
    let condition = headers
        .get("if-none-exist")
        .map(|v| {
            v.to_str()
                .map_err(|_| FhirError::bad_request("If-None-Exist is not ASCII"))
                .and_then(|text| Condition::parse(&server, &resource_type, text))
        })
        .transpose()?;
    let ret = prefer(&headers).0;
    retrying!(server, async {
        let mut tx = server.store.begin().await?;
        let response =
            match ops::plan_create(&server, &mut tx, &resource_type, condition.as_ref()).await? {
                CreateTarget::Existing(existing) => FhirServer::existing(&existing, ret),
                CreateTarget::New(id) => {
                    let (version, stored) = ops::write(
                        &mut tx,
                        &resource_type,
                        id,
                        &json,
                        Precondition::MustNotExist,
                    )
                    .await?;
                    references::check_written(&server, &mut tx, &stored).await?;
                    tx.commit().await?;
                    let written = Written {
                        created: true,
                        version,
                        stored,
                    };
                    server.written(&resource_type, id, written, ret)
                }
            };
        Ok::<_, Attempt>(response)
    })
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
    let ret = prefer(&headers).0;
    retrying!(server, async {
        let mut tx = server.store.begin().await?;
        let plan = ops::plan_update(&mut tx, &resource_type, id_value, expected).await?;
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

/// `DELETE /{type}/{id}`: writes a tombstone (204). Deleting a deleted resource is 204 again;
/// an unknown one is 404. `If-Match` is honoured when sent.
pub(crate) async fn delete(
    State(server): State<FhirServer>,
    Path((type_name, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, FhirError> {
    let resource_type = resource_type(&type_name)?;
    let id = resource_id(&id)
        .ok_or_else(|| FhirError::not_found(format!("{type_name}/{id} is not known")))?;
    let expected = if_match(&headers)?;
    retrying!(server, async {
        let mut tx = server.store.begin().await?;
        let version = ops::delete(&mut tx, &resource_type, id, expected).await?;
        if version.is_some() {
            references::check_deletable(&mut tx, &resource_type, id).await?;
        }
        tx.commit().await?;
        Ok::<_, Attempt>(deleted(version))
    })
}
