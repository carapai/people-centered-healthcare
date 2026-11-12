//! The Tauri desktop client (desktop-client.md; ADRs 0036, 0039–0041): the React app in a
//! webview, its `api_request` command dispatched in-process into the node's router
//! (`health_desktop_runtime::ipc`), the embedded store in the per-machine data directory, and the
//! OS keystore. One instance per PC.
//!
//! At startup the volumes are checked for full-disk encryption ([`encryption`]); a shortfall
//! blocks the client unless the country lets it warn, and `GET /api/desktop/encryption` reports
//! it to the app either way.
//!
//! The FHIR and application routes join the router with local sign-in (#133, #134): until a
//! session can be checked here, the router serves only the system routes, never records.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;

mod encryption;
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

/// The node's API, as the webview reaches it.
struct Api {
    router: Router,
}

/// `invoke('api_request', { envelope })`: the client's TauriIpcTransport.
#[tauri::command]
async fn api_request(api: tauri::State<'_, Api>, envelope: Envelope) -> Result<Reply, ()> {
    Ok(ipc::dispatch(&api.router, envelope).await)
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
    let readiness = Readiness::new(Vec::new());
    let report = Arc::new(report);
    let desktop = Router::new().route(
        "/api/desktop/encryption",
        get(move || {
            let report = Arc::clone(&report);
            async move { axum::Json((*report).clone()) }
        }),
    );
    Ok(Api {
        router: health_http::app(desktop, readiness),
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
        .invoke_handler(tauri::generate_handler![api_request])
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
