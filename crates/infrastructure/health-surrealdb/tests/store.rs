//! Repository tests: the health-persistence contract on SurrealDB 3.3.5 (in-memory engine).

use std::collections::BTreeSet;
use std::sync::Arc;

use health_identifiers::{NumberFormat, OpId, ResourceId};
use health_persistence::{
    CounterKey, GuardKey, OutboxEntry, OutboxPayload, Precondition, RawJson, ResourceType,
    RetryPolicy, Store, StoreError, Transaction, WriteError, WriteRequest,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{FixedClock, Timestamp};

const OBSERVATION: &str = r#"{"resourceType":"Observation","status":"final","valueQuantity":{"value":1.50,"unit":"kg"},"note":[{"text":"x"}]}"#;

async fn store() -> (SurrealStore, Connection) {
    let settings = Settings {
        target: Target::Memory,
        namespace: "test".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let at: Timestamp = "2026-10-29T08:00:00.25Z".parse().expect("timestamp");
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

fn update(id: ResourceId, op: OpId, if_match: OpId) -> WriteRequest {
    WriteRequest {
        precondition: Precondition::IfMatch(if_match),
        ..create(id, op)
    }
}

async fn committed_create(store: &SurrealStore, id: ResourceId) -> OpId {
    let op = OpId::new();
    let mut tx = store.begin().await.expect("begin");
    tx.write(create(id, op)).await.expect("create");
    tx.commit().await.expect("commit");
    op
}

#[tokio::test]
async fn stores_and_serves_the_stamped_input_bytes() {
    let (store, _) = store().await;
    let id = ResourceId::new();
    let op = committed_create(&store, id).await;
    let mut tx = store.begin().await.expect("begin");
    let head = tx.read(&obs(), id).await.expect("read").expect("exists");
    assert_eq!(
        (
            head.version.version_id,
            head.version.version_seq,
            head.version.deleted
        ),
        (op, 1, false)
    );
    assert_eq!(
        head.version.last_updated.to_string(),
        "2026-10-29T08:00:00.25Z"
    );
    let json = head.json.as_str();
    assert!(
        json.starts_with(
            r#"{"resourceType":"Observation","status":"final","valueQuantity":{"value":1.50,"#
        ),
        "{json}"
    );
    assert!(
        json.contains(&format!(
            r#""meta":{{"versionId":"{op}","lastUpdated":"2026-10-29T08:00:00.250Z"}}"#
        )),
        "{json}"
    );
}

#[tokio::test]
async fn updates_require_the_current_version_and_append_history() {
    let (store, connection) = store().await;
    let id = ResourceId::new();
    let first = committed_create(&store, id).await;
    let mut tx = store.begin().await.expect("begin");
    assert_eq!(
        tx.write(create(id, OpId::new())).await,
        Err(WriteError::PreconditionFailed {
            current: Some(first)
        })
    );
    assert_eq!(
        tx.write(update(id, OpId::new(), OpId::new())).await,
        Err(WriteError::PreconditionFailed {
            current: Some(first)
        })
    );
    let version = tx
        .write(update(id, OpId::new(), first))
        .await
        .expect("update");
    assert_eq!(version.version_seq, 2);
    tx.commit().await.expect("commit");
    let mut history = connection
        .query(&format!(
            "RETURN count(SELECT * FROM observation_history:[\"{id}\", 1]..=[\"{id}\", 99])"
        ))
        .await
        .expect("history");
    assert_eq!(history.take::<Option<i64>>(0).expect("count"), Some(2));
}

#[tokio::test]
async fn concurrent_writers_one_wins_and_the_other_gets_412_after_retry() {
    let (store, _) = store().await;
    let id = ResourceId::new();
    let first = committed_create(&store, id).await;
    let policy = RetryPolicy::default();

    let mut a = store.begin().await.expect("a");
    let mut b = store.begin().await.expect("b");
    a.write(update(id, OpId::new(), first))
        .await
        .expect("a writes");
    b.write(update(id, OpId::new(), first))
        .await
        .expect("b writes from the same snapshot");
    a.commit().await.expect("a wins");
    let lost = b.commit().await.expect_err("b loses the race");
    assert_eq!(lost, WriteError::Conflict);
    assert!(policy.should_retry(1, &lost));

    // The rerun of b's use case reads the new head, so If-Match now fails: 412.
    let mut rerun = store.begin().await.expect("rerun");
    let result = rerun.write(update(id, OpId::new(), first)).await;
    assert!(
        matches!(result, Err(WriteError::PreconditionFailed { current: Some(v) }) if v != first),
        "{result:?}"
    );
}

#[tokio::test]
async fn dropping_a_transaction_rolls_it_back() {
    let (store, _) = store().await;
    let id = ResourceId::new();
    {
        let mut tx = store.begin().await.expect("begin");
        tx.write(create(id, OpId::new())).await.expect("write");
    }
    let mut tx = store.begin().await.expect("begin");
    assert_eq!(tx.read(&obs(), id).await.expect("read"), None);
}

#[tokio::test]
async fn tombstones_keep_the_id_and_refuse_writes() {
    let (store, _) = store().await;
    let id = ResourceId::new();
    let first = committed_create(&store, id).await;
    let mut tx = store.begin().await.expect("begin");
    let tombstone = tx
        .delete(&obs(), id, OpId::new(), Some(first))
        .await
        .expect("delete");
    assert!(tombstone.deleted);
    assert_eq!(
        tx.write(create(id, OpId::new())).await,
        Err(WriteError::Gone)
    );
    tx.commit().await.expect("commit");
    let mut tx = store.begin().await.expect("begin");
    assert!(
        tx.read(&obs(), id)
            .await
            .expect("read")
            .expect("tombstone")
            .version
            .deleted
    );
}

#[tokio::test]
async fn guards_are_unique_without_read_then_check() {
    let (store, _) = store().await;
    let key = GuardKey("mrn|KLA01|KLA01-E1-42".into());
    let mut a = store.begin().await.expect("a");
    let mut b = store.begin().await.expect("b");
    a.claim_guard(key.clone()).await.expect("a claims");
    b.claim_guard(key.clone())
        .await
        .expect("b claims in its snapshot");
    a.commit().await.expect("a commits");
    assert_eq!(b.commit().await, Err(WriteError::Conflict));
    let mut c = store.begin().await.expect("c");
    assert_eq!(
        c.claim_guard(key.clone()).await,
        Err(WriteError::GuardHeld(key))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_sequence_allocation_never_duplicates() {
    let (store, _) = store().await;
    let counter = CounterKey("mrn|KLA01|E1".into());
    let writers = 8;
    let per_writer = 25;
    let mut handles = Vec::new();
    for _ in 0..writers {
        let (store, counter) = (store.clone(), counter.clone());
        handles.push(tokio::spawn(async move {
            let (mut minted, mut conflicts) = (Vec::new(), 0u32);
            while minted.len() < per_writer {
                let mut tx = store.begin().await.expect("begin");
                let value = tx
                    .next_sequence(&counter, &NumberFormat::default(), 2026)
                    .await;
                let value = match value {
                    Ok(v) => v,
                    Err(WriteError::Conflict) => {
                        conflicts += 1;
                        continue;
                    }
                    Err(other) => panic!("unexpected {other}"),
                };
                match tx.commit().await {
                    Ok(()) => minted.push(value),
                    Err(WriteError::Conflict) => conflicts += 1,
                    Err(other) => panic!("unexpected {other}"),
                }
            }
            (minted, conflicts)
        }));
    }
    let mut all = Vec::new();
    let mut conflicts = 0;
    for handle in handles {
        let (minted, c) = handle.await.expect("writer");
        all.extend(minted);
        conflicts += c;
    }
    let distinct: BTreeSet<u64> = all.iter().copied().collect();
    assert_eq!(distinct.len(), all.len(), "a value was minted twice");
    assert_eq!(
        distinct,
        (1..=u64::try_from(all.len()).expect("len")).collect(),
        "gap in the sequence"
    );
    eprintln!("{} values, {conflicts} conflicts retried", all.len());
}

#[tokio::test]
async fn outbox_rows_commit_with_the_write() {
    let (store, connection) = store().await;
    let (id, op) = (ResourceId::new(), OpId::new());
    let mut tx = store.begin().await.expect("begin");
    let version = tx.write(create(id, op)).await.expect("write");
    tx.append_outbox(OutboxEntry {
        op_id: op,
        payload: OutboxPayload::ResourceVersion {
            resource_type: obs(),
            id,
            version_id: version.version_id,
        },
    })
    .await
    .expect("outbox");
    let mut before = connection
        .query("RETURN count(SELECT * FROM outbox)")
        .await
        .expect("count");
    assert_eq!(
        before.take::<Option<i64>>(0).expect("count"),
        Some(0),
        "invisible before commit"
    );
    tx.commit().await.expect("commit");
    let mut after = connection
        .query("SELECT VALUE kind FROM outbox")
        .await
        .expect("kinds");
    assert_eq!(
        after.take::<Vec<String>>(0).expect("kinds"),
        vec!["resource-version".to_owned()]
    );
}

#[tokio::test]
async fn unknown_resource_types_have_no_table() {
    let (store, _) = store().await;
    let mut tx = store.begin().await.expect("begin");
    let unknown: ResourceType = "Basic".parse().expect("type");
    assert_eq!(
        tx.read(&unknown, ResourceId::new()).await,
        Err(StoreError::UnknownResourceType("Basic".into()))
    );
}

#[tokio::test]
async fn condition_locks_serialize_the_same_condition_and_leave_nothing() {
    let (store, connection) = store().await;
    let condition = |v: &str| vec![("identifier".to_owned(), v.to_owned())];
    let mut a = store.begin().await.expect("a");
    let mut b = store.begin().await.expect("b");
    a.lock_condition(&obs(), &condition("urn:x|1"))
        .await
        .expect("a locks");
    b.lock_condition(&obs(), &condition("urn:x|1"))
        .await
        .expect("b locks in its snapshot");
    a.commit().await.expect("a commits");
    assert_eq!(b.commit().await, Err(WriteError::Conflict));

    let mut c = store.begin().await.expect("c");
    let mut d = store.begin().await.expect("d");
    c.lock_condition(&obs(), &condition("urn:x|2"))
        .await
        .expect("c");
    d.lock_condition(&obs(), &condition("urn:x|3"))
        .await
        .expect("d");
    c.commit()
        .await
        .expect("different conditions do not conflict");
    d.commit()
        .await
        .expect("different conditions do not conflict");

    let mut left = connection
        .query("RETURN count(SELECT * FROM condition_lock)")
        .await
        .expect("count");
    assert_eq!(left.take::<Option<i64>>(0).expect("count"), Some(0));
}
