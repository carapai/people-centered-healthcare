//! Signed updates through the built binary, installed in a temporary directory: staging, a
//! committed update, a failed migration rolled back by the service manager's restarts, and a
//! broken binary rolled back by the previous one (facility-edge.md § Updates and migrations).

#![cfg(unix)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

use common::{BINARY, STARTUP, edge_at, free_port, terminate, wait_exit, wait_ready};
use health_edge_runtime::update;
use health_surrealdb::{Connection, MIGRATIONS, Migration, Settings, Target, migrate};
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};

/// An edge installed under `root`: its binary, store, update directory and pinned key.
struct Edge {
    binary: PathBuf,
    store: PathBuf,
    updates: PathBuf,
    key_file: PathBuf,
    key: SigningKey,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn install(root: &Path) -> Edge {
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let binary = bin.join("facility-edge");
    fs::copy(BINARY, &binary).expect("install");
    let key = SigningKey::from_slice(&[9_u8; 32]).expect("key");
    let key_file = root.join("release.pub");
    fs::write(
        &key_file,
        hex(key.verifying_key().to_encoded_point(true).as_bytes()),
    )
    .expect("key file");
    Edge {
        binary,
        store: root.join("store"),
        updates: root.join("updates"),
        key_file,
        key,
    }
}

impl Edge {
    /// `program update args…`, with this edge's environment.
    fn update(&self, program: &Path, args: &[&str]) -> Output {
        Command::new(program)
            .arg("update")
            .args(args)
            .env_clear()
            .env("HEALTH_DATA_DIR", &self.store)
            .env("HEALTH_UPDATE_KEY_FILE", &self.key_file)
            .output()
            .expect("update")
    }

    /// Writes a package of `binary` at `version` and its signature, signed by `key`.
    fn package(&self, binary: &[u8], version: &str, key: &SigningKey) -> (PathBuf, PathBuf) {
        let package = update::pack(binary, "facility-edge", version).expect("pack");
        let signature: Signature = key.sign(&package);
        let dir = self.updates.with_file_name("downloads");
        fs::create_dir_all(&dir).expect("downloads");
        let (p, s) = (dir.join("edge.tar.gz"), dir.join("edge.sig"));
        fs::write(&p, package).expect("package");
        fs::write(&s, hex(&signature.to_bytes())).expect("signature");
        (p, s)
    }

    fn stage_and_apply(&self, binary: &[u8], version: &str) {
        let (package, signature) = self.package(binary, version, &self.key);
        let staged = self.update(&self.binary, &["stage", path(&package), path(&signature)]);
        assert!(staged.status.success(), "{staged:?}");
        let applied = self.update(&self.binary, &["apply"]);
        assert!(applied.status.success(), "{applied:?}");
    }

    /// Starts the installed binary until it reports ready, then stops it.
    fn run_until_ready(&self) {
        let port = free_port();
        let mut node = edge_at(&self.binary, &self.store, port, &[], &[]);
        wait_ready(port);
        terminate(&node);
        assert_eq!(wait_exit(&mut node).code(), Some(0));
    }

    /// Starts the installed binary and returns its exit code without waiting for ready.
    fn start(&self) -> Option<i32> {
        let mut node = edge_at(&self.binary, &self.store, free_port(), &[], &[]);
        wait_exit(&mut node).code()
    }

    fn journal(&self) -> String {
        let status = self.update(&self.binary, &["status"]);
        String::from_utf8_lossy(&status.stdout).into_owned()
    }
}

fn path(p: &Path) -> &str {
    p.to_str().expect("utf-8 path")
}

#[test]
fn a_signed_update_is_staged_applied_and_committed_once_ready() {
    let dir = tempfile::tempdir().expect("dir");
    let edge = install(dir.path());
    edge.run_until_ready();
    let new = fs::read(BINARY).expect("binary");

    // A package signed by any other key is refused (exit 7), and nothing is staged.
    let stranger = SigningKey::from_slice(&[3_u8; 32]).expect("key");
    let (package, signature) = edge.package(&new, "9.9.9", &stranger);
    let refused = edge.update(&edge.binary, &["stage", path(&package), path(&signature)]);
    assert_eq!(refused.status.code(), Some(7));
    assert!(
        edge.journal().contains("\"staged\":null"),
        "{}",
        edge.journal()
    );

    edge.stage_and_apply(&new, "9.9.9");
    assert!(edge.journal().contains("\"state\":\"pending\""));
    edge.run_until_ready();
    let journal = edge.journal();
    assert!(journal.contains("\"state\":\"committed\""), "{journal}");
    assert!(journal.contains("\"to\":\"9.9.9\""), "{journal}");
    assert!(edge.updates.join("pre-update-store").is_dir());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_migration_is_rolled_back_by_the_next_start() {
    let dir = tempfile::tempdir().expect("dir");
    let edge = install(dir.path());
    // The store has a migration this release does not know, so the new binary's migrations
    // fail (the same refusal as a broken migration).
    {
        let connection = Connection::open(&Settings {
            target: Target::RocksDb {
                path: edge.store.clone(),
            },
            namespace: "ug".into(),
            database: "main".into(),
        })
        .await
        .expect("open");
        let mut newer = MIGRATIONS.to_vec();
        newer.push(Migration {
            version: 9999,
            name: "9999_unknown",
            sql: "DEFINE TABLE unknown SCHEMALESS;",
        });
        migrate(&connection, &newer).await.expect("migrate");
    }
    edge.stage_and_apply(&fs::read(BINARY).expect("binary"), "9.9.9");

    // Wait out the embedded store's background close (a locked store exits 3).
    let started = Instant::now();
    let first = loop {
        match edge.start() {
            Some(3) if started.elapsed() < STARTUP => {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            code => break code,
        }
    };
    assert_eq!(first, Some(4), "the migration fails");
    // The service manager's restart rolls back before opening the store.
    assert_eq!(edge.start(), Some(6), "rolled back");
    let journal = edge.journal();
    assert!(journal.contains("\"state\":\"rolled-back\""), "{journal}");
    assert!(edge.updates.join("failed-store").is_dir());
    assert!(edge.store.is_dir());
}

#[test]
fn a_broken_binary_is_rolled_back_by_the_previous_one() {
    let dir = tempfile::tempdir().expect("dir");
    let edge = install(dir.path());
    edge.run_until_ready();
    let original = fs::read(&edge.binary).expect("binary");

    // A release that crashes before it is ready.
    edge.stage_and_apply(b"#!/bin/sh\nexit 1\n", "9.9.9");
    assert_eq!(edge.start(), Some(1));
    // The service manager runs the previous binary's rollback (OnFailure=).
    let previous = edge.updates.join("previous/facility-edge");
    let rolled = edge.update(&previous, &["rollback", path(&edge.binary)]);
    assert!(rolled.status.success(), "{rolled:?}");
    assert_eq!(fs::read(&edge.binary).expect("binary"), original);
    assert!(edge.journal().contains("\"state\":\"rolled-back\""));
    edge.run_until_ready();
}
