//! The embedded and remote engines, behind the same `Connection` and migrations runner.
//! `cargo test -p health-surrealdb --features embedded,remote`; the remote test also needs
//! `HEALTH_TEST_SURREALDB_WS=ws://127.0.0.1:<port>` pointing at `surreal start --user root --pass root`.

#![cfg(any(feature = "embedded", feature = "remote"))]

use health_surrealdb::{Connection, Migration, Settings, Target, migrate};

const TABLE_A: Migration = Migration {
    version: 1,
    name: "0001_table_a",
    sql: "DEFINE TABLE table_a SCHEMAFULL; DEFINE FIELD label ON table_a TYPE string;",
};

async fn round_trip(target: Target) {
    let settings = Settings {
        target,
        namespace: "test".into(),
        database: "main".into(),
    };
    {
        let db = Connection::open(&settings).await.expect("open");
        migrate(&db, &[TABLE_A]).await.expect("migrate");
        db.query("CREATE table_a:one SET label = 'kept'")
            .await
            .expect("write");
    }
    // SDK 3.3.5 has no awaitable close: an embedded datastore shuts down in the background after
    // its last handle drops, so a same-process reopen waits for the RocksDB lock to be released.
    let mut attempts = 0;
    let db = loop {
        match Connection::open(&settings).await {
            Ok(db) => break db,
            Err(error) if attempts < 50 => {
                attempts += 1;
                eprintln!("reopen attempt {attempts}: {error}");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            Err(error) => panic!("reopen: {error}"),
        }
    };
    let report = migrate(&db, &[TABLE_A]).await.expect("migrate again");
    assert_eq!(report.already_applied, vec![1]);
    let mut result = db.query("RETURN table_a:one.label").await.expect("read");
    assert_eq!(
        result.take::<Option<String>>(0).expect("label").as_deref(),
        Some("kept")
    );
}

#[cfg(feature = "embedded")]
#[tokio::test]
async fn embedded_rocksdb_persists_across_reopen_with_sync_every() {
    let dir = std::env::temp_dir().join(format!("health-surrealdb-{}", std::process::id()));
    let target = Target::RocksDb { path: dir.clone() };
    assert!(target.address().ends_with("?sync=every"));
    round_trip(target).await;
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(feature = "remote")]
#[tokio::test]
async fn remote_server_runs_the_same_migrations() {
    let Some(endpoint) = std::env::var("HEALTH_TEST_SURREALDB_WS").ok() else {
        eprintln!("skipped: set HEALTH_TEST_SURREALDB_WS");
        return;
    };
    let namespace = format!("t{}", std::process::id());
    let settings_target = Target::Remote {
        endpoint,
        username: "root".into(),
        password: "root".into(),
    };
    let settings = Settings {
        target: settings_target.clone(),
        namespace,
        database: "main".into(),
    };
    let db = Connection::open(&settings).await.expect("open remote");
    let first = migrate(&db, &[TABLE_A]).await.expect("migrate");
    assert_eq!(first.applied, vec![1]);
    let again = migrate(&db, &[TABLE_A]).await.expect("idempotent");
    assert_eq!(again.already_applied, vec![1]);
}
