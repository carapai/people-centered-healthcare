//! The export → import round trip on representative data (backup-and-recovery.md: CI runs it),
//! and corrupted backups detected before anything is restored.

use std::path::Path;
use std::sync::Arc;

use health_backup::{BackupError, BackupKey, create, restore, verify};
use health_fhir_search::Registry;
use health_files::FsBlobStore;
use health_identifiers::{OpId, ResourceId};
use health_persistence::{DerivedKeys, Precondition, RawJson, Store, Transaction, WriteRequest};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use serde_json::{Value, json};

struct ByFamily;

impl DerivedKeys for ByFamily {
    fn keys(&self, resource_type: &str, resource: &Value) -> Vec<(String, String)> {
        match (resource_type, resource["name"][0]["family"].as_str()) {
            ("Patient", Some(family)) => vec![("family".into(), family.to_uppercase())],
            _ => Vec::new(),
        }
    }
}

async fn connection(database: &str) -> Connection {
    let settings = Settings {
        target: Target::Memory,
        namespace: "backup".into(),
        database: database.into(),
    };
    Connection::open(&settings).await.expect("open")
}

fn key(byte: u8) -> BackupKey {
    BackupKey::from_bytes([byte; 32])
}

/// A database with every kind of record a node keeps: versions and history, a tombstone,
/// search and reference index entries, blocking rows and counts, the migration ledger, the
/// outbox's tables, the inbound queue and the registry review queue.
async fn representative() -> Connection {
    let db = connection("source").await;
    migrate(&db, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(db.clone(), Arc::new(SystemClock))
        .with_registry(Registry::new(jiff::tz::Offset::UTC))
        .with_blocking(Arc::new(ByFamily));
    let write = |id: ResourceId, json: Value, precondition: Precondition| {
        let store = store.clone();
        async move {
            let mut tx = store.begin().await.expect("tx");
            let resource_type = json["resourceType"]
                .as_str()
                .expect("type")
                .parse()
                .expect("type");
            let version = tx
                .write(WriteRequest {
                    resource_type,
                    id,
                    json: RawJson::from_validated(json.to_string()),
                    op_id: OpId::new(),
                    precondition,
                })
                .await
                .expect("write");
            tx.commit().await.expect("commit");
            version.version_id
        }
    };
    let (patient, gone) = (ResourceId::new(), ResourceId::new());
    let v1 = write(
        patient,
        json!({ "resourceType": "Patient", "name": [{ "family": "Okello" }], "birthDate": "1990" }),
        Precondition::MustNotExist,
    )
    .await;
    write(patient, json!({ "resourceType": "Patient", "name": [{ "family": "Okello", "given": ["Grace"] }], "birthDate": "1990" }), Precondition::IfMatch(v1)).await;
    write(ResourceId::new(), serde_json::from_str(&format!(r#"{{"resourceType":"Observation","status":"final","code":{{"text":"Hb"}},"subject":{{"reference":"Patient/{patient}"}},"valueQuantity":{{"value":12.30,"unit":"g/dL"}}}}"#)).expect("json"), Precondition::MustNotExist).await;
    write(
        gone,
        json!({ "resourceType": "Patient", "name": [{ "family": "Akello" }] }),
        Precondition::MustNotExist,
    )
    .await;
    let mut tx = store.begin().await.expect("tx");
    tx.delete(&"Patient".parse().expect("type"), gone, OpId::new(), None)
        .await
        .expect("delete");
    tx.commit().await.expect("commit");
    store.recount_blocks().await.expect("recount");
    db.query(
        "CREATE inbound:a CONTENT { kind: 'dead_letter', integration: 'lab', received_at: d'2026-01-01T00:00:00Z', \
         retain_until: d'2026-04-01T00:00:00Z', payload: '{not json', issues: ['structure'], status: 'pending', \
         resolved_at: NONE, note: NONE }",
    )
    .await
    .expect("inbound");
    db
}

/// Every table's rows, in id order, as text.
async fn contents(db: &Connection) -> Vec<(String, String)> {
    let mut info = db
        .query("RETURN object::keys((INFO FOR DB).tables)")
        .await
        .expect("info");
    let mut tables: Vec<String> = info.take(0).expect("tables");
    tables.sort();
    let mut out = Vec::new();
    for table in tables {
        let mut rows = db
            .query(&format!(
                "RETURN <string> (SELECT * FROM type::table('{table}') ORDER BY id)"
            ))
            .await
            .expect("rows");
        let text: Option<String> = rows.take(0).expect("text");
        out.push((table, text.unwrap_or_default()));
    }
    out
}

fn flip(path: &Path, at: usize) {
    let mut bytes = std::fs::read(path).expect("read");
    let i = at.min(bytes.len() - 1);
    bytes[i] ^= 0x40;
    std::fs::write(path, bytes).expect("write");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_round_trip_preserves_every_record_and_blob() {
    let source = representative().await;
    let dir = tempfile::tempdir().expect("tmp");
    let blob_dir = tempfile::tempdir().expect("tmp");
    let blobs = FsBlobStore::open(blob_dir.path()).expect("blobs");
    let scan = blobs
        .put(&b"%PDF-1.7 a consent scan"[..])
        .expect("put")
        .blob;
    blobs.put(&b"a signature image"[..]).expect("put");

    let backup = create(
        &source,
        Some(&blobs),
        dir.path(),
        "KAM013-E1",
        &key(7),
        Timestamp::now(),
    )
    .await
    .expect("backup");
    let manifest = verify(&backup, &key(7)).expect("verify");
    assert_eq!(manifest.blobs.len(), 2);
    assert_eq!(manifest.node, "KAM013-E1");
    // Never stored in the clear.
    let sealed = std::fs::read(backup.join("export.surql.enc")).expect("export");
    assert!(!String::from_utf8_lossy(&sealed).contains("Okello"));

    let target = connection("restored").await;
    let restored_blobs_dir = tempfile::tempdir().expect("tmp");
    let restored_blobs = FsBlobStore::open(restored_blobs_dir.path()).expect("blobs");
    restore(&backup, &key(7), &target, Some(&restored_blobs))
        .await
        .expect("restore");

    let (before, after) = (contents(&source).await, contents(&target).await);
    assert!(before.len() > 20, "{} tables", before.len());
    assert_eq!(before, after, "every table, row for row");
    // The decimal kept its text, and the restored node is fully migrated.
    assert!(after.iter().any(|(_, rows)| rows.contains("12.30")));
    assert!(
        migrate(&target, MIGRATIONS)
            .await
            .expect("migrate")
            .applied
            .is_empty()
    );
    assert_eq!(
        restored_blobs.read(&scan.hash).expect("blob"),
        b"%PDF-1.7 a consent scan"
    );

    // A second backup copies no blob again.
    let before_files = std::fs::read_dir(dir.path().join("blobs"))
        .expect("dir")
        .count();
    create(
        &source,
        Some(&blobs),
        dir.path(),
        "KAM013-E1",
        &key(7),
        Timestamp::now(),
    )
    .await
    .expect("second");
    assert_eq!(
        std::fs::read_dir(dir.path().join("blobs"))
            .expect("dir")
            .count(),
        before_files
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_schema_has_no_set_of_records() {
    // backup-and-recovery.md: set<record> values do not survive export and import (SurrealDB
    // #7621), so no field may use them.
    let db = representative().await;
    let mut info = db
        .query("RETURN object::keys((INFO FOR DB).tables)")
        .await
        .expect("info");
    let tables: Vec<String> = info.take(0).expect("tables");
    for table in tables {
        let mut fields = db
            .query(&format!("RETURN <string> (INFO FOR TABLE {table}).fields"))
            .await
            .expect("fields");
        let text: Option<String> = fields.take(0).expect("text");
        assert!(
            !text.unwrap_or_default().contains("set<record"),
            "{table} has a set<record> field"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_corrupted_backup_is_detected_and_never_restored() {
    let source = representative().await;
    let dir = tempfile::tempdir().expect("tmp");
    let blob_dir = tempfile::tempdir().expect("tmp");
    let blobs = FsBlobStore::open(blob_dir.path()).expect("blobs");
    let blob = blobs.put(&b"a scan"[..]).expect("put").blob;
    let backup = create(
        &source,
        Some(&blobs),
        dir.path(),
        "E1",
        &key(1),
        Timestamp::now(),
    )
    .await
    .expect("backup");

    // The wrong key.
    assert!(matches!(
        verify(&backup, &key(2)),
        Err(BackupError::Corrupt(_))
    ));

    // A flipped byte in the export.
    let export = backup.join("export.surql.enc");
    let pristine = std::fs::read(&export).expect("read");
    flip(&export, 5_000);
    assert!(matches!(
        verify(&backup, &key(1)),
        Err(BackupError::Corrupt(_))
    ));
    let target = connection("never").await;
    assert!(matches!(
        restore(&backup, &key(1), &target, None).await,
        Err(BackupError::Corrupt(_))
    ));
    assert!(contents(&target).await.is_empty(), "nothing was restored");
    std::fs::write(&export, pristine).expect("repair");
    assert!(verify(&backup, &key(1)).is_ok());

    // A damaged or swapped blob.
    let blob_file = dir.path().join("blobs").join(format!("{}.enc", blob.hash));
    flip(&blob_file, 30);
    assert!(matches!(
        verify(&backup, &key(1)),
        Err(BackupError::Corrupt(_))
    ));
    let other = blobs.put(&b"another scan"[..]).expect("put").blob;
    let second = create(
        &source,
        Some(&blobs),
        dir.path(),
        "E1",
        &key(1),
        Timestamp::now(),
    )
    .await;
    assert!(
        second.is_err(),
        "the damaged blob is caught when the next backup verifies"
    );
    std::fs::copy(
        dir.path().join("blobs").join(format!("{}.enc", other.hash)),
        &blob_file,
    )
    .expect("swap");
    assert!(
        matches!(verify(&backup, &key(1)), Err(BackupError::Corrupt(_))),
        "a swapped blob fails its binding"
    );

    // A tampered manifest.
    std::fs::write(backup.join("manifest.json"), b"{}").expect("tamper");
    assert!(matches!(
        verify(&backup, &key(1)),
        Err(BackupError::Corrupt(_))
    ));
}
