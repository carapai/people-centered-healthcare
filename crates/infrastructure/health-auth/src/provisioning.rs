//! Importing central's provisioning bundle on an edge (ADR 0059; offline-auth.md § Credentials):
//! `POST /api/admin/provision/accounts` with the bundle's JWS as the body, for the caller to
//! guard (an edge serves it to its own console).
//!
//! The bundle must be central's, for this node and this facility. Each snapshot inside must be
//! central's, for an account in the bundle and this facility. Each hash is decrypted with the
//! node's facility intermediate key and stored re-encrypted under its own pepper key. An account
//! already here keeps its failed-login state.

use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use health_identity::{Lockout, LoginAccount};
use p256::SecretKey;
use serde_json::json;

use crate::bundle::{Bundle, BundleError};
use crate::credentials::Credentials;
use crate::snapshot::SnapshotVerifier;
use crate::store::AuthStore;
use crate::surreal::SurrealAuthStore;

/// The node's facility intermediate key, unsealed from its keystore. Blocking: it may run the
/// platform keystore's tools.
pub type RecipientKey = Arc<dyn Fn() -> Result<SecretKey, String> + Send + Sync>;

/// Why a bundle was not imported.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    /// The bundle itself was refused.
    #[error(transparent)]
    Bundle(#[from] BundleError),
    /// For another facility.
    #[error("the bundle is for {0}, not this edge's facility")]
    WrongFacility(String),
    /// A snapshot inside is not central's, or not for an account in the bundle at this facility.
    #[error("a snapshot in the bundle is not valid here")]
    Snapshot,
    /// The node's key or store failed.
    #[error("{0}")]
    Unavailable(String),
}

/// What an import stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Imported {
    /// Login Accounts stored.
    pub accounts: usize,
    /// Snapshots stored.
    pub snapshots: usize,
}

/// The import, over the edge's store.
#[derive(Clone)]
pub struct Provisioning {
    store: SurrealAuthStore,
    credentials: Credentials,
    verifier: SnapshotVerifier,
    node_id: String,
    facility: String,
    recipient: RecipientKey,
}

impl Provisioning {
    /// The import for `node_id` at `facility`, storing into `store` under `credentials`.
    pub fn new(
        store: SurrealAuthStore,
        credentials: Credentials,
        verifier: SnapshotVerifier,
        node_id: impl Into<String>,
        facility: impl Into<String>,
        recipient: RecipientKey,
    ) -> Self {
        Self {
            store,
            credentials,
            verifier,
            node_id: node_id.into(),
            facility: facility.into(),
            recipient,
        }
    }

    /// Imports the bundle in `jws`.
    pub async fn import(&self, jws: &str) -> Result<Imported, ImportError> {
        let recipient = Arc::clone(&self.recipient);
        let key = tokio::task::spawn_blocking(move || recipient())
            .await
            .map_err(|e| ImportError::Unavailable(e.to_string()))?
            .map_err(ImportError::Unavailable)?;
        let bundle = Bundle::open(jws.trim(), &self.verifier, &self.node_id, &key)?;
        if bundle.facility != self.facility {
            return Err(ImportError::WrongFacility(bundle.facility));
        }
        let mut snapshots = Vec::new();
        for jws in &bundle.snapshots {
            let snapshot = self
                .verifier
                .verify(jws)
                .map_err(|_| ImportError::Snapshot)?;
            if snapshot.facility != self.facility
                || !bundle.accounts.iter().any(|a| a.id == snapshot.account)
            {
                return Err(ImportError::Snapshot);
            }
            snapshots.push((snapshot.account, jws));
        }
        let unavailable = |e: crate::store::StoreError| ImportError::Unavailable(e.to_string());
        for account in &bundle.accounts {
            let lockout = self
                .store
                .account(account.id)
                .await
                .map_err(unavailable)?
                .map_or_else(Lockout::default, |existing| existing.lockout);
            let credential = self
                .credentials
                .seal(account.id, &account.phc)
                .map_err(|e| ImportError::Unavailable(e.to_string()))?;
            self.store
                .put_account(&LoginAccount {
                    id: account.id,
                    username: account.username.clone(),
                    credential,
                    status: account.status,
                    lockout,
                })
                .await
                .map_err(unavailable)?;
        }
        self.store
            .put_local_config(&bundle.local)
            .await
            .map_err(unavailable)?;
        for (account, jws) in &snapshots {
            self.store
                .put_snapshot(*account, &self.facility, jws)
                .await
                .map_err(unavailable)?;
        }
        Ok(Imported {
            accounts: bundle.accounts.len(),
            snapshots: snapshots.len(),
        })
    }

    /// `POST /api/admin/provision/accounts`.
    pub fn router(self) -> Router {
        Router::new()
            .route("/api/admin/provision/accounts", post(import))
            .with_state(Arc::new(self))
    }
}

async fn import(State(provisioning): State<Arc<Provisioning>>, body: String) -> Response {
    let no_store = [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))];
    match provisioning.import(&body).await {
        Ok(imported) => {
            tracing::info!(
                outcome = "accounts-provisioned",
                accounts = imported.accounts,
                snapshots = imported.snapshots,
                "provisioning bundle imported"
            );
            (
                no_store,
                axum::Json(
                    json!({ "accounts": imported.accounts, "snapshots": imported.snapshots }),
                ),
            )
                .into_response()
        }
        Err(ImportError::Unavailable(detail)) => {
            tracing::error!(error.kind = %detail, "the provisioning bundle could not be imported");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                no_store,
                axum::Json(json!({ "error": "unavailable" })),
            )
                .into_response()
        }
        Err(refused) => {
            tracing::warn!(alert = "bundle-refused", reason = %refused, "a provisioning bundle was refused");
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                no_store,
                axum::Json(json!({ "error": "refused", "detail": refused.to_string() })),
            )
                .into_response()
        }
    }
}
