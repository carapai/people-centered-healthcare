//! Financial Gates in workflows (#122; ADR 0033) over SurrealDB 3.3.5 (in-memory): a gate state
//! asks Finance on entry, urgent triage always defers, a block waits with the cashier's task
//! until a payment event rechecks it, and an unanswered gate defers.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use health_billing_contract::{FinancialGates, GateOutcome, GateQuery, GateUnavailable, Urgency};
use health_persistence::Store;
use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealStore, SurrealTx, Target, migrate,
};
use health_time::{Clock, SystemClock};
use health_workflow::definition::WorkflowDefinition;
use health_workflow::worklists::TaskView;
use health_workflow::{DataModel, Effect, Event, Guards, WorkflowStore};
use health_workflow_app::{
    ActionCall, ActionFailed, Actions, Compiled, Outcome, WorkflowError, Workflows,
};

const LAB: &str = "https://health.go.ug/fhir/Library/lab|1.0.0";
const GATE: &str = "https://health.go.ug/fhir/FinancialGate/before-lab";

/// Ordered (triage sets the urgency) → payment (the gate, the cashier's task) → collect → done.
fn lab() -> Compiled {
    let definition: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "url": "https://health.go.ug/fhir/Library/lab", "version": "1.0.0",
        "dataModel": [{ "name": "urgency", "kind": "text" }],
        "statechart": { "states": [
            { "id": "ordered", "kind": "atomic", "transitions": [
                { "events": ["sent"], "targets": ["payment"] },
                { "events": ["sent-urgent"], "targets": ["payment"],
                  "actions": [{ "kind": "assign", "field": "urgency", "value": "urgent" }] }
            ] },
            { "id": "payment", "kind": "atomic", "gate": GATE,
              "task": { "role": "cashier", "worklist": "cashier" },
              "transitions": [
                { "events": ["gate.pass"], "targets": ["collect"] },
                { "events": ["gate.defer"], "targets": ["collect"] }
              ] },
            { "id": "collect", "kind": "atomic",
              "transitions": [{ "events": ["collected"], "targets": ["done"] }] },
            { "id": "done", "kind": "final" }
        ] }
    }))
    .expect("a definition");
    assert_eq!(definition.problems(&[]), vec![]);
    Compiled::new(definition.statechart).expect("compiled")
}

/// Finance as configured: blocks routine care while `paid` is false, defers urgent care, and
/// fails when `down`.
#[derive(Default)]
struct Finance {
    paid: Mutex<bool>,
    down: Mutex<bool>,
    asked: Mutex<Vec<GateQuery>>,
}

impl FinancialGates for Finance {
    async fn evaluate_gate(&self, query: &GateQuery) -> Result<GateOutcome, GateUnavailable> {
        self.asked.lock().expect("asked").push(query.clone());
        if *self.down.lock().expect("down") {
            return Err(GateUnavailable("the billing store is unreachable".into()));
        }
        Ok(if *self.paid.lock().expect("paid") {
            GateOutcome::Pass("paid".into())
        } else if query.urgency.never_blocks() {
            GateOutcome::Defer("urgent care is never blocked".into())
        } else {
            GateOutcome::Block("20000 is owed".into())
        })
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

async fn app(finance: Option<Arc<Finance>>) -> (App, SurrealStore) {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "workflow-gates".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let store = SurrealStore::new(connection, clock.clone());
    let app = Workflows::new(
        store.clone(),
        BTreeMap::from([(LAB.to_owned(), Arc::new(lab()))]),
        NoActions,
        Always,
        clock,
        vec![],
        "KAM013",
    );
    let app = match finance {
        Some(finance) => app.with_gates(finance),
        None => app,
    };
    (app, store)
}

async fn started(app: &App) -> String {
    match app
        .start(LAB, "Encounter/e-1", "clinician-1", "op-start")
        .await
        .expect("start")
    {
        Outcome::Applied(record) => record.id,
        other => panic!("expected a start, got {other:?}"),
    }
}

async fn sent(app: &App, id: &str, event: &str) -> Vec<String> {
    match app
        .command(id, &Event::named(event), "lab-1", "op-sent")
        .await
        .expect("sent")
    {
        Outcome::Applied(record) => record.state.configuration,
        other => panic!("expected a transition, got {other:?}"),
    }
}

fn open(tasks: &[TaskView]) -> Vec<&str> {
    tasks
        .iter()
        .filter(|t| t.status.is_open())
        .map(|t| t.state.as_str())
        .collect()
}

#[tokio::test]
async fn a_paid_gate_passes_in_the_same_step_and_its_answer_is_in_history() {
    let finance = Arc::new(Finance::default());
    *finance.paid.lock().expect("paid") = true;
    let (app, store) = app(Some(finance.clone())).await;
    let id = started(&app).await;
    assert_eq!(sent(&app, &id, "sent").await, ["collect"]);
    assert!(
        open(&app.tasks(&id).await.expect("tasks")).is_empty(),
        "a gate that passes is never seen waiting"
    );
    let asked = finance.asked.lock().expect("asked").clone();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].gate, GATE);
    assert_eq!(asked[0].account, "Encounter/e-1");
    assert_eq!(asked[0].urgency, Urgency::Routine);

    let mut tx = store.begin().await.expect("begin");
    let history = tx.instance_history(&id).await.expect("history");
    assert!(
        history
            .last()
            .expect("entry")
            .effects
            .contains(&Effect::GateEvaluated {
                state: "payment".into(),
                gate: GATE.into(),
                outcome: "pass".into(),
                reason: "paid".into(),
            })
    );
}

#[tokio::test]
async fn urgent_triage_always_defers_unpaid() {
    let finance = Arc::new(Finance::default());
    let (app, _) = app(Some(finance.clone())).await;
    let id = started(&app).await;
    assert_eq!(
        sent(&app, &id, "sent-urgent").await,
        ["collect"],
        "urgent care goes on, unpaid"
    );
    assert_eq!(
        finance.asked.lock().expect("asked")[0].urgency,
        Urgency::Urgent
    );
}

#[tokio::test]
async fn a_block_waits_with_the_cashiers_task_until_a_payment_event_rechecks_it() {
    let finance = Arc::new(Finance::default());
    let (app, _) = app(Some(finance.clone())).await;
    let id = started(&app).await;
    assert_eq!(sent(&app, &id, "sent").await, ["payment"]);
    assert_eq!(open(&app.tasks(&id).await.expect("tasks")), ["payment"]);
    assert_eq!(
        app.worklist("cashier").await.expect("worklist").len(),
        1,
        "the cashier's Task"
    );

    assert_eq!(
        app.recheck_gates("payment-1", &id).await,
        Ok(Outcome::Ignored),
        "still unpaid: it keeps waiting"
    );
    *finance.paid.lock().expect("paid") = true;
    let Ok(Outcome::Applied(record)) = app.recheck_gates("payment-2", &id).await else {
        panic!("expected the gate to pass");
    };
    assert_eq!(record.state.configuration, ["collect"]);
    assert!(app.worklist("cashier").await.expect("worklist").is_empty());
    assert_eq!(
        app.recheck_gates("payment-2", &id).await,
        Ok(Outcome::Replayed)
    );
    assert!(matches!(
        app.recheck_gates("payment-3", "missing").await,
        Err(WorkflowError::NotFound(_))
    ));
}

#[tokio::test]
async fn an_unanswered_gate_defers_and_never_blocks() {
    let finance = Arc::new(Finance::default());
    *finance.down.lock().expect("down") = true;
    let (app, _) = app(Some(finance)).await;
    let id = started(&app).await;
    assert_eq!(sent(&app, &id, "sent").await, ["collect"]);

    let (without, _) = app_without_finance().await;
    let id = started(&without).await;
    assert_eq!(
        sent(&without, &id, "sent").await,
        ["collect"],
        "a node with no gate port defers"
    );
}

async fn app_without_finance() -> (App, SurrealStore) {
    app(None).await
}
