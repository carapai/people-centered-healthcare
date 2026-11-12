//! The Tauri desktop client (desktop-client.md; ADRs 0036, 0039–0041): the React app in a
//! webview, its `api_request` command dispatched in-process into the node's router
//! (`health_desktop_runtime::ipc`), the embedded store in the per-machine data directory, and the
//! OS keystore. One instance per PC.
//!
//! At startup the volumes are checked for full-disk encryption ([`encryption`]); a shortfall
//! blocks the client unless the country lets it warn, and `GET /api/desktop/encryption` reports
//! it to the app either way.
//!
//! Once provisioned and its accounts imported, the desktop is its facility's authorization server
//! ([`auth`]): staff sign in over IPC (`sign_in`), the session stays in Rust and is attached to
//! every request, and the clinical API answers behind it. Before that, the router serves only the
//! system routes, never records.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;

mod auth;
mod edge;
mod encryption;
mod enroll;
mod node;
mod provision;

use axum::Router;
use axum::routing::get;
use health_desktop_runtime::ipc::{self, Envelope, Reply};
use health_desktop_runtime::keystore::{self, Keystore, KeystoreError};
use health_http::Readiness;
use health_surrealdb::{Connection, MIGRATIONS, Settings, Target, migrate};
use tauri::Manager as _;

/// The app's name: its keystore service and its per-machine directory.
const APP: &str = "health-desktop";

/// The node's API, as the webview reaches it, and the signed-in session, held in Rust.
struct Api {
    router: Router,
    /// Whether local sign-in is set up: provisioned, with the accounts imported.
    sign_in: bool,
    session: tokio::sync::Mutex<Option<String>>,
    /// Under an edge: the link commands pass through while it can be reached (#133).
    edge: Option<Arc<edge::EdgeLink>>,
}

/// `invoke('api_request', { envelope })`: the client's TauriIpcTransport. The session's token is
/// attached here; any credential the webview put in the envelope is dropped.
#[tauri::command]
async fn api_request(api: tauri::State<'_, Api>, mut envelope: Envelope) -> Result<Reply, ()> {
    envelope.headers.retain(|name, _| {
        !name.eq_ignore_ascii_case("authorization") && !name.eq_ignore_ascii_case("dpop")
    });
    // Under an edge, commands pass through to it while it can be reached; the desktop's own
    // routes are always answered here.
    if let Some(edge) = &api.edge
        && !envelope.path.starts_with("/api/desktop/")
        && let Some(reply) = edge.forward(&envelope).await
    {
        return Ok(reply);
    }
    if let Some(token) = api.session.lock().await.as_ref() {
        envelope
            .headers
            .insert("authorization".into(), format!("Bearer {token}"));
    }
    Ok(ipc::dispatch(&api.router, envelope).await)
}

/// `invoke('sign_in', { username, password })`: local sign-in (#134). On success the session is
/// held here; the answer says only whether it worked, and why not.
#[tauri::command]
async fn sign_in(
    api: tauri::State<'_, Api>,
    username: String,
    password: String,
) -> Result<(), String> {
    // Under an edge, staff sign in there while it can be reached (desktop-client.md: online
    // under an edge, the edge is the authorization server).
    let at_edge = match &api.edge {
        Some(edge) => edge.sign_in(&username, &password).await,
        None => None,
    };
    if let Some(signed) = at_edge {
        return signed.map_err(|refused| reason(refused).into());
    }
    if !api.sign_in {
        return Err("unavailable".into());
    }
    match auth::sign_in(&api.router, &username, &password).await {
        Ok(token) => {
            *api.session.lock().await = Some(token);
            Ok(())
        }
        Err(refused) => Err(reason(refused).into()),
    }
}

/// What the sign-in screen is told for `refused`.
fn reason(refused: auth::SignInError) -> &'static str {
    match refused {
        auth::SignInError::Refused => "refused",
        auth::SignInError::Facility => "facility",
        auth::SignInError::ChangePassword => "change_password",
        auth::SignInError::Unavailable(why) => {
            eprintln!("health-desktop: sign-in unavailable: {why}");
            "unavailable"
        }
    }
}

/// `invoke('sign_out')`: ends the session held here, and at the edge.
#[tauri::command]
async fn sign_out(api: tauri::State<'_, Api>) -> Result<(), ()> {
    *api.session.lock().await = None;
    if let Some(edge) = &api.edge {
        edge.sign_out().await;
    }
    Ok(())
}

/// The OS keystore (Windows Credential Manager, the Linux Secret Service) through `keyring`.
/// Secrets stay in Rust: no command hands one to the webview.
struct OsKeystore;

impl OsKeystore {
    fn entry(name: &str) -> Result<keyring::Entry, KeystoreError> {
        keyring::Entry::new(APP, name).map_err(|e| KeystoreError::Unavailable(e.to_string()))
    }
}

impl Keystore for OsKeystore {
    fn get(&self, name: &str) -> Result<Option<Vec<u8>>, KeystoreError> {
        match Self::entry(name)?.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(KeystoreError::Refused(e.to_string())),
        }
    }

    fn set(&self, name: &str, secret: &[u8]) -> Result<(), KeystoreError> {
        Self::entry(name)?
            .set_secret(secret)
            .map_err(|e| KeystoreError::Refused(e.to_string()))
    }

    fn delete(&self, name: &str) -> Result<(), KeystoreError> {
        match Self::entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(KeystoreError::Refused(e.to_string())),
        }
    }
}

/// The data directory: `HEALTH_DESKTOP_DATA_DIR` in development, else the per-machine one the
/// installer creates.
fn data_dir() -> PathBuf {
    std::env::var_os("HEALTH_DESKTOP_DATA_DIR").map_or_else(
        || health_desktop_runtime::data_dir::per_machine(APP),
        PathBuf::from,
    )
}

/// The embedded store in `dir`, opened and migrated.
async fn connect(dir: &std::path::Path) -> Result<Connection, String> {
    let connection = Connection::open(&Settings {
        target: Target::SurrealKv {
            path: dir.join("store"),
        },
        namespace: "ug".into(),
        database: "main".into(),
    })
    .await
    .map_err(|e| format!("cannot open the store in {}: {e}", dir.display()))?;
    migrate(&connection, MIGRATIONS)
        .await
        .map_err(|e| format!("cannot migrate the store: {e:?}"))?;
    Ok(connection)
}

/// Opens and migrates the embedded store, and builds the router over it.
async fn open() -> Result<Api, String> {
    // No keystore, no client (desktop-client.md § Secrets): there is no file fallback.
    keystore::check(&OsKeystore).map_err(|e| e.to_string())?;
    let dir = data_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("cannot create the data directory {}: {e}", dir.display()))?;
    // SurrealDB has no encryption at rest: the OS must encrypt the volumes (ADR 0040).
    let probed = dir.clone();
    let state = tokio::task::spawn_blocking(move || {
        health_node_identity::encryption::probe(
            &health_node_identity::encryption::system_root(),
            &probed,
            health_time::Timestamp::now(),
        )
    })
    .await
    .map_err(|e| e.to_string())?;
    let policy = encryption::Policy::from_setting(
        std::env::var("HEALTH_DESKTOP_ENCRYPTION").ok().as_deref(),
    );
    let report = encryption::Report::of(state, policy);
    if !report.may_start() {
        return Err(format!(
            "full-disk encryption is required here: {}",
            report.shortfalls.join("; ")
        ));
    }
    if !report.encrypted {
        eprintln!(
            "health-desktop: warning: not fully encrypted ({}); the country allows it",
            report.shortfalls.join("; ")
        );
    }
    let connection = connect(&dir).await?;
    // Once provisioned, the desktop syncs with central for its facility (#134). Once enrolled
    // with an edge, it syncs with the edge, and its rounds tell the link whether the edge is
    // there (#133).
    let edge = match edge::EdgeLink::open(&dir, &OsKeystore) {
        Ok(link) => link.map(Arc::new),
        Err(e) => {
            eprintln!("health-desktop: cannot reach the edge: {e}");
            None
        }
    };
    match node::sync(connection.clone(), &dir, &OsKeystore) {
        Ok(Some(sync)) => match &edge {
            Some(edge) => {
                let edge = Arc::clone(edge);
                tokio::spawn(async move { edge.sync(sync, node::interval()).await });
            }
            None => {
                tokio::spawn(async move {
                    if let Err(e) = sync.run(node::interval()).await {
                        eprintln!("health-desktop: sync stopped: {e}");
                    }
                });
            }
        },
        Ok(None) => eprintln!("health-desktop: neither provisioned nor enrolled: no sync"),
        Err(e) => eprintln!("health-desktop: cannot sync: {e}"),
    }
    // Hourly encrypted backups, as an edge takes them (#134).
    match node::backups(&dir, |name| {
        std::env::var(name).ok().filter(|v| !v.trim().is_empty())
    }) {
        Ok(schedule) => {
            let connection = connection.clone();
            tokio::spawn(async move {
                if let Err(e) = schedule.run(connection).await {
                    eprintln!("health-desktop: backups stopped: {e}");
                }
            });
        }
        Err(why) => eprintln!("health-desktop: warning: backups are off: {why}"),
    }
    // Local sign-in and, behind it, the clinical API, once the accounts are imported.
    let signed = match auth::routes(&connection, &dir, &OsKeystore) {
        Ok(routes) => routes,
        Err(e) => {
            eprintln!("health-desktop: no sign-in: {e}");
            None
        }
    };
    if signed.is_none() {
        eprintln!("health-desktop: no accounts imported: no sign-in, and no records served");
    }
    let readiness = Readiness::new(Vec::new());
    let report = Arc::new(report);
    let desktop = Router::new().route(
        "/api/desktop/encryption",
        get(move || {
            let report = Arc::clone(&report);
            async move { axum::Json((*report).clone()) }
        }),
    );
    let sign_in = signed.is_some();
    let desktop = match signed {
        Some(routes) => desktop.merge(routes),
        None => desktop,
    };
    Ok(Api {
        router: health_http::app(desktop, readiness),
        sign_in,
        session: tokio::sync::Mutex::new(None),
        edge,
    })
}

/// Provisions this machine against central, with the volumes' encryption reported to it.
fn provision_here(args: &[String]) -> Result<health_sync_client::provisioning::Redeemed, String> {
    let args = provision::Args::parse(args)?;
    let dir = data_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("cannot create the data directory {}: {e}", dir.display()))?;
    let state = health_node_identity::encryption::probe(
        &health_node_identity::encryption::system_root(),
        &dir,
        health_time::Timestamp::now(),
    );
    let report = encryption::Report::of(state, encryption::Policy::Warn);
    if !report.encrypted {
        eprintln!(
            "health-desktop: not encrypted ({}); central accepts this only under an exemption on \
             the code",
            report.shortfalls.join("; ")
        );
    }
    provision::provision(
        &args,
        &dir,
        &OsKeystore,
        report.encrypted,
        health_sync_client::provisioning::redeem,
    )
}

/// Enrolls this machine with its edge, showing the pairing code for the admin.
fn enroll_here(args: &[String]) -> Result<String, String> {
    let args = enroll::Args::parse(args)?;
    let dir = data_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("cannot create the data directory {}: {e}", dir.display()))?;
    let mut edge = enroll::Https::new(&args)?;
    enroll::enroll(&args, &dir, &OsKeystore, &mut edge, |code| {
        eprintln!(
            "health-desktop: ask a facility admin to approve this desktop on the edge with the \
             pairing code {code}; waiting…"
        );
    })
}

fn main() -> std::process::ExitCode {
    // `health-desktop provision …`: once, by an admin, before the client is used (#134).
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("provision") {
        return match provision_here(&args[1..]) {
            Ok(redeemed) => {
                eprintln!(
                    "health-desktop: provisioned as {} ({}) at {}",
                    redeemed.short_code, redeemed.node_id, redeemed.facility
                );
                std::process::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("health-desktop: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    // `health-desktop enroll …`: once, under an edge, with an admin there to approve it (#133).
    if args.first().map(String::as_str) == Some("enroll") {
        return match enroll_here(&args[1..]) {
            Ok(device) => {
                eprintln!("health-desktop: enrolled with the edge as device {device}");
                std::process::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("health-desktop: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    // SurrealDB 4 needs a larger stack than Tokio's default (`health_surrealdb::THREAD_STACK`),
    // so Tauri runs on this runtime.
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(health_surrealdb::THREAD_STACK)
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("health-desktop: cannot start the async runtime: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    tauri::async_runtime::set(runtime.handle().clone());
    // `health-desktop seed import <file>`: with the client closed, since the store is locked.
    // `health-desktop accounts import <bundle> --snapshot-keys <keys>`: central's staff bundle.
    if let [accounts, import, bundle, flag, keys_file] = args.as_slice()
        && accounts == "accounts"
        && import == "import"
        && flag == "--snapshot-keys"
    {
        let dir = data_dir();
        let imported = runtime.block_on(async {
            let bundle = std::fs::read_to_string(bundle).map_err(|e| format!("{bundle}: {e}"))?;
            let connection = connect(&dir).await?;
            auth::import(
                &connection,
                &dir,
                &OsKeystore,
                &bundle,
                std::path::Path::new(keys_file),
            )
            .await
        });
        return match imported {
            Ok(accounts) => {
                eprintln!("health-desktop: {accounts} accounts imported; staff can sign in");
                std::process::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("health-desktop: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    if let [seed, import, file] = args.as_slice()
        && seed == "seed"
        && import == "import"
    {
        let dir = data_dir();
        let seeded = runtime.block_on(async {
            let connection = connect(&dir).await?;
            node::seed_import(connection, &dir, std::path::Path::new(file)).await
        });
        return match seeded {
            Ok(applied) => {
                eprintln!("health-desktop: seeded {applied} records; sync resumes after the seed");
                std::process::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("health-desktop: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    let api = match runtime.block_on(open()) {
        Ok(api) => api,
        Err(e) => {
            eprintln!("health-desktop: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let result = tauri::Builder::default()
        // A second launch focuses the running window; the store's lock backs it up.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .manage(api)
        .invoke_handler(tauri::generate_handler![api_request, sign_in, sign_out])
        .run(tauri::generate_context!());
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("health-desktop: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[tokio::test]
    async fn the_router_serves_the_system_routes_and_no_records_yet() {
        let router = health_http::app(Router::new(), Readiness::new(Vec::new()));
        let ask = |path: &str| Envelope {
            method: "GET".into(),
            path: path.into(),
            query: Vec::new(),
            headers: BTreeMap::new(),
            body: None,
        };
        assert_eq!(ipc::dispatch(&router, ask("/health")).await.status, 200);
        assert_eq!(
            ipc::dispatch(&router, ask("/fhir/Patient")).await.status,
            404
        );
    }

    #[test]
    fn the_data_directory_can_be_set_for_development() {
        let dir = data_dir();
        assert!(dir.ends_with(APP) || std::env::var_os("HEALTH_DESKTOP_DATA_DIR").is_some());
    }
}
