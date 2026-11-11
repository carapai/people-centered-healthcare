//! The desktop's IPC transport for the client's contract tests (`apps/client`,
//! `bun run test:contract`): one [`FhirServer`] on a SurrealDB memory engine, served at `/fhir`
//! over HTTP and through `POST /ipc`, which takes the `{ envelope }` the webview's
//! `invoke('api_request', …)` sends and answers through [`ipc::dispatch`], as the Tauri command
//! does. The suite reads through the client's own IPC transport, so both halves of the IPC
//! boundary meet the same contract as HTTP. Prints one JSON line with the two URLs, then serves
//! until killed. Development and CI only: there is no authentication.

use std::io::Write;
use std::sync::Arc;

use axum::Router;
use axum::routing::post;
use health_desktop_runtime::ipc::{self, Envelope};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::FhirServer;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use serde_json::{Value, json};
use tokio::net::TcpListener;

type Failure = Box<dyn std::error::Error + Send + Sync>;

/// The search parameters the contract suite uses, as `contract_server` registers them.
fn registry() -> Result<Registry, Failure> {
    let bundle: Value = serde_json::from_str(include_str!(
        "../../../contracts/health-fhir-search/tests/fixtures/search-parameters.json"
    ))?;
    let entries = bundle["entry"]
        .as_array()
        .ok_or("search parameters: no entries")?;
    let catalog = Catalog::from_definitions(entries.iter().map(|e| &e["resource"]));
    let mut registry = Registry::new(jiff::tz::Offset::UTC);
    for (resource_type, codes) in [
        (
            "Patient",
            &[
                "_id",
                "_lastUpdated",
                "name",
                "family",
                "birthdate",
                "identifier",
            ][..],
        ),
        ("Observation", &["_id", "code", "subject", "status"][..]),
    ] {
        for code in codes {
            registry.support(&catalog, resource_type, code, ParamState::Active)?;
        }
    }
    Ok(registry)
}

#[derive(serde::Deserialize)]
struct Invoke {
    envelope: Envelope,
}

fn main() -> Result<(), Failure> {
    // SurrealDB 4 needs a larger stack than Tokio's default (`health_surrealdb::THREAD_STACK`).
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(health_surrealdb::THREAD_STACK)
        .build()?
        .block_on(run())
}

async fn run() -> Result<(), Failure> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let origin = format!("http://{}", listener.local_addr()?);
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "contract".into(),
        database: "desktop".into(),
    })
    .await?;
    migrate(&connection, MIGRATIONS).await?;
    let store = SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry()?);
    let api = Router::new().nest(
        "/fhir",
        FhirServer::new(store, format!("{origin}/fhir"), Arc::new(Unrestricted)).router(),
    );
    let dispatched = api.clone();
    let app = api.route(
        "/ipc",
        post(move |axum::Json(invoke): axum::Json<Invoke>| {
            let router = dispatched.clone();
            async move { axum::Json(ipc::dispatch(&router, invoke.envelope).await) }
        }),
    );
    let line = json!({ "base": format!("{origin}/fhir"), "ipc": format!("{origin}/ipc") })
        .to_string()
        + "\n";
    let mut stdout = std::io::stdout();
    stdout.write_all(line.as_bytes())?;
    stdout.flush()?;
    axum::serve(listener, app).await?;
    Ok(())
}
