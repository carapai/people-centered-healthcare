//! Workflows on a node (#100; workflow-engine.md): the edge and the standalone desktop run the
//! engine over their own store, at `/api/workflow`, with the Workflow Definitions of their active
//! packages, as central does. A desktop under an edge runs it on its replicas while the edge
//! cannot be reached (§ Offline and sync).
//!
//! [`spawn_upkeep`] runs every [`SCHEDULER_INTERVAL`](health_workflow_app::SCHEDULER_INTERVAL):
//! - **Definitions:** when the active package set has changed, the active packages' Workflow
//!   Definitions are read and run from then on; on any problem the previous ones keep running.
//! - **Timers** due on the instances this node decides are fired.
//! - **Candidates** other nodes sent are judged, opening a Conflict Record for each that does not
//!   apply.

use std::time::Duration;

use health_configuration::activation::ActiveSet;
use health_fhir_server::WorkflowConfig;
use health_files::FsBlobStore;
use health_persistence::Store as _;
use health_surrealdb::SurrealStore;

/// The active packages' Workflow Definitions, run by `config` from now on, when the active set
/// differs from `last`; `last` becomes the set read.
async fn reload(
    config: &WorkflowConfig,
    store: &SurrealStore,
    blobs: &FsBlobStore,
    last: &mut Option<String>,
) -> Result<Option<usize>, String> {
    let json = {
        let mut tx = store.begin().await.map_err(|e| e.to_string())?;
        tx.active_packages().await.map_err(|e| e.to_string())?
    };
    if json == *last {
        return Ok(None);
    }
    let active: ActiveSet = match &json {
        Some(json) => serde_json::from_str(json).map_err(|e| e.to_string())?,
        None => ActiveSet::new(),
    };
    let packages = health_activation::definitions::active_files(store, blobs, &active).await?;
    let mut definitions = Vec::new();
    for (id, files) in &packages {
        let (found, problems) = health_publication::workflow_definitions(files);
        if let Some(problem) = problems.first() {
            return Err(format!("{id}: {problem}"));
        }
        definitions.extend(found.into_iter().map(|(_, definition)| definition));
    }
    let running = config
        .replace(definitions)
        .map_err(|e| format!("the active Workflow Definitions cannot run: {e}"))?;
    *last = json;
    Ok(Some(running))
}

/// Runs the node's workflow upkeep over `store` until the process ends: the active packages'
/// definitions (their tarballs in `blobs`, when the node keeps packages), due timers, and the
/// candidates other nodes sent.
pub fn spawn_upkeep(config: WorkflowConfig, store: SurrealStore, blobs: Option<FsBlobStore>) {
    let every = Duration::from_secs(
        u64::try_from(health_workflow_app::SCHEDULER_INTERVAL.as_secs()).unwrap_or(30),
    );
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(every);
        let mut last = None;
        loop {
            interval.tick().await;
            if let Some(blobs) = &blobs {
                match reload(&config, &store, blobs, &mut last).await {
                    Ok(Some(running)) => {
                        tracing::info!(running, "running the active packages' workflows");
                    }
                    Ok(None) => {}
                    Err(error) => tracing::error!(
                        alert = "package-activation-failed",
                        %error,
                        "the previous workflows keep running"
                    ),
                }
            }
            match config.upkeep(&store).await {
                Ok(done) if done.conflicts > 0 => tracing::warn!(
                    alert = "workflow-conflicts",
                    fired = done.fired,
                    applied = done.applied,
                    conflicts = done.conflicts,
                    "workflow candidates opened Conflict Records"
                ),
                Ok(done) if done.fired + done.applied > 0 => tracing::info!(
                    fired = done.fired,
                    applied = done.applied,
                    "workflow upkeep"
                ),
                Ok(_) => {}
                Err(error) => tracing::warn!(%error, "workflow upkeep failed; it runs again"),
            }
        }
    });
}
