//! The terminology a node serves (#843; terminology.md § Runtime service): the active packages'
//! CodeSystems, ValueSets (with their pinned expansions) and ConceptMaps, read from their
//! tarballs, so `ValueSet/$expand` answers a form's bound questions offline, as central's does.
//!
//! [`spawn_terminology`] looks every [`INTERVAL`] for a new active set and serves its terminology
//! from then on; a set whose terminology does not load keeps the previous one.

use std::time::Duration;

use health_configuration::activation::ActiveSet;
use health_fhir_server::ActiveTerminology;
use health_files::FsBlobStore;
use health_persistence::Store as _;
use health_surrealdb::SurrealStore;
use health_terminology::Terminology;

/// How often a node looks for a new active set.
pub const INTERVAL: Duration = Duration::from_secs(30);

/// The terminology of `packages` (each `name@version` and its files), or why it does not load.
pub fn load(packages: &[(String, health_packages::Files)]) -> Result<Terminology, String> {
    let mut terminology = Terminology::new();
    for (id, files) in packages {
        health_packages::terminology::load(&mut terminology, files)
            .map_err(|e| format!("{id}: its terminology: {e}"))?;
    }
    Ok(terminology)
}

/// Serves the active packages' terminology (their tarballs in `blobs`) through `served` when the
/// active set differs from `last`, which becomes the set read. Whether it changed.
pub async fn reload(
    served: &ActiveTerminology,
    store: &SurrealStore,
    blobs: &FsBlobStore,
    last: &mut Option<String>,
) -> Result<bool, String> {
    let json = {
        let mut tx = store.begin().await.map_err(|e| e.to_string())?;
        tx.active_packages().await.map_err(|e| e.to_string())?
    };
    if json == *last {
        return Ok(false);
    }
    let active: ActiveSet = match &json {
        Some(json) => serde_json::from_str(json).map_err(|e| e.to_string())?,
        None => ActiveSet::new(),
    };
    let packages = health_activation::definitions::active_files(store, blobs, &active).await?;
    served.replace(load(&packages)?);
    *last = json;
    Ok(true)
}

/// Keeps `served` the active packages' terminology (their tarballs in `blobs`) until the process
/// ends.
pub fn spawn_terminology(served: ActiveTerminology, store: SurrealStore, blobs: FsBlobStore) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(INTERVAL);
        let mut last = None;
        loop {
            interval.tick().await;
            match reload(&served, &store, &blobs, &mut last).await {
                Ok(true) => tracing::info!("serving the active packages' terminology"),
                Ok(false) => {}
                Err(error) => tracing::error!(
                    alert = "package-activation-failed",
                    %error,
                    "the previous terminology stays"
                ),
            }
        }
    });
}
