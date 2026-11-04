//! Repository tests: the workflow engine's records (#97; ADR 0044) on SurrealDB 3.3.5.

use std::collections::BTreeMap;
use std::sync::Arc;

use health_persistence::{Store, Transaction};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{FixedClock, Timestamp};
use health_workflow::{
    Effect, Event, HistoryEntry, Instance, InstanceRecord, TimerRecord, TimerStatus, WorkflowStore,
};

fn at() -> Timestamp {
    "2026-11-04T08:00:00Z".parse().expect("timestamp")
}

async fn store() -> (SurrealStore, Connection) {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "workflow".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    (
        SurrealStore::new(connection.clone(), Arc::new(FixedClock::new(at()))),
        connection,
    )
}

fn instance(version: u64, configuration: &[&str]) -> InstanceRecord {
    InstanceRecord {
        id: "i-1".into(),
        definition: "urn:opd|1.0.0".into(),
        context: "Encounter/e-1".into(),
        authority: "KAM013".into(),
        state: Instance {
            configuration: configuration.iter().map(|s| (*s).to_owned()).collect(),
            data: BTreeMap::new(),
            sequence: version,
            invoked: vec![],
            running: true,
        },
        version,
    }
}

fn entry(seq: u64, event: &str) -> HistoryEntry {
    HistoryEntry {
        instance: "i-1".into(),
        seq,
        event: Event::named(event),
        from: vec!["triage".into()],
        to: vec!["consultation".into()],
        effects: vec![Effect::Execute {
            action: "patient-flow/open-queue-entry@1".into(),
            params: BTreeMap::new(),
            key: (seq, 0),
        }],
        actor: "Practitioner/p-1".into(),
        at: at(),
        op_id: format!("op-{seq}"),
    }
}

#[tokio::test]
async fn an_instance_changes_only_from_the_version_it_was_read_at() {
    let (store, _) = store().await;
    let mut tx = store.begin().await.expect("begin");
    tx.create_instance(&instance(1, &["triage"]))
        .await
        .expect("create");
    tx.commit().await.expect("commit");

    let mut tx = store.begin().await.expect("begin");
    assert!(tx.create_instance(&instance(1, &["triage"])).await.is_err());
    drop(tx);

    let mut tx = store.begin().await.expect("begin");
    let next = instance(2, &["consultation"]);
    assert!(tx.update_instance(&next, 1).await.expect("update"));
    assert!(
        !tx.update_instance(&instance(2, &["discharge"]), 1)
            .await
            .expect("stale update"),
        "a change from a version already moved on is refused"
    );
    tx.commit().await.expect("commit");

    let mut tx = store.begin().await.expect("begin");
    assert_eq!(tx.instance("i-1").await.expect("read"), Some(next));
    assert_eq!(tx.instance("i-2").await.expect("read"), None);
}

#[tokio::test]
async fn history_is_appended_in_order_and_never_rewritten() {
    let (store, connection) = store().await;
    let mut tx = store.begin().await.expect("begin");
    for seq in [2, 1, 3] {
        tx.append_history(&entry(seq, &format!("e{seq}")))
            .await
            .expect("append");
    }
    tx.commit().await.expect("commit");

    let mut tx = store.begin().await.expect("begin");
    assert!(
        tx.append_history(&entry(2, "again")).await.is_err(),
        "a taken seq cannot be appended again"
    );
    drop(tx);

    let rewrite = connection
        .query("UPDATE workflow_history SET data = 'x'")
        .await
        .and_then(surrealdb::IndexedResults::check);
    assert!(rewrite.is_err(), "history fields are READONLY");

    let mut tx = store.begin().await.expect("begin");
    let history = tx.instance_history("i-1").await.expect("history");
    let seqs: Vec<u64> = history.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, [1, 2, 3]);
    assert_eq!(history[1], entry(2, "e2"));
    assert!(
        tx.instance_history("i-2")
            .await
            .expect("history")
            .is_empty()
    );
}

#[tokio::test]
async fn a_domain_event_is_claimed_once() {
    let (store, _) = store().await;
    let mut tx = store.begin().await.expect("begin");
    assert!(tx.claim_event("evt-1", at()).await.expect("claim"));
    assert!(!tx.claim_event("evt-1", at()).await.expect("claim again"));
    tx.commit().await.expect("commit");

    let mut tx = store.begin().await.expect("begin");
    assert!(!tx.claim_event("evt-1", at()).await.expect("after commit"));
    assert!(tx.claim_event("evt-2", at()).await.expect("another"));
    drop(tx);

    let mut tx = store.begin().await.expect("begin");
    assert!(
        tx.claim_event("evt-2", at()).await.expect("rolled back"),
        "a claim in a transaction that did not commit is not kept"
    );
}

#[tokio::test]
async fn timers_are_kept_and_change_status() {
    let (store, _) = store().await;
    let timer = TimerRecord {
        instance: "i-1".into(),
        id: "triage#0".into(),
        fire_at: "2026-11-04T09:00:00Z".parse().expect("timestamp"),
        event: "triage.overdue".into(),
        status: TimerStatus::Pending,
    };
    let mut tx = store.begin().await.expect("begin");
    tx.put_timer(&timer).await.expect("put");
    let cancelled = TimerRecord {
        status: TimerStatus::Cancelled,
        ..timer.clone()
    };
    tx.put_timer(&cancelled).await.expect("cancel");
    tx.commit().await.expect("commit");

    let mut tx = store.begin().await.expect("begin");
    assert_eq!(tx.timers("i-1").await.expect("timers"), [cancelled]);
}

#[tokio::test]
async fn due_timers_are_the_pending_ones_whose_time_has_come() {
    let (store, _) = store().await;
    let timer = |instance: &str, id: &str, fire_at: &str, status| TimerRecord {
        instance: instance.into(),
        id: id.into(),
        fire_at: fire_at.parse().expect("timestamp"),
        event: "overdue".into(),
        status,
    };
    let mut tx = store.begin().await.expect("begin");
    for t in [
        timer("i-1", "a#0", "2026-11-04T08:30:00Z", TimerStatus::Pending),
        timer("i-2", "a#0", "2026-11-04T08:10:00Z", TimerStatus::Pending),
        timer("i-3", "a#0", "2026-11-04T08:20:00Z", TimerStatus::Fired),
        timer("i-4", "a#0", "2026-11-04T08:05:00Z", TimerStatus::Cancelled),
        timer("i-5", "a#0", "2026-11-04T09:00:00Z", TimerStatus::Pending),
    ] {
        tx.put_timer(&t).await.expect("put");
    }
    tx.commit().await.expect("commit");

    let now: Timestamp = "2026-11-04T08:45:00Z".parse().expect("timestamp");
    let mut tx = store.begin().await.expect("begin");
    let due: Vec<String> = tx
        .due_timers(now, 10)
        .await
        .expect("due")
        .into_iter()
        .map(|t| t.instance)
        .collect();
    assert_eq!(due, ["i-2", "i-1"], "pending and due, earliest first");
    assert_eq!(tx.due_timers(now, 1).await.expect("due").len(), 1);
}

#[tokio::test]
async fn projected_tasks_are_replaced_per_instance_and_listed_by_urgency_then_arrival() {
    use health_workflow::Priority;
    use health_workflow::worklists::{TaskStatus, TaskView};

    let (store, _) = store().await;
    let task = |id: &str, instance: &str, priority, arrived: &str, status| TaskView {
        id: id.into(),
        instance: instance.into(),
        definition: "urn:opd|1.0.0".into(),
        context: "Encounter/e-1".into(),
        state: "triage".into(),
        status,
        role: "nurse".into(),
        form: None,
        worklist: "opd-triage".into(),
        priority,
        owner: None,
        claim_lapses: None,
        authored_on: arrived.parse().expect("timestamp"),
        last_modified: arrived.parse().expect("timestamp"),
    };
    let mut tx = store.begin().await.expect("begin");
    tx.replace_tasks(
        "i-1",
        &[
            task(
                "a",
                "i-1",
                Priority::Routine,
                "2026-11-04T08:00:00Z",
                TaskStatus::Completed,
            ),
            task(
                "b",
                "i-1",
                Priority::Routine,
                "2026-11-04T08:10:00Z",
                TaskStatus::Ready,
            ),
        ],
    )
    .await
    .expect("replace");
    tx.replace_tasks(
        "i-2",
        &[task(
            "c",
            "i-2",
            Priority::Stat,
            "2026-11-04T08:20:00Z",
            TaskStatus::Ready,
        )],
    )
    .await
    .expect("replace");
    tx.commit().await.expect("commit");

    let ids = |tasks: Vec<TaskView>| tasks.into_iter().map(|t| t.id).collect::<Vec<_>>();
    let mut tx = store.begin().await.expect("begin");
    assert_eq!(
        ids(tx.open_tasks("opd-triage").await.expect("open")),
        ["c", "b"]
    );
    tx.replace_tasks("i-2", &[]).await.expect("replace");
    tx.commit().await.expect("commit");

    let mut tx = store.begin().await.expect("begin");
    assert_eq!(ids(tx.open_tasks("opd-triage").await.expect("open")), ["b"]);
    assert!(tx.open_tasks("opd-consult").await.expect("open").is_empty());
}
