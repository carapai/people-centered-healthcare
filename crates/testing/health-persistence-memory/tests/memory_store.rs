//! The fake honours the health-persistence contract: preconditions, stamping, rollback,
//! first-committer-wins conflicts, guards, counters and the outbox.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::thread;

use health_identifiers::{NumberFormat, OpId, ResourceId};
use health_persistence::{
    CounterKey, GuardKey, OutboxEntry, OutboxPayload, Precondition, RawJson, ResourceType,
    RetryPolicy, Store, Transaction, WriteError, WriteRequest,
};
use health_persistence_memory::MemoryStore;
use health_time::{FixedClock, Timestamp};
use pollster::block_on;

const OBSERVATION: &str = r#"{"resourceType":"Observation","status":"final","valueQuantity":{"value":1.50,"unit":"kg"},"note":[{"text":"x"}]}"#;

fn store() -> MemoryStore {
    let at: Timestamp = "2026-10-29T08:00:00Z".parse().expect("timestamp");
    MemoryStore::new(Arc::new(FixedClock::new(at)))
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

#[test]
fn create_stamps_id_and_meta_and_keeps_the_input_text() {
    let store = store();
    let (id, op) = (ResourceId::new(), OpId::new());
    let mut tx = block_on(store.begin()).expect("begin");
    let version = block_on(tx.write(create(id, op))).expect("write");
    block_on(tx.commit()).expect("commit");
    assert_eq!(
        (version.version_seq, version.version_id, version.deleted),
        (1, op, false)
    );

    let mut tx = block_on(store.begin()).expect("begin");
    let head = block_on(tx.read(&obs(), id))
        .expect("read")
        .expect("exists");
    let json = head.json.as_str();
    assert!(
        json.contains(r#""value":1.50"#),
        "decimal text kept: {json}"
    );
    assert!(
        json.starts_with(r#"{"resourceType":"Observation","status":"final","valueQuantity""#),
        "key order kept: {json}"
    );
    let value: serde_json::Value = serde_json::from_str(json).expect("json");
    assert_eq!(value["id"], id.to_string());
    assert_eq!(value["meta"]["versionId"], op.to_string());
    assert_eq!(value["meta"]["lastUpdated"], "2026-10-29T08:00:00.000Z");
}

#[test]
fn preconditions_are_enforced() {
    let store = store();
    let (id, first) = (ResourceId::new(), OpId::new());
    let mut tx = block_on(store.begin()).expect("begin");
    block_on(tx.write(create(id, first))).expect("create");
    assert_eq!(
        block_on(tx.write(create(id, OpId::new()))),
        Err(WriteError::PreconditionFailed {
            current: Some(first)
        })
    );
    assert_eq!(
        block_on(tx.write(update(id, OpId::new(), OpId::new()))),
        Err(WriteError::PreconditionFailed {
            current: Some(first)
        })
    );
    let second = OpId::new();
    let version = block_on(tx.write(update(id, second, first))).expect("update");
    assert_eq!(version.version_seq, 2);
    block_on(tx.commit()).expect("commit");
    assert_eq!(store.history(&obs(), id).len(), 2);
}

#[test]
fn update_of_a_missing_resource_fails_its_precondition() {
    let store = store();
    let mut tx = block_on(store.begin()).expect("begin");
    let result = block_on(tx.write(update(ResourceId::new(), OpId::new(), OpId::new())));
    assert_eq!(
        result,
        Err(WriteError::PreconditionFailed { current: None })
    );
}

#[test]
fn dropping_a_transaction_rolls_it_back() {
    let store = store();
    let id = ResourceId::new();
    {
        let mut tx = block_on(store.begin()).expect("begin");
        block_on(tx.write(create(id, OpId::new()))).expect("write");
        block_on(tx.append_outbox(OutboxEntry {
            op_id: OpId::new(),
            payload: OutboxPayload::DomainEvent {
                name: "e".into(),
                body: RawJson::from_validated("{}"),
            },
        }))
        .expect("outbox");
    }
    let mut tx = block_on(store.begin()).expect("begin");
    assert_eq!(block_on(tx.read(&obs(), id)).expect("read"), None);
    assert!(store.outbox().is_empty());
}

#[test]
fn concurrent_writers_of_one_resource_conflict_and_the_rerun_gets_412() {
    let store = store();
    let (id, first) = (ResourceId::new(), OpId::new());
    let mut tx = block_on(store.begin()).expect("begin");
    block_on(tx.write(create(id, first))).expect("create");
    block_on(tx.commit()).expect("commit");

    let mut a = block_on(store.begin()).expect("a");
    let mut b = block_on(store.begin()).expect("b");
    block_on(a.write(update(id, OpId::new(), first))).expect("a writes");
    block_on(b.write(update(id, OpId::new(), first))).expect("b writes");
    block_on(a.commit()).expect("a commits first");
    let lost = block_on(b.commit()).expect_err("b conflicts");
    assert_eq!(lost, WriteError::Conflict);
    assert!(RetryPolicy::default().should_retry(1, &lost));

    let mut rerun = block_on(store.begin()).expect("rerun");
    let result = block_on(rerun.write(update(id, OpId::new(), first)));
    assert!(matches!(
        result,
        Err(WriteError::PreconditionFailed { current: Some(_) })
    ));
}

#[test]
fn deleted_resources_keep_their_id_and_refuse_writes() {
    let store = store();
    let (id, first) = (ResourceId::new(), OpId::new());
    let mut tx = block_on(store.begin()).expect("begin");
    block_on(tx.write(create(id, first))).expect("create");
    let tombstone = block_on(tx.delete(&obs(), id, OpId::new(), Some(first))).expect("delete");
    assert!(tombstone.deleted);
    assert_eq!(tombstone.version_seq, 2);
    assert_eq!(
        block_on(tx.write(create(id, OpId::new()))),
        Err(WriteError::Gone)
    );
    assert_eq!(
        block_on(tx.delete(&obs(), id, OpId::new(), None)),
        Err(WriteError::Gone)
    );
    let head = block_on(tx.read(&obs(), id))
        .expect("read")
        .expect("tombstone");
    assert!(head.version.deleted);
}

#[test]
fn guards_are_unique_across_transactions() {
    let store = store();
    let key = GuardKey("mrn|KLA01|KLA01-E1-42".into());
    let mut a = block_on(store.begin()).expect("a");
    let mut b = block_on(store.begin()).expect("b");
    block_on(a.claim_guard(key.clone())).expect("a claims");
    block_on(b.claim_guard(key.clone())).expect("b claims in its snapshot");
    block_on(a.commit()).expect("a commits");
    assert_eq!(block_on(b.commit()), Err(WriteError::Conflict));

    let mut c = block_on(store.begin()).expect("c");
    assert_eq!(
        block_on(c.claim_guard(key.clone())),
        Err(WriteError::GuardHeld(key.clone()))
    );
    block_on(c.release_guard(key.clone())).expect("release");
    block_on(c.commit()).expect("commit release");
    let mut d = block_on(store.begin()).expect("d");
    block_on(d.claim_guard(key)).expect("free again");
}

#[test]
fn concurrent_sequence_allocation_never_duplicates() {
    let store = store();
    let counter = CounterKey("mrn|KLA01|E1".into());
    let format = NumberFormat::default();
    let threads = 8;
    let per_thread = 200;
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let (store, counter, format) = (store.clone(), counter.clone(), format.clone());
            thread::spawn(move || {
                let mut minted = Vec::new();
                while minted.len() < per_thread {
                    let mut tx = block_on(store.begin()).expect("begin");
                    let value = block_on(tx.next_sequence(&counter, &format, 2026)).expect("next");
                    match block_on(tx.commit()) {
                        Ok(()) => minted.push(value),
                        Err(WriteError::Conflict) => {} // rerun the use case
                        Err(other) => panic!("unexpected {other}"),
                    }
                }
                minted
            })
        })
        .collect();
    let all: Vec<u64> = handles
        .into_iter()
        .flat_map(|h| h.join().expect("thread"))
        .collect();
    let distinct: BTreeSet<u64> = all.iter().copied().collect();
    assert_eq!(all.len(), threads * per_thread);
    assert_eq!(distinct.len(), all.len(), "no value minted twice");
    assert_eq!(
        distinct,
        (1..=u64::try_from(all.len()).expect("len")).collect(),
        "no gaps either"
    );
}

#[test]
fn outbox_rows_commit_with_the_write() {
    let store = store();
    let (id, op) = (ResourceId::new(), OpId::new());
    let mut tx = block_on(store.begin()).expect("begin");
    let version = block_on(tx.write(create(id, op))).expect("write");
    let row = OutboxEntry {
        op_id: op,
        payload: OutboxPayload::ResourceVersion {
            resource_type: obs(),
            id,
            version_id: version.version_id,
        },
    };
    block_on(tx.append_outbox(row.clone())).expect("outbox");
    assert!(store.outbox().is_empty(), "invisible before commit");
    block_on(tx.commit()).expect("commit");
    assert_eq!(store.outbox(), vec![row]);
}

#[test]
fn retry_policy_retries_only_conflicts_up_to_the_limit() {
    let policy = RetryPolicy::default();
    assert!(policy.should_retry(3, &WriteError::Conflict));
    assert!(!policy.should_retry(4, &WriteError::Conflict));
    assert!(!policy.should_retry(1, &WriteError::PreconditionFailed { current: None }));
}
