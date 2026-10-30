//! The FHIR REST API for the client's contract tests (`apps/client`, `bun run test:contract`):
//! two independent nodes, "central" and "edge", each a [`FhirServer`] on its own SurrealDB
//! memory engine and its own port. Prints one JSON line with both FHIR base URLs, then serves
//! until killed. Development and CI only: there is no authentication.

use std::io::Write;
use std::sync::Arc;

use axum::Router;
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::FhirServer;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use serde_json::{Value, json};
use tokio::net::TcpListener;

type Failure = Box<dyn std::error::Error + Send + Sync>;

/// The search parameters the contract suite uses.
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

/// One node: a fresh memory database behind a listener on an ephemeral local port.
async fn node(name: &str) -> Result<(TcpListener, Router, String), Failure> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}/fhir", listener.local_addr()?);
    let settings = Settings {
        target: Target::Memory,
        namespace: "contract".into(),
        database: name.into(),
    };
    let connection = Connection::open(&settings).await?;
    migrate(&connection, MIGRATIONS).await?;
    let store = SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry()?);
    let router = Router::new().nest(
        "/fhir",
        FhirServer::new(store, base.as_str(), Arc::new(Unrestricted)).router(),
    );
    Ok((listener, router, base))
}

#[tokio::main]
async fn main() -> Result<(), Failure> {
    let (central, central_router, central_base) = node("central").await?;
    let (edge, edge_router, edge_base) = node("edge").await?;
    let line = json!({ "central": central_base, "edge": edge_base }).to_string() + "\n";
    let mut stdout = std::io::stdout();
    stdout.write_all(line.as_bytes())?;
    stdout.flush()?;
    tokio::try_join!(
        axum::serve(central, central_router),
        axum::serve(edge, edge_router)
    )?;
    Ok(())
}
