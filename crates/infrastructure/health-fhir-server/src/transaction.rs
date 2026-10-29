//! `transaction` and `batch` Bundles (R5 http `#transaction`; persistence.md: a transaction is
//! one SurrealDB transaction, all or nothing; batch entries are independent transactions).
//!
//! A transaction runs in the R5 order, whatever the order of its entries:
//! 1. DELETE;
//! 2. POST, planned first so every create has its id;
//! 3. PUT;
//! 4. GET, after the writes.
//!
//! `urn:uuid:` `fullUrl`s of created and updated entries are rewritten to `Type/id` in every
//! reference before anything is written. Any failure fails the whole transaction with that
//! entry's status and an OperationOutcome naming it, and a write-write conflict reruns it whole.
//! A batch runs each entry alone. Its entries cannot refer to each other, so a `urn:uuid:`
//! reference fails that entry.

use std::collections::BTreeMap;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use health_identifiers::{OpId, ResourceId};
use health_persistence::{Precondition, RawJson, ResourceType, Store, Transaction, VersionMeta};
use health_surrealdb::SurrealTx;
use serde_json::{Map, Value, json};

use crate::bundle::{Entry, bundle};
use crate::http::{Return, fhir_json, prefer, require_json_body};
use crate::ops::{self, Attempt, Condition, CreateTarget, UpdatePlan};
use crate::outcome::{FhirError, operation_outcome};
use crate::{FhirServer, etag, resource_type};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Method {
    Get,
    Post,
    Put,
    Delete,
}

/// What an entry's `request.url` names.
enum Target {
    /// `[type]` (POST).
    Type,
    /// `[type]/[id]`.
    Instance(ResourceId),
    /// `[type]/[id]/_history/[vid]` (GET).
    Version(ResourceId, OpId),
    /// `[type]?…` (conditional PUT or DELETE).
    Conditional(Condition),
}

struct Request {
    index: usize,
    full_url: Option<String>,
    method: Method,
    resource_type: ResourceType,
    target: Target,
    resource: Option<Value>,
    if_match: Option<OpId>,
    if_none_exist: Option<Condition>,
}

/// One entry's result.
#[derive(Default)]
struct Outcome {
    status: u16,
    resource_type: Option<ResourceType>,
    id: Option<ResourceId>,
    version: Option<VersionMeta>,
    location: bool,
    resource: Option<RawJson>,
    /// GET results always carry their resource; writes follow `Prefer: return`.
    from_read: bool,
    outcome: Option<Value>,
}

impl Outcome {
    fn written(
        resource_type: &ResourceType,
        id: ResourceId,
        created: bool,
        version: VersionMeta,
        stored: RawJson,
    ) -> Self {
        Self {
            status: if created { 201 } else { 200 },
            resource_type: Some(resource_type.clone()),
            id: Some(id),
            version: Some(version),
            location: created,
            resource: Some(stored),
            from_read: false,
            outcome: None,
        }
    }

    fn deleted(version: Option<VersionMeta>) -> Self {
        Self {
            status: 204,
            version,
            ..Self::default()
        }
    }

    fn failed(error: &FhirError) -> Self {
        Self {
            status: error.status.as_u16(),
            outcome: Some(operation_outcome("error", error.code, &error.diagnostics)),
            ..Self::default()
        }
    }
}

fn entry_error(index: usize, error: FhirError) -> FhirError {
    FhirError {
        diagnostics: format!("entry {index}: {}", error.diagnostics),
        ..error
    }
}

fn parse_request(server: &FhirServer, index: usize, entry: &Value) -> Result<Request, FhirError> {
    let request = entry
        .get("request")
        .ok_or_else(|| FhirError::bad_request("an entry has no request"))?;
    let method = match request.get("method").and_then(Value::as_str) {
        Some("GET") => Method::Get,
        Some("POST") => Method::Post,
        Some("PUT") => Method::Put,
        Some("DELETE") => Method::Delete,
        Some(other) => {
            return Err(FhirError::not_supported(format!(
                "{other} entries are not supported"
            )));
        }
        None => return Err(FhirError::bad_request("request.method is required")),
    };
    let url = request
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| FhirError::bad_request("request.url is required"))?;
    let url = url
        .strip_prefix(server.base.as_ref())
        .unwrap_or(url)
        .trim_start_matches('/');
    let (path, query) = url
        .split_once('?')
        .map_or((url, None), |(p, q)| (p, Some(q)));
    let segments: Vec<&str> = path.split('/').collect();
    let resource_type = resource_type(segments[0])?;
    let not_found = || FhirError::not_found(format!("{path} is not known"));
    let target = match (method, segments.as_slice(), query) {
        (Method::Get, _, Some(_)) => {
            return Err(FhirError::not_supported(
                "search entries are not supported in Bundles yet",
            ));
        }
        (Method::Post, [_], None) => Target::Type,
        (Method::Put | Method::Delete, [_], Some(query)) => {
            Target::Conditional(Condition::parse(server, &resource_type, query)?)
        }
        (Method::Put, [_, id], None) => Target::Instance(id.parse().map_err(|_| {
            FhirError::bad_request("resource ids are client-generated UUIDs (ADR 0013)")
        })?),
        (Method::Get | Method::Delete, [_, id], None) => {
            Target::Instance(id.parse().map_err(|_| not_found())?)
        }
        (Method::Get, [_, id, "_history", vid], None) => Target::Version(
            id.parse().map_err(|_| not_found())?,
            vid.parse().map_err(|_| not_found())?,
        ),
        _ => {
            return Err(FhirError::bad_request(format!(
                "{url} is not a valid request.url here"
            )));
        }
    };
    let resource = entry.get("resource").cloned();
    if matches!(method, Method::Post | Method::Put) {
        let body = resource
            .as_ref()
            .ok_or_else(|| FhirError::bad_request("POST and PUT entries need a resource"))?;
        if body.get("resourceType").and_then(Value::as_str) != Some(resource_type.as_str()) {
            return Err(FhirError::bad_request(format!(
                "the resource is not a {resource_type}"
            )));
        }
        if let Target::Instance(id) = &target
            && body.get("id").and_then(Value::as_str) != Some(id.to_string().as_str())
        {
            return Err(FhirError::bad_request(
                "the resource id must be present and match request.url",
            ));
        }
    }
    let if_match = request
        .get("ifMatch")
        .and_then(Value::as_str)
        .map(|v| {
            v.trim()
                .trim_start_matches("W/")
                .trim_matches('"')
                .parse()
                .map_err(|_| FhirError::bad_request("ifMatch must be W/\"<versionId>\""))
        })
        .transpose()?;
    let if_none_exist = match request.get("ifNoneExist").and_then(Value::as_str) {
        Some(text) if method == Method::Post => {
            Some(Condition::parse(server, &resource_type, text)?)
        }
        Some(_) => return Err(FhirError::bad_request("ifNoneExist is only for POST")),
        None => None,
    };
    Ok(Request {
        index,
        full_url: entry
            .get("fullUrl")
            .and_then(Value::as_str)
            .map(str::to_owned),
        method,
        resource_type,
        target,
        resource,
        if_match,
        if_none_exist,
    })
}

/// Replaces every `reference` naming a `urn:uuid:` entry with its `Type/id`. An unresolved
/// `urn:uuid:` reference is an error: it can only mean an entry that is not in the Bundle.
fn rewrite(value: &mut Value, ids: &BTreeMap<String, String>) -> Result<(), FhirError> {
    match value {
        Value::Object(object) => {
            for (key, child) in object.iter_mut() {
                if key == "reference"
                    && let Value::String(reference) = child
                    && reference.starts_with("urn:uuid:")
                {
                    let resolved = ids.get(reference.as_str()).ok_or_else(|| {
                        FhirError::bad_request(format!(
                            "{reference} does not name an entry of this Bundle"
                        ))
                    })?;
                    *reference = resolved.clone();
                } else {
                    rewrite(child, ids)?;
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite(item, ids)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn raw(value: &Value) -> RawJson {
    RawJson::from_validated(value.to_string())
}

async fn read(tx: &mut SurrealTx, request: &Request) -> Result<Outcome, Attempt> {
    let (id, version) = match request.target {
        Target::Instance(id) => (id, None),
        Target::Version(id, vid) => (id, Some(vid)),
        _ => return Err(FhirError::bad_request("unsupported GET").into()),
    };
    let not_found = || FhirError::not_found(format!("{}/{id} is not known", request.resource_type));
    let (version, json) = match version {
        None => {
            let head = tx
                .read(&request.resource_type, id)
                .await?
                .ok_or_else(not_found)?;
            (head.version, head.json)
        }
        Some(vid) => {
            let entry = tx
                .version(&request.resource_type, id, vid)
                .await?
                .ok_or_else(not_found)?;
            (entry.version, entry.json)
        }
    };
    if version.deleted {
        return Err(FhirError::gone().into());
    }
    Ok(Outcome {
        status: 200,
        resource_type: Some(request.resource_type.clone()),
        id: Some(id),
        version: Some(version),
        resource: Some(json),
        from_read: true,
        ..Outcome::default()
    })
}

async fn delete(
    server: &FhirServer,
    tx: &mut SurrealTx,
    request: &Request,
) -> Result<Outcome, Attempt> {
    let version = match &request.target {
        Target::Instance(id) => {
            ops::delete(tx, &request.resource_type, *id, request.if_match).await?
        }
        Target::Conditional(condition) => {
            ops::conditional_delete(
                server,
                tx,
                &request.resource_type,
                condition,
                request.if_match,
            )
            .await?
        }
        _ => return Err(FhirError::bad_request("unsupported DELETE").into()),
    };
    Ok(Outcome::deleted(version))
}

async fn plan_update(
    server: &FhirServer,
    tx: &mut SurrealTx,
    request: &Request,
) -> Result<UpdatePlan, Attempt> {
    match &request.target {
        Target::Instance(id) => {
            ops::plan_update(tx, &request.resource_type, *id, request.if_match).await
        }
        Target::Conditional(condition) => {
            let body_id = request
                .resource
                .as_ref()
                .and_then(|r| r.get("id"))
                .and_then(Value::as_str)
                .map(|id| {
                    id.parse::<ResourceId>().map_err(|_| {
                        Attempt::Fail(FhirError::bad_request(
                            "resource ids are client-generated UUIDs (ADR 0013)",
                        ))
                    })
                })
                .transpose()?;
            ops::plan_conditional_update(
                server,
                tx,
                &request.resource_type,
                condition,
                body_id,
                request.if_match,
            )
            .await
        }
        _ => Err(FhirError::bad_request("unsupported PUT").into()),
    }
}

/// Runs a transaction Bundle's entries in one transaction.
async fn transaction(server: &FhirServer, requests: &[Request]) -> Result<Vec<Outcome>, Attempt> {
    let mut tx = server.store.begin().await?;
    let mut outcomes: Vec<Option<Outcome>> = requests.iter().map(|_| None).collect();
    fn fail(request: &Request) -> impl Fn(Attempt) -> Attempt + '_ {
        move |error| match error {
            Attempt::Fail(e) => Attempt::Fail(entry_error(request.index, e)),
            conflict @ Attempt::Conflict => conflict,
        }
    }
    let of = |method: Method| requests.iter().filter(move |r| r.method == method);

    for request in of(Method::Delete) {
        outcomes[request.index] = Some(
            delete(server, &mut tx, request)
                .await
                .map_err(fail(request))?,
        );
    }
    let mut ids: BTreeMap<String, String> = BTreeMap::new();
    let mut creates = Vec::new();
    for request in of(Method::Post) {
        let target = ops::plan_create(
            server,
            &mut tx,
            &request.resource_type,
            request.if_none_exist.as_ref(),
        )
        .await
        .map_err(fail(request))?;
        let id = match &target {
            CreateTarget::New(id) => *id,
            CreateTarget::Existing(existing) => existing.id,
        };
        if let Some(full_url) = &request.full_url {
            ids.insert(full_url.clone(), format!("{}/{id}", request.resource_type));
        }
        creates.push((request, target));
    }
    let mut updates = Vec::new();
    for request in of(Method::Put) {
        let plan = plan_update(server, &mut tx, request)
            .await
            .map_err(fail(request))?;
        if let Some(full_url) = &request.full_url {
            ids.insert(
                full_url.clone(),
                format!("{}/{}", request.resource_type, plan.id),
            );
        }
        updates.push((request, plan));
    }
    for (request, target) in creates {
        let outcome = match target {
            CreateTarget::Existing(existing) => Outcome {
                status: 200,
                resource_type: Some(request.resource_type.clone()),
                id: Some(existing.id),
                version: Some(existing.version),
                resource: Some(existing.json),
                ..Outcome::default()
            },
            CreateTarget::New(id) => {
                let mut body = request.resource.clone().unwrap_or(Value::Null);
                rewrite(&mut body, &ids).map_err(|e| entry_error(request.index, e))?;
                let (version, stored) = ops::write(
                    &mut tx,
                    &request.resource_type,
                    id,
                    &raw(&body),
                    Precondition::MustNotExist,
                )
                .await
                .map_err(fail(request))?;
                Outcome::written(&request.resource_type, id, true, version, stored)
            }
        };
        outcomes[request.index] = Some(outcome);
    }
    for (request, plan) in updates {
        let mut body = request.resource.clone().unwrap_or(Value::Null);
        rewrite(&mut body, &ids).map_err(|e| entry_error(request.index, e))?;
        let (version, stored) = ops::write(
            &mut tx,
            &request.resource_type,
            plan.id,
            &raw(&body),
            plan.precondition,
        )
        .await
        .map_err(fail(request))?;
        outcomes[request.index] = Some(Outcome::written(
            &request.resource_type,
            plan.id,
            plan.creates(),
            version,
            stored,
        ));
    }
    for request in of(Method::Get) {
        outcomes[request.index] = Some(read(&mut tx, request).await.map_err(fail(request))?);
    }
    tx.commit().await?;
    Ok(outcomes
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect())
}

/// Runs one batch entry in its own transaction.
async fn batch_entry(server: &FhirServer, request: &Request) -> Result<Outcome, Attempt> {
    let mut tx = server.store.begin().await?;
    let empty = BTreeMap::new();
    let outcome = match request.method {
        Method::Get => read(&mut tx, request).await?,
        Method::Delete => delete(server, &mut tx, request).await?,
        Method::Post => match ops::plan_create(
            server,
            &mut tx,
            &request.resource_type,
            request.if_none_exist.as_ref(),
        )
        .await?
        {
            CreateTarget::Existing(existing) => Outcome {
                status: 200,
                resource_type: Some(request.resource_type.clone()),
                id: Some(existing.id),
                version: Some(existing.version),
                resource: Some(existing.json),
                ..Outcome::default()
            },
            CreateTarget::New(id) => {
                let mut body = request.resource.clone().unwrap_or(Value::Null);
                rewrite(&mut body, &empty)?;
                let (version, stored) = ops::write(
                    &mut tx,
                    &request.resource_type,
                    id,
                    &raw(&body),
                    Precondition::MustNotExist,
                )
                .await?;
                Outcome::written(&request.resource_type, id, true, version, stored)
            }
        },
        Method::Put => {
            let plan = plan_update(server, &mut tx, request).await?;
            let mut body = request.resource.clone().unwrap_or(Value::Null);
            rewrite(&mut body, &empty)?;
            let (version, stored) = ops::write(
                &mut tx,
                &request.resource_type,
                plan.id,
                &raw(&body),
                plan.precondition,
            )
            .await?;
            Outcome::written(
                &request.resource_type,
                plan.id,
                plan.creates(),
                version,
                stored,
            )
        }
    };
    tx.commit().await?;
    Ok(outcome)
}

fn response_entry(server: &FhirServer, outcome: Outcome, ret: Return) -> Entry {
    let status = StatusCode::from_u16(outcome.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut response = Map::new();
    response.insert(
        "status".into(),
        json!(format!(
            "{} {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or_default()
        )),
    );
    let path = match (&outcome.resource_type, outcome.id) {
        (Some(t), Some(id)) => Some(format!("{t}/{id}")),
        _ => None,
    };
    if let Some(version) = &outcome.version {
        response.insert("etag".into(), json!(etag(version.version_id)));
        response.insert(
            "lastModified".into(),
            json!(format!("{:.3}", version.last_updated)),
        );
        if outcome.location
            && let Some(path) = &path
        {
            response.insert(
                "location".into(),
                json!(server.url(&format!("{path}/_history/{}", version.version_id))),
            );
        }
    }
    if let Some(outcome) = outcome.outcome {
        response.insert("outcome".into(), outcome);
    }
    let mut rest = Map::new();
    rest.insert("response".into(), Value::Object(response));
    let include = outcome.from_read || ret == Return::Representation;
    Entry {
        full_url: path.as_deref().map(|p| server.url(p)),
        resource: outcome
            .resource
            .filter(|_| include)
            .map(|r| r.as_str().to_owned()),
        rest,
    }
}

/// `POST /` with a `transaction` or `batch` Bundle.
pub(crate) async fn post(
    State(server): State<FhirServer>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<Response, FhirError> {
    require_json_body(&headers)?;
    let bundle_value: Value = std::str::from_utf8(&bytes)
        .ok()
        .and_then(|text| serde_json::from_str(text).ok())
        .ok_or_else(|| FhirError::bad_request("the body is not JSON"))?;
    if bundle_value.get("resourceType").and_then(Value::as_str) != Some("Bundle") {
        return Err(FhirError::bad_request("POST to the base takes a Bundle"));
    }
    let kind = bundle_value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let entries = bundle_value
        .get("entry")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let ret = prefer(&headers).0;
    let outcomes = match kind {
        "transaction" => {
            let requests = entries
                .iter()
                .enumerate()
                .map(|(i, e)| parse_request(&server, i, e).map_err(|err| entry_error(i, err)))
                .collect::<Result<Vec<_>, _>>()?;
            check_unique(&requests)?;
            retrying!(server, transaction(&server, &requests))?
        }
        "batch" => {
            let mut outcomes = Vec::with_capacity(entries.len());
            for (i, entry) in entries.iter().enumerate() {
                let outcome = match parse_request(&server, i, entry) {
                    Ok(request) => retrying!(server, batch_entry(&server, &request))
                        .unwrap_or_else(|e| Outcome::failed(&e)),
                    Err(error) => Outcome::failed(&error),
                };
                outcomes.push(outcome);
            }
            outcomes
        }
        _ => {
            return Err(FhirError::bad_request(
                "the Bundle type must be transaction or batch",
            ));
        }
    };
    let entries = outcomes
        .into_iter()
        .map(|o| response_entry(&server, o, ret))
        .collect();
    let body = bundle(&format!("{kind}-response"), None, &[], entries);
    Ok(fhir_json(StatusCode::OK, body))
}

/// A transaction may name each resource and each `fullUrl` only once (R5 http).
fn check_unique(requests: &[Request]) -> Result<(), FhirError> {
    let mut targets = BTreeMap::new();
    let mut full_urls = BTreeMap::new();
    for request in requests {
        if let Target::Instance(id) = &request.target
            && request.method != Method::Get
            && let Some(first) =
                targets.insert(format!("{}/{id}", request.resource_type), request.index)
        {
            return Err(FhirError::bad_request(format!(
                "entries {first} and {} change the same resource",
                request.index
            )));
        }
        if let Some(full_url) = &request.full_url
            && let Some(first) = full_urls.insert(full_url.clone(), request.index)
        {
            return Err(FhirError::bad_request(format!(
                "entries {first} and {} share the fullUrl {full_url}",
                request.index
            )));
        }
    }
    Ok(())
}
