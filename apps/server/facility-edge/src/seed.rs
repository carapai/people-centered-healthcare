//! `facility-edge seed import <file>` (#83; synchronization.md § Seeding): applies a seed central
//! wrote with `central-server seed export`, with `<file>.jws` beside it, to a stopped edge. The
//! signature must be the publication signer's under the national root installed at provisioning
//! (`<pki dir>/root.pem`), and the seed must be for this edge's facility.

use std::io::Write as _;
use std::path::Path;

use health_edge_runtime::EdgeConfig;
use health_pki::Trust;
use health_surrealdb::{MIGRATIONS, SurrealStore, migrate};
use health_time::{SystemClock, Timestamp};

const USAGE: &str = "usage: facility-edge seed import <file>";

/// Runs `args` (after `seed`); the error is printed and becomes exit code 12.
pub(crate) fn run(config: &EdgeConfig, args: &[String]) -> Result<(), String> {
    let [cmd, file] = args else {
        return Err(USAGE.into());
    };
    if cmd != "import" {
        return Err(USAGE.into());
    }
    let facility = config
        .facility
        .as_deref()
        .ok_or("this edge has no facility: provision it first")?;
    let root = config.pki_dir.join("root.pem");
    let trust = std::fs::read_to_string(&root)
        .map_err(|e| format!("{}: {e}: provision the edge first", root.display()))
        .and_then(|pem| Trust::new(&pem).map_err(|e| e.to_string()))?;
    let path = Path::new(file);
    let signature = path.with_file_name(format!(
        "{}.jws",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("seed")
    ));
    let jws =
        std::fs::read_to_string(&signature).map_err(|e| format!("{}: {e}", signature.display()))?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(health_surrealdb::THREAD_STACK)
        .build()
        .map_err(|_| "cannot start the async runtime")?;
    runtime.block_on(async {
        let db = health_edge_runtime::backup::open_store(
            &config.data_dir,
            &config.tenant,
            &config.database,
        )
        .await
        .map_err(|_| "cannot open the store: stop the service first")?;
        migrate(&db, MIGRATIONS)
            .await
            .map_err(|_| "the store cannot be migrated by this binary")?;
        let store = SurrealStore::new(db, std::sync::Arc::new(SystemClock));
        let imported = health_edge_runtime::seed::import(
            &store,
            path,
            jws.trim(),
            &trust,
            facility,
            Timestamp::now(),
        )
        .await?;
        writeln!(
            std::io::stdout(),
            "seeded {} heads from central {}; sync resumes after {}",
            imported.applied,
            imported.central,
            imported.seq
        )
        .map_err(|e| e.to_string())
    })
}
