//! `_history` at instance, type and system level: Bundle type `history`, newest first (oldest
//! last), deletes included, with `_since` and `_count` (persistence.md § REST semantics).

use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::Response;
use health_fhir_search::MAX_COUNT;
use health_persistence::{Store, Transaction};
use health_surrealdb::{DEFAULT_COUNT, HistoryEntry, HistoryScope};
use health_time::Timestamp;
use serde_json::{Map, json};

use crate::bundle::{Entry, bundle};
use crate::http::{fhir_json, require_json_format};
use crate::outcome::FhirError;
use crate::{FhirServer, etag, pairs, resource_id, resource_type};
use crate::{access, audit};
use health_audit::Interaction;

struct Params {
    since: Option<Timestamp>,
    count: usize,
}

fn params(query: Option<&str>) -> Result<Params, FhirError> {
    let pairs = pairs(query);
    require_json_format(&pairs)?;
    let mut params = Params {
        since: None,
        count: usize::try_from(DEFAULT_COUNT).unwrap_or(50),
    };
    for (name, value) in &pairs {
        match name.as_str() {
            "_since" => {
                params.since = Some(value.parse().map_err(|_| {
                    FhirError::bad_request("_since must be an instant with a time zone")
                })?);
            }
            "_count" => {
                let count: u32 = value
                    .parse()
                    .map_err(|_| FhirError::bad_request("_count must be a non-negative integer"))?;
                params.count = usize::try_from(count.min(MAX_COUNT)).unwrap_or(0);
            }
            "_format" | "_pretty" => {}
            "_at" | "_list" | "_sort" => {
                return Err(FhirError::not_supported(format!(
                    "{name} is not supported on _history yet"
                )));
            }
            other => {
                return Err(FhirError::bad_request(format!(
                    "unknown _history parameter {other}"
                )));
            }
        }
    }
    Ok(params)
}

fn entry(server: &FhirServer, e: HistoryEntry) -> Entry {
    let path = format!("{}/{}", e.resource_type, e.id);
    let (method, url, status) = match (e.version.deleted, e.version.version_seq) {
        (true, _) => ("DELETE", path.clone(), "204"),
        (false, 1) => ("POST", e.resource_type.to_string(), "201"),
        (false, _) => ("PUT", path.clone(), "200"),
    };
    let mut rest = Map::new();
    rest.insert("request".into(), json!({ "method": method, "url": url }));
    rest.insert(
        "response".into(),
        json!({
            "status": status,
            "etag": etag(e.version.version_id),
            "lastModified": format!("{:.3}", e.version.last_updated),
        }),
    );
    Entry {
        full_url: Some(server.url(&path)),
        resource: (!e.version.deleted).then(|| e.json.as_str().to_owned()),
        rest,
    }
}

async fn respond(
    server: &FhirServer,
    scope: HistoryScope,
    path: &str,
    query: Option<&str>,
) -> Result<Response, FhirError> {
    let params = params(query)?;
    let mut tx = server.store.begin().await?;
    match &scope {
        HistoryScope::Instance(resource_type, id) => {
            let not_found = || FhirError::not_found(format!("{resource_type}/{id} is not known"));
            let head = tx.read(resource_type, *id).await?.ok_or_else(not_found)?;
            access::readable(&head.json, not_found)?;
        }
        HistoryScope::Type(resource_type) => {
            access::search_policy(server, resource_type.as_str(), None)?;
        }
        HistoryScope::System => {}
    }
    let mut entries = tx.history(&scope, params.since, params.count).await?;
    // Versions the subject may not read are left out (a page may come back short).
    entries.retain(|e| access::may_read(&e.json));
    let served = entries
        .iter()
        .filter(|e| !e.version.deleted)
        .map(|e| &e.json);
    if audit::read(&mut tx, Interaction::History, served)
        .await
        .map_err(audit::failed)?
    {
        tx.commit().await?;
    }
    let self_link = match query.filter(|q| !q.is_empty()) {
        Some(q) => server.url(&format!("{path}?{q}")),
        None => server.url(path),
    };
    let entries = entries.into_iter().map(|e| entry(server, e)).collect();
    let body = bundle("history", None, &[("self", self_link)], entries);
    Ok(fhir_json(StatusCode::OK, body))
}

/// `GET /{type}/{id}/_history`.
pub(crate) async fn instance(
    State(server): State<FhirServer>,
    Path((type_name, id)): Path<(String, String)>,
    RawQuery(query): RawQuery,
) -> Result<Response, FhirError> {
    let resource_type = resource_type(&type_name)?;
    let id = resource_id(&id)
        .ok_or_else(|| FhirError::not_found(format!("{type_name}/{id} is not known")))?;
    let path = format!("{resource_type}/{id}/_history");
    respond(
        &server,
        HistoryScope::Instance(resource_type, id),
        &path,
        query.as_deref(),
    )
    .await
}

/// `GET /{type}/_history`.
pub(crate) async fn of_type(
    State(server): State<FhirServer>,
    Path(type_name): Path<String>,
    RawQuery(query): RawQuery,
) -> Result<Response, FhirError> {
    let resource_type = resource_type(&type_name)?;
    let path = format!("{resource_type}/_history");
    respond(
        &server,
        HistoryScope::Type(resource_type),
        &path,
        query.as_deref(),
    )
    .await
}

/// `GET /_history`.
pub(crate) async fn system(
    State(server): State<FhirServer>,
    RawQuery(query): RawQuery,
) -> Result<Response, FhirError> {
    respond(&server, HistoryScope::System, "_history", query.as_deref()).await
}
