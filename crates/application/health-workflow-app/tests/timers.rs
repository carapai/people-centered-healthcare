//! Timer tests (#98; ADR 0044) over SurrealDB 3.3.5 (in-memory): escalation fires once, only at
//! the authority, never on a broken clock, and exactly once across a restart.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use health_persistence::Store;
use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealStore, SurrealTx, Target, migrate,
};
use health_time::{Clock, FixedClock, SignedDuration, Timestamp};
use health_workflow::{
    Action, DataModel, Event, Guards, Kind, State, Statechart, Timer, TimerStatus, Transition,
    WorkflowStore,
};
use health_workflow_actions::{ActionDescriptor, Compensation};
use health_workflow_app::{
    ActionCall, ActionFailed, Actions, Compiled, Fired, Outcome, PRINCIPAL, TimeFloor,
    WorkflowError, Workflows,
};

const ESCALATE: &str = "urn:escalate|1.0.0";
const REMIND: &str = "urn:remind|1.0.0";
const EDGE: &str = "KAM013";

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn execute(action: &str) -> Action {
    Action::Execute {
        action: action.into(),
        params: BTreeMap::new(),
    }
}

/// Triage with an hour's timer. `targets` empty: the overdue transition escalates and stays
/// (the timer is not re-armed); `["triage"]`: it re-enters triage, re-arming the timer.
fn chart(targets: &[&str]) -> Statechart {
    let mut triage = State::new("triage", Kind::Atomic);
    triage.timers = vec![Timer {
        after: SignedDuration::from_hours(1),
        event: "triage.overdue".into(),
    }];
    triage.transitions = vec![
        Transition {
            events: vec!["triage.overdue".into()],
            guard: None,
            targets: targets.iter().map(|t| (*t).to_owned()).collect(),
            internal: false,
            actions: vec![execute("flow/escalate@1")],
        },
        Transition {
            events: vec!["triage.completed".into()],
            guard: None,
            targets: vec!["done".into()],
            internal: false,
            actions: vec![],
        },
    ];
    Statechart {
        states: vec![triage, State::new("done", Kind::Final)],
        initial: vec![],
    }
}

struct Always;

impl Guards for Always {
    fn holds(&self, _: &str, _: Option<&Event>, _: &DataModel) -> bool {
        true
    }
}

/// Records each call; fails every call while `failing`, like a node that goes down mid-firing.
#[derive(Clone, Default)]
struct Recorded {
    calls: Arc<Mutex<Vec<ActionCall>>>,
    failing: bool,
}

impl Recorded {
    fn calls(&self) -> Vec<ActionCall> {
        self.calls.lock().expect("calls").clone()
    }
}

impl Actions<SurrealTx> for Recorded {
    async fn execute(&self, _: &mut SurrealTx, call: &ActionCall) -> Result<(), ActionFailed> {
        self.calls.lock().expect("calls").push(call.clone());
        if self.failing {
            return Err(ActionFailed("power lost".into()));
        }
        Ok(())
    }
}

struct Floor(Timestamp);

impl TimeFloor for Floor {
    fn floor(&self) -> Option<Timestamp> {
        Some(self.0)
    }
}

type App = Workflows<SurrealStore, BTreeMap<String, Arc<Compiled>>, Recorded, Always>;

struct Node {
    store: SurrealStore,
    clock: Arc<FixedClock>,
}

impl Node {
    async fn new() -> Self {
        let connection = Connection::open(&Settings {
            target: Target::Memory,
            namespace: "workflow-timers".into(),
            database: "main".into(),
        })
        .await
        .expect("open");
        migrate(&connection, MIGRATIONS).await.expect("migrate");
        let clock = Arc::new(FixedClock::new(at("2026-11-04T08:00:00Z")));
        let dyn_clock: Arc<dyn Clock> = clock.clone();
        Self {
            store: SurrealStore::new(connection, dyn_clock),
            clock,
        }
    }

    /// A workflow application over this node's store, as `authority`: a new one is a restart.
    fn app(&self, authority: &str, actions: Recorded) -> App {
        let definitions = BTreeMap::from([
            (
                ESCALATE.to_owned(),
                Arc::new(Compiled::new(chart(&[])).expect("escalate")),
            ),
            (
                REMIND.to_owned(),
                Arc::new(Compiled::new(chart(&["triage"])).expect("remind")),
            ),
        ]);
        let clock: Arc<dyn Clock> = self.clock.clone();
        Workflows::new(
            self.store.clone(),
            definitions,
            actions,
            Always,
            clock,
            vec![ActionDescriptor {
                id: "flow/escalate",
                major: 1,
                permission: "flow/escalate",
                compensation: Compensation::None,
                params: vec![],
            }],
            authority,
        )
    }

    fn advance(&self, minutes: i64) {
        self.clock.advance(SignedDuration::from_mins(minutes));
    }

    async fn timer(&self, instance: &str) -> health_workflow::TimerRecord {
        let mut tx = self.store.begin().await.expect("begin");
        tx.timers(instance)
            .await
            .expect("timers")
            .into_iter()
            .next()
            .expect("a timer")
    }

    async fn history(&self, instance: &str) -> Vec<health_workflow::HistoryEntry> {
        let mut tx = self.store.begin().await.expect("begin");
        tx.instance_history(instance).await.expect("history")
    }
}

async fn started(app: &App, definition: &str) -> String {
    match app
        .start(definition, "Encounter/e-1", "p-1", "op-start")
        .await
        .expect("start")
    {
        Outcome::Applied(record) => record.id,
        other => panic!("expected a start, got {other:?}"),
    }
}

fn one(id: &str) -> Vec<(String, String)> {
    vec![(id.to_owned(), "triage#0".to_owned())]
}

#[tokio::test]
async fn an_escalation_fires_once_when_due() {
    let node = Node::new().await;
    let actions = Recorded::default();
    let app = node.app(EDGE, actions.clone());
    let id = started(&app, ESCALATE).await;

    node.advance(30);
    assert_eq!(app.fire_due().await.expect("run"), Fired::default());

    node.advance(31);
    let report = app.fire_due().await.expect("run");
    assert_eq!(report.fired, one(&id));
    let calls = actions.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].action, "flow/escalate@1");
    assert_eq!(calls[0].actor, PRINCIPAL);

    let history = node.history(&id).await;
    let last = history.last().expect("entry");
    assert_eq!(last.event.name, "triage.overdue");
    assert_eq!(last.actor, PRINCIPAL);
    assert_eq!(last.to, ["triage"], "an escalation stays in the state");
    assert_eq!(node.timer(&id).await.status, TimerStatus::Fired);

    node.advance(60);
    assert_eq!(
        app.fire_due().await.expect("run"),
        Fired::default(),
        "a fired timer does not fire again"
    );
    assert_eq!(actions.calls().len(), 1);
}

#[tokio::test]
async fn a_node_that_is_not_the_authority_never_fires() {
    let node = Node::new().await;
    let edge = node.app(EDGE, Recorded::default());
    let id = started(&edge, ESCALATE).await;
    node.advance(61);

    let desktop_actions = Recorded::default();
    let desktop = node.app("DESKTOP-7", desktop_actions.clone());
    let report = desktop.fire_due().await.expect("run");
    assert_eq!(report.skipped, one(&id));
    assert!(report.fired.is_empty());
    assert!(desktop_actions.calls().is_empty());
    assert_eq!(node.timer(&id).await.status, TimerStatus::Pending);

    assert_eq!(edge.fire_due().await.expect("run").fired, one(&id));
}

#[tokio::test]
async fn nothing_fires_while_the_clock_reads_before_the_floor() {
    let node = Node::new().await;
    let actions = Recorded::default();
    let id = started(&node.app(EDGE, actions.clone()), ESCALATE).await;
    node.advance(61);

    let broken = node
        .app(EDGE, actions.clone())
        .with_floor(Arc::new(Floor(at("2026-11-05T00:00:00Z"))));
    assert_eq!(
        broken.fire_due().await,
        Err(WorkflowError::ClockBroken {
            local: at("2026-11-04T09:01:00Z"),
            floor: at("2026-11-05T00:00:00Z"),
        })
    );
    assert_eq!(node.timer(&id).await.status, TimerStatus::Pending);
    assert!(actions.calls().is_empty());

    let fixed = node
        .app(EDGE, actions.clone())
        .with_floor(Arc::new(Floor(at("2026-11-04T09:00:00Z"))));
    assert_eq!(fixed.fire_due().await.expect("run").fired, one(&id));
}

#[tokio::test]
async fn a_timer_fires_exactly_once_across_a_restart() {
    let node = Node::new().await;
    let id = started(&node.app(EDGE, Recorded::default()), ESCALATE).await;
    node.advance(61);

    let crashing = node.app(
        EDGE,
        Recorded {
            failing: true,
            ..Recorded::default()
        },
    );
    let report = crashing.fire_due().await.expect("run");
    assert_eq!(report.failed.len(), 1);
    assert_eq!(
        node.timer(&id).await.status,
        TimerStatus::Pending,
        "a failed firing keeps nothing, so the timer is still due"
    );
    assert_eq!(node.history(&id).await.len(), 1);

    let actions = Recorded::default();
    let restarted = node.app(EDGE, actions.clone());
    assert_eq!(restarted.fire_due().await.expect("run").fired, one(&id));
    assert_eq!(restarted.fire_due().await.expect("run"), Fired::default());
    let again = node.app(EDGE, actions.clone());
    assert_eq!(again.fire_due().await.expect("run"), Fired::default());
    assert_eq!(actions.calls().len(), 1);
    assert_eq!(node.history(&id).await.len(), 2);
}

#[tokio::test]
async fn re_entering_the_state_arms_the_timer_again() {
    let node = Node::new().await;
    let app = node.app(EDGE, Recorded::default());
    let id = started(&app, REMIND).await;
    node.advance(61);

    assert_eq!(app.fire_due().await.expect("run").fired, one(&id));
    let timer = node.timer(&id).await;
    assert_eq!(timer.status, TimerStatus::Pending);
    assert_eq!(timer.fire_at, at("2026-11-04T10:01:00Z"));
    assert_eq!(app.fire_due().await.expect("run"), Fired::default());

    node.advance(60);
    assert_eq!(app.fire_due().await.expect("run").fired, one(&id));
}
