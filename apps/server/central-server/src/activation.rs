//! Central's package activation (#89; configuration.md § Distribution and activation on nodes):
//! every minute, central activates the national set its activation records give it now,
//! verifying each package it published against the national root (`HEALTH_NATIONAL_ROOT_FILE`)
//! from its blob store (`HEALTH_FILES_DIR`). When the set changes, and once at startup, the
//! active packages' definitions are written into the store, where clients read them and from
//! where they sync down to every node.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use health_activation::definitions::write_definitions;
use health_activation::{Activator, Outcome, activate};
use health_configuration::activation::{ActiveSet, Chain};
use health_files::FsBlobStore;
use health_persistence::Store as _;
use health_surrealdb::{Connection, SurrealStore};
use health_time::{SystemClock, Timestamp};

/// How often central looks for a new set.
const EVERY: Duration = Duration::from_secs(60);

/// The active set and its definitions, once.
async fn once(
    store: &SurrealStore,
    blobs: &FsBlobStore,
    activator: &Activator,
    first: bool,
) -> Result<(), String> {
    let report = activate(
        store,
        Some(blobs),
        activator,
        Timestamp::now(),
        |digest| async move {
            Err(format!(
                "{digest} is not in central's blob store: publish the package again"
            ))
        },
    )
    .await?;
    let switched = report
        .as_ref()
        .is_some_and(|r| r.outcome == Outcome::Activated);
    if !(switched || first) {
        return Ok(());
    }
    let mut tx = store.begin().await.map_err(|e| e.to_string())?;
    let active: ActiveSet = match tx.active_packages().await.map_err(|e| e.to_string())? {
        Some(json) => serde_json::from_str(&json).map_err(|e| e.to_string())?,
        None => ActiveSet::new(),
    };
    drop(tx);
    let written = write_definitions(store, blobs, &active).await?;
    if written > 0 {
        tracing::info!(written, "definitions of the active packages written");
    }
    Ok(())
}

/// Activates the national set every minute, until central stops.
pub(crate) async fn run(connection: Connection, root: PathBuf, files: PathBuf) {
    // Indexed with central's search registry, like every write of the FHIR server.
    let store = SurrealStore::new(connection, Arc::new(SystemClock))
        .with_registry(crate::auth::search_registry());
    let started = (|| {
        let activator = Activator::from_root_file(&root, Chain::default())?;
        let blobs = FsBlobStore::open(&files)
            .map_err(|e| format!("the blob store {}: {e}", files.display()))?;
        Ok::<_, String>((activator, blobs))
    })();
    let (activator, blobs) = match started {
        Ok(started) => started,
        Err(error) => {
            tracing::warn!(%error, "packages are not activated on central");
            return;
        }
    };
    let mut first = true;
    loop {
        if let Err(error) = once(&store, &blobs, &activator, first).await {
            tracing::warn!(%error, "activating the national packages failed");
        } else {
            first = false;
        }
        tokio::time::sleep(EVERY).await;
    }
}
