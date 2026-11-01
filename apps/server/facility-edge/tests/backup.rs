//! Backups through the built binary: the running node's scheduled backup and test restore,
//! then on-demand backups, a USB copy, verification and a restore into a fresh store
//! (backup-and-recovery.md; testing.md § recovery).

#![cfg(unix)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use common::{STARTUP, edge, free_port, terminate, wait_exit, wait_ready};

struct Setup {
    store: PathBuf,
    backups: PathBuf,
    key_file: PathBuf,
}

impl Setup {
    fn new(root: &Path) -> Self {
        let key_file = root.join("backup.key");
        fs::write(&key_file, "5a".repeat(32)).expect("key");
        Self {
            store: root.join("store"),
            backups: root.join("backups"),
            key_file,
        }
    }

    fn env(&self) -> Vec<(&'static str, String)> {
        vec![
            ("HEALTH_BACKUP_DIR", self.backups.display().to_string()),
            (
                "HEALTH_BACKUP_KEY_FILE",
                self.key_file.display().to_string(),
            ),
            ("HEALTH_BACKUP_INTERVAL_SECS", "3600".into()),
        ]
    }

    /// `facility-edge backup args…` against `store`.
    fn backup(&self, store: &Path, args: &[&str]) -> Output {
        Command::new(common::BINARY)
            .arg("backup")
            .args(args)
            .env_clear()
            .env("HEALTH_DATA_DIR", store)
            .envs(self.env())
            .output()
            .expect("backup")
    }

    fn status(&self) -> serde_json::Value {
        fs::read(self.backups.join("status.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn the_node_backs_up_and_test_restores_then_an_admin_restores_from_usb() {
    let dir = tempfile::tempdir().expect("dir");
    let setup = Setup::new(dir.path());
    let port = free_port();
    let mut node = edge(&setup.store, port, &[], &setup.env());
    wait_ready(port);
    // The first backup is due at once, and the first test restore follows it.
    let started = Instant::now();
    while setup.status()["last_test"].is_null() {
        assert!(started.elapsed() < STARTUP, "no backup in time");
        std::thread::sleep(Duration::from_millis(200));
    }
    let status = setup.status();
    assert!(status["last_backup"]["backup"].is_string(), "{status}");
    assert!(status["last_test"]["error"].is_null(), "{status}");

    // The node owns its store, so a backup on demand needs it stopped.
    let refused = setup.backup(&setup.store, &["now"]);
    assert_eq!(refused.status.code(), Some(9), "{}", text(&refused));
    terminate(&node);
    assert_eq!(wait_exit(&mut node).code(), Some(0));
    let now = setup.backup(&setup.store, &["now"]);
    assert!(now.status.success(), "{}", text(&now));

    let verified = setup.backup(&setup.store, &["verify"]);
    assert!(verified.status.success(), "{}", text(&verified));
    assert!(text(&verified).contains("2 backups verified"));

    let usb = dir.path().join("usb");
    let copied = setup.backup(&setup.store, &["copy", usb.to_str().expect("utf-8")]);
    assert!(copied.status.success(), "{}", text(&copied));
    let copy = fs::read_dir(&usb)
        .expect("usb")
        .map(|e| e.expect("entry").path())
        .find(|p| p.join("manifest.json").is_file())
        .expect("a copy");

    // A fresh store from the USB copy: the restored node starts and is ready.
    let fresh = dir.path().join("fresh");
    let restored = setup.backup(&fresh, &["restore", copy.to_str().expect("utf-8")]);
    assert!(restored.status.success(), "{}", text(&restored));
    let again = setup.backup(&fresh, &["restore", copy.to_str().expect("utf-8")]);
    assert_eq!(again.status.code(), Some(9), "never restored over a store");
    let port = free_port();
    let mut node = edge(&fresh, port, &[], &[]);
    wait_ready(port);
    terminate(&node);
    assert_eq!(wait_exit(&mut node).code(), Some(0));
}

#[test]
fn a_corrupt_backup_fails_its_test_and_verification() {
    let dir = tempfile::tempdir().expect("dir");
    let setup = Setup::new(dir.path());
    let now = setup.backup(&setup.store, &["now"]);
    assert!(now.status.success(), "{}", text(&now));
    let backup = fs::read_dir(&setup.backups)
        .expect("backups")
        .map(|e| e.expect("entry").path())
        .find(|p| p.join("manifest.json").is_file())
        .expect("a backup");
    let export = backup.join("export.surql.enc");
    let mut bytes = fs::read(&export).expect("export");
    bytes[30] ^= 0xff;
    fs::write(&export, bytes).expect("corrupt");

    let tested = setup.backup(&setup.store, &["test"]);
    assert_eq!(tested.status.code(), Some(9), "{}", text(&tested));
    assert!(setup.status()["last_test"]["error"].is_string());
    let verified = setup.backup(&setup.store, &["verify"]);
    assert_eq!(verified.status.code(), Some(9), "{}", text(&verified));
    assert!(text(&verified).contains("FAILED"));
}

#[test]
fn backups_without_their_settings_are_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let output = Command::new(common::BINARY)
        .args(["backup", "now"])
        .env_clear()
        .env("HEALTH_DATA_DIR", dir.path().join("store"))
        .output()
        .expect("backup");
    assert_eq!(output.status.code(), Some(9));
    assert!(text(&output).contains("HEALTH_BACKUP_DIR"));
    // The key file is required alongside the directory, before anything runs.
    let output = Command::new(common::BINARY)
        .args(["backup", "now"])
        .env_clear()
        .env("HEALTH_DATA_DIR", dir.path().join("store"))
        .env("HEALTH_BACKUP_DIR", dir.path().join("backups"))
        .output()
        .expect("backup");
    assert_eq!(output.status.code(), Some(2));
}
