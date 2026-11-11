//! Seeding from removable media (#83): central writes a facility's snapshot to a file and signs
//! it; an edge with no link applies it, for its own facility only, and syncs on from its cursor.

use std::sync::Arc;

use health_identifiers::{OpId, ResourceId};
use health_packages::{Digest, LocalSigner, SEED_TYP, TYP, sign_detached};
use health_persistence::{Precondition, RawJson, Store as _, Transaction as _, WriteRequest};
use health_pki::{Authority, Trust, new_key, request};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use serde_json::json;

async fn store(name: &str) -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: name.into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

async fn observations(store: &SurrealStore, facility: &str, count: usize) -> Vec<ResourceId> {
    let mut ids = Vec::new();
    let mut tx = store.begin().await.expect("begin");
    for n in 0..count {
        let id = ResourceId::new();
        let body = json!({
            "resourceType": "Observation", "status": "final", "valueInteger": n,
            "meta": {"tag": [{"system": "urn:health:facility", "code": facility}]}
        });
        tx.write(WriteRequest {
            resource_type: "Observation".parse().expect("type"),
            id,
            json: RawJson::from_validated(body.to_string()),
            op_id: OpId::new(),
            precondition: Precondition::MustNotExist,
        })
        .await
        .expect("write");
        ids.push(id);
    }
    tx.commit().await.expect("commit");
    ids
}

async fn held(store: &SurrealStore, id: ResourceId) -> bool {
    let mut tx = store.begin().await.expect("begin");
    tx.read(&"Observation".parse().expect("type"), id)
        .await
        .expect("read")
        .is_some()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_signed_seed_brings_its_facility_s_records_and_nothing_else() {
    // The national root and central's publication signer.
    let now = Timestamp::now();
    let root = Authority::root("Uganda Health Root", now).expect("root");
    let key = new_key().expect("key");
    let issued = root
        .issue_publication(
            &request(&key, "MoH publication signing").expect("csr"),
            "MoH publication signing",
            now,
        )
        .expect("publication certificate");
    let signer = LocalSigner::from_pem(&key.serialize_pem(), issued.pem()).expect("signer");
    let trust = Trust::new(&root.certificate_pem()).expect("trust");

    // Central holds two facilities' records: more than one of the edge's batches of its own.
    let central = store("central").await;
    let ours = observations(&central, "KLA01", 250).await;
    let theirs = observations(&central, "KLA02", 10).await;

    // `central-server seed export KLA01`: the file and its detached signature.
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("KLA01.seed");
    let mut out = std::fs::File::create(&file).expect("create");
    let (digest, heads) = health_sync_server::write_seed(&central, "KLA01", now, &mut out)
        .await
        .expect("export");
    drop(out);
    assert!(heads >= ours.len(), "{heads} heads");
    let jws = sign_detached(&Digest(digest), &signer, now, SEED_TYP).expect("sign");

    // Refused before anything is applied: another facility's edge, a package signature, and a
    // changed byte.
    let elsewhere = store("elsewhere").await;
    let refused = health_edge_runtime::seed::import(&elsewhere, &file, &jws, &trust, "KLA03", now)
        .await
        .expect_err("another facility");
    assert!(refused.contains("not this edge's"), "{refused}");
    assert!(!held(&elsewhere, ours[0]).await, "nothing applied");
    let package = sign_detached(&Digest(digest), &signer, now, TYP).expect("sign");
    let edge = store("edge").await;
    let refused = health_edge_runtime::seed::import(&edge, &file, &package, &trust, "KLA01", now)
        .await
        .expect_err("a package's signature");
    assert!(refused.contains("signature"), "{refused}");
    let tampered = dir.path().join("tampered.seed");
    let mut bytes = std::fs::read(&file).expect("read");
    let middle = bytes.len() / 2;
    bytes[middle] ^= 0x01;
    std::fs::write(&tampered, bytes).expect("write");
    let refused = health_edge_runtime::seed::import(&edge, &tampered, &jws, &trust, "KLA01", now)
        .await
        .expect_err("a changed byte");
    assert!(refused.contains("signature"), "{refused}");
    assert!(!held(&edge, ours[0]).await, "nothing applied");

    // The edge of KLA01, with no link: its records arrive, the other facility's do not.
    let imported = health_edge_runtime::seed::import(&edge, &file, &jws, &trust, "KLA01", now)
        .await
        .expect("import");
    assert!(imported.applied >= ours.len(), "{imported:?}");
    assert_eq!(
        imported.central,
        central.node_id().await.expect("central id")
    );
    assert_eq!(
        imported.seq,
        central.cursor().await.expect("cursor").seq,
        "sync resumes after the seed"
    );
    for id in &ours {
        assert!(held(&edge, *id).await, "Observation/{id} at the edge");
    }
    for id in &theirs {
        assert!(
            !held(&edge, *id).await,
            "KLA02's Observation/{id} at KLA01's edge"
        );
    }
    // A second import changes nothing.
    let again = health_edge_runtime::seed::import(&edge, &file, &jws, &trust, "KLA01", now)
        .await
        .expect("again");
    assert_eq!(again.applied, 0);
}
