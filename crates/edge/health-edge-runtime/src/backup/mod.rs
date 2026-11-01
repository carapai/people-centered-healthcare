//! Scheduled backups (backup-and-recovery.md, #78), built on `health-backup`.
//!
//! - **Hourly** while the node runs: an encrypted export, verified after writing
//!   ([`Backups::take`]). The first one is due one interval after the last attempt, so a restart
//!   neither skips nor repeats one.
//! - **Retention:** the 48 newest, and the newest of each of the last 14 days. Pruning removes
//!   the rest, unfinished backups, and blobs no kept backup refers to.
//! - **Weekly test restore** of the newest backup into a scratch store beside it
//!   ([`Backups::test`]).
//! - **Status and alerts:** every outcome is written to `status.json` ([`Status`]). A failed
//!   backup or test restore is logged with an `alert` field, and the status page reports it.
//! - **Second volume:** the node warns when the backup directory shares the data volume.
//! - **On demand:** `facility-edge backup …` takes one while the node is stopped, copies the
//!   newest to a USB drive, verifies, and restores.
//!
//! The pre-update backup is the cold copy of the store `update` takes (facility-edge.md
//! § Updates and migrations). The key's escrow at central is #138.

mod retention;
mod status;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use health_backup::{BackupError, BackupKey, Manifest};
use health_files::FsBlobStore;
use health_surrealdb::{Connection, Settings, Target};
use health_time::Timestamp;

pub use status::{Done, Failed, Status, Test};

/// A backup under the root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Its directory.
    pub dir: PathBuf,
    /// When it was taken.
    pub created: Timestamp,
    /// The blob hashes it refers to.
    pub blobs: Vec<String>,
}

/// What a prune removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pruned {
    /// Backups past retention, and unfinished ones.
    pub backups: usize,
    /// Blobs no kept backup refers to.
    pub blobs: usize,
}

/// A backup key file that cannot be used. Never names the key.
#[derive(Debug, thiserror::Error)]
#[error("the backup key file is unreadable, or not 64 hex characters")]
pub struct KeyError;

/// The node's backup key, from its file: 64 hex characters.
pub fn read_key(path: &Path) -> Result<BackupKey, KeyError> {
    let text = fs::read_to_string(path).map_err(|_| KeyError)?;
    let bytes = crate::update::hex(text.trim()).ok_or(KeyError)?;
    let bytes: [u8; 32] = bytes.try_into().map_err(|_| KeyError)?;
    Ok(BackupKey::from_bytes(bytes))
}

/// The backups under one root, on the second volume.
#[derive(Debug, Clone)]
pub struct Backups {
    root: PathBuf,
    node: String,
}

impl Backups {
    /// The backups under `root`, taken as `node`.
    pub fn new(root: impl Into<PathBuf>, node: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            node: node.into(),
        }
    }

    /// The root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// What the scheduler last did.
    pub fn status(&self) -> Status {
        Status::read(&self.root)
    }

    /// The finished backups, newest first. A directory without a readable manifest is
    /// unfinished, and is left out.
    pub fn list(&self) -> std::io::Result<Vec<Entry>> {
        let mut entries: Vec<Entry> = self
            .candidates()?
            .into_iter()
            .filter_map(|dir| {
                let manifest = read_manifest(&dir)?;
                let created = manifest.created_at.parse().ok()?;
                Some(Entry {
                    dir,
                    created,
                    blobs: manifest.blobs,
                })
            })
            .collect();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.created));
        Ok(entries)
    }

    /// The backup directories, finished or not: those named by their time.
    fn candidates(&self) -> std::io::Result<Vec<PathBuf>> {
        let mut dirs = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let named = entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(|c: char| c.is_ascii_digit()));
            if named && entry.file_type()?.is_dir() {
                dirs.push(entry.path());
            }
        }
        Ok(dirs)
    }

    /// Takes a backup of `db`, verifies it, records it in the status and prunes. A failure is
    /// recorded and raised as an alert.
    pub async fn take(
        &self,
        db: &Connection,
        key: &BackupKey,
        now: Timestamp,
    ) -> Result<PathBuf, BackupError> {
        fs::create_dir_all(&self.root)?;
        match health_backup::create(db, None, &self.root, &self.node, key, now).await {
            Ok(dir) => {
                let backup = file_name(&dir);
                tracing::info!(outcome = "backed-up", backup = %backup, "backup taken");
                self.update(|status| {
                    status.last_attempt = Some(now);
                    status.last_backup = Some(Done { at: now, backup });
                });
                match self.prune(now) {
                    Ok(pruned) => tracing::info!(
                        backups = pruned.backups,
                        blobs = pruned.blobs,
                        "backups pruned"
                    ),
                    Err(error) => tracing::warn!(error.kind = %error, "backups not pruned"),
                }
                Ok(dir)
            }
            Err(error) => {
                self.failed(now, &error);
                Err(error)
            }
        }
    }

    /// Records a failed backup and raises its alert.
    pub fn failed(&self, now: Timestamp, error: &dyn std::fmt::Display) {
        tracing::error!(alert = "backup-failed", error.kind = %error, "backup failed");
        let error = error.to_string();
        self.update(|status| {
            status.last_attempt = Some(now);
            status.last_failure = Some(Failed { at: now, error });
        });
    }

    /// Removes backups past retention and unfinished ones, then blobs no kept backup refers
    /// to.
    pub fn prune(&self, now: Timestamp) -> std::io::Result<Pruned> {
        let entries = self.list()?;
        let created: Vec<_> = entries.iter().map(|entry| entry.created).collect();
        let keep = retention::keep(&created, now);
        let mut kept = std::collections::BTreeSet::new();
        let mut pruned = Pruned::default();
        for (entry, keep) in entries.iter().zip(keep) {
            if keep {
                kept.insert(entry.dir.clone());
            } else {
                fs::remove_dir_all(&entry.dir)?;
                pruned.backups += 1;
            }
        }
        for dir in self.candidates()? {
            if !kept.contains(&dir) && read_manifest(&dir).is_none() {
                fs::remove_dir_all(&dir)?;
                pruned.backups += 1;
            }
        }
        let referenced: std::collections::BTreeSet<_> = entries
            .iter()
            .filter(|entry| kept.contains(&entry.dir))
            .flat_map(|entry| entry.blobs.iter().map(|hash| format!("{hash}.enc")))
            .collect();
        if let Ok(blobs) = fs::read_dir(self.root.join("blobs")) {
            for blob in blobs {
                let blob = blob?;
                let name = blob.file_name().to_string_lossy().into_owned();
                if !referenced.contains(&name) {
                    fs::remove_file(blob.path())?;
                    pruned.blobs += 1;
                }
            }
        }
        Ok(pruned)
    }

    /// Restores the newest backup into a scratch store beside it, checks that the store holds
    /// its migration ledger, and records the outcome. A failure raises an alert.
    pub async fn test(&self, key: &BackupKey, now: Timestamp) -> Result<String, BackupError> {
        let newest = self.list()?.into_iter().next();
        let outcome = match &newest {
            Some(entry) => self.restore_scratch(entry, key).await,
            None => Err(BackupError::Corrupt("no backup to test".into())),
        };
        let backup = newest.as_ref().map(|entry| file_name(&entry.dir));
        match &outcome {
            Ok(()) => {
                tracing::info!(outcome = "test-restored", backup = ?backup, "test restore passed")
            }
            Err(error) => tracing::error!(
                alert = "backup-test-failed",
                error.kind = %error,
                backup = ?backup,
                "test restore failed"
            ),
        }
        let error = outcome.as_ref().err().map(ToString::to_string);
        self.update(|status| {
            status.last_test = Some(Test {
                at: now,
                backup: backup.clone(),
                error,
            });
        });
        outcome.map(|()| backup.unwrap_or_default())
    }

    async fn restore_scratch(&self, entry: &Entry, key: &BackupKey) -> Result<(), BackupError> {
        // Scratch stores start with a dot, so they are never taken for backups; one left by a
        // crash is removed here.
        for old in fs::read_dir(&self.root)? {
            let old = old?;
            if old
                .file_name()
                .to_string_lossy()
                .starts_with(".restore-test-")
            {
                let _ = fs::remove_dir_all(old.path());
            }
        }
        let scratch = tempfile::Builder::new()
            .prefix(".restore-test-")
            .tempdir_in(&self.root)?;
        let outcome = Self::check_restore(&scratch.path().join("store"), entry, key).await;
        remove_scratch(scratch.keep()).await;
        outcome
    }

    async fn check_restore(
        store: &Path,
        entry: &Entry,
        key: &BackupKey,
    ) -> Result<(), BackupError> {
        let db = open_store(store, "restore", "test").await?;
        let blobs = FsBlobStore::open(store.with_file_name("files"))?;
        health_backup::restore(&entry.dir, key, &db, Some(&blobs)).await?;
        let migrations: Option<i64> = db
            .query("RETURN count(SELECT * FROM _migration)")
            .await
            .and_then(|mut response| response.take(0))
            .map_err(|e| BackupError::Database(e.to_string()))?;
        if migrations.unwrap_or(0) < 1 {
            return Err(BackupError::Corrupt(
                "the restored store has no migration ledger".into(),
            ));
        }
        Ok(())
    }

    /// Copies the newest backup and its blobs to `target` (a USB drive, say), then verifies the
    /// copy. Returns the copy's directory.
    pub fn copy_newest(&self, target: &Path, key: &BackupKey) -> Result<PathBuf, BackupError> {
        let newest = self
            .list()?
            .into_iter()
            .next()
            .ok_or_else(|| BackupError::Corrupt("no backup to copy".into()))?;
        let dir = target.join(file_name(&newest.dir));
        fs::create_dir_all(&dir)?;
        fs::create_dir_all(target.join("blobs"))?;
        // The layout health-backup writes: the backup's own files, and blobs shared under the
        // root as `blobs/{hash}.enc`.
        for file in fs::read_dir(&newest.dir)? {
            let file = file?;
            copy_durably(&file.path(), &dir.join(file.file_name()))?;
        }
        for hash in &newest.blobs {
            let name = format!("{hash}.enc");
            let to = target.join("blobs").join(&name);
            if !to.exists() {
                copy_durably(&self.root.join("blobs").join(&name), &to)?;
            }
        }
        health_backup::verify(&dir, key)?;
        Ok(dir)
    }

    fn update(&self, change: impl FnOnce(&mut Status)) {
        let mut status = Status::read(&self.root);
        change(&mut status);
        if let Err(error) = status.write(&self.root) {
            tracing::warn!(error.kind = %error, "backup status not written");
        }
    }
}

/// The hourly backups and the weekly test restore, as one supervised task.
#[derive(Debug, Clone)]
pub(crate) struct Schedule {
    pub(crate) backups: Backups,
    pub(crate) key_file: PathBuf,
    pub(crate) every: Duration,
    pub(crate) test_every: Duration,
    pub(crate) data_dir: PathBuf,
}

impl Schedule {
    /// Runs until the node stops. A failure is recorded and alerted, never returned: a full
    /// backup drive must not stop clinical work.
    pub(crate) async fn run(self, db: Connection) -> Result<(), String> {
        if let Err(error) = fs::create_dir_all(self.backups.root()) {
            tracing::error!(alert = "backup-failed", error.kind = %error, "no backup directory");
        }
        if same_volume(&self.data_dir, self.backups.root()) == Some(true) {
            tracing::warn!(
                alert = "backup-on-data-volume",
                "backups share the data volume; give them a second volume"
            );
        }
        loop {
            let status = self.backups.status();
            tokio::time::sleep(due_in(status.last_attempt, Timestamp::now(), self.every)).await;
            self.round(&db, Timestamp::now()).await;
        }
    }

    /// One backup, and the test restore when one is due.
    pub(crate) async fn round(&self, db: &Connection, now: Timestamp) {
        let key = match read_key(&self.key_file) {
            Ok(key) => key,
            Err(error) => return self.backups.failed(now, &error),
        };
        if self.backups.take(db, &key, now).await.is_err() {
            return;
        }
        let tested = self.backups.status().last_test.map(|test| test.at);
        if due_in(tested, now, self.test_every).is_zero() {
            let _ = self.backups.test(&key, now).await;
        }
    }
}

/// How long until one `every` after `last` (at once if never, or overdue).
fn due_in(last: Option<Timestamp>, now: Timestamp, every: Duration) -> Duration {
    let Some(last) = last else {
        return Duration::ZERO;
    };
    let elapsed = Duration::try_from(now.duration_since(last)).unwrap_or(Duration::ZERO);
    every.saturating_sub(elapsed)
}

/// Opens (creating) an embedded store at `path`. Fails while another process holds it.
pub async fn open_store(
    path: &Path,
    namespace: &str,
    database: &str,
) -> Result<Connection, BackupError> {
    Connection::open(&Settings {
        target: Target::RocksDb {
            path: path.to_path_buf(),
        },
        namespace: namespace.to_owned(),
        database: database.to_owned(),
    })
    .await
    .map_err(|e| BackupError::Database(e.to_string()))
}

/// Restores `backup` into a new store at `data_dir`, which must not exist yet or be empty:
/// the runbook's "stop the node, restore the latest verified backup, start".
pub async fn restore_into(
    backup: &Path,
    key: &BackupKey,
    data_dir: &Path,
    namespace: &str,
    database: &str,
) -> Result<Manifest, BackupError> {
    if fs::read_dir(data_dir).is_ok_and(|mut entries| entries.next().is_some()) {
        return Err(BackupError::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "the data directory is not empty; move it aside first",
        )));
    }
    let db = open_store(data_dir, namespace, database).await?;
    health_backup::restore(backup, key, &db, None).await
}

/// Removes a scratch store once its engine has let go of it: on Windows the store's files stay
/// open for a moment after the connection is dropped. A store still held after that is left for
/// the next test restore to remove.
async fn remove_scratch(dir: PathBuf) {
    for _ in 0..50 {
        if fs::remove_dir_all(&dir).is_ok() || !dir.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tracing::warn!(scratch = %dir.display(), "test restore scratch store not removed");
}

fn read_manifest(dir: &Path) -> Option<Manifest> {
    serde_json::from_slice(&fs::read(dir.join("manifest.json")).ok()?).ok()
}

fn file_name(dir: &Path) -> String {
    dir.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn copy_durably(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::copy(from, to)?;
    // Windows flushes a file only through a handle that may write it.
    fs::OpenOptions::new().write(true).open(to)?.sync_all()
}

/// Whether two directories are on one volume; `None` when that cannot be told.
fn same_volume(a: &Path, b: &Path) -> Option<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        Some(fs::metadata(a).ok()?.dev() == fs::metadata(b).ok()?.dev())
    }
    #[cfg(windows)]
    {
        let volume = |p: &Path| {
            fs::canonicalize(p)
                .ok()?
                .components()
                .next()
                .map(|c| c.as_os_str().to_ascii_lowercase())
        };
        Some(volume(a)? == volume(b)?)
    }
}

#[cfg(test)]
mod tests;
