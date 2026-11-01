//! `facility-edge backup …`: backups on demand (`health_edge_runtime::backup`). The running
//! node takes the hourly ones itself.
//!
//! - `backup now`: with the service stopped (the node owns its store), take a backup.
//! - `backup copy <dir>`: copy the newest backup to `dir` (a USB drive) and verify the copy.
//!   The node may keep running.
//! - `backup verify`: check every backup's checksums, decryption and blobs.
//! - `backup test`: test-restore the newest backup into a scratch store, as the weekly test does.
//! - `backup restore <backup>`: with the service stopped and the data directory moved aside,
//!   restore `backup` into a new store.
//! - `backup status`: print the scheduler's status and the backups kept.

use std::io::Write;
use std::path::Path;

use health_edge_runtime::EdgeConfig;
use health_edge_runtime::backup::{self, Backups};
use health_surrealdb::{MIGRATIONS, migrate};

const USAGE: &str =
    "usage: facility-edge backup (now | copy <dir> | verify | test | restore <backup> | status)";

/// Runs `args` (after `backup`); the error is printed and becomes exit code 9.
pub(crate) fn run(config: &EdgeConfig, args: &[String]) -> Result<(), String> {
    let root = config
        .backup_dir
        .as_ref()
        .ok_or("HEALTH_BACKUP_DIR is not set")?;
    let backups = Backups::new(root, &config.node_id);
    if let [cmd] = args
        && cmd == "status"
    {
        let list: Vec<_> = backups
            .list()
            .map_err(|e| e.to_string())?
            .iter()
            .map(|entry| entry.created.to_string())
            .collect();
        let text = serde_json::json!({ "status": backups.status(), "backups": list });
        return say(&text.to_string());
    }
    let key = config
        .backup_key_file
        .as_deref()
        .ok_or("HEALTH_BACKUP_KEY_FILE is not set")
        .and_then(|path| backup::read_key(path).map_err(|_| "the backup key file is unusable"))?;
    let now = health_time::Timestamp::now();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|_| "cannot start the async runtime")?;
    let fail = |e: health_backup::BackupError| e.to_string();
    match args {
        [cmd] if cmd == "now" => runtime.block_on(async {
            let db = backup::open_store(&config.data_dir, &config.tenant, &config.database)
                .await
                .map_err(|_| "cannot open the store: stop the service first")?;
            migrate(&db, MIGRATIONS)
                .await
                .map_err(|_| "the store cannot be migrated by this binary")?;
            let dir = backups.take(&db, &key, now).await.map_err(fail)?;
            say(&format!("backed up to {}", dir.display()))
        }),
        [cmd, target] if cmd == "copy" => {
            let dir = backups.copy_newest(Path::new(target), &key).map_err(fail)?;
            say(&format!("copied and verified {}", dir.display()))
        }
        [cmd] if cmd == "verify" => {
            let list = backups.list().map_err(|e| e.to_string())?;
            let mut bad = 0;
            for entry in &list {
                match health_backup::verify(&entry.dir, &key) {
                    Ok(_) => say(&format!("ok {}", entry.created))?,
                    Err(error) => {
                        bad += 1;
                        say(&format!("FAILED {}: {error}", entry.created))?;
                    }
                }
            }
            match bad {
                0 => say(&format!("{} backups verified", list.len())),
                n => Err(format!("{n} of {} backups failed verification", list.len())),
            }
        }
        [cmd] if cmd == "test" => runtime.block_on(async {
            let tested = backups.test(&key, now).await.map_err(fail)?;
            say(&format!("test restore of {tested} passed"))
        }),
        [cmd, from] if cmd == "restore" => runtime.block_on(async {
            let manifest = backup::restore_into(
                Path::new(from),
                &key,
                &config.data_dir,
                &config.tenant,
                &config.database,
            )
            .await
            .map_err(fail)?;
            say(&format!(
                "restored the backup taken {}; start the service",
                manifest.created_at
            ))
        }),
        _ => Err(USAGE.into()),
    }
}

fn say(line: &str) -> Result<(), String> {
    writeln!(std::io::stdout(), "{line}").map_err(|_| "cannot write to stdout".into())
}
