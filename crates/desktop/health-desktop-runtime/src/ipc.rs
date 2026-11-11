//! The IPC transport (ADR 0036; client-application.md): the webview calls
//! `invoke('api_request', { envelope })` and the Rust side answers as the HTTP API would. The
//! envelope is the request the client's `TauriIpcTransport` builds (`apps/client/src/platform/
//! tauri.ts`): bodies cross as JSON text, so numbers stay lossless, and query parameters as
//! ordered pairs, so repeated ones survive.

use std::collections::BTreeMap;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderName, HeaderValue, Method, Request, StatusCode, Uri};
use http_body_util::BodyExt as _;
use serde::{Deserialize, Serialize};
use tower::ServiceExt as _;

/// A request from the webview.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Envelope {
    /// `GET`, `POST`, …
    pub method: String,
    /// The origin-relative path, e.g. `/fhir/Patient/123`.
    pub path: String,
    /// Query parameters, in order; a name may repeat.
    #[serde(default)]
    pub query: Vec<(String, String)>,
    /// Request headers.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// The body as text (JSON, or a form), if any.
    #[serde(default)]
    pub body: Option<String>,
}

/// The answer to the webview.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Reply {
    /// The HTTP status.
    pub status: u16,
    /// Response headers. A repeated header's values are joined with `, `.
    pub headers: BTreeMap<String, String>,
    /// The body as text: empty when there is none.
    pub body: String,
}

/// Why an envelope cannot be dispatched: answered as 400, never sent on.
#[derive(Debug, thiserror::Error)]
pub enum Refused {
    /// The method is not an HTTP token.
    #[error("not an HTTP method: {0}")]
    Method(String),
    /// The path is not origin-relative, or does not parse.
    #[error("not an origin-relative path: {0}")]
    Path(String),
    /// A header name or value cannot be sent.
    #[error("an unusable header: {0}")]
    Header(String),
}

/// Dispatches `envelope` into `router` in-process and returns its answer. A body that is not
/// UTF-8 (the API answers JSON, forms and text) is refused as a 500 rather than mangled.
pub async fn dispatch(router: &Router, envelope: Envelope) -> Reply {
    let request = match request(envelope) {
        Ok(request) => request,
        Err(refused) => return problem(StatusCode::BAD_REQUEST, &refused.to_string()),
    };
    let response = match router.clone().oneshot(request).await {
        Ok(response) => response,
        Err(infallible) => match infallible {},
    };
    let status = response.status().as_u16();
    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    for (name, value) in response.headers() {
        let Ok(value) = value.to_str() else { continue };
        headers
            .entry(name.as_str().to_owned())
            .and_modify(|joined| {
                joined.push_str(", ");
                joined.push_str(value);
            })
            .or_insert_with(|| value.to_owned());
    }
    let bytes = match response.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => {
            return problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the answer was cut short",
            );
        }
    };
    match String::from_utf8(bytes.to_vec()) {
        Ok(body) => Reply {
            status,
            headers,
            body,
        },
        Err(_) => problem(
            StatusCode::INTERNAL_SERVER_ERROR,
            "the answer is not text: binary bodies use the raw IPC channel",
        ),
    }
}

/// The HTTP request an envelope stands for.
pub fn request(envelope: Envelope) -> Result<Request<Body>, Refused> {
    let method = Method::from_bytes(envelope.method.as_bytes())
        .map_err(|_| Refused::Method(envelope.method.clone()))?;
    // Origin-relative only: the webview addresses this node's API, never another host.
    if !envelope.path.starts_with('/')
        || envelope.path.starts_with("//")
        || envelope.path.contains('?')
    {
        return Err(Refused::Path(envelope.path));
    }
    let mut target = envelope.path.clone();
    if !envelope.query.is_empty() {
        let query = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(&envelope.query)
            .finish();
        target.push('?');
        target.push_str(&query);
    }
    let uri: Uri = target.parse().map_err(|_| Refused::Path(envelope.path))?;
    let mut request = Request::builder().method(method).uri(uri);
    for (name, value) in &envelope.headers {
        let name =
            HeaderName::from_bytes(name.as_bytes()).map_err(|_| Refused::Header(name.clone()))?;
        let value = HeaderValue::from_str(value).map_err(|_| Refused::Header(name.to_string()))?;
        request = request.header(name, value);
    }
    request
        .body(envelope.body.map_or_else(Body::empty, Body::from))
        .map_err(|e| Refused::Header(e.to_string()))
}

fn problem(status: StatusCode, diagnostics: &str) -> Reply {
    let body = format!(
        r#"{{"resourceType":"OperationOutcome","issue":[{{"severity":"error","code":"invalid","diagnostics":{}}}]}}"#,
        serde_json_string(diagnostics)
    );
    Reply {
        status: status.as_u16(),
        headers: BTreeMap::from([(
            "content-type".to_owned(),
            "application/fhir+json".to_owned(),
        )]),
        body,
    }
}

/// `text` as a JSON string literal.
fn serde_json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use axum::extract::RawQuery;
    use axum::routing::{get, post};

    use super::*;

    fn router() -> Router {
        Router::new()
            .route(
                "/echo",
                post(|headers: axum::http::HeaderMap, body: String| async move {
                    let kind = headers
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or_default()
                        .to_owned();
                    ([("x-kind", kind)], body)
                }),
            )
            .route(
                "/query",
                get(|RawQuery(query): RawQuery| async move { query.unwrap_or_default() }),
            )
    }

    fn envelope(method: &str, path: &str) -> Envelope {
        Envelope {
            method: method.into(),
            path: path.into(),
            query: Vec::new(),
            headers: BTreeMap::new(),
            body: None,
        }
    }

    #[tokio::test]
    async fn a_body_crosses_as_text_and_comes_back_as_text() {
        // A 28-digit decimal: the text crosses unchanged, so nothing rounds it.
        let body = r#"{"valueQuantity":{"value":3.1415926535897932384626433832}}"#;
        let mut sent = envelope("POST", "/echo");
        sent.headers
            .insert("content-type".into(), "application/fhir+json".into());
        sent.body = Some(body.into());
        let reply = dispatch(&router(), sent).await;
        assert_eq!(reply.status, 200);
        assert_eq!(reply.body, body);
        assert_eq!(reply.headers["x-kind"], "application/fhir+json");
    }

    #[tokio::test]
    async fn repeated_query_parameters_keep_their_order() {
        let mut sent = envelope("GET", "/query");
        sent.query = vec![
            ("code".into(), "a b".into()),
            ("code".into(), "c&d".into()),
            ("_count".into(), "2".into()),
        ];
        let reply = dispatch(&router(), sent).await;
        assert_eq!(reply.body, "code=a+b&code=c%26d&_count=2");
    }

    #[tokio::test]
    async fn only_origin_relative_requests_are_dispatched() {
        for path in [
            "https://elsewhere.example/fhir",
            "//elsewhere/fhir",
            "fhir/Patient",
            "/q?x=1",
        ] {
            let reply = dispatch(&router(), envelope("GET", path)).await;
            assert_eq!(reply.status, 400, "{path}");
            assert!(reply.body.contains("OperationOutcome"));
        }
        let reply = dispatch(&router(), envelope("G\nET", "/query")).await;
        assert_eq!(reply.status, 400);
        let reply = dispatch(&router(), envelope("GET", "/nowhere")).await;
        assert_eq!(reply.status, 404, "unknown paths answer as over HTTP");
    }
}
