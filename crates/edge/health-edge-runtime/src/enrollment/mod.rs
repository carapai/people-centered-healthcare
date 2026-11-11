//! LAN device enrollment on the edge (lan-security.md § Device enrollment; ADR 0009; #74):
//! `health_auth::DeviceGate` over the embedded store, composed once the store is open.
//!
//! - `POST` / `GET /api/enrollment` are open to the LAN, rate-limited per address.
//! - The registry's admin routes (`/api/admin/enrollments…`, `/api/admin/devices…`) are served
//!   to this machine only, like the status page, until staff sign-in (#75) names the admin.
//! - Every other `/api/…` route is behind the gate: a DPoP proof by an enrolled, active device.
//!
//! Until the store is open, `/api/…` answers 503 and the app shell still loads.

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use health_auth::{DeviceConfig, DeviceGate, SurrealAuthStore};
use health_identity::SessionPolicy;
use health_surrealdb::{Connection, SurrealStore};
use health_time::{Clock, SystemClock};
use tower::ServiceExt;

use crate::admin;
use crate::auth::{self, Secrets};
use crate::config::EdgeConfig;

/// The API, set once the store is open.
#[derive(Clone, Default)]
pub(crate) struct Api(Arc<OnceLock<Router>>);

impl Api {
    /// Composes the API over the open store.
    pub(crate) fn open(
        &self,
        config: &EdgeConfig,
        connection: &Connection,
        secrets: Option<&Secrets>,
    ) {
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let store = SurrealAuthStore::new(
            SurrealStore::new(connection.clone(), Arc::clone(&clock)),
            SessionPolicy::default().hard_cap,
        );
        let gate = DeviceGate::new(
            store.clone(),
            DeviceConfig::new(config.origins.clone(), config.node_id.clone()),
            Arc::clone(&clock),
        );
        let mut api = gate
            .enrollment_router()
            .merge(admin::local_only(gate.admin_router()))
            .merge(gate.device_router());
        match auth::routes(config, connection, store, &gate, secrets, clock) {
            Ok(routes) => {
                api = api.merge(routes);
                tracing::info!(
                    outcome = "sign-in-on",
                    "the local authorization server is serving"
                );
            }
            Err(reason) => {
                tracing::warn!(alert = "sign-in-off", reason = %reason, "no staff sign-in on this edge");
            }
        }
        // A path the API does not serve is behind the gate too: a device not enrolled cannot
        // tell it from a gated one (#140).
        let api = api.fallback_service(gate.layer(Router::new()));
        let _ = self.0.set(api);
    }

    /// `/api/…`, forwarded to the API once it is open. The explicit routes merged beside it,
    /// such as the status page's, take precedence.
    pub(crate) fn routes(&self) -> Router {
        Router::new()
            .route("/api/{*rest}", any(forward))
            .route("/auth/{*rest}", any(forward))
            .with_state(self.clone())
    }
}

async fn forward(State(api): State<Api>, request: Request) -> Response {
    match api.0.get() {
        Some(router) => match router.clone().oneshot(request).await {
            Ok(response) => response,
            Err(infallible) => match infallible {},
        },
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use axum::body::Body;
    use axum::extract::connect_info::MockConnectInfo;
    use axum::http::Request;
    use health_surrealdb::{MIGRATIONS, Settings, Target, migrate};

    use super::*;

    async fn status(api: &Api, peer: &str, method: &str, path: &str) -> StatusCode {
        let peer: SocketAddr = peer.parse().expect("peer");
        let router = api.routes().layer(MockConnectInfo(peer));
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .expect("request");
        router.oneshot(request).await.expect("response").status()
    }

    #[tokio::test]
    async fn the_api_waits_for_the_store_and_the_registry_answers_this_machine_only() {
        let api = Api::default();
        assert_eq!(
            status(&api, "127.0.0.1:1", "GET", "/api/device").await,
            StatusCode::SERVICE_UNAVAILABLE
        );
        let dir = tempfile::tempdir().expect("dir");
        let connection = Connection::open(&Settings {
            target: Target::SurrealKv {
                path: dir.path().join("store"),
            },
            namespace: "ug".into(),
            database: "main".into(),
        })
        .await
        .expect("open");
        migrate(&connection, MIGRATIONS).await.expect("migrate");
        let config =
            EdgeConfig::from_lookup(|name| (name == "HEALTH_DATA_DIR").then(|| "/d".into()))
                .expect("config");
        api.open(&config, &connection, None);

        let lan = "192.168.1.20:5000";
        for (method, path) in [
            ("GET", "/api/admin/enrollments"),
            ("POST", "/api/admin/enrollments/approve"),
            ("GET", "/api/admin/devices"),
        ] {
            assert_eq!(
                status(&api, lan, method, path).await,
                StatusCode::FORBIDDEN,
                "{path}"
            );
        }
        assert_eq!(
            status(&api, "127.0.0.1:1", "GET", "/api/admin/devices").await,
            StatusCode::OK
        );
        // Without a proof, the gate and enrollment refuse alike.
        assert_eq!(
            status(&api, lan, "GET", "/api/device").await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status(&api, lan, "POST", "/api/enrollment").await,
            StatusCode::UNAUTHORIZED
        );
        // Unknown paths too: an unenrolled device cannot map the API.
        assert_eq!(
            status(&api, lan, "GET", "/api/anything").await,
            StatusCode::UNAUTHORIZED
        );
    }

    /// The edge's LAN surface (#140): with sign-in on, a LAN peer with no enrolled device reaches
    /// nothing but sign-in, its discovery documents and the enrolment request. The administrator's
    /// routes answer this machine only, and everything else needs an enrolled device first, before
    /// a body is read.
    #[tokio::test]
    async fn a_lan_peer_without_an_enrolled_device_reaches_only_sign_in_and_enrolment() {
        use std::collections::BTreeMap;

        let dir = tempfile::tempdir().expect("dir");
        let connection = Connection::open(&Settings {
            target: Target::SurrealKv {
                path: dir.path().join("store"),
            },
            namespace: "ug".into(),
            database: "main".into(),
        })
        .await
        .expect("open");
        migrate(&connection, MIGRATIONS).await.expect("migrate");
        let keys = dir.path().join("snapshot-keys.json");
        std::fs::write(
            &keys,
            serde_json::to_string(&[health_auth::SnapshotSigner::generate().public()])
                .expect("json"),
        )
        .expect("keys");
        let keys = keys.to_string_lossy().into_owned();
        let config = EdgeConfig::from_lookup(|name| match name {
            "HEALTH_DATA_DIR" => Some("/d".into()),
            "HEALTH_FACILITY" => Some("KLA001".into()),
            "HEALTH_SNAPSHOT_KEYS_FILE" => Some(keys.clone()),
            "HEALTH_ORIGINS" => Some("https://edge.test".into()),
            _ => None,
        })
        .expect("config");
        let sealed: BTreeMap<&str, Vec<u8>> = BTreeMap::from([
            (
                auth::INTERMEDIATE_KEY,
                health_pki::new_key()
                    .expect("key")
                    .serialize_pem()
                    .into_bytes(),
            ),
            (auth::PEPPER_KEY, vec![5; 32]),
            (auth::TOKEN_KEY, vec![7; 32]),
        ]);
        let secrets: Secrets = Arc::new(move |name| Ok(sealed.get(name).cloned()));
        let api = Api::default();
        api.open(&config, &connection, Some(&secrets));

        let id = "018f0000-0000-7000-8000-000000000000";
        // Open to the LAN by design: discovery, sign-in, and a device asking to be enrolled.
        let public = [
            "/auth/.well-known/openid-configuration",
            "/auth/.well-known/smart-configuration",
            "/auth/jwks",
            "/auth/authorize",
            "/auth/token",
            "/auth/revoke",
            "/auth/otp",
            "/auth/password",
            "/auth/unlock",
            "/api/enrollment",
        ];
        // This machine only: the administrator's screens and provisioning.
        let local = [
            "/api/admin/enrollments".to_owned(),
            "/api/admin/enrollments/approve".to_owned(),
            "/api/admin/devices".to_owned(),
            format!("/api/admin/devices/{id}/revoke"),
            "/api/admin/provision/accounts".to_owned(),
        ];
        // An enrolled device first.
        let gated = [
            "/api/device".to_owned(),
            "/api/session".to_owned(),
            "/api/access/roles".to_owned(),
            "/api/access/grants".to_owned(),
            "/api/access/revocations".to_owned(),
            "/api/access/accounts".to_owned(),
            "/api/fhir".to_owned(),
            "/api/fhir/metadata".to_owned(),
            "/api/fhir/Patient".to_owned(),
            format!("/api/fhir/Patient/{id}"),
            "/api/fhir/Patient/$register".to_owned(),
            "/api/reception/check-in".to_owned(),
            "/api/reception/queues/triage".to_owned(),
            format!("/api/consultation/encounters/{id}/notes"),
            format!("/api/notes/patients/{id}/notes"),
            "/api/anything".to_owned(),
            "/auth/pin".to_owned(),
        ];
        let lan = "192.168.1.20:5000";
        let mut problems = Vec::new();
        for method in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
            for path in public {
                let answered = status(&api, lan, method, path).await;
                // Only the discovery documents and the keys are served; signing in, and asking
                // to be enrolled, need the device's own proof.
                let discovery = method == "GET"
                    && (path.starts_with("/auth/.well-known/") || path == "/auth/jwks");
                if answered.is_server_error() || (answered.is_success() && !discovery) {
                    problems.push(format!("{method} {path}: {answered}"));
                }
            }
            for path in &local {
                let answered = status(&api, lan, method, path).await;
                if !matches!(answered.as_u16(), 403 | 405) {
                    problems.push(format!(
                        "{method} {path}: {answered}, not this machine only"
                    ));
                }
            }
            for path in &gated {
                let answered = status(&api, lan, method, path).await;
                if !matches!(answered.as_u16(), 401 | 405) {
                    problems.push(format!("{method} {path}: {answered}, not behind the gate"));
                }
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
