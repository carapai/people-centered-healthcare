use health_surrealdb::{MIGRATIONS, migrate};

use super::*;

fn key() -> BackupKey {
    BackupKey::from_bytes([7; 32])
}

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

async fn store(dir: &Path) -> Connection {
    let db = open_store(&dir.join("data"), "ug", "main")
        .await
        .expect("a store");
    migrate(&db, MIGRATIONS).await.expect("migrated");
    db
}

/// A finished backup's manifest, without an export: enough for listing and pruning.
fn fake(root: &Path, created: Timestamp, blobs: &[&str]) -> PathBuf {
    let dir = root.join(created.strftime("%Y%m%dT%H%M%S%.3fZ").to_string());
    fs::create_dir_all(&dir).expect("dir");
    let manifest = serde_json::json!({
        "format": 1, "node": "n", "created_at": created.to_string(),
        "export": { "file": "export.surql.enc", "bytes": 0, "plaintext_sha256": "", "file_sha256": "" },
        "blobs": blobs,
    });
    fs::write(dir.join("manifest.json"), manifest.to_string()).expect("manifest");
    dir
}

#[tokio::test]
async fn a_backup_is_verified_recorded_and_test_restored() {
    let temp = tempfile::tempdir().expect("temp");
    let db = store(temp.path()).await;
    let backups = Backups::new(temp.path().join("backups"), "edge-1");
    let now = at("2026-11-01T10:00:00Z");
    let dir = backups.take(&db, &key(), now).await.expect("a backup");
    health_backup::verify(&dir, &key()).expect("it verifies");
    let status = backups.status();
    assert_eq!(status.last_attempt, Some(now));
    assert_eq!(
        status.last_backup.expect("recorded").backup,
        file_name(&dir)
    );

    let tested = backups
        .test(&key(), now)
        .await
        .expect("the test restore passes");
    assert_eq!(tested, file_name(&dir));
    let test = backups.status().last_test.expect("recorded");
    assert_eq!((test.backup, test.error), (Some(file_name(&dir)), None));
    // The scratch store is gone, and was never taken for a backup.
    assert_eq!(backups.list().expect("list").len(), 1);
    let left: Vec<_> = fs::read_dir(backups.root())
        .expect("root")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with('.'))
        .collect();
    assert!(left.is_empty(), "{left:?}");
}

#[tokio::test]
async fn a_failed_test_restore_is_recorded_for_the_alert() {
    let temp = tempfile::tempdir().expect("temp");
    let db = store(temp.path()).await;
    let backups = Backups::new(temp.path().join("backups"), "edge-1");
    let now = at("2026-11-01T10:00:00Z");
    let dir = backups.take(&db, &key(), now).await.expect("a backup");
    // The drive corrupts the export after it was written and verified.
    let export = dir.join("export.surql.enc");
    let mut bytes = fs::read(&export).expect("export");
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    fs::write(&export, bytes).expect("corrupt");

    assert!(backups.test(&key(), now).await.is_err());
    let test = backups.status().last_test.expect("recorded");
    assert_eq!(test.backup, Some(file_name(&dir)));
    assert!(test.error.expect("an error").contains("verification"));

    // A wrong key fails the same way.
    let other = BackupKey::from_bytes([8; 32]);
    let dir = backups
        .take(&db, &key(), at("2026-11-01T11:00:00Z"))
        .await
        .expect("a backup");
    assert!(backups.test(&other, now).await.is_err());
    assert!(health_backup::verify(&dir, &key()).is_ok());
}

#[tokio::test]
async fn a_round_without_its_key_records_a_failure_and_waits_an_interval() {
    let temp = tempfile::tempdir().expect("temp");
    let db = store(temp.path()).await;
    let schedule = Schedule {
        backups: Backups::new(temp.path().join("backups"), "edge-1"),
        key_file: temp.path().join("missing.key"),
        every: Duration::from_secs(3600),
        test_every: Duration::from_secs(604_800),
        data_dir: temp.path().join("data"),
    };
    fs::create_dir_all(schedule.backups.root()).expect("root");
    let now = at("2026-11-01T10:00:00Z");
    schedule.round(&db, now).await;
    let status = schedule.backups.status();
    assert!(status.last_backup.is_none());
    let failure = status.last_failure.expect("recorded");
    assert_eq!(failure.at, now);
    assert!(!failure.error.contains("missing.key"));
    assert_eq!(
        due_in(
            status.last_attempt,
            at("2026-11-01T10:20:00Z"),
            schedule.every
        ),
        Duration::from_secs(40 * 60)
    );

    // With its key, a round backs up and runs the first test restore.
    fs::write(&schedule.key_file, "07".repeat(32)).expect("key");
    schedule.round(&db, at("2026-11-01T11:00:00Z")).await;
    let status = schedule.backups.status();
    assert!(status.last_backup.is_some());
    assert_eq!(status.last_test.expect("tested").error, None);
}

#[test]
fn due_in_waits_out_the_interval_from_the_last_attempt() {
    let every = Duration::from_secs(3600);
    let now = at("2026-11-01T10:00:00Z");
    assert_eq!(due_in(None, now, every), Duration::ZERO);
    assert_eq!(
        due_in(Some(at("2026-11-01T09:45:00Z")), now, every),
        Duration::from_secs(45 * 60)
    );
    assert_eq!(
        due_in(Some(at("2026-11-01T08:00:00Z")), now, every),
        Duration::ZERO
    );
    // A clock set back never waits longer than one interval.
    assert_eq!(due_in(Some(at("2026-11-02T00:00:00Z")), now, every), every);
}

#[test]
fn pruning_keeps_retention_and_the_blobs_it_needs() {
    let temp = tempfile::tempdir().expect("temp");
    let root = temp.path();
    let backups = Backups::new(root, "n");
    let now = at("2026-11-01T12:00:00Z");
    let old = fake(root, at("2026-10-01T12:00:00Z"), &["aa", "bb"]);
    let recent: Vec<_> = (0..50)
        .map(|h| {
            let created = Timestamp::from_second(now.as_second() - h * 3600).expect("ts");
            fake(root, created, &["aa"])
        })
        .collect();
    let unfinished = root.join("20261101T115959.000Z");
    fs::create_dir_all(&unfinished).expect("unfinished");
    fs::create_dir_all(root.join("blobs")).expect("blobs");
    for blob in ["aa.enc", "bb.enc", "cc.enc.partial"] {
        fs::write(root.join("blobs").join(blob), b"x").expect("blob");
    }
    fs::write(root.join("status.json"), b"{}").expect("status");

    let pruned = backups.prune(now).expect("pruned");
    // The 48 newest stay. The two after them share a day with the 48th, so they go, as do the
    // month-old backup and the unfinished one.
    assert_eq!(
        pruned,
        Pruned {
            backups: 4,
            blobs: 2
        }
    );
    assert!(!old.exists() && !unfinished.exists());
    assert!(recent[..48].iter().all(|dir| dir.exists()));
    assert_eq!(backups.list().expect("list").len(), 48);
    assert!(root.join("blobs/aa.enc").exists());
    assert!(!root.join("blobs/bb.enc").exists());
    assert!(root.join("status.json").exists());
}

#[tokio::test]
async fn the_newest_backup_copies_to_a_drive_and_restores_into_an_empty_store() {
    let temp = tempfile::tempdir().expect("temp");
    let db = store(temp.path()).await;
    db.query("CREATE probe:1 SET note = 'kept'")
        .await
        .expect("a record");
    let backups = Backups::new(temp.path().join("backups"), "edge-1");
    backups
        .take(&db, &key(), at("2026-11-01T10:00:00Z"))
        .await
        .expect("a backup");
    let usb = temp.path().join("usb");
    let copy = backups.copy_newest(&usb, &key()).expect("copied");
    assert!(copy.starts_with(&usb));

    let restored = temp.path().join("restored");
    restore_into(&copy, &key(), &restored, "ug", "main")
        .await
        .expect("restored");
    let again = restore_into(&copy, &key(), &restored, "ug", "main").await;
    assert!(again.is_err(), "a store with data is never restored over");
}

#[test]
fn a_backup_key_file_is_64_hex_characters() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("backup.key");
    fs::write(&path, format!("{}\n", "ab".repeat(32))).expect("key");
    assert!(read_key(&path).is_ok());
    fs::write(&path, "ab".repeat(31)).expect("short");
    assert!(read_key(&path).is_err());
    assert!(read_key(&temp.path().join("none")).is_err());
}
