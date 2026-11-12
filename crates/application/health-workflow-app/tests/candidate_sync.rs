//! Workflow candidates over sync (#100): a desktop holding a replica of an edge's instance moves
//! it offline; the move goes up as an operation, waits at the edge, and the edge's engine
//! applies it, or opens a Conflict Record when the instance has moved on elsewhere.

use std::collections::BTreeMap;
use std::sync::Arc;

use health_identifiers::NodeId;
use health_persistence::Store as _;
use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealAuthority, SurrealReplica, SurrealStore, SurrealTx,
    SyncScope, Target, migrate,
};
use health_sync_protocol::{
    Lane, Outcome as Pushed, PullRequest, PullResponse, apply_pull, receive,
};
use health_time::{Clock, FixedClock, Timestamp};
use health_workflow::{
    DataModel, Event, Guards, InstanceRecord, Kind, State, Statechart, Transition,
    WorkflowStore as _,
};
use health_workflow_app::{
    ActionCall, ActionFailed, Actions, Compiled, Judged, Outcome, Workflows,
};

const OPD: &str = "urn:opd|1.0.0";

fn on(event: &str, target: &str) -> Transition {
    Transition {
        events: vec![event.into()],
        guard: None,
        targets: vec![target.into()],
        internal: false,
        actions: vec![],
    }
}

fn opd() -> Statechart {
    let mut triage = State::new("triage", Kind::Atomic);
    triage.transitions = vec![
        on("triage.completed", "consultation"),
        on("triage.referred", "referred"),
    ];
    Statechart {
        states: vec![
            triage,
            State::new("consultation", Kind::Atomic),
            State::new("referred", Kind::Atomic),
        ],
        initial: vec![],
    }
}

struct Always;

impl Guards for Always {
    fn holds(&self, _: &str, _: Option<&Event>, _: &DataModel) -> bool {
        true
    }
}

struct Nothing;

impl Actions<SurrealTx> for Nothing {
    async fn execute(&self, _: &mut SurrealTx, _: &ActionCall) -> Result<(), ActionFailed> {
        Ok(())
    }
}

type App = Workflows<SurrealStore, BTreeMap<String, Arc<Compiled>>, Nothing, Always>;

struct Node {
    id: NodeId,
    store: SurrealStore,
    app: App,
}

async fn node(name: &str) -> Node {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "candidate-sync".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(
        "2026-11-12T08:00:00Z".parse::<Timestamp>().expect("now"),
    ));
    let store = SurrealStore::new(connection, clock.clone());
    let app = Workflows::new(
        store.clone(),
        BTreeMap::from([(OPD.to_owned(), Arc::new(Compiled::new(opd()).expect("opd")))]),
        Nothing,
        Always,
        clock,
        Vec::new(),
        name,
    );
    Node {
        id: NodeId::new(),
        store,
        app,
    }
}

fn applied(outcome: Outcome) -> InstanceRecord {
    match outcome {
        Outcome::Applied(record) => record,
        other => panic!("expected Applied, got {other:?}"),
    }
}

/// The edge, and a desktop that pulled its replica of the edge's instance started with `op`.
async fn replicated(op: &str) -> (Node, Node, InstanceRecord) {
    let edge = node("edge").await;
    let central = NodeId::new().to_string();
    edge.store.add_upstream(&central).await.expect("central");
    let desktop = node("desktop").await;
    desktop
        .store
        .add_upstream(&edge.id.to_string())
        .await
        .expect("upstream");
    let record = applied(
        edge.app
            .start(OPD, "Encounter/e-1", "p-1", op)
            .await
            .expect("start"),
    );
    pull(&desktop, &edge).await;
    let mut tx = desktop.store.begin().await.expect("begin");
    let replica = tx
        .instance(&record.id)
        .await
        .expect("read")
        .expect("replica");
    assert_eq!(replica, record, "the desktop holds the edge's instance");
    (edge, desktop, record)
}

/// Pulls the edge's changes down to the desktop: how many items came.
async fn pull(desktop: &Node, edge: &Node) -> usize {
    let mut pulled = 0;
    loop {
        let request = PullRequest {
            cursor: desktop
                .store
                .checkpoint(edge.id, Lane::Clinical)
                .await
                .expect("checkpoint"),
            lane: Lane::Clinical,
            limit: 50,
        };
        let response = edge
            .store
            .pull(
                &SyncScope::Facility("KLA01".into()),
                edge.id,
                desktop.id,
                &request,
            )
            .await
            .expect("pull");
        let more = matches!(response, PullResponse::Items { more: true, .. });
        if let PullResponse::Items { items, .. } = &response {
            pulled += items.len();
        }
        apply_pull(
            &SurrealReplica::new(&desktop.store, edge.id, Lane::Clinical),
            response,
        )
        .await
        .expect("apply");
        if !more {
            return pulled;
        }
    }
}

/// Pushes the desktop's queued operations to the edge: their outcomes.
async fn push(desktop: &Node, edge: &Node) -> Vec<Pushed> {
    let sent = desktop
        .store
        .outbox(&edge.id.to_string(), desktop.id, 50)
        .await
        .expect("outbox");
    let authority =
        SurrealAuthority::new(&edge.store, desktop.id, SyncScope::Facility("KLA01".into()));
    let mut outcomes = Vec::new();
    for outgoing in &sent {
        outcomes.push(
            receive(&authority, &outgoing.group)
                .await
                .expect("receive")
                .outcome,
        );
    }
    outcomes
}

#[tokio::test]
async fn an_offline_move_goes_up_and_the_edge_applies_it_once() {
    let (edge, desktop, record) = replicated("op-start").await;
    let moved = applied(
        desktop
            .app
            .command(
                &record.id,
                &Event::named("triage.completed"),
                "nurse",
                "op-1",
            )
            .await
            .expect("offline"),
    );
    assert_eq!(moved.state.configuration, ["consultation"]);
    assert_eq!(push(&desktop, &edge).await, vec![Pushed::Accepted]);
    // Sent again, as after a lost answer: kept once.
    assert_eq!(push(&desktop, &edge).await, vec![Pushed::Accepted]);

    let judged = edge.app.apply_candidates(10).await.expect("judge");
    assert!(
        matches!(&judged[..], [(_, Judged::Applied(next))] if next.state.configuration == ["consultation"])
    );
    assert!(
        edge.app
            .apply_candidates(10)
            .await
            .expect("again")
            .is_empty()
    );
}

#[tokio::test]
async fn an_offline_move_from_a_state_the_edge_has_left_opens_a_conflict_record() {
    let (edge, desktop, record) = replicated("op-start").await;
    applied(
        edge.app
            .command(
                &record.id,
                &Event::named("triage.referred"),
                "doctor",
                "op-edge",
            )
            .await
            .expect("edge"),
    );
    applied(
        desktop
            .app
            .command(
                &record.id,
                &Event::named("triage.completed"),
                "nurse",
                "op-1",
            )
            .await
            .expect("offline"),
    );
    assert_eq!(push(&desktop, &edge).await, vec![Pushed::Accepted]);

    let judged = edge.app.apply_candidates(10).await.expect("judge");
    assert!(
        matches!(&judged[..], [(_, Judged::Conflict(why))] if why.contains("no longer in triage")),
        "{judged:?}"
    );
    let conflicts = edge.store.open_conflicts(None).await.expect("conflicts");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].class, "workflow");
    assert_eq!(conflicts[0].peer, desktop.id.to_string());
    assert_eq!(
        conflicts[0].candidates.len(),
        1,
        "the pushed candidate is kept"
    );
    // The instance stays where the edge put it.
    let mut tx = edge.store.begin().await.expect("begin");
    let held = tx
        .instance(&record.id)
        .await
        .expect("read")
        .expect("instance");
    assert_eq!(held.state.configuration, ["referred"]);
}
