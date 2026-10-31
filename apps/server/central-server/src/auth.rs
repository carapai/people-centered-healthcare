//! The authorization server, mounted at `/auth` (offline-auth.md; ADR 0010). Central is the
//! identity provider of record; its routes come up once the database is migrated, and answer
//! 503 until then.

use std::convert::Infallible;
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use health_auth::{AuthConfig, AuthServer, Client, Credentials, SurrealAuthStore, TokenSigner};
use health_identity::SessionPolicy;
use health_surrealdb::{Connection, SurrealStore};
use health_time::SystemClock;
use tokio::sync::OnceCell;
use tower::ServiceExt;

/// What the authorization server needs, read at startup.
#[derive(Clone)]
pub(crate) struct AuthSettings {
    /// The issuer URL, e.g. `https://central.example/auth`.
    pub(crate) issuer: String,
    /// The signing keys: the first signs, the rest (rotated out) still verify.
    pub(crate) keys: Vec<[u8; 32]>,
    /// The Argon2id pepper (empty for none).
    pub(crate) pepper: Vec<u8>,
    /// The registered clients.
    pub(crate) clients: Vec<Client>,
    /// The web client's sign-in page, if it fronts `/authorize`.
    pub(crate) login_page: Option<String>,
}

impl std::fmt::Debug for AuthSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthSettings")
            .field("issuer", &self.issuer)
            .field("keys", &self.keys.len())
            .field("clients", &self.clients)
            .finish_non_exhaustive()
    }
}

/// A key file: one hex-encoded 32-byte P-256 secret per line, the active key first. Blank lines
/// and `#` comments are ignored.
pub(crate) fn read_keys(path: &Path) -> Result<Vec<[u8; 32]>, &'static str> {
    let text = std::fs::read_to_string(path).map_err(|_| "cannot read HEALTH_AUTH_KEYS_FILE")?;
    let keys: Vec<[u8; 32]> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|line| {
            let bytes: Vec<u8> = (0..line.len())
                .step_by(2)
                .map(|i| {
                    line.get(i..i + 2)
                        .and_then(|h| u8::from_str_radix(h, 16).ok())
                })
                .collect::<Option<_>>()
                .ok_or("HEALTH_AUTH_KEYS_FILE holds a line that is not hex")?;
            bytes
                .try_into()
                .map_err(|_| "HEALTH_AUTH_KEYS_FILE keys are 32 bytes")
        })
        .collect::<Result<_, _>>()?;
    if keys.is_empty() {
        return Err("HEALTH_AUTH_KEYS_FILE holds no key");
    }
    Ok(keys)
}

/// The authorization server's routes over `connection`.
pub(crate) fn routes(
    connection: Connection,
    settings: &AuthSettings,
) -> Result<Router, &'static str> {
    let signer = TokenSigner::from_secrets(&settings.keys)
        .map_err(|_| "HEALTH_AUTH_KEYS_FILE holds an invalid key")?;
    let credentials =
        Credentials::new(settings.pepper.clone()).map_err(|_| "the pepper cannot be used")?;
    let policy = SessionPolicy::default();
    let store = SurrealAuthStore::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        policy.hard_cap,
    );
    let mut config = AuthConfig::new(settings.issuer.as_str(), "central", false);
    config.clients.clone_from(&settings.clients);
    config.login_page.clone_from(&settings.login_page);
    let server = AuthServer::new(config, signer, credentials, store, Arc::new(SystemClock));
    Ok(Router::new()
        .nest("/auth", server.router())
        .nest("/api", server.session_route()))
}

/// `/auth` and `/api/session`, forwarded (paths intact) to the routes once they are built, and
/// 503 until then.
pub(crate) fn gate(ready: Arc<OnceCell<Router>>) -> Router {
    let service = tower::service_fn(move |request: Request<Body>| {
        let ready = Arc::clone(&ready);
        async move {
            let response = match ready.get() {
                Some(router) => router.clone().oneshot(request).await.into_response(),
                None => (StatusCode::SERVICE_UNAVAILABLE, "sign-in is starting").into_response(),
            };
            Ok::<Response, Infallible>(response)
        }
    });
    Router::new()
        .route_service("/auth/{*rest}", service.clone())
        .route_service("/api/session", service)
}

#[cfg(test)]
mod tests {
    use health_surrealdb::{MIGRATIONS, Settings, Target, migrate};
    use http_body_util::BodyExt;

    use super::*;

    fn settings() -> AuthSettings {
        AuthSettings {
            issuer: "https://central.test/auth".into(),
            keys: vec![[9u8; 32], [8u8; 32]],
            pepper: Vec::new(),
            clients: vec![Client {
                id: "health-client".into(),
                redirect_uris: vec!["https://central.test/callback".into()],
            }],
            login_page: Some("/sign-in".into()),
        }
    }

    #[tokio::test]
    async fn the_gate_answers_503_until_the_database_is_ready() {
        let ready = Arc::new(OnceCell::new());
        let app = gate(Arc::clone(&ready));
        let discovery = || {
            Request::get("/auth/.well-known/openid-configuration")
                .body(Body::empty())
                .expect("request")
        };
        assert_eq!(
            app.clone()
                .oneshot(discovery())
                .await
                .expect("response")
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );

        let connection = Connection::open(&Settings {
            target: Target::Memory,
            namespace: "central".into(),
            database: "main".into(),
        })
        .await
        .expect("open");
        migrate(&connection, MIGRATIONS).await.expect("migrate");
        let _ = ready.set(routes(connection, &settings()).expect("routes"));
        let response = app.clone().oneshot(discovery()).await.expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let metadata: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(
            metadata["token_endpoint"],
            "https://central.test/auth/token"
        );
        // Both keys are published: tokens signed before a rotation stay verifiable.
        let jwks = app
            .clone()
            .oneshot(
                Request::get("/auth/jwks")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let keys: serde_json::Value =
            serde_json::from_slice(&jwks.into_body().collect().await.expect("body").to_bytes())
                .expect("json");
        assert_eq!(keys["keys"].as_array().map(Vec::len), Some(2));
        // The authorized route answers 401 without a token, and /authorize sends the browser to
        // the web client's sign-in page.
        let session = app
            .clone()
            .oneshot(
                Request::get("/api/session")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(session.status(), StatusCode::UNAUTHORIZED);
        let authorize = app
            .oneshot(Request::get("/auth/authorize?response_type=code&client_id=health-client&redirect_uri=https%3A%2F%2Fcentral.test%2Fcallback&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256").body(Body::empty()).expect("request"))
            .await
            .expect("response");
        assert_eq!(authorize.status(), StatusCode::SEE_OTHER);
        assert!(
            authorize.headers()["location"]
                .to_str()
                .expect("location")
                .starts_with("/sign-in?")
        );
    }

    #[test]
    fn key_files_are_hex_lines_active_first() {
        let dir = tempfile::tempdir().expect("tmp");
        let path = dir.path().join("keys");
        std::fs::write(
            &path,
            format!("# active\n{}\n\n{}\n", "09".repeat(32), "08".repeat(32)),
        )
        .expect("write");
        assert_eq!(read_keys(&path), Ok(vec![[9u8; 32], [8u8; 32]]));
        std::fs::write(&path, "zz\n").expect("write");
        assert!(read_keys(&path).is_err());
        std::fs::write(&path, "0909\n").expect("write");
        assert!(read_keys(&path).is_err());
        std::fs::write(&path, "# nothing\n").expect("write");
        assert!(read_keys(&path).is_err());
    }
}
