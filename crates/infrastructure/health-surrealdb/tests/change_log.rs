//! Repository and stress tests of the sync change log, commit counter and outbox (#79; ADR 0019),
//! on the in-memory engine.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store, Transaction, WriteError, WriteRequest,
};
use health_surrealdb::{
    Change, Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate, new_epoch,
};
use health_time::{FixedClock, Timestamp};

const OBSERVATION: &str = r#"{"resourceType":"Observation","status":"final"}"#;

async fn store() -> (SurrealStore, Connection) {
    let settings = Settings {
        target: Target::Memory,
        namespace: "test".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let at: Timestamp = "2026-11-07T08:00:00Z".parse().expect("timestamp");
    (
        SurrealStore::new(connection.clone(), Arc::new(FixedClock::new(at))),
        connection,
    )
}

fn obs() -> ResourceType {
    "Observation".parse().expect("type")
}

fn create(id: ResourceId, op: OpId) -> WriteRequest {
    WriteRequest {
        resource_type: obs(),
        id,
        json: RawJson::from_validated(OBSERVATION),
        op_id: op,
        precondition: Precondition::MustNotExist,
    }
}

#[tokio::test]
async fn each_version_gets_the_next_seq_and_a_transaction_shares_its_tx_id() {
    let (store, _) = store().await;
    let start = store.cursor().await.expect("cursor");
    assert_eq!(start.seq, 0);

    let (a, b, c) = (ResourceId::new(), ResourceId::new(), ResourceId::new());
    let mut tx = store.begin().await.expect("begin");
    let first = tx.write(create(a, OpId::new())).await.expect("a");
    tx.commit().await.expect("commit");
    let mut tx = store.begin().await.expect("begin");
    tx.write(create(b, OpId::new())).await.expect("b");
    tx.write(create(c, OpId::new())).await.expect("c");
    tx.delete(&obs(), a, OpId::new(), Some(first.version_id))
        .await
        .expect("delete a");
    tx.commit().await.expect("commit");

    let changes = store.changes_after(0, 100).await.expect("changes");
    let summary: Vec<(u64, ResourceId, u64, bool)> = changes
        .iter()
        .map(|c| {
            (
                c.seq,
                c.resource_id.parse().expect("id"),
                c.version_seq,
                c.deleted,
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (1, a, 1, false),
            (2, b, 1, false),
            (3, c, 1, false),
            (4, a, 2, true)
        ]
    );
    assert_eq!(changes[0].op_id, first.version_id);
    assert_ne!(changes[0].tx_id, changes[1].tx_id);
    assert!(changes[1..].iter().all(|c| c.tx_id == changes[1].tx_id));
    assert!(changes.iter().all(|c| c.resource_type == obs().as_str()));

    let cursor = store.cursor().await.expect("cursor");
    assert_eq!(
        (cursor.epoch.as_str(), cursor.seq),
        (start.epoch.as_str(), 4)
    );
    assert_eq!(
        store.changes_after(2, 1).await.expect("page"),
        vec![changes[2].clone()]
    );
    assert!(store.changes_after(4, 10).await.expect("tail").is_empty());
}

#[tokio::test]
async fn a_rolled_back_transaction_takes_no_seq() {
    let (store, _) = store().await;
    {
        let mut tx = store.begin().await.expect("begin");
        tx.write(create(ResourceId::new(), OpId::new()))
            .await
            .expect("write");
    }
    let mut tx = store.begin().await.expect("begin");
    let failed = tx.write(WriteRequest {
        precondition: Precondition::IfMatch(OpId::new()),
        ..create(ResourceId::new(), OpId::new())
    });
    assert!(failed.await.is_err());
    tx.commit()
        .await
        .expect("a transaction that wrote nothing commits");
    assert_eq!(store.cursor().await.expect("cursor").seq, 0);

    let id = ResourceId::new();
    let mut tx = store.begin().await.expect("begin");
    tx.write(create(id, OpId::new())).await.expect("write");
    tx.commit().await.expect("commit");
    let changes = store.changes_after(0, 10).await.expect("changes");
    assert_eq!(
        changes
            .iter()
            .map(|c| (c.seq, c.resource_id.clone()))
            .collect::<Vec<_>>(),
        vec![(1, id.to_string())]
    );
}

#[tokio::test]
async fn each_upstream_gets_an_outbox_row_until_it_acknowledges() {
    let (store, _) = store().await;
    let before = ResourceId::new();
    let mut tx = store.begin().await.expect("begin");
    tx.write(create(before, OpId::new())).await.expect("write");
    tx.commit().await.expect("commit");

    store.add_upstream("central").await.expect("upstream");
    store.add_upstream("edge-b").await.expect("upstream");
    let ids: Vec<ResourceId> = (0..3).map(|_| ResourceId::new()).collect();
    for id in &ids {
        let mut tx = store.begin().await.expect("begin");
        tx.write(create(*id, OpId::new())).await.expect("write");
        tx.commit().await.expect("commit");
    }

    let seqs = |changes: Vec<Change>| changes.iter().map(|c| c.seq).collect::<Vec<_>>();
    // A new peer is queued the whole log, including what was written before it was added.
    assert_eq!(
        seqs(store.pending_for("central", 10).await.expect("pending")),
        vec![1, 2, 3, 4]
    );
    assert_eq!(
        seqs(store.pending_for("central", 2).await.expect("pending")),
        vec![1, 2]
    );
    store.acknowledge("central", 3).await.expect("ack");
    assert_eq!(
        seqs(store.pending_for("central", 10).await.expect("pending")),
        vec![4]
    );
    // Adding it again queues nothing again.
    store.add_upstream("central").await.expect("again");
    assert_eq!(
        seqs(store.pending_for("central", 10).await.expect("pending")),
        vec![4]
    );
    assert_eq!(
        seqs(store.pending_for("edge-b", 10).await.expect("pending")),
        vec![1, 2, 3, 4],
        "each peer has its own queue"
    );
    assert_eq!(
        store.pending_for("central", 10).await.expect("pending")[0].resource_id,
        ids[2].to_string()
    );

    store.remove_upstream("edge-b").await.expect("remove");
    assert!(
        store
            .pending_for("edge-b", 10)
            .await
            .expect("pending")
            .is_empty()
    );
    let mut tx = store.begin().await.expect("begin");
    tx.write(create(ResourceId::new(), OpId::new()))
        .await
        .expect("write");
    tx.commit().await.expect("commit");
    assert!(
        store
            .pending_for("edge-b", 10)
            .await
            .expect("pending")
            .is_empty()
    );
    assert_eq!(
        seqs(store.pending_for("central", 10).await.expect("pending")),
        vec![4, 5]
    );
}

#[tokio::test]
async fn a_new_epoch_keeps_the_seq() {
    let (store, connection) = store().await;
    let mut tx = store.begin().await.expect("begin");
    tx.write(create(ResourceId::new(), OpId::new()))
        .await
        .expect("write");
    tx.commit().await.expect("commit");
    let before = store.cursor().await.expect("cursor");
    let after = new_epoch(&connection).await.expect("epoch");
    assert_ne!(after.epoch, before.epoch);
    assert_eq!(after.seq, before.seq);
    assert_eq!(store.cursor().await.expect("cursor"), after);
}

/// The acceptance criterion: `seq` order is commit order under concurrent writers. A reader
/// tails the log while they write; if a lower `seq` could commit after a higher one was
/// visible, the reader's cursor would pass it and the entry would never be read.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn seq_order_is_commit_order_under_concurrent_writers() {
    let (store, _) = store().await;
    let writers = 8;
    let per_writer = 25;
    let done = Arc::new(AtomicBool::new(false));

    let reader = {
        let (store, done) = (store.clone(), Arc::clone(&done));
        tokio::spawn(async move {
            let (mut cursor, mut seen) = (0u64, Vec::new());
            loop {
                let finished = done.load(Ordering::SeqCst);
                let page = store.changes_after(cursor, 50).await.expect("tail");
                for change in &page {
                    assert_eq!(change.seq, cursor + 1, "the tail skipped or repeated a seq");
                    cursor = change.seq;
                    seen.push(change.resource_id.clone());
                }
                if finished && page.is_empty() {
                    return seen;
                }
                tokio::task::yield_now().await;
            }
        })
    };

    let mut handles = Vec::new();
    for _ in 0..writers {
        let store = store.clone();
        handles.push(tokio::spawn(async move {
            let (mut written, mut conflicts) = (Vec::new(), 0u32);
            while written.len() < per_writer {
                let id = ResourceId::new();
                let mut tx = store.begin().await.expect("begin");
                match tx.write(create(id, OpId::new())).await {
                    Ok(_) => {}
                    Err(WriteError::Conflict) => {
                        conflicts += 1;
                        continue;
                    }
                    Err(other) => panic!("unexpected {other}"),
                }
                match tx.commit().await {
                    Ok(()) => written.push(id),
                    Err(WriteError::Conflict) => conflicts += 1,
                    Err(other) => panic!("unexpected {other}"),
                }
            }
            (written, conflicts)
        }));
    }
    let (mut written, mut conflicts) = (Vec::new(), 0);
    for handle in handles {
        let (ids, c) = handle.await.expect("writer");
        written.extend(ids);
        conflicts += c;
    }
    done.store(true, Ordering::SeqCst);
    let mut seen = reader.await.expect("reader");

    let total = writers * per_writer;
    assert_eq!(seen.len(), total, "the tail missed an entry");
    assert_eq!(
        store.cursor().await.expect("cursor").seq,
        u64::try_from(total).expect("total")
    );
    seen.sort();
    let mut written: Vec<String> = written.iter().map(ToString::to_string).collect();
    written.sort();
    assert_eq!(seen, written, "every committed version is on the log once");
    eprintln!("{total} versions, {conflicts} conflicts retried");
}
