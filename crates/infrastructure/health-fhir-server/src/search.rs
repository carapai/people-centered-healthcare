//! `search-type` over GET and POST `_search`: a `searchset` Bundle with `total`, `self` and
//! keyset `next` links, and the stored bytes of each match.

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use health_fhir_search::{Handling, parse_query};
use health_identifiers::ResourceId;
use health_persistence::Store;
use serde_json::{Map, json};

use crate::bundle::{Entry, bundle};
use crate::http::{fhir_json, prefer, require_json_format};
use crate::outcome::FhirError;
use crate::{FhirServer, pairs, resource_type};

/// The keyset cursor parameter of `next` links: the last resource id of the previous page.
pub const CURSOR: &str = "_cursor";

fn encode(pairs: &[(String, String)]) -> String {
    let mut out = form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        out.append_pair(k, v);
    }
    out.finish()
}

async fn run(
    server: &FhirServer,
    type_name: &str,
    params: Vec<(String, String)>,
    headers: &HeaderMap,
) -> Result<Response, FhirError> {
    let resource_type = resource_type(type_name)?;
    require_json_format(&params)?;
    let mut after: Option<ResourceId> = None;
    let mut search_params = Vec::new();
    for (name, value) in params {
        match name.as_str() {
            CURSOR => {
                after = Some(
                    value
                        .parse()
                        .map_err(|_| FhirError::bad_request("invalid _cursor"))?,
                );
            }
            "_format" | "_pretty" => {}
            _ => search_params.push((name, value)),
        }
    }
    let handling = if prefer(headers).1 {
        Handling::Lenient
    } else {
        Handling::Strict
    };
    let registry = server.store.registry();
    let query = parse_query(&registry, type_name, &search_params, handling)?;
    for (name, value) in &query.control {
        match name.as_str() {
            "_total" if matches!(value.as_str(), "none" | "estimate" | "accurate") => {}
            _ => {
                return Err(FhirError::not_supported(format!(
                    "{name} is not supported yet"
                )));
            }
        }
    }
    let mut tx = server.store.begin().await?;
    let page = tx.search(&query, server.policy.as_ref(), after).await?;

    let used: Vec<(String, String)> = search_params
        .into_iter()
        .filter(|(name, _)| !query.ignored.contains(name))
        .collect();
    let link = |extra: Option<ResourceId>| {
        let mut all = used.clone();
        if let Some(cursor) = extra {
            all.push((CURSOR.to_owned(), cursor.to_string()));
        }
        let query = encode(&all);
        if query.is_empty() {
            server.url(resource_type.as_str())
        } else {
            server.url(&format!("{resource_type}?{query}"))
        }
    };
    let mut links = vec![("self", link(after))];
    if let Some(next) = page.next {
        links.push(("next", link(Some(next))));
    }
    let entries = page
        .resources
        .into_iter()
        .map(|r| {
            let mut rest = Map::new();
            rest.insert("search".into(), json!({ "mode": "match" }));
            Entry {
                full_url: Some(server.url(&format!("{}/{}", r.resource_type, r.id))),
                resource: Some(r.json.as_str().to_owned()),
                rest,
            }
        })
        .collect();
    let body = bundle("searchset", page.total, &links, entries);
    Ok(fhir_json(StatusCode::OK, body))
}

/// `GET /{type}?…`.
pub(crate) async fn get(
    State(server): State<FhirServer>,
    Path(type_name): Path<String>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, FhirError> {
    run(&server, &type_name, pairs(query.as_deref()), &headers).await
}

/// `POST /{type}/_search` with a form body; URL parameters are merged in.
pub(crate) async fn post(
    State(server): State<FhirServer>,
    Path(type_name): Path<String>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, FhirError> {
    let form = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/x-www-form-urlencoded"));
    if !body.is_empty() && !form {
        return Err(FhirError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "not-supported",
            "send search parameters as application/x-www-form-urlencoded",
        ));
    }
    let mut params = pairs(query.as_deref());
    params.extend(pairs(std::str::from_utf8(&body).ok()));
    run(&server, &type_name, params, &headers).await
}
