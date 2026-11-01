//! The web app, served from the edge's own origin (lan-security.md § App delivery to browsers).
//!
//! The bundle is embedded at build time (`build.rs`), so a LAN browser always runs the edge's own
//! version and needs nothing from the internet. Beside it the edge serves:
//! - `runtime.json`: the edge platform, and the build for the client's version check;
//! - `sw.js`: the service worker that keeps the shell loadable across edge restarts. It caches
//!   the shell's fixed list of paths and never an API response (ADR 0037).
//!
//! Any other path that is not a file is a client route, answered with the page.

use axum::Router;
use axum::http::{HeaderValue, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/web.rs"));
}

/// The service worker's logic; [`Bundle::service_worker`] prepends the shell's paths.
const SERVICE_WORKER: &str = include_str!("sw.js");

/// First path segments that are never client routes: an unknown API path is a 404, not the page.
const RESERVED: &[&str] = &[
    "api", "fhir", "auth", "health", "ready", "metrics", "events",
];

/// Hashed bundle files, which never change under the same name.
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// Everything else is revalidated, so an updated edge is seen at once.
const REVALIDATE: &str = "no-cache";

/// A built web app: its files by URL path, and its build.
#[derive(Clone, Copy, Debug)]
pub struct Bundle {
    files: &'static [(&'static str, &'static [u8])],
    build: Option<&'static str>,
}

impl Bundle {
    /// The bundle compiled into this binary; empty when it was built without one.
    #[must_use]
    pub const fn embedded() -> Self {
        Self::new(embedded::ASSETS, embedded::BUILD)
    }

    /// A bundle of `files` (URL path, contents), including `/index.html`.
    #[must_use]
    pub const fn new(
        files: &'static [(&'static str, &'static [u8])],
        build: Option<&'static str>,
    ) -> Self {
        Self { files, build }
    }

    /// Whether there is an app to serve.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.file("/index.html").is_none()
    }

    /// The build, from the bundle's `build.json`.
    #[must_use]
    pub const fn build(&self) -> Option<&'static str> {
        self.build
    }

    fn file(&self, path: &str) -> Option<&'static [u8]> {
        self.files
            .iter()
            .find_map(|(name, body)| (*name == path).then_some(*body))
    }

    /// What the service worker caches: the page (at `/`), `runtime.json` and the bundle's files.
    fn shell(&self) -> Vec<&'static str> {
        let files = self
            .files
            .iter()
            .map(|(name, _)| *name)
            .filter(|name| *name != "/index.html");
        ["/", "/runtime.json"].into_iter().chain(files).collect()
    }

    fn service_worker(&self) -> String {
        let build = self.build.unwrap_or("unversioned");
        let shell = serde_json::to_string(&self.shell()).unwrap_or_else(|_| "[]".to_owned());
        let build = serde_json::to_string(build).unwrap_or_else(|_| "\"\"".to_owned());
        format!("const BUILD = {build}\nconst SHELL = {shell}\n\n{SERVICE_WORKER}")
    }

    fn runtime(&self) -> serde_json::Value {
        let mut runtime = serde_json::json!({ "mode": "edge", "apiBase": "/api" });
        if let Some(build) = self.build {
            runtime["build"] = build.into();
        }
        runtime
    }
}

/// The app's routes, to merge beside the API's. Without a bundle only `runtime.json` is served.
pub(crate) fn routes(bundle: Bundle) -> Router {
    Router::new()
        .route(
            "/runtime.json",
            get(move || async move { (revalidated(), axum::Json(bundle.runtime())) }),
        )
        .route(
            "/sw.js",
            get(move || async move {
                if bundle.is_empty() {
                    return StatusCode::NOT_FOUND.into_response();
                }
                let body = bundle.service_worker();
                (revalidated(), typed("text/javascript; charset=utf-8"), body).into_response()
            }),
        )
        .fallback(move |method: Method, uri: Uri| async move { page(bundle, &method, uri.path()) })
}

fn page(bundle: Bundle, method: &Method, path: &str) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::NOT_FOUND.into_response();
    }
    if let Some(body) = bundle.file(path).filter(|_| path != "/index.html") {
        let cache = if path.starts_with("/assets/") {
            IMMUTABLE
        } else {
            REVALIDATE
        };
        return (
            [(header::CACHE_CONTROL, HeaderValue::from_static(cache))],
            typed(content_type(path)),
            nosniff(),
            body,
        )
            .into_response();
    }
    let first = path.trim_start_matches('/').split('/').next().unwrap_or("");
    let last = path.rsplit('/').next().unwrap_or("");
    if RESERVED.contains(&first) || last.contains('.') {
        return StatusCode::NOT_FOUND.into_response();
    }
    match bundle.file("/index.html") {
        Some(body) => (
            revalidated(),
            typed("text/html; charset=utf-8"),
            nosniff(),
            body,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn revalidated() -> [(header::HeaderName, HeaderValue); 1] {
    [(header::CACHE_CONTROL, HeaderValue::from_static(REVALIDATE))]
}

fn typed(content_type: &'static str) -> [(header::HeaderName, HeaderValue); 1] {
    [(header::CONTENT_TYPE, HeaderValue::from_static(content_type))]
}

fn nosniff() -> [(header::HeaderName, HeaderValue); 1] {
    [(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    )]
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("webmanifest") => "application/manifest+json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt as _;

    const FILES: &[(&str, &[u8])] = &[
        ("/index.html", b"<!doctype html><div id=root></div>"),
        ("/assets/index-abc123.js", b"console.log(1)"),
        ("/favicon.svg", b"<svg/>"),
    ];

    async fn get(bundle: Bundle, path: &str) -> (StatusCode, axum::http::HeaderMap, String) {
        let response = routes(bundle)
            .oneshot(Request::get(path).body(Body::empty()).expect("a request"))
            .await
            .expect("the router answers");
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("a body");
        (
            status,
            headers,
            String::from_utf8(body.to_vec()).expect("UTF-8"),
        )
    }

    #[tokio::test]
    async fn client_routes_get_the_page_and_it_is_revalidated() {
        let bundle = Bundle::new(FILES, Some("abc"));
        for path in ["/", "/patients/123", "/stock/receive"] {
            let (status, headers, body) = get(bundle, path).await;
            assert_eq!(status, StatusCode::OK, "{path}");
            assert!(body.contains("id=root"), "{path}");
            assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
            assert_eq!(headers[header::CONTENT_TYPE], "text/html; charset=utf-8");
        }
    }

    #[tokio::test]
    async fn hashed_files_are_immutable_and_others_revalidated() {
        let bundle = Bundle::new(FILES, Some("abc"));
        let (status, headers, body) = get(bundle, "/assets/index-abc123.js").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "console.log(1)");
        assert_eq!(headers[header::CACHE_CONTROL], IMMUTABLE);
        assert_eq!(
            headers[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        let (_, headers, _) = get(bundle, "/favicon.svg").await;
        assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
    }

    #[tokio::test]
    async fn api_paths_and_missing_files_are_not_the_page() {
        let bundle = Bundle::new(FILES, Some("abc"));
        for path in [
            "/api/unknown",
            "/fhir/Patient",
            "/auth/x",
            "/assets/gone.js",
            "/index.html",
        ] {
            assert_eq!(get(bundle, path).await.0, StatusCode::NOT_FOUND, "{path}");
        }
    }

    #[tokio::test]
    async fn runtime_names_the_edge_platform_and_the_build() {
        let (status, headers, body) = get(Bundle::new(FILES, Some("abc")), "/runtime.json").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
        let runtime: serde_json::Value = serde_json::from_str(&body).expect("JSON");
        assert_eq!(
            runtime,
            serde_json::json!({ "mode": "edge", "apiBase": "/api", "build": "abc" })
        );
        let (_, _, body) = get(Bundle::new(FILES, None), "/runtime.json").await;
        let runtime: serde_json::Value = serde_json::from_str(&body).expect("JSON");
        assert_eq!(runtime.get("build"), None);
    }

    #[tokio::test]
    async fn the_worker_caches_the_shell_and_nothing_else() {
        let (status, headers, body) = get(Bundle::new(FILES, Some("abc")), "/sw.js").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
        assert!(body.starts_with(
            "const BUILD = \"abc\"\n\
             const SHELL = [\"/\",\"/runtime.json\",\"/assets/index-abc123.js\",\"/favicon.svg\"]\n"
        ));
        assert!(body.contains("cache.addAll(SHELL)"));
        // The shell is cached at install only; nothing is put in the cache afterwards.
        assert!(!body.contains(".put("));
    }

    #[tokio::test]
    async fn an_edge_without_a_bundle_serves_no_app() {
        let bundle = Bundle::new(&[], None);
        assert!(bundle.is_empty());
        assert_eq!(get(bundle, "/").await.0, StatusCode::NOT_FOUND);
        assert_eq!(get(bundle, "/sw.js").await.0, StatusCode::NOT_FOUND);
        assert_eq!(get(bundle, "/runtime.json").await.0, StatusCode::OK);
    }
}
