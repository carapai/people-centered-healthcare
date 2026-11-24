//! Importing central's provisioning bundle on an edge (ADR 0059; offline-auth.md § Credentials):
//! `POST /api/admin/provision/accounts` with the bundle's JWS as the body, for the caller to
//! guard (an edge serves it to its own console).
//!
//! The bundle must be central's, for this node and this facility. Each snapshot inside must be
//! central's, for an account in the bundle and this facility. Each hash is decrypted with the
//! node's facility intermediate key and stored re-encrypted under its own pepper key. An account
//! already here keeps its failed-login state.
//!
//! The same bundle comes with every sync round (#889): [`Provisioning::replace`] imports it and
//! removes the snapshots at the facility that central no longer holds, so a role withdrawn at
//! central is refused here from that round on. [`facility_bundle`] is central's side, for the
//! sync endpoint and the `provision bundle` command alike.

use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use health_identity::{Lockout, LoginAccount};
use p256::SecretKey;
use serde_json::json;

use crate::bundle::{Bundle, BundleAccount, BundleError};
use crate::credentials::Credentials;
use crate::snapshot::{SnapshotSigner, SnapshotVerifier};
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
    /// Snapshots removed because central no longer holds them ([`Provisioning::replace`]).
    pub withdrawn: usize,
}

/// Central's provisioning bundle of `facility` for the node `node_id` (ADR 0059): every account
/// holding a snapshot at the facility in `store`, its hash opened from central's pepper
/// (`credentials`) and encrypted to the node's facility intermediate key `recipient`, with the
/// snapshots as central signed them and the country's `local` configuration, signed by `signer`,
/// central's snapshot key. An account whose hash does not open under the pepper fails the whole
/// bundle, naming it.
#[allow(clippy::too_many_arguments)]
pub async fn facility_bundle(
    store: &SurrealAuthStore,
    credentials: &Credentials,
    facility: &str,
    node_id: &str,
    local: crate::store::LocalConfig,
    signer: &SnapshotSigner,
    recipient: &p256::PublicKey,
    now: health_time::Timestamp,
) -> Result<String, String> {
    let mut accounts = Vec::new();
    let mut snapshots = Vec::new();
    for (id, jws) in store
        .snapshots_at(facility)
        .await
        .map_err(|e| e.to_string())?
    {
        let Some(account) = store.account(id).await.map_err(|e| e.to_string())? else {
            continue;
        };
        // Central's own sealing comes off; the bundle encrypts the hash to the node instead.
        let phc = credentials
            .open(account.id, &account.credential)
            .ok_or_else(|| {
                format!(
                    "{}'s credential does not open under this pepper key",
                    account.username
                )
            })?;
        accounts.push(BundleAccount {
            id: account.id,
            username: account.username,
            status: account.status,
            phc,
        });
        snapshots.push(jws);
    }
    Bundle {
        facility: facility.to_owned(),
        node_id: node_id.to_owned(),
        issued_at: now,
        accounts,
        snapshots,
        local,
    }
    .seal(signer, recipient)
    .map_err(|e| e.to_string())
}

/// The import, over the edge's store.
#[derive(Clone)]
pub struct Provisioning {
    store: SurrealAuthStore,
    credentials: Credentials,
    verifier: SnapshotVerifier,
    /// Who may sign the bundle, when not the snapshot keys: the edge, for its desktops.
    bundle_verifier: Option<SnapshotVerifier>,
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
            bundle_verifier: None,
            node_id: node_id.into(),
            facility: facility.into(),
            recipient,
        }
    }

    /// Accepts bundles signed by `bundle_verifier`'s keys rather than central's snapshot keys,
    /// which still verify every snapshot inside: a desktop under an edge, whose bundle the edge
    /// signs with its facility intermediate key (#133).
    #[must_use]
    pub fn with_bundle_verifier(mut self, bundle_verifier: SnapshotVerifier) -> Self {
        self.bundle_verifier = Some(bundle_verifier);
        self
    }

    /// A bundle for the enrolled desktop `node_id` at `location`, its hashes encrypted to the
    /// desktop's device key `recipient`, signed with `signer` (the edge's facility intermediate
    /// key): the staff whose snapshots here hold a facility-wide role or one at `location`, with
    /// those snapshots as central signed them (#133; desktop-client.md: a desktop under an edge
    /// gets snapshots for staff assigned to its service points).
    pub async fn export(
        &self,
        node_id: &str,
        location: &str,
        recipient: &p256::PublicKey,
        signer: &SnapshotSigner,
        now: health_time::Timestamp,
    ) -> Result<String, ImportError> {
        let unavailable = |e: crate::store::StoreError| ImportError::Unavailable(e.to_string());
        let mut accounts = Vec::new();
        let mut snapshots = Vec::new();
        for (id, jws) in self
            .store
            .snapshots_at(&self.facility)
            .await
            .map_err(unavailable)?
        {
            let Ok(snapshot) = self.verifier.verify(&jws) else {
                continue;
            };
            let here = snapshot.roles.iter().any(|role| {
                role.locations.is_empty() || role.locations.iter().any(|l| l == location)
            });
            if snapshot.account != id || snapshot.not_after <= now || !here {
                continue;
            }
            let Some(account) = self.store.account(id).await.map_err(unavailable)? else {
                continue;
            };
            let Some(phc) = self.credentials.open(id, &account.credential) else {
                continue;
            };
            accounts.push(BundleAccount {
                id,
                username: account.username,
                status: account.status,
                phc,
            });
            snapshots.push(jws);
        }
        let local = self
            .store
            .local_config()
            .await
            .map_err(unavailable)?
            .unwrap_or_default();
        Ok(Bundle {
            facility: self.facility.clone(),
            node_id: node_id.to_owned(),
            issued_at: now,
            accounts,
            snapshots,
            local,
        }
        .seal(signer, recipient)?)
    }

    /// Imports the bundle in `jws` as the whole of central's staff at the facility (#889): as
    /// [`import`](Self::import), then removes each snapshot stored at the facility whose account
    /// has none in the bundle, since central has withdrawn every role it held there. A bundle
    /// refused is refused whole, and nothing is removed.
    pub async fn replace(&self, jws: &str) -> Result<Imported, ImportError> {
        let (mut imported, kept) = self.import_bundle(jws).await?;
        let unavailable = |e: crate::store::StoreError| ImportError::Unavailable(e.to_string());
        for (account, _) in self
            .store
            .snapshots_at(&self.facility)
            .await
            .map_err(unavailable)?
        {
            if !kept.contains(&account) {
                self.store
                    .remove_snapshot(account, &self.facility)
                    .await
                    .map_err(unavailable)?;
                imported.withdrawn += 1;
            }
        }
        Ok(imported)
    }

    /// Imports the bundle in `jws`.
    pub async fn import(&self, jws: &str) -> Result<Imported, ImportError> {
        self.import_bundle(jws).await.map(|(imported, _)| imported)
    }

    /// Imports the bundle in `jws`: what it stored, and the accounts it holds snapshots for.
    async fn import_bundle(
        &self,
        jws: &str,
    ) -> Result<(Imported, Vec<health_identity::AccountId>), ImportError> {
        let recipient = Arc::clone(&self.recipient);
        let key = tokio::task::spawn_blocking(move || recipient())
            .await
            .map_err(|e| ImportError::Unavailable(e.to_string()))?
            .map_err(ImportError::Unavailable)?;
        let signers = self.bundle_verifier.as_ref().unwrap_or(&self.verifier);
        let bundle = Bundle::open(jws.trim(), signers, &self.node_id, &key)?;
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
        Ok((
            Imported {
                accounts: bundle.accounts.len(),
                snapshots: snapshots.len(),
                withdrawn: 0,
            },
            snapshots.iter().map(|(account, _)| *account).collect(),
        ))
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
