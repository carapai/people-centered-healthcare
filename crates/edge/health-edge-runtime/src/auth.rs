//! The edge's local authorization server (offline-auth.md; ADRs 0009, 0010, 0011, 0059; #75):
//! `health_auth::AuthServer` over the embedded store, so staff sign in on an enrolled device with
//! no internet.
//!
//! - **`/auth`:** authorization code with PKCE, DPoP-bound tokens issued only to an enrolled
//!   device's key, with `acr` `offline-local`. Every login and refresh needs a permission snapshot
//!   for this facility, signed by central, unexpired at a clock that has not gone back.
//! - **`/api/session`**, behind the device gate: who the access token belongs to.
//! - **`/api/admin/provision/accounts`**, for the edge's own screen: central's provisioning
//!   bundle, imported with each hash re-encrypted under this node's pepper key.
//! - **`/api/access/…`**, behind the device gate: offline administration by a signed-in facility
//!   admin (`access.administer-local`): local grants, signed with the node's key, and local
//!   revocations.
//!
//! It needs `HEALTH_FACILITY`, `HEALTH_SNAPSHOT_KEYS_FILE`, and the pepper and token-signing keys
//! that `facility-edge provision request` seals in the keystore. Without any of them the node
//! serves no sign-in, and says why in a `sign-in-off` alert.

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use health_auth::jose::EcJwk;
use health_auth::{
    AuthConfig, AuthServer, Client, Credentials, DeviceGate, GrantSigner, LocalAdmin, Provisioning,
    RecipientKey, SnapshotGate, SnapshotVerifier, SurrealAuthStore, TokenSigner,
};
use health_identity::snapshot::SnapshotPolicy;
use health_time::Clock;
use p256::SecretKey;
use p256::pkcs8::DecodePrivateKey;

use crate::admin;
use crate::config::EdgeConfig;

/// A sealed secret by its credential name, from the composition root's keystore
/// (`health_node_identity::keystore`); `None` when it is not sealed. Blocking: it may run the
/// platform keystore's tools.
pub type Secrets = Arc<dyn Fn(&str) -> Result<Option<Vec<u8>>, String> + Send + Sync>;

/// The facility intermediate's key (PKCS#8 PEM), sealed at `provision request`.
pub const INTERMEDIATE_KEY: &str = "health.facility-intermediate.key";
/// The pepper key every stored hash is encrypted under (ADR 0059): 32 bytes.
pub const PEPPER_KEY: &str = "health.pepper.key";
/// The access-token signing key: a 32-byte P-256 secret.
pub const TOKEN_KEY: &str = "health.token-signing.key";

/// The web client registered with the edge's authorization server.
pub const CLIENT_ID: &str = "health-edge";
/// Where it receives the authorization code.
pub const REDIRECT_PATH: &str = "/callback";

/// The issuer: the node's first origin in `HEALTH_ORIGINS`, under `/auth`. List the https origin
/// LAN devices use first; the default is `https://<node-id>.local`.
pub(crate) fn issuer(config: &EdgeConfig) -> Option<String> {
    let origin = config.origins.first()?;
    Some(format!("{origin}/auth"))
}

/// Whether the configuration asks for sign-in: what `runtime.json` tells the web client.
pub(crate) fn runtime(config: &EdgeConfig) -> Option<serde_json::Value> {
    if config.facility.is_none() || config.snapshot_keys_file.is_none() {
        return None;
    }
    Some(serde_json::json!({
        "issuer": issuer(config)?,
        "clientId": CLIENT_ID,
        "redirectPath": REDIRECT_PATH,
    }))
}

/// Central's snapshot public keys, as a JSON array of JWKs.
fn snapshot_keys(path: &Path) -> Result<SnapshotVerifier, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read HEALTH_SNAPSHOT_KEYS_FILE: {e}"))?;
    let keys: Vec<EcJwk> = serde_json::from_str(&text)
        .map_err(|_| "HEALTH_SNAPSHOT_KEYS_FILE is not a JSON array of JWKs".to_owned())?;
    if keys.is_empty() {
        return Err("HEALTH_SNAPSHOT_KEYS_FILE holds no key".into());
    }
    SnapshotVerifier::new(&keys)
        .map_err(|_| "HEALTH_SNAPSHOT_KEYS_FILE holds an invalid key".into())
}

fn secret(secrets: &Secrets, name: &str) -> Result<Vec<u8>, String> {
    secrets(name)?
        .ok_or_else(|| format!("{name} is not sealed: run `facility-edge provision request`"))
}

/// The authorization server's routes over `store`, with `gate` checking devices; or why there are
/// none.
pub(crate) fn routes(
    config: &EdgeConfig,
    connection: &health_surrealdb::Connection,
    store: SurrealAuthStore,
    gate: &DeviceGate<SurrealAuthStore>,
    secrets: Option<&Secrets>,
    clock: Arc<dyn Clock>,
) -> Result<Router, String> {
    let (Some(facility), Some(keys_file)) = (&config.facility, &config.snapshot_keys_file) else {
        return Err("no HEALTH_FACILITY or HEALTH_SNAPSHOT_KEYS_FILE".into());
    };
    let secrets = secrets.ok_or("no keystore")?;
    let issuer = issuer(config).ok_or("HEALTH_ORIGINS names no origin")?;
    let verifier = snapshot_keys(keys_file)?;
    let credentials = Credentials::new(secret(secrets, PEPPER_KEY)?)
        .map_err(|_| format!("{PEPPER_KEY} is not a 32-byte key"))?;
    let token_key: [u8; 32] = secret(secrets, TOKEN_KEY)?
        .try_into()
        .map_err(|_| format!("{TOKEN_KEY} is not a 32-byte key"))?;
    let signer =
        TokenSigner::from_secrets(&[token_key]).map_err(|_| format!("{TOKEN_KEY} is invalid"))?;

    let mut auth = AuthConfig::new(issuer, "offline-local", true);
    auth.clients.push(Client {
        id: CLIENT_ID.into(),
        redirect_uris: config
            .origins
            .iter()
            .map(|origin| format!("{origin}{REDIRECT_PATH}"))
            .collect(),
        patient: false,
    });
    auth.login_page = Some("/sign-in".into());
    auth.snapshots = Some(SnapshotGate {
        facility: Some(facility.clone()),
        verifier: verifier.clone(),
        policy: SnapshotPolicy::default(),
    });
    auth.devices = Some(gate.check());
    // Sessions lock after 15 minutes idle, and a PIN resumes or switches users on a shared
    // device (offline-auth.md § Tokens and sessions).
    auth.idle_lock = true;
    let server = AuthServer::new(auth, signer, credentials.clone(), store.clone(), clock);

    let recipient: RecipientKey = {
        let secrets = Arc::clone(secrets);
        Arc::new(move || {
            let pem = String::from_utf8(secret(&secrets, INTERMEDIATE_KEY)?)
                .map_err(|_| format!("{INTERMEDIATE_KEY} is not PEM"))?;
            SecretKey::from_pkcs8_pem(&pem)
                .map_err(|_| format!("{INTERMEDIATE_KEY} is not a P-256 key"))
        })
    };
    let provisioning_store = store.clone();
    let provisioning = Provisioning::new(
        store,
        credentials,
        verifier,
        config.node_id.clone(),
        facility.clone(),
        recipient,
    );
    // Offline administration (ADR 0011): grants are signed with the node's key, the facility
    // intermediate's, which central trusts through the certificate it issued.
    let grant_signer: GrantSigner = {
        let secrets = Arc::clone(secrets);
        Arc::new(move |claims| {
            let pem = String::from_utf8(secret(&secrets, INTERMEDIATE_KEY)?)
                .map_err(|_| format!("{INTERMEDIATE_KEY} is not PEM"))?;
            let key = p256::ecdsa::SigningKey::from_pkcs8_pem(&pem)
                .map_err(|_| format!("{INTERMEDIATE_KEY} is not a P-256 key"))?;
            let header = serde_json::json!({
                "alg": "ES256",
                "typ": "local-grant+jwt",
                "jwk": health_auth::jose::EcJwk::of(key.verifying_key()),
            });
            Ok(health_auth::jose::sign(&key, &header, claims))
        })
    };
    let local_admin = LocalAdmin::new(server.verifier(), grant_signer, config.node_id.clone());
    // The clinical API (#106), under the bundle's role mappings and label rules.
    let clinical = health_node_api::Clinical::new(
        connection,
        provisioning_store,
        server.verifier(),
        config.origins.first().map_or("", String::as_str),
        &config.node_id,
        config
            .registry_nin_system
            .clone()
            .zip(config.node_code.clone()),
    );
    Ok(Router::new()
        .nest("/auth", server.router())
        .merge(
            gate.layer(
                Router::new().nest("/api", server.session_route().merge(local_admin.router())),
            ),
        )
        .merge(gate.layer(clinical.routes()))
        .merge(admin::local_only(provisioning.router())))
}
