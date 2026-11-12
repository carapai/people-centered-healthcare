//! Workflow transitions made away from the authority (#100; workflow-engine.md § Offline and
//! sync): a desktop's replica makes them offline, and the authority applies the ones that still
//! apply from where the instance now stands, once, and turns the rest into conflicts.

use std::collections::BTreeMap;
use std::sync::Arc;

use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealStore, SurrealTx, Target, migrate,
};
use health_time::{Clock, FixedClock, Timestamp};
use health_workflow::{
    DataModel, Event, Guards, InstanceRecord, Kind, State, Statechart, Transition,
};
use health_workflow_app::{
    ActionCall, ActionFailed, Actions, Candidate, Compiled, Judged, Outcome, Workflows,
};

const OPD: &str = "urn:opd|1.0.0";
const BOTH: &str = "urn:both|1.0.0";

fn on(event: &str, target: &str) -> Transition {
    Transition {
        events: vec![event.into()],
        guard: None,
        targets: vec![target.into()],
        internal: false,
        actions: vec![],
    }
}

fn atomic(id: &str, transitions: Vec<Transition>) -> State {
    let mut state = State::new(id, Kind::Atomic);
    state.transitions = transitions;
    state
}

/// Triage → consultation → done, or triage → referred. Completing triage is guarded.
fn opd() -> Statechart {
    let mut completed = on("triage.completed", "consultation");
    completed.guard = Some("triage.ready".into());
    Statechart {
        states: vec![
            atomic("triage", vec![completed, on("triage.referred", "referred")]),
            atomic("referred", vec![]),
            atomic("consultation", vec![on("consultation.completed", "done")]),
            State::new("done", Kind::Final),
        ],
        initial: vec![],
    }
}

/// The visit and its bill, side by side.
fn both() -> Statechart {
    let mut visit = State::new("visit", Kind::Compound);
    visit.children = vec![
        atomic("triage", vec![on("triage.completed", "consultation")]),
        atomic("consultation", vec![]),
    ];
    let mut bill = State::new("bill", Kind::Compound);
    bill.children = vec![
        atomic("open", vec![on("bill.paid", "paid")]),
        atomic("paid", vec![]),
    ];
    let mut parallel = State::new("both", Kind::Parallel);
    parallel.children = vec![visit, bill];
    Statechart {
        states: vec![parallel],
        initial: vec![],
    }
}

struct Always;

impl Guards for Always {
    fn holds(&self, _: &str, _: Option<&Event>, _: &DataModel) -> bool {
        true
    }
}

struct Never;

impl Guards for Never {
    fn holds(&self, _: &str, _: Option<&Event>, _: &DataModel) -> bool {
        false
    }
}

struct Nothing;

impl Actions<SurrealTx> for Nothing {
    async fn execute(&self, _: &mut SurrealTx, _: &ActionCall) -> Result<(), ActionFailed> {
        Ok(())
    }
}

type App<G> = Workflows<SurrealStore, BTreeMap<String, Arc<Compiled>>, Nothing, G>;

async fn app<G: Guards + Send + Sync>(node: &str, guards: G) -> App<G> {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "candidates".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(
        "2026-11-12T08:00:00Z".parse::<Timestamp>().expect("now"),
    ));
    let definitions = BTreeMap::from([
        (OPD.to_owned(), Arc::new(Compiled::new(opd()).expect("opd"))),
        (
            BOTH.to_owned(),
            Arc::new(Compiled::new(both()).expect("both")),
        ),
    ]);
    Workflows::new(
        SurrealStore::new(connection, clock.clone()),
        definitions,
        Nothing,
        guards,
        clock,
        Vec::new(),
        node,
    )
}

fn applied(outcome: Outcome) -> InstanceRecord {
    match outcome {
        Outcome::Applied(record) => record,
        other => panic!("expected Applied, got {other:?}"),
    }
}

/// The edge (the authority) and a desktop holding a replica of the same instance.
async fn started<G: Guards + Send + Sync>(
    definition: &str,
    guards: G,
) -> (App<G>, App<Always>, InstanceRecord) {
    let edge = app("edge", guards).await;
    let desktop = app("desktop", Always).await;
    let record = applied(
        edge.start(definition, "Encounter/e-1", "p-1", "op-start")
            .await
            .expect("start"),
    );
    applied(
        desktop
            .start(definition, "Encounter/e-1", "p-1", "op-start")
            .await
            .expect("replica"),
    );
    (edge, desktop, record)
}

/// The desktop moves its replica with `event`: the candidate it sends.
async fn offline(
    desktop: &App<Always>,
    before: &InstanceRecord,
    event: &str,
    op: &str,
) -> Candidate {
    let event = Event::named(event);
    let after = applied(
        desktop
            .command(&before.id, &event, "nurse", op)
            .await
            .expect("offline"),
    );
    Candidate::of(before, &after, event, "nurse", op, "desktop")
}

#[tokio::test]
async fn a_candidate_from_the_current_version_applies_once() {
    let (edge, desktop, record) = started(OPD, Always).await;
    let candidate = offline(&desktop, &record, "triage.completed", "op-1").await;
    let Judged::Applied(next) = edge.apply_candidate(&candidate).await.expect("judge") else {
        panic!("not applied");
    };
    assert_eq!(next.state.configuration, ["consultation"]);
    assert_eq!(next.version, 2);
    assert_eq!(
        edge.apply_candidate(&candidate).await.expect("replay"),
        Judged::Replayed
    );
}

#[tokio::test]
async fn a_candidate_from_a_state_the_instance_has_left_is_a_conflict() {
    let (edge, desktop, record) = started(OPD, Always).await;
    // At the edge, the patient was referred; offline, triage was completed.
    applied(
        edge.command(
            &record.id,
            &Event::named("triage.referred"),
            "doctor",
            "op-edge",
        )
        .await
        .expect("edge"),
    );
    let candidate = offline(&desktop, &record, "triage.completed", "op-1").await;
    let judged = edge.apply_candidate(&candidate).await.expect("judge");
    assert!(
        matches!(&judged, Judged::Conflict(why) if why.contains("no longer in triage")),
        "{judged:?}"
    );
}

#[tokio::test]
async fn moves_in_parallel_regions_commute() {
    let (edge, desktop, record) = started(BOTH, Always).await;
    // At the edge, the bill was paid; offline, triage was completed.
    applied(
        edge.command(&record.id, &Event::named("bill.paid"), "cashier", "op-edge")
            .await
            .expect("edge"),
    );
    let candidate = offline(&desktop, &record, "triage.completed", "op-1").await;
    assert_eq!(candidate.left, ["triage"]);
    let Judged::Applied(next) = edge.apply_candidate(&candidate).await.expect("judge") else {
        panic!("not applied");
    };
    for state in ["consultation", "paid"] {
        assert!(
            next.state.configuration.contains(&state.to_owned()),
            "{:?}",
            next.state.configuration
        );
    }
}

#[tokio::test]
async fn the_authority_re_evaluates_the_guards() {
    // Offline, triage looked ready; at the edge, its guard does not hold.
    let (edge, desktop, record) = started(OPD, Never).await;
    let candidate = offline(&desktop, &record, "triage.completed", "op-1").await;
    let judged = edge.apply_candidate(&candidate).await.expect("judge");
    assert!(
        matches!(&judged, Judged::Conflict(why) if why.contains("not enabled at the authority")),
        "{judged:?}"
    );
}
