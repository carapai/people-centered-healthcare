//! Local sign-in on the standalone desktop (#134; desktop-client.md § Provisioning, mode B;
//! offline-auth.md; ADR 0036): the desktop is its facility's authorization server, as an edge is.
//!
//! - **Accounts** come from central's provisioning bundle, imported once with
//!   `health-desktop accounts import <bundle.jws> --snapshot-keys <keys.json>`. Its hashes are
//!   re-encrypted under this node's pepper, and central's snapshot keys are kept beside the chain.
//! - **Sign-in runs in process.** The `sign_in` command takes the username and password over IPC
//!   and runs the authorization-code flow with PKCE against `health_auth::AuthServer` through the
//!   router, the code that serves edges and central, with its lockout, snapshot and clock checks.
//!   The access token stays in Rust: `api_request` attaches it to every request, and the webview
//!   never sees it.
//! - **The pepper and token-signing keys** are made on first use and kept in the OS keystore.
//!
//! The clinical API (`health-node-api`) is served behind the session, under the bundle's rules.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use health_auth::jose::EcJwk;
use health_auth::{
    AuthConfig, AuthServer, Client, Credentials, Provisioning, RecipientKey, SnapshotGate,
    SnapshotVerifier, SurrealAuthStore, TokenSigner,
};
use health_desktop_runtime::ipc::{self, Envelope};
use health_desktop_runtime::keystore::Keystore;
use health_identity::SessionPolicy;
use health_identity::snapshot::SnapshotPolicy;
use health_surrealdb::{Connection, SurrealStore};
use health_time::SystemClock;
use p256::pkcs8::DecodePrivateKey as _;
use rand_core::RngCore as _;
use sha2::Digest as _;

use crate::provision::{KEY, pki_dir};

/// The pepper key every stored hash is encrypted under (ADR 0059): 32 bytes.
const PEPPER_KEY: &str = "health.pepper.key";
/// The access-token signing key: a P-256 secret.
const TOKEN_KEY: &str = "health.token-signing.key";
/// The in-process origin: no socket serves it.
const ORIGIN: &str = "http://desktop.local";
/// The client the desktop signs in as.
const CLIENT_ID: &str = "health-desktop";
/// Where the authorization code is returned, read in process.
const REDIRECT: &str = "http://desktop.local/callback";

/// Central's snapshot public keys, kept beside the chain at import.
fn snapshot_keys_path(data_dir: &Path) -> PathBuf {
    pki_dir(data_dir).join("snapshot-keys.json")
}

fn verifier(path: &Path) -> Result<SnapshotVerifier, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let keys: Vec<EcJwk> = serde_json::from_str(&text)
        .map_err(|_| format!("{} is not a JSON array of JWKs", path.display()))?;
    if keys.is_empty() {
        return Err(format!("{} holds no key", path.display()));
    }
    SnapshotVerifier::new(&keys).map_err(|_| format!("{} holds an invalid key", path.display()))
}

/// The secret `name`, made with `make` and kept in the keystore the first time.
fn secret(
    keys: &dyn Keystore,
    name: &str,
    make: impl FnOnce() -> Vec<u8>,
) -> Result<Vec<u8>, String> {
    if let Some(held) = keys.get(name).map_err(|e| e.to_string())? {
        return Ok(held);
    }
    let made = make();
    keys.set(name, &made).map_err(|e| e.to_string())?;
    Ok(made)
}

fn credentials(keys: &dyn Keystore) -> Result<Credentials, String> {
    let pepper = secret(keys, PEPPER_KEY, || {
        let mut bytes = vec![0; 32];
        rand_core::OsRng.fill_bytes(&mut bytes);
        bytes
    })?;
    Credentials::new(pepper).map_err(|_| format!("{PEPPER_KEY} is not a 32-byte key"))
}

fn signer(keys: &dyn Keystore) -> Result<TokenSigner, String> {
    let token = secret(keys, TOKEN_KEY, || {
        p256::SecretKey::random(&mut rand_core::OsRng)
            .to_bytes()
            .to_vec()
    })?;
    let token: [u8; 32] = token
        .try_into()
        .map_err(|_| format!("{TOKEN_KEY} is not a 32-byte key"))?;
    TokenSigner::from_secrets(&[token]).map_err(|_| format!("{TOKEN_KEY} is invalid"))
}

/// The node's identity from `pki/node.json`: its node ID and facility.
fn identity(data_dir: &Path) -> Result<(String, String), String> {
    let path = pki_dir(data_dir).join("node.json");
    if !path.is_file() {
        // Under an edge: the enrolled device, at the facility the edge named (#133).
        let edge = pki_dir(data_dir).join("edge.json");
        let text = std::fs::read_to_string(&edge)
            .map_err(|_| "this desktop is neither provisioned nor enrolled".to_owned())?;
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        return match (value["device_id"].as_str(), value["facility"].as_str()) {
            (Some(device), Some(facility)) => Ok((device.to_owned(), facility.to_owned())),
            _ => Err("the edge has not yet handed over its staff".into()),
        };
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|_| "this desktop is not provisioned: provision it first".to_owned())?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    match (value["node_id"].as_str(), value["facility"].as_str()) {
        (Some(node), Some(facility)) => Ok((node.to_owned(), facility.to_owned())),
        _ => Err(format!("{} names no node or facility", path.display())),
    }
}

fn auth_store(connection: &Connection) -> SurrealAuthStore {
    SurrealAuthStore::new(
        SurrealStore::new(connection.clone(), Arc::new(SystemClock)),
        SessionPolicy::default().hard_cap,
    )
}

/// `health-desktop accounts import <bundle> --snapshot-keys <keys>`: central's provisioning
/// bundle for this node, opened with its key from the keystore. Returns the accounts imported.
pub(crate) async fn import(
    connection: &Connection,
    data_dir: &Path,
    keys: &(dyn Keystore + 'static),
    bundle: &str,
    snapshot_keys: &Path,
) -> Result<usize, String> {
    let (node, facility) = identity(data_dir)?;
    let verifier = verifier(snapshot_keys)?;
    let pem = keys
        .get(KEY)
        .map_err(|e| e.to_string())?
        .ok_or("the node key is not in the keystore: provision this desktop first")?;
    let pem = String::from_utf8(pem).map_err(|_| format!("{KEY} is not PEM"))?;
    let recipient: RecipientKey = Arc::new(move || {
        p256::SecretKey::from_pkcs8_pem(&pem).map_err(|_| format!("{KEY} is not a P-256 key"))
    });
    let provisioning = Provisioning::new(
        auth_store(connection),
        credentials(keys)?,
        verifier,
        node,
        facility,
        recipient,
    );
    let imported = provisioning
        .import(bundle)
        .await
        .map_err(|e| e.to_string())?;
    // Kept for sign-in, which checks every snapshot against them.
    std::fs::copy(snapshot_keys, snapshot_keys_path(data_dir))
        .map_err(|e| format!("cannot keep the snapshot keys: {e}"))?;
    Ok(imported.accounts)
}

/// The edge's staff for this enrolled desktop (#133), its answer to `POST /api/desktop/accounts`:
/// the bundle, signed with the edge's facility intermediate key (`signer`), its hashes encrypted
/// to the device key; and central's snapshot keys, which every snapshot inside must verify
/// against, kept for sign-in. The facility is the edge's, and stays: an answer for another is
/// refused. Returns the accounts imported.
pub(crate) async fn import_from_edge(
    connection: &Connection,
    data_dir: &Path,
    keys: &(dyn Keystore + 'static),
    answer: &serde_json::Value,
) -> Result<usize, String> {
    let edge_file = pki_dir(data_dir).join("edge.json");
    let mut edge: serde_json::Value = std::fs::read_to_string(&edge_file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .ok_or("this desktop is not enrolled with an edge")?;
    let device = edge["device_id"]
        .as_str()
        .ok_or("edge.json names no device")?
        .to_owned();
    let facility = answer["facility"]
        .as_str()
        .ok_or("the edge's answer names no facility")?
        .to_owned();
    if let Some(known) = edge["facility"].as_str()
        && known != facility
    {
        return Err(format!("the edge now names {facility}, not {known}"));
    }
    let signer: EcJwk = serde_json::from_value(answer["signer"].clone())
        .map_err(|_| "the edge's answer has no signer key")?;
    let snapshot_keys: Vec<EcJwk> = serde_json::from_value(answer["snapshot_keys"].clone())
        .map_err(|_| "the edge's answer has no snapshot keys")?;
    let bundle = answer["bundle"]
        .as_str()
        .ok_or("the edge's answer has no bundle")?;
    let verifier = SnapshotVerifier::new(&snapshot_keys)
        .map_err(|_| "the edge's snapshot keys are invalid")?;
    let bundle_verifier =
        SnapshotVerifier::new(&[signer]).map_err(|_| "the edge's signer key is invalid")?;
    let pem = keys
        .get(crate::enroll::DEVICE_KEY)
        .map_err(|e| e.to_string())?
        .ok_or("the device key is not in the keystore")?;
    let pem = String::from_utf8(pem).map_err(|_| "the device key is not PEM".to_owned())?;
    let recipient: RecipientKey = Arc::new(move || {
        p256::SecretKey::from_pkcs8_pem(&pem)
            .map_err(|_| "the device key is not a P-256 key".into())
    });
    let imported = Provisioning::new(
        auth_store(connection),
        credentials(keys)?,
        verifier,
        device,
        facility.clone(),
        recipient,
    )
    .with_bundle_verifier(bundle_verifier)
    .import(bundle)
    .await
    .map_err(|e| e.to_string())?;
    // Kept for sign-in, which checks every snapshot against them; the facility last, since its
    // presence is what lets local sign-in start.
    std::fs::write(
        snapshot_keys_path(data_dir),
        serde_json::to_string(&snapshot_keys).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("cannot keep the snapshot keys: {e}"))?;
    edge["facility"] = serde_json::Value::String(facility);
    std::fs::write(&edge_file, edge.to_string()).map_err(|e| format!("edge.json: {e}"))?;
    Ok(imported.accounts)
}

/// The authorization server and, behind its sessions, the clinical API; `None` until the desktop
/// is provisioned and its accounts imported.
pub(crate) fn routes(
    connection: &Connection,
    data_dir: &Path,
    keys: &dyn Keystore,
    workflows: Option<&health_node_api::WorkflowConfig>,
) -> Result<Option<Router>, String> {
    let keys_file = snapshot_keys_path(data_dir);
    if !keys_file.is_file() {
        return Ok(None);
    }
    let (node, facility) = identity(data_dir)?;
    let mut config = AuthConfig::new(format!("{ORIGIN}/auth"), "offline-local", false);
    config.clients.push(Client {
        id: CLIENT_ID.into(),
        redirect_uris: vec![REDIRECT.into()],
        patient: false,
    });
    config.snapshots = Some(SnapshotGate {
        facility: Some(facility),
        verifier: verifier(&keys_file)?,
        policy: SnapshotPolicy::default(),
    });
    // Refusals come back as redirects to this page with `error=refused|facility|change_password`,
    // which `sign_in` reads in process: no page is served.
    config.login_page = Some(format!("{ORIGIN}/sign-in"));
    // Sessions lock after 15 minutes idle (offline-auth.md § Tokens and sessions).
    config.idle_lock = true;
    let store = auth_store(connection);
    let server = AuthServer::new(
        config,
        signer(keys)?,
        credentials(keys)?,
        store.clone(),
        Arc::new(SystemClock),
    );
    let clinical = health_node_api::Clinical::new(
        connection,
        store.clone(),
        server.verifier(),
        ORIGIN,
        &node,
        None,
    );
    // Workflows (#100): on this node's own instances, or on its replicas of its edge's.
    let clinical = match workflows {
        Some(workflows) => clinical.with_workflows(workflows.clone()),
        None => clinical,
    };
    Ok(Some(
        Router::new()
            .nest("/auth", server.router())
            .nest("/api", server.session_route())
            .merge(clinical.routes()),
    ))
}

/// Why sign-in failed, as the sign-in screen shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SignInError {
    /// Wrong credentials, a locked account, or no valid snapshot here.
    Refused,
    /// The account works at several facilities and named none.
    Facility,
    /// The password is a temporary one an admin set.
    ChangePassword,
    /// Sign-in is not set up, or failed otherwise.
    Unavailable(String),
}

/// An authorization server's client, as the desktop signs in to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OAuthClient<'a> {
    /// Its `client_id`.
    pub(crate) id: &'a str,
    /// Its registered `redirect_uri`.
    pub(crate) redirect: &'a str,
}

/// The desktop's own authorization server's client.
pub(crate) const LOCAL: OAuthClient<'static> = OAuthClient {
    id: CLIENT_ID,
    redirect: REDIRECT,
};

/// Signs `username` in with `password` through `router`'s authorization server, in process:
/// authorization code with PKCE, then the token. Returns the access token, which stays in Rust.
pub(crate) async fn sign_in(
    router: &Router,
    username: &str,
    password: &str,
) -> Result<String, SignInError> {
    sign_in_with(LOCAL, router, username, password).await
}

/// Where sign-in's requests go.
pub(crate) trait Transport: Sync {
    /// Sends `envelope`; the answer.
    fn send(&self, envelope: Envelope) -> impl Future<Output = ipc::Reply> + Send;
}

/// The local authorization server, in process.
impl Transport for Router {
    fn send(&self, envelope: Envelope) -> impl Future<Output = ipc::Reply> + Send {
        ipc::dispatch(self, envelope)
    }
}

/// Signs `username` in with `password` at the authorization server `send` reaches, as `client`:
/// the local one in process, or the edge's over the LAN (#133), through `transport`. Returns the
/// access token.
pub(crate) async fn sign_in_with(
    client: OAuthClient<'_>,
    transport: &impl Transport,
    username: &str,
    password: &str,
) -> Result<String, SignInError> {
    let mut verifier = [0u8; 32];
    rand_core::OsRng.fill_bytes(&mut verifier);
    let verifier = URL_SAFE_NO_PAD.encode(verifier);
    let challenge = URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()));
    let mut state = [0u8; 16];
    rand_core::OsRng.fill_bytes(&mut state);
    let state = URL_SAFE_NO_PAD.encode(state);
    let form = |pairs: &[(&str, &str)]| {
        form_urlencoded::Serializer::new(String::new())
            .extend_pairs(pairs)
            .finish()
    };
    let post = |path: &str, body: String| Envelope {
        method: "POST".into(),
        path: path.into(),
        query: Vec::new(),
        headers: [(
            "content-type".to_owned(),
            "application/x-www-form-urlencoded".to_owned(),
        )]
        .into(),
        body: Some(body),
    };
    let authorized = transport
        .send(post(
            "/auth/authorize",
            form(&[
                ("response_type", "code"),
                ("client_id", client.id),
                ("redirect_uri", client.redirect),
                ("scope", "openid"),
                ("state", &state),
                ("code_challenge", &challenge),
                ("code_challenge_method", "S256"),
                ("username", username),
                ("password", password),
            ]),
        ))
        .await;
    let location = authorized
        .headers
        .get("location")
        .cloned()
        .unwrap_or_default();
    let query = location.split_once('?').map_or("", |(_, q)| q);
    let params: Vec<(String, String)> = form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    let param = |name: &str| {
        params
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    };
    let Some(code) = param("code") else {
        return Err(match param("error").as_deref() {
            Some("facility") => SignInError::Facility,
            Some("change_password") => SignInError::ChangePassword,
            Some(_) => SignInError::Refused,
            None => SignInError::Unavailable(format!(
                "the authorization server answered {}",
                authorized.status
            )),
        });
    };
    if param("state").as_deref() != Some(state.as_str()) {
        return Err(SignInError::Unavailable(
            "the authorization answered another request".into(),
        ));
    }
    let token = transport
        .send(post(
            "/auth/token",
            form(&[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", client.redirect),
                ("client_id", client.id),
                ("code_verifier", &verifier),
            ]),
        ))
        .await;
    if token.status != 200 {
        return Err(SignInError::Refused);
    }
    let answer: serde_json::Value = serde_json::from_str(&token.body)
        .map_err(|_| SignInError::Unavailable("the token answer is not JSON".into()))?;
    answer["access_token"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| SignInError::Unavailable("the token answer has no access token".into()))
}

#[cfg(test)]
mod tests {
    use health_auth::SnapshotSigner;
    use health_desktop_runtime::keystore::MemoryKeystore;
    use health_identity::snapshot::{PermissionSnapshot, RoleAssignment};
    use health_identity::{AccountId, AccountStatus, Lockout, LoginAccount};
    use health_surrealdb::{MIGRATIONS, Settings, Target, migrate};

    use super::*;

    /// A provisioned desktop of KLA001 with one clinician, as an imported bundle leaves it.
    async fn desktop() -> (Connection, tempfile::TempDir, MemoryKeystore) {
        let connection = Connection::open(&Settings {
            target: Target::Memory,
            namespace: "desktop".into(),
            database: "main".into(),
        })
        .await
        .expect("open");
        migrate(&connection, MIGRATIONS).await.expect("migrate");
        let dir = tempfile::tempdir().expect("dir");
        let pki = pki_dir(dir.path());
        std::fs::create_dir_all(&pki).expect("pki");
        std::fs::write(
            pki.join("node.json"),
            r#"{"node_id":"018f0000-0000-7000-8000-000000000000","facility":"KLA001","central":"https://central.example"}"#,
        )
        .expect("node");
        let central = SnapshotSigner::generate();
        std::fs::write(
            snapshot_keys_path(dir.path()),
            serde_json::to_string(&[central.public()]).expect("keys"),
        )
        .expect("keys");
        let keys = MemoryKeystore::default();
        let store = auth_store(&connection);
        let account = AccountId(uuid::Uuid::now_v7());
        store
            .put_account(&LoginAccount {
                id: account,
                username: "nakato".into(),
                credential: credentials(&keys)
                    .expect("credentials")
                    .credential(account, "correct horse battery")
                    .expect("hash"),
                status: AccountStatus::Active,
                lockout: Lockout::default(),
            })
            .await
            .expect("account");
        let issued = health_time::Timestamp::now();
        let snapshot = PermissionSnapshot {
            account,
            facility: "KLA001".into(),
            roles: vec![RoleAssignment {
                role: "clinician".into(),
                locations: vec![],
            }],
            programs: vec![],
            issued_at: issued,
            not_after: SnapshotPolicy::default().not_after(issued),
            snapshot_version: 1,
        };
        let roles = serde_json::from_value(serde_json::json!({ "clinician": ["Patient.crs"] }))
            .expect("roles");
        let jws = central.sign(&snapshot, &roles).expect("sign");
        store
            .put_snapshot(account, "KLA001", &jws)
            .await
            .expect("snapshot");
        (connection, dir, keys)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn staff_sign_in_in_process_and_the_token_never_leaves_rust() {
        let (connection, dir, keys) = desktop().await;
        let router = routes(&connection, dir.path(), &keys, None)
            .expect("routes")
            .expect("set up");
        // The keystore now holds the pepper and the token key, made on first use.
        assert!(keys.get(PEPPER_KEY).expect("get").is_some());
        assert!(keys.get(TOKEN_KEY).expect("get").is_some());

        assert_eq!(
            sign_in(&router, "nakato", "wrong").await,
            Err(SignInError::Refused)
        );
        let token = sign_in(&router, "nakato", "correct horse battery")
            .await
            .expect("signed in");
        // The session answers with the token, and not without it.
        let session = |token: Option<&str>| Envelope {
            method: "GET".into(),
            path: "/api/session".into(),
            query: Vec::new(),
            headers: token
                .map(|t| ("authorization".to_owned(), format!("Bearer {t}")))
                .into_iter()
                .collect(),
            body: None,
        };
        assert_eq!(
            ipc::dispatch(&router, session(Some(&token))).await.status,
            200
        );
        assert_eq!(ipc::dispatch(&router, session(None)).await.status, 401);
    }

    #[tokio::test]
    async fn without_imported_accounts_there_is_no_sign_in() {
        let (connection, dir, keys) = desktop().await;
        std::fs::remove_file(snapshot_keys_path(dir.path())).expect("remove");
        assert!(
            routes(&connection, dir.path(), &keys, None)
                .expect("routes")
                .is_none()
        );
    }

    /// Under an edge (#133): the edge's staff for this desktop's location arrive encrypted to its
    /// device key, are kept under its own pepper, and sign in here while the edge cannot be
    /// reached. An answer naming another facility afterwards is refused.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_edges_staff_sign_in_here_while_it_cannot_be_reached() {
        // The edge, holding a clinician of KLA001 with a snapshot central signed.
        let (edge_connection, edge_dir, edge_keys) = desktop().await;
        let central_keys: Vec<EcJwk> = serde_json::from_str(
            &std::fs::read_to_string(snapshot_keys_path(edge_dir.path())).expect("keys"),
        )
        .expect("keys");
        let intermediate = SnapshotSigner::generate();
        let device_pem = health_pki::new_key().expect("key").serialize_pem();
        let device_key = p256::SecretKey::from_pkcs8_pem(&device_pem).expect("device key");
        let device = "0193a000-0000-7000-8000-000000000002";
        let unused: RecipientKey = Arc::new(|| Err("not importing".into()));
        let bundle = Provisioning::new(
            auth_store(&edge_connection),
            credentials(&edge_keys).expect("credentials"),
            SnapshotVerifier::new(&central_keys).expect("verifier"),
            "edge",
            "KLA001",
            unused,
        )
        .export(
            device,
            "OPD",
            &device_key.public_key(),
            &intermediate,
            health_time::Timestamp::now(),
        )
        .await
        .expect("export");
        let answer = |facility: &str| {
            serde_json::json!({
                "facility": facility,
                "snapshot_keys": central_keys,
                "signer": intermediate.public(),
                "bundle": bundle,
            })
        };

        // The desktop, enrolled with that edge.
        let connection = Connection::open(&Settings {
            target: Target::Memory,
            namespace: "enrolled".into(),
            database: "main".into(),
        })
        .await
        .expect("open");
        migrate(&connection, MIGRATIONS).await.expect("migrate");
        let dir = tempfile::tempdir().expect("dir");
        let pki = pki_dir(dir.path());
        std::fs::create_dir_all(&pki).expect("pki");
        std::fs::write(
            pki.join("edge.json"),
            serde_json::json!({ "edge": "https://edge.local", "device_id": device }).to_string(),
        )
        .expect("edge");
        let keys = MemoryKeystore::default();
        keys.set(crate::enroll::DEVICE_KEY, device_pem.as_bytes())
            .expect("device key");
        assert!(
            routes(&connection, dir.path(), &keys, None)
                .expect("routes")
                .is_none()
        );

        let imported = import_from_edge(&connection, dir.path(), &keys, &answer("KLA001"))
            .await
            .expect("import");
        assert_eq!(imported, 1);
        let router = routes(&connection, dir.path(), &keys, None)
            .expect("routes")
            .expect("set up");
        assert_eq!(
            sign_in(&router, "nakato", "wrong").await,
            Err(SignInError::Refused)
        );
        assert!(
            sign_in(&router, "nakato", "correct horse battery")
                .await
                .is_ok()
        );

        let moved = import_from_edge(&connection, dir.path(), &keys, &answer("GUL002")).await;
        assert!(moved.is_err_and(|e| e.contains("GUL002")));
    }
}
