//! Task and worklist tests (#99; ADRs 0002, 0044) over SurrealDB 3.3.5 (in-memory): claims,
//! release, reassignment, escalation, completion and cancellation, all as workflow commands,
//! with the worklists rebuilt from instance state.

use std::collections::BTreeMap;
use std::sync::Arc;

use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealStore, SurrealTx, Target, migrate,
};
use health_time::{Clock, FixedClock, SignedDuration, Timestamp};
use health_workflow::worklists::{TaskStatus, TaskView, owner_field};
use health_workflow::{
    CLAIM_TIMEOUT, DataModel, Event, Guards, HumanTask, Kind, Priority, State, Statechart,
    Transition,
};
use health_workflow_app::{
    ActionCall, ActionFailed, Actions, Compiled, Outcome, WorkflowError, Workflows,
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

fn human(role: &str, worklist: &str) -> HumanTask {
    HumanTask {
        role: role.into(),
        form: Some("https://health.go.ug/fhir/Questionnaire/triage|2.0.0".into()),
        worklist: worklist.into(),
        priority: Priority::Routine,
        claim_timeout: CLAIM_TIMEOUT,
    }
}

/// Triage (a nurse's task) → consultation (a clinician's task) → done.
fn opd() -> Statechart {
    let mut triage = State::new("triage", Kind::Atomic);
    triage.task = Some(human("nurse", "opd-triage"));
    triage.transitions = vec![on("triaged", "consultation")];
    let mut consultation = State::new("consultation", Kind::Atomic);
    consultation.task = Some(human("clinician", "opd-consult"));
    consultation.transitions = vec![on("seen", "done")];
    Statechart {
        states: vec![triage, consultation, State::new("done", Kind::Final)],
        initial: vec![],
    }
}

struct Always;

impl Guards for Always {
    fn holds(&self, _: &str, _: Option<&Event>, _: &DataModel) -> bool {
        true
    }
}

struct NoActions;

impl Actions<SurrealTx> for NoActions {
    async fn execute(&self, _: &mut SurrealTx, _: &ActionCall) -> Result<(), ActionFailed> {
        Ok(())
    }
}

type App = Workflows<SurrealStore, BTreeMap<String, Arc<Compiled>>, NoActions, Always>;

async fn app() -> (App, Arc<FixedClock>) {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "workflow-tasks".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let clock = Arc::new(FixedClock::new(
        "2026-11-04T08:00:00Z".parse::<Timestamp>().expect("t"),
    ));
    let dyn_clock: Arc<dyn Clock> = clock.clone();
    let store = SurrealStore::new(connection, dyn_clock.clone());
    let definitions =
        BTreeMap::from([(OPD.to_owned(), Arc::new(Compiled::new(opd()).expect("opd")))]);
    let app = Workflows::new(
        store,
        definitions,
        NoActions,
        Always,
        dyn_clock,
        vec![],
        "KAM013",
    );
    (app, clock)
}

async fn start(app: &App, op: &str) -> String {
    match app
        .start(OPD, &format!("Encounter/{op}"), "reception-1", op)
        .await
        .expect("start")
    {
        Outcome::Applied(record) => record.id,
        other => panic!("expected a start, got {other:?}"),
    }
}

fn summary(tasks: &[TaskView]) -> Vec<(&str, &str, Option<&str>)> {
    tasks
        .iter()
        .map(|t| (t.state.as_str(), t.status.code(), t.owner.as_deref()))
        .collect()
}

#[tokio::test]
async fn a_claim_puts_the_task_in_progress_until_it_lapses_when_idle() {
    let (app, clock) = app().await;
    let id = start(&app, "op-1").await;
    assert_eq!(
        summary(&app.worklist("opd-triage").await.expect("worklist")),
        [("triage", "ready", None)]
    );

    app.claim(&id, "triage", "nurse-1", "op-2")
        .await
        .expect("claim");
    assert_eq!(
        summary(&app.worklist("opd-triage").await.expect("worklist")),
        [("triage", "in-progress", Some("nurse-1"))]
    );
    assert_eq!(
        app.claim(&id, "triage", "nurse-2", "op-3").await,
        Err(WorkflowError::Claimed("nurse-1".into()))
    );
    assert_eq!(
        app.claim(&id, "triage", "nurse-1", "op-2")
            .await
            .expect("replay"),
        Outcome::Replayed
    );

    clock.advance(SignedDuration::from_mins(10));
    app.claim(&id, "triage", "nurse-1", "op-4")
        .await
        .expect("worked on again: the claim is refreshed");
    clock.advance(SignedDuration::from_mins(14));
    assert_eq!(
        summary(&app.worklist("opd-triage").await.expect("worklist")),
        [("triage", "in-progress", Some("nurse-1"))]
    );

    clock.advance(SignedDuration::from_mins(1));
    assert_eq!(
        summary(&app.worklist("opd-triage").await.expect("worklist")),
        [("triage", "ready", None)],
        "idle for 15 minutes, the claim has lapsed"
    );
    app.claim(&id, "triage", "nurse-2", "op-5")
        .await
        .expect("anyone may claim a lapsed task");
}

#[tokio::test]
async fn only_the_holder_releases_and_a_reassignment_moves_the_claim() {
    let (app, _) = app().await;
    let id = start(&app, "op-1").await;
    app.claim(&id, "triage", "nurse-1", "op-2")
        .await
        .expect("claim");
    assert_eq!(
        app.release(&id, "triage", "nurse-2", "op-3").await,
        Err(WorkflowError::NotOwner("nurse-2".into()))
    );

    let reassigned = app
        .reassign(&id, "triage", "nurse-3", "charge-nurse", "op-4")
        .await
        .expect("reassign");
    let Outcome::Applied(record) = reassigned else {
        panic!("expected a reassignment");
    };
    assert_eq!(
        summary(&app.tasks(&id).await.expect("tasks")),
        [("triage", "in-progress", Some("nurse-3"))]
    );

    app.release(&id, "triage", "nurse-3", "op-5")
        .await
        .expect("release");
    assert_eq!(
        summary(&app.worklist("opd-triage").await.expect("worklist")),
        [("triage", "ready", None)]
    );
    assert!(record.version > 1);
}

#[tokio::test]
async fn escalation_moves_a_task_up_the_worklist_and_only_ever_raises_priority() {
    let (app, clock) = app().await;
    let first = start(&app, "op-1").await;
    clock.advance(SignedDuration::from_mins(5));
    let second = start(&app, "op-2").await;
    let order = |tasks: Vec<TaskView>| tasks.into_iter().map(|t| t.instance).collect::<Vec<_>>();
    assert_eq!(
        order(app.worklist("opd-triage").await.expect("worklist")),
        [first.clone(), second.clone()],
        "by arrival"
    );

    app.escalate(&second, "triage", Priority::Stat, "triage-lead", "op-3")
        .await
        .expect("escalate");
    let worklist = app.worklist("opd-triage").await.expect("worklist");
    assert_eq!(order(worklist.clone()), [second.clone(), first]);
    assert_eq!(worklist[0].priority, Priority::Stat);
    assert_eq!(
        app.escalate(&second, "triage", Priority::Urgent, "triage-lead", "op-4")
            .await,
        Err(WorkflowError::NotEscalation("urgent".into()))
    );
}

#[tokio::test]
async fn completing_a_task_moves_the_work_to_the_next_worklist_and_drops_the_claim() {
    let (app, _) = app().await;
    let id = start(&app, "op-1").await;
    app.claim(&id, "triage", "nurse-1", "op-2")
        .await
        .expect("claim");
    let Outcome::Applied(record) = app
        .command(&id, &Event::named("triaged"), "nurse-1", "op-3")
        .await
        .expect("complete triage")
    else {
        panic!("expected a transition");
    };
    assert!(
        !record.state.data.contains_key(&owner_field("triage")),
        "the claim of a state left is dropped"
    );
    assert!(
        app.worklist("opd-triage")
            .await
            .expect("worklist")
            .is_empty()
    );
    assert_eq!(
        summary(&app.worklist("opd-consult").await.expect("worklist")),
        [("consultation", "ready", None)]
    );
    assert_eq!(
        summary(&app.tasks(&id).await.expect("tasks")),
        [
            ("triage", "completed", None),
            ("consultation", "ready", None)
        ]
    );
    assert_eq!(
        app.claim(&id, "triage", "nurse-1", "op-4").await,
        Err(WorkflowError::NoTask("triage".into())),
        "a state no longer active has no open task"
    );
}

#[tokio::test]
async fn cancelling_an_instance_cancels_its_open_tasks() {
    let (app, _) = app().await;
    let id = start(&app, "op-1").await;
    app.cancel(&id, "patient left", "reception-1", "op-2")
        .await
        .expect("cancel");
    assert_eq!(
        app.tasks(&id).await.expect("tasks")[0].status,
        TaskStatus::Cancelled
    );
    assert!(
        app.worklist("opd-triage")
            .await
            .expect("worklist")
            .is_empty()
    );
    assert_eq!(
        app.claim(&id, "triage", "nurse-1", "op-3").await,
        Err(WorkflowError::Ended(id))
    );
}

#[tokio::test]
async fn the_stored_worklist_matches_a_rebuild_from_instance_state() {
    let (app, clock) = app().await;
    let id = start(&app, "op-1").await;
    app.claim(&id, "triage", "nurse-1", "op-2")
        .await
        .expect("claim");
    clock.advance(SignedDuration::from_mins(3));
    let rebuilt: Vec<TaskView> = app
        .tasks(&id)
        .await
        .expect("tasks")
        .into_iter()
        .filter(|t| t.status.is_open())
        .collect();
    assert_eq!(app.worklist("opd-triage").await.expect("worklist"), rebuilt);
}
