//! The migrations runner on the in-memory engine: ordering, idempotence, modified and unknown
//! migrations, rollback of a failed migration, and the schema snapshot.

use health_surrealdb::{
    Connection, MIGRATIONS, Migration, MigrationError, Settings, Target, migrate,
};

async fn fresh() -> Connection {
    let settings = Settings {
        target: Target::Memory,
        namespace: "test".into(),
        database: "main".into(),
    };
    Connection::open(&settings)
        .await
        .expect("open in-memory database")
}

const TABLE_A: Migration = Migration {
    version: 1,
    name: "0001_table_a",
    sql: "DEFINE TABLE table_a SCHEMAFULL; DEFINE FIELD label ON table_a TYPE string;",
};

const TABLE_B: Migration = Migration {
    version: 2,
    name: "0002_table_b",
    sql: "DEFINE TABLE table_b SCHEMAFULL;",
};

/// The schema as sorted `DEFINE` statements: tables, then each table's fields and indexes.
async fn schema(connection: &Connection) -> String {
    let mut lines = Vec::new();
    let mut db = connection
        .query("RETURN object::entries((INFO FOR DB).tables)")
        .await
        .expect("info for db");
    let mut tables: Vec<(String, String)> = db.take(0).expect("tables");
    tables.sort();
    for (table, definition) in tables {
        lines.push(definition);
        let mut info = connection
            .query(&format!("RETURN object::values((INFO FOR TABLE {table}).fields).concat(object::values((INFO FOR TABLE {table}).indexes))"))
            .await
            .expect("info for table");
        let mut definitions: Vec<String> = info.take(0).expect("definitions");
        definitions.sort();
        lines.extend(definitions.into_iter().map(|d| format!("  {d}")));
    }
    lines.join("\n") + "\n"
}

#[tokio::test]
async fn applies_pending_migrations_once_in_order() {
    let db = fresh().await;
    let first = migrate(&db, &[TABLE_A, TABLE_B]).await.expect("migrate");
    assert_eq!((first.already_applied, first.applied), (vec![], vec![1, 2]));
    let second = migrate(&db, &[TABLE_A, TABLE_B]).await.expect("rerun");
    assert_eq!(
        (second.already_applied, second.applied),
        (vec![1, 2], vec![])
    );
    let mut count = db
        .query("RETURN count(SELECT * FROM _migration)")
        .await
        .expect("count");
    assert_eq!(count.take::<Option<i64>>(0).expect("count"), Some(2));
}

#[tokio::test]
async fn a_modified_applied_migration_fails_startup() {
    let db = fresh().await;
    migrate(&db, &[TABLE_A]).await.expect("migrate");
    let edited = Migration {
        sql: "DEFINE TABLE table_a SCHEMALESS;",
        ..TABLE_A
    };
    let error = migrate(&db, &[edited]).await.expect_err("must refuse");
    assert!(
        matches!(error, MigrationError::Modified { version: 1, .. }),
        "{error}"
    );
}

#[tokio::test]
async fn an_unknown_applied_migration_fails_startup() {
    let db = fresh().await;
    migrate(&db, &[TABLE_A, TABLE_B]).await.expect("migrate");
    let error = migrate(&db, &[TABLE_A])
        .await
        .expect_err("binary is older than the data");
    assert!(
        matches!(error, MigrationError::Unknown { version: 2, .. }),
        "{error}"
    );
}

#[tokio::test]
async fn out_of_order_lists_are_refused() {
    let db = fresh().await;
    let error = migrate(&db, &[TABLE_B, TABLE_A])
        .await
        .expect_err("out of order");
    assert!(matches!(error, MigrationError::OutOfOrder(1)), "{error}");
}

#[tokio::test]
async fn a_failing_migration_applies_nothing() {
    let db = fresh().await;
    let broken = Migration {
        version: 3,
        name: "0003_broken",
        sql: "DEFINE TABLE table_c SCHEMAFULL; THROW 'boom';",
    };
    let error = migrate(&db, &[TABLE_A, broken]).await.expect_err("fails");
    assert!(
        matches!(error, MigrationError::Failed { version: 3, .. }),
        "{error}"
    );
    let tables = schema(&db).await;
    assert!(tables.contains("table_a"), "{tables}");
    assert!(
        !tables.contains("table_c"),
        "the failed migration rolled back: {tables}"
    );
    let report = migrate(&db, &[TABLE_A])
        .await
        .expect("ledger has only the good one");
    assert_eq!(report.already_applied, vec![1]);
}

#[test]
fn checksums_are_sha256_of_the_surrealql() {
    let m = Migration {
        version: 9,
        name: "n",
        sql: "",
    };
    assert_eq!(
        m.checksum(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

/// Applies the real migrations to a fresh database and compares the schema with the committed
/// snapshot, so a schema change without a migration (or a migration without its snapshot
/// update) fails CI. `UPDATE_SCHEMA_SNAPSHOT=1` rewrites the snapshot.
#[tokio::test]
async fn schema_matches_the_committed_snapshot() {
    let db = fresh().await;
    migrate(&db, MIGRATIONS).await.expect("migrate");
    let actual = schema(&db).await;
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/schema.snapshot"
    );
    if std::env::var_os("UPDATE_SCHEMA_SNAPSHOT").is_some() {
        std::fs::write(path, &actual).expect("write snapshot");
    }
    let expected =
        std::fs::read_to_string(path).expect("snapshot exists; run with UPDATE_SCHEMA_SNAPSHOT=1");
    assert_eq!(
        actual, expected,
        "schema differs from tests/fixtures/schema.snapshot"
    );
}

#[tokio::test]
async fn the_snapshot_comparison_detects_a_missing_migration() {
    let with = fresh().await;
    migrate(&with, &[TABLE_A]).await.expect("migrate");
    let without = fresh().await;
    migrate(&without, &[]).await.expect("migrate");
    assert_ne!(schema(&with).await, schema(&without).await);
}

#[test]
fn every_migration_file_is_listed() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/migrations");
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .expect("migrations dir")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with(".surql"))
        .map(|name| name.trim_end_matches(".surql").to_owned())
        .collect();
    files.sort();
    let listed: Vec<String> = MIGRATIONS.iter().map(|m| m.name.to_owned()).collect();
    assert_eq!(
        files, listed,
        "migrations/*.surql and MIGRATIONS must match"
    );
}
