//! `central-server backup …` (#138; ADR 0046; ops/runbooks/central-dr.md): central's backups on
//! demand. The running server takes the hourly ones and the weekly test restore itself.
//!
//! - `backup key <path>`: write a new random backup key to `path` (owner-only), refusing a path
//!   that exists. The key is then split for its custodians (the runbook).
//! - `backup now`: take a backup. The server may keep running: the export is one snapshot.
//! - `backup copy <dir>`: copy the newest backup and its attachments to `dir` (the offsite
//!   in-country copy) and verify the copy.
//! - `backup verify`: check every backup's checksums, decryption and attachments.
//! - `backup test`: test-restore the newest backup into the scratch database, as weekly.
//! - `backup restore <backup> --into <database>`: restore `backup` into a new database on the
//!   server, and its attachments into `HEALTH_FILES_DIR`; then point `HEALTH_DATABASE` at it and
//!   start. A database that holds any table is refused.
//! - `backup status`: print the scheduler's status and the backups kept.
//!
//! All but `key` need `HEALTH_BACKUP_DIR` and `HEALTH_BACKUP_KEY_FILE`.

use std::io::Write as _;
use std::path::Path;

use health_backup::node::{self, Backups, Retention};
use health_surrealdb::{Connection, MIGRATIONS, Settings, Target, migrate};

use crate::config::{Config, SCRATCH_DATABASE};

const USAGE: &str = "usage: central-server backup (key <path> | now | copy <dir> | verify | test \
                     | restore <backup> --into <database> | status)";

/// Runs `args` (after `backup`).
pub(crate) async fn run(
    config: &Config,
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    if let [cmd, path] = args
        && cmd == "key"
    {
        node::write_new_key(Path::new(path))
            .map_err(|e| format!("cannot write a new key to {path}: {e}"))?;
        return say(&format!("wrote a new backup key to {path}"));
    }
    let files = config
        .backup
        .as_ref()
        .ok_or("HEALTH_BACKUP_DIR is not set")?;
    let mut backups = Backups::new(files.dir.clone(), "central")
        .with_retention(Retention::CENTRAL)
        .with_scratch_database(settings(config, SCRATCH_DATABASE)?);
    let blobs = match config.auth.as_ref().and_then(|a| a.files_dir.as_ref()) {
        Some(dir) => Some(health_files::FsBlobStore::open(dir)?),
        None => None,
    };
    if let Some(blobs) = &blobs {
        backups = backups.with_blobs(blobs.clone());
    }
    if let [cmd] = args
        && cmd == "status"
    {
        let list: Vec<_> = backups
            .list()?
            .iter()
            .map(|entry| entry.created.to_string())
            .collect();
        let text = serde_json::json!({ "status": backups.status(), "backups": list });
        return say(&text.to_string());
    }
    let key = node::read_key(&files.key_file)?;
    let now = health_time::Timestamp::now();
    match args {
        [cmd] if cmd == "now" => {
            let db = Connection::open(&settings(config, &config.database)?)
                .await
                .map_err(|_| "the database is unavailable")?;
            migrate(&db, MIGRATIONS)
                .await
                .map_err(|_| "the database could not be migrated by this binary")?;
            let dir = backups.take(&db, &key, now).await?;
            say(&format!("backed up to {}", dir.display()))
        }
        [cmd, target] if cmd == "copy" => {
            let dir = backups.copy_newest(Path::new(target), &key)?;
            say(&format!("copied and verified {}", dir.display()))
        }
        [cmd] if cmd == "verify" => {
            let list = backups.list()?;
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
                n => Err(format!("{n} of {} backups failed verification", list.len()).into()),
            }
        }
        [cmd] if cmd == "test" => {
            let tested = backups.test(&key, now).await?;
            say(&format!("test restore of {tested} passed"))
        }
        [cmd, from, flag, into] if cmd == "restore" && flag == "--into" => {
            if into == &config.database {
                return Err(
                    "restore into a new database, not HEALTH_DATABASE; then point HEALTH_DATABASE at it"
                        .into(),
                );
            }
            if into == SCRATCH_DATABASE {
                return Err(
                    format!("{SCRATCH_DATABASE} is the test restores' scratch database").into(),
                );
            }
            let manifest = node::restore_into_database(
                Path::new(from),
                &key,
                &settings(config, into)?,
                blobs.as_ref(),
            )
            .await?;
            say(&format!(
                "restored the backup taken {} into {into}; set HEALTH_DATABASE={into} and start central",
                manifest.created_at
            ))
        }
        _ => Err(USAGE.into()),
    }
}

/// The server connection for `database`.
fn settings(config: &Config, database: &str) -> Result<Settings, Box<dyn std::error::Error>> {
    let password = std::fs::read_to_string(&config.surrealdb_password_file)
        .map_err(|_| "cannot read HEALTH_SURREALDB_PASSWORD_FILE")?
        .trim()
        .to_owned();
    Ok(Settings {
        target: Target::Remote {
            endpoint: config.surrealdb_endpoint.clone(),
            username: config.surrealdb_user.clone(),
            password,
        },
        namespace: config.tenant.clone(),
        database: database.to_owned(),
    })
}

fn say(line: &str) -> Result<(), Box<dyn std::error::Error>> {
    writeln!(std::io::stdout(), "{line}")?;
    Ok(())
}
