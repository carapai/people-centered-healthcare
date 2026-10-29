//! HTTP details: media types, ETags and dates (persistence.md § REST semantics: weak ETag
//! `W/"<versionId>"`, `Last-Modified` from `meta.lastUpdated`).

use axum::body::Body;
use axum::http::header::{CONTENT_TYPE, ETAG, HeaderMap, LAST_MODIFIED};
use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;
use health_identifiers::OpId;
use health_persistence::VersionMeta;
use health_time::Timestamp;

use crate::outcome::FhirError;

/// The media type of every response body.
pub const FHIR_JSON: &str = "application/fhir+json; charset=utf-8";

/// A response with a FHIR JSON body.
pub(crate) fn fhir_json(status: StatusCode, body: String) -> Response {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(FHIR_JSON));
    response
}

/// `W/"<versionId>"`.
pub fn etag(version_id: OpId) -> String {
    format!("W/\"{version_id}\"")
}

/// An RFC 9110 HTTP-date.
pub fn http_date(at: Timestamp) -> String {
    at.strftime("%a, %d %b %Y %H:%M:%S GMT").to_string()
}

/// Adds `ETag` and `Last-Modified` for `version`.
pub(crate) fn version_headers(response: &mut Response, version: &VersionMeta) {
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&etag(version.version_id)) {
        headers.insert(ETAG, value);
    }
    if let Ok(value) = HeaderValue::from_str(&http_date(version.last_updated)) {
        headers.insert(LAST_MODIFIED, value);
    }
}

/// The version an `If-Match` header names: `W/"<versionId>"`, or the bare quoted form.
pub(crate) fn if_match(headers: &HeaderMap) -> Result<Option<OpId>, FhirError> {
    let Some(value) = headers.get("if-match") else {
        return Ok(None);
    };
    let text = value
        .to_str()
        .map_err(|_| FhirError::bad_request("If-Match is not ASCII"))?
        .trim();
    let unquoted = text.strip_prefix("W/").unwrap_or(text).trim_matches('"');
    unquoted
        .parse()
        .map(Some)
        .map_err(|_| FhirError::bad_request("If-Match must be W/\"<versionId>\""))
}

/// Refuses request bodies that are not FHIR JSON (415).
pub(crate) fn require_json_body(headers: &HeaderMap) -> Result<(), FhirError> {
    let media = headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(str::trim)
        .unwrap_or_default();
    if matches!(media, "application/fhir+json" | "application/json") {
        Ok(())
    } else {
        Err(FhirError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "not-supported",
            "send application/fhir+json",
        ))
    }
}

/// Refuses `_format` values other than JSON (406).
pub(crate) fn require_json_format(params: &[(String, String)]) -> Result<(), FhirError> {
    match params.iter().find(|(k, _)| k == "_format") {
        None => Ok(()),
        Some((_, v))
            if matches!(
                v.as_str(),
                "json" | "application/json" | "application/fhir+json"
            ) =>
        {
            Ok(())
        }
        Some(_) => Err(FhirError::new(
            StatusCode::NOT_ACCEPTABLE,
            "not-supported",
            "only JSON is supported",
        )),
    }
}

/// What `Prefer: return=` asks for on writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Return {
    Representation,
    Minimal,
    OperationOutcome,
}

/// Reads `Prefer`: `return=` and `handling=lenient`.
pub(crate) fn prefer(headers: &HeaderMap) -> (Return, bool) {
    let mut ret = Return::Representation;
    let mut lenient = false;
    for value in headers.get_all("prefer") {
        for part in value.to_str().unwrap_or_default().split([',', ';']) {
            match part.trim() {
                "return=minimal" => ret = Return::Minimal,
                "return=OperationOutcome" => ret = Return::OperationOutcome,
                "return=representation" => ret = Return::Representation,
                "handling=lenient" => lenient = true,
                "handling=strict" => lenient = false,
                _ => {}
            }
        }
    }
    (ret, lenient)
}
