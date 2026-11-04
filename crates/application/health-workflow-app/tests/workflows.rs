//! Workflow tests (#97; ADR 0044) over SurrealDB 3.3.5 (in-memory): commands in one
//! transaction with their actions, domain events through the inbox, history, timers and
//! compensation.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use health_persistence::Store;
use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealStore, SurrealTx, Target, migrate,
};
use health_time::{Clock, FixedClock, SignedDuration, Timestamp};
use health_workflow::{
    Action, DataModel, Effect, Event, Guards, Invoke, Kind, State, Statechart, Timer, TimerStatus,
    Transition, Value, WorkflowStore,
};
use health_workflow_actions::{ActionDescriptor, Compensation, ParamKind, ParamSpec};
use health_workflow_app::{
    ActionCall, ActionFailed, Actions, CANCELLED, Compiled, Outcome, PRINCIPAL, WorkflowError,
    Workflows, action_key, instance_id,
};

const OPD: &str = "urn:opd|1.0.0";
const NESTED: &str = "urn:nested|1.0.0";

fn at() -> Timestamp {
    "2026-11-04T08:00:00Z".parse().expect("timestamp")
}

fn execute(action: &str, params: &[(&str, &str)]) -> Action {
    Action::Execute {
        action: action.into(),
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), Value::Text((*v).to_owned())))
            .collect(),
    }
}

fn on(event: &str, target: &str, actions: Vec<Action>) -> Transition {
    Transition {
        events: vec![event.into()],
        guard: None,
        targets: vec![target.into()],
        internal: false,
        actions,
    }
}

/// Triage (opens a queue entry, an hour's timer) → consultation (orders a test) → done.
fn opd() -> Statechart {
    let mut triage = State::new("triage", Kind::Atomic);
    triage.on_entry = vec![vec![execute("flow/open-queue@1", &[("queue", "triage")])]];
    triage.timers = vec![Timer {
        after: SignedDuration::from_hours(1),
        event: "triage.overdue".into(),
    }];
    triage.transitions = vec![on(
        "triage.completed",
        "consultation",
        vec![execute("flow/notify@1", &[])],
    )];
    let mut consultation = State::new("consultation", Kind::Atomic);
    consultation.on_entry = vec![vec![execute("lab/order@1", &[])]];
    consultation.transitions = vec![
        on("lab.resulted", "done", vec![]),
        on(
            "fail",
            "done",
            vec![execute("flow/notify@1", &[]), execute("flow/broken@1", &[])],
        ),
    ];
    Statechart {
        states: vec![triage, consultation, State::new("done", Kind::Final)],
        initial: vec![],
    }
}

fn nested() -> Statechart {
    let mut waiting = State::new("waiting", Kind::Atomic);
    waiting.invoke = vec![Invoke {
        id: "sub".into(),
        src: OPD.into(),
    }];
    waiting.transitions = vec![on("done.invoke.sub", "done", vec![])];
    Statechart {
        states: vec![waiting, State::new("done", Kind::Final)],
        initial: vec![],
    }
}

fn descriptor(id: &'static str, compensation: Compensation) -> ActionDescriptor {
    ActionDescriptor {
        id,
        major: 1,
        permission: "workflow/act",
        compensation,
        params: vec![ParamSpec {
            name: "queue",
            kind: ParamKind::Text,
            required: false,
        }],
    }
}

fn catalog() -> Vec<ActionDescriptor> {
    vec![
        descriptor("flow/open-queue", Compensation::Action("flow/close-queue")),
        descriptor("flow/close-queue", Compensation::None),
        descriptor("flow/notify", Compensation::None),
        descriptor("lab/order", Compensation::HumanTask),
        descriptor("flow/broken", Compensation::None),
    ]
}

struct Always;

impl Guards for Always {
    fn holds(&self, _: &str, _: Option<&Event>, _: &DataModel) -> bool {
        true
    }
}

/// Records each call, and writes a marker through the workflow's transaction, so a test can see
/// that an action's writes commit or roll back with the transition. `flow/broken` fails.
#[derive(Clone, Default)]
struct Recorded(Arc<Mutex<Vec<ActionCall>>>);

impl Recorded {
    fn calls(&self) -> Vec<String> {
        self.0
            .lock()
            .expect("calls")
            .iter()
            .map(|c| c.action.clone())
            .collect()
    }

    fn all(&self) -> Vec<ActionCall> {
        self.0.lock().expect("calls").clone()
    }
}

impl Actions<SurrealTx> for Recorded {
    async fn execute(&self, tx: &mut SurrealTx, call: &ActionCall) -> Result<(), ActionFailed> {
        self.0.lock().expect("calls").push(call.clone());
        if call.action == "flow/broken@1" {
            return Err(ActionFailed("the printer is out of paper".into()));
        }
        tx.claim_event(&format!("effect|{}", call.key), at())
            .await
            .map_err(|e| ActionFailed(e.to_string()))?;
        Ok(())
    }
}

type App = Workflows<SurrealStore, BTreeMap<String, Arc<Compiled>>, Recorded, Always>;

async fn app() -> (App, SurrealStore, Recorded) {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "workflow-app".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(at()));
    let store = SurrealStore::new(connection, clock.clone());
    let definitions = BTreeMap::from([
        (OPD.to_owned(), Arc::new(Compiled::new(opd()).expect("opd"))),
        (
            NESTED.to_owned(),
            Arc::new(Compiled::new(nested()).expect("nested")),
        ),
    ]);
    let actions = Recorded::default();
    let app = Workflows::new(
        store.clone(),
        definitions,
        actions.clone(),
        Always,
        clock,
        catalog(),
        "KAM013",
    );
    (app, store, actions)
}

fn applied(outcome: Outcome) -> health_workflow::InstanceRecord {
    match outcome {
        Outcome::Applied(record) => record,
        other => panic!("expected Applied, got {other:?}"),
    }
}

/// Whether the action with `key` committed its marker.
async fn committed(store: &SurrealStore, key: uuid::Uuid) -> bool {
    let mut tx = store.begin().await.expect("begin");
    !tx.claim_event(&format!("effect|{key}"), at())
        .await
        .expect("claim")
}

#[tokio::test]
async fn a_start_runs_its_entry_actions_and_timers_in_one_transaction() {
    let (app, store, actions) = app().await;
    let record = applied(
        app.start(OPD, "Encounter/e-1", "Practitioner/p-1", "op-start")
            .await
            .expect("start"),
    );
    assert_eq!(record.id, instance_id("op-start"));
    assert_eq!(record.state.configuration, ["triage"]);
    assert_eq!(record.version, 1);
    assert_eq!(record.authority, "KAM013");

    let calls = actions.all();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].action, "flow/open-queue@1");
    assert_eq!(calls[0].actor, "Practitioner/p-1");
    assert_eq!(
        calls[0].params.get("queue"),
        Some(&Value::Text("triage".into()))
    );
    assert!(committed(&store, calls[0].key).await);

    let mut tx = store.begin().await.expect("begin");
    let history = tx.instance_history(&record.id).await.expect("history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].seq, 1);
    assert!(history[0].from.is_empty());
    assert_eq!(history[0].to, ["triage"]);
    assert_eq!(history[0].op_id, "op-start");
    let timers = tx.timers(&record.id).await.expect("timers");
    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].event, "triage.overdue");
    assert_eq!(
        timers[0].fire_at,
        "2026-11-04T09:00:00Z".parse::<Timestamp>().expect("t")
    );
    assert_eq!(timers[0].status, TimerStatus::Pending);
    drop(tx);

    assert_eq!(
        app.start(OPD, "Encounter/e-1", "Practitioner/p-1", "op-start")
            .await
            .expect("replay"),
        Outcome::Replayed
    );
    assert_eq!(actions.calls().len(), 1, "a replayed start does nothing");
}

#[tokio::test]
async fn a_command_moves_the_instance_appends_history_and_replays_as_a_no_op() {
    let (app, store, actions) = app().await;
    let id = applied(
        app.start(OPD, "Encounter/e-1", "p-1", "op-1")
            .await
            .expect("start"),
    )
    .id;

    let record = applied(
        app.command(&id, &Event::named("triage.completed"), "p-2", "op-2")
            .await
            .expect("command"),
    );
    assert_eq!(record.state.configuration, ["consultation"]);
    assert_eq!(record.version, 2);
    assert_eq!(
        actions.calls(),
        ["flow/open-queue@1", "flow/notify@1", "lab/order@1"]
    );

    let mut tx = store.begin().await.expect("begin");
    let history = tx.instance_history(&id).await.expect("history");
    assert_eq!(history.len(), 2);
    assert_eq!(history[1].from, ["triage"]);
    assert_eq!(history[1].to, ["consultation"]);
    assert_eq!(history[1].actor, "p-2");
    assert!(history[1].effects.contains(&Effect::CancelTimer {
        id: "triage#0".into()
    }));
    assert_eq!(
        tx.timers(&id).await.expect("timers")[0].status,
        TimerStatus::Cancelled,
        "leaving triage cancels its timer"
    );
    drop(tx);

    assert_eq!(
        app.command(&id, &Event::named("triage.completed"), "p-2", "op-2")
            .await
            .expect("replay"),
        Outcome::Replayed
    );
    assert_eq!(actions.calls().len(), 3);

    assert_eq!(
        app.command(&id, &Event::named("triage.completed"), "p-2", "op-3")
            .await,
        Err(WorkflowError::NotEnabled("triage.completed".into())),
        "a command the current state does not take is refused"
    );
    assert_eq!(
        app.command("missing", &Event::named("x"), "p-2", "op-4")
            .await,
        Err(WorkflowError::NotFound("missing".into()))
    );
}

#[tokio::test]
async fn replaying_a_domain_event_is_a_no_op() {
    let (app, store, actions) = app().await;
    let id = applied(
        app.start(OPD, "Encounter/e-1", "p-1", "op-1")
            .await
            .expect("start"),
    )
    .id;
    app.command(&id, &Event::named("triage.completed"), "p-1", "op-2")
        .await
        .expect("command");

    let resulted = Event::named("lab.resulted");
    let record = applied(app.deliver("evt-1", &id, &resulted).await.expect("deliver"));
    assert!(!record.state.running);
    assert_eq!(record.state.configuration, ["done"]);

    let calls = actions.calls().len();
    assert_eq!(
        app.deliver("evt-1", &id, &resulted).await.expect("replay"),
        Outcome::Replayed
    );
    assert_eq!(actions.calls().len(), calls);
    let mut tx = store.begin().await.expect("begin");
    let history = tx.instance_history(&id).await.expect("history");
    assert_eq!(history.len(), 3, "the replay appended nothing");
    assert_eq!(history[2].actor, PRINCIPAL);
    assert_eq!(history[2].op_id, "evt-1");
    assert_eq!(
        tx.instance(&id).await.expect("instance").expect("exists"),
        record
    );
    drop(tx);

    assert_eq!(
        app.deliver("evt-2", &id, &resulted).await.expect("ended"),
        Outcome::Ignored,
        "an event for an ended instance is consumed and does nothing"
    );
    assert_eq!(
        app.deliver("evt-2", &id, &resulted).await.expect("replay"),
        Outcome::Replayed
    );
}

#[tokio::test]
async fn a_failed_action_keeps_nothing_of_the_transition() {
    let (app, store, actions) = app().await;
    let id = applied(
        app.start(OPD, "Encounter/e-1", "p-1", "op-1")
            .await
            .expect("start"),
    )
    .id;
    let before = applied(
        app.command(&id, &Event::named("triage.completed"), "p-1", "op-2")
            .await
            .expect("command"),
    );

    let failed = app.command(&id, &Event::named("fail"), "p-1", "op-3").await;
    assert_eq!(
        failed,
        Err(WorkflowError::Action {
            action: "flow/broken@1".into(),
            reason: "the printer is out of paper".into(),
        })
    );
    let notify = actions
        .all()
        .into_iter()
        .rev()
        .find(|c| c.action == "flow/notify@1")
        .expect("notify ran");
    assert!(
        !committed(&store, notify.key).await,
        "the earlier action's write rolled back with the transition"
    );

    let mut tx = store.begin().await.expect("begin");
    assert_eq!(
        tx.instance(&id).await.expect("instance").expect("exists"),
        before
    );
    assert_eq!(tx.instance_history(&id).await.expect("history").len(), 2);
    drop(tx);

    assert!(
        matches!(
            app.command(&id, &Event::named("fail"), "p-1", "op-3").await,
            Err(WorkflowError::Action { .. })
        ),
        "the failed command's op_id was not kept, so it can be retried"
    );
}

#[tokio::test]
async fn cancelling_compensates_completed_actions_in_reverse() {
    let (app, store, actions) = app().await;
    let id = applied(
        app.start(OPD, "Encounter/e-1", "p-1", "op-1")
            .await
            .expect("start"),
    )
    .id;
    app.command(&id, &Event::named("triage.completed"), "p-1", "op-2")
        .await
        .expect("command");

    let record = applied(
        app.cancel(&id, "patient left", "p-3", "op-3")
            .await
            .expect("cancel"),
    );
    assert!(!record.state.running);
    assert!(record.state.configuration.is_empty());

    let calls = actions.all();
    let undo = calls.last().expect("a compensation");
    assert_eq!(undo.action, "flow/close-queue@1");
    assert_eq!(
        undo.params.get("queue"),
        Some(&Value::Text("triage".into())),
        "a compensating action gets the parameters of the action it undoes"
    );
    assert_eq!(undo.key, action_key(&id, record.state.sequence, 0));

    let mut tx = store.begin().await.expect("begin");
    let history = tx.instance_history(&id).await.expect("history");
    let last = history.last().expect("entry");
    assert_eq!(last.event.name, CANCELLED);
    assert_eq!(
        last.event.data.get("reason"),
        Some(&Value::Text("patient left".into()))
    );
    assert_eq!(
        last.event.data.get("human-task.0"),
        Some(&Value::Text("lab/order@1".into())),
        "the newest action is compensated first"
    );
    assert_eq!(
        last.event.data.get("not-undone.0"),
        Some(&Value::Text("flow/notify@1".into()))
    );
    assert_eq!(last.from, ["consultation"]);
    assert!(last.to.is_empty());
    drop(tx);

    assert_eq!(
        app.command(&id, &Event::named("lab.resulted"), "p-1", "op-4")
            .await,
        Err(WorkflowError::Ended(id.clone()))
    );
    assert_eq!(
        app.cancel(&id, "again", "p-3", "op-5").await,
        Err(WorkflowError::Ended(id))
    );
}

#[tokio::test]
async fn what_the_engine_cannot_do_yet_fails_closed() {
    let (app, store, actions) = app().await;
    assert_eq!(
        app.start(NESTED, "Encounter/e-1", "p-1", "op-1").await,
        Err(WorkflowError::Unsupported("sub-workflow invocations"))
    );
    let mut tx = store.begin().await.expect("begin");
    assert_eq!(tx.instance(&instance_id("op-1")).await.expect("read"), None);
    drop(tx);
    assert_eq!(
        app.start("urn:unknown|1", "Encounter/e-1", "p-1", "op-2")
            .await,
        Err(WorkflowError::UnknownDefinition("urn:unknown|1".into()))
    );
    assert!(actions.calls().is_empty());
}
