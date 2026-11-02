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
use crate::config::EdgeConfig;

/// The API, set once the store is open.
#[derive(Clone, Default)]
pub(crate) struct Api(Arc<OnceLock<Router>>);

impl Api {
    /// Composes the API over the open store.
    pub(crate) fn open(&self, config: &EdgeConfig, connection: &Connection) {
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let store = SurrealAuthStore::new(
            SurrealStore::new(connection.clone(), Arc::clone(&clock)),
            SessionPolicy::default().hard_cap,
        );
        let gate = DeviceGate::new(
            store,
            DeviceConfig::new(config.origins.clone(), config.node_id.clone()),
            clock,
        );
        let api = gate
            .enrollment_router()
            .merge(admin::local_only(gate.admin_router()))
            .merge(gate.device_router());
        let _ = self.0.set(api);
    }

    /// `/api/…`, forwarded to the API once it is open. The explicit routes merged beside it,
    /// such as the status page's, take precedence.
    pub(crate) fn routes(&self) -> Router {
        Router::new()
            .route("/api/{*rest}", any(forward))
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
            target: Target::RocksDb {
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
        api.open(&config, &connection);

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
}
