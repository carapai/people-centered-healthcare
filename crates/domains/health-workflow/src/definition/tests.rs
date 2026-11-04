use std::collections::BTreeMap;

use health_workflow_actions::{ActionDescriptor, Compensation, ParamKind, ParamSpec};

use super::*;
use crate::model::Transition;

fn catalog() -> Vec<ActionDescriptor> {
    vec![ActionDescriptor {
        id: "patient-flow/open-queue-entry",
        major: 1,
        permission: "patient-flow.manage",
        compensation: Compensation::Action("patient-flow/close-queue-entry"),
        params: vec![
            ParamSpec {
                name: "service_point",
                kind: ParamKind::Text,
                required: true,
            },
            ParamSpec {
                name: "priority",
                kind: ParamKind::Code(&["routine", "urgent"]),
                required: false,
            },
        ],
    }]
}

fn execute(action: &str, params: &[(&str, Value)]) -> Action {
    Action::Execute {
        action: action.into(),
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn to(target: &str, event: &str, actions: Vec<Action>) -> Transition {
    Transition {
        events: vec![event.into()],
        guard: None,
        targets: vec![target.into()],
        internal: false,
        actions,
    }
}

/// waiting --arrive--> queued --seen--> done (final).
fn definition(actions: Vec<Action>) -> WorkflowDefinition {
    let mut waiting = State::new("waiting", Kind::Atomic);
    waiting.transitions.push(to("queued", "arrive", actions));
    let mut queued = State::new("queued", Kind::Atomic);
    queued.transitions.push(to("done", "seen", vec![]));
    WorkflowDefinition {
        url: "https://health.go.ug/fhir/Library/opd-visit".into(),
        version: "1.0.0".into(),
        statechart: Statechart {
            states: vec![waiting, queued, State::new("done", Kind::Final)],
            initial: vec![],
        },
        data_model: vec![Field {
            name: "urgent".into(),
            kind: FieldKind::Bool,
        }],
        data_requirements: vec![DataRequirement {
            name: "encounter".into(),
            resource_type: "Encounter".into(),
            paths: vec!["Encounter.priority".into()],
        }],
        decision_tables: vec![],
        principal: vec!["patient-flow.manage".into()],
    }
}

fn open(params: &[(&str, Value)]) -> Action {
    execute("patient-flow/open-queue-entry@1", params)
}

fn text(value: &str) -> Value {
    Value::Text(value.into())
}

#[test]
fn a_valid_definition_has_no_problems() {
    let action = open(&[
        ("service_point", text("triage")),
        ("priority", text("urgent")),
    ]);
    assert_eq!(definition(vec![action]).problems(&catalog()), vec![]);
}

#[test]
fn unknown_actions_and_bad_references_fail() {
    let cases = [
        (
            execute(
                "patient-flow/open-queue-entry@2",
                &[("service_point", text("t"))],
            ),
            Problem::UnknownAction("patient-flow/open-queue-entry@2".into()),
        ),
        (
            execute("billing/charge@1", &[]),
            Problem::UnknownAction("billing/charge@1".into()),
        ),
        (
            execute("patient-flow/open-queue-entry", &[]),
            Problem::BadReference("patient-flow/open-queue-entry".into()),
        ),
    ];
    for (action, expected) in cases {
        let problems = definition(vec![action]).problems(&catalog());
        assert!(problems.contains(&expected), "{problems:?}");
    }
}

#[test]
fn invalid_parameters_fail() {
    let action = "patient-flow/open-queue-entry@1".to_owned();
    let cases = [
        (
            open(&[]),
            Problem::MissingParameter {
                action: action.clone(),
                param: "service_point".into(),
            },
        ),
        (
            open(&[("service_point", Value::Int(3))]),
            Problem::InvalidParameter {
                action: action.clone(),
                param: "service_point".into(),
            },
        ),
        (
            open(&[("service_point", text("t")), ("priority", text("whenever"))]),
            Problem::InvalidParameter {
                action: action.clone(),
                param: "priority".into(),
            },
        ),
        (
            open(&[("service_point", text("t")), ("room", text("4"))]),
            Problem::UnknownParameter {
                action,
                param: "room".into(),
            },
        ),
    ];
    for (action, expected) in cases {
        let problems = definition(vec![action]).problems(&catalog());
        assert_eq!(problems, vec![expected]);
    }
}

#[test]
fn assignments_must_name_a_declared_field_of_its_kind() {
    let set = |field: &str, value: Value| Action::Assign {
        field: field.into(),
        value,
    };
    let fine = definition(vec![set("urgent", Value::Bool(true))]);
    assert_eq!(
        fine.problems(&[]),
        vec![Problem::UnneededPermission("patient-flow.manage".into())]
    );
    for (field, value) in [("urgent", Value::Int(1)), ("score", Value::Int(1))] {
        let problems = definition(vec![set(field, value)]).problems(&[]);
        assert!(
            problems.contains(&Problem::BadAssignment(field.into())),
            "{problems:?}"
        );
    }
}

#[test]
fn unreachable_states_and_dead_ends_fail() {
    let mut chart = definition(vec![]);
    chart
        .statechart
        .states
        .push(State::new("orphan", Kind::Atomic));
    chart.principal.clear();
    let problems = chart.problems(&catalog());
    assert_eq!(problems, vec![Problem::Unreachable("orphan".into()),]);

    let mut stuck = definition(vec![]);
    stuck.principal.clear();
    stuck.statechart.states[1].transitions.clear();
    let problems = stuck.problems(&catalog());
    assert!(
        problems.contains(&Problem::DeadEnd("queued".into())),
        "{problems:?}"
    );
    assert!(
        problems.contains(&Problem::Unreachable("done".into())),
        "{problems:?}"
    );
}

#[test]
fn an_ancestor_s_transition_is_a_way_out_and_parallel_regions_are_entered() {
    // visit (compound: consult) runs beside billing (compound: open), and leaving visit exits all.
    let mut visit = State::new("visit", Kind::Compound);
    visit.children.push(State::new("consult", Kind::Atomic));
    visit.transitions.push(to("closed", "cancel", vec![]));
    let mut billing = State::new("billing", Kind::Compound);
    let mut open_bill = State::new("open", Kind::Atomic);
    open_bill.transitions.push(to("closed", "paid", vec![]));
    billing.children.push(open_bill);
    let mut both = State::new("both", Kind::Parallel);
    both.children = vec![visit, billing];
    let mut chart = definition(vec![]);
    chart.principal.clear();
    chart.statechart.states = vec![both, State::new("closed", Kind::Final)];
    assert_eq!(chart.problems(&catalog()), vec![]);
}

#[test]
fn decision_tables_must_be_well_formed() {
    let row = |when: &[&str]| Row {
        when: when.iter().map(|w| (*w).to_owned()).collect(),
        then: text("refer"),
    };
    let table = |id: &str, rows| DecisionTable {
        id: id.into(),
        rows,
        default: Some(text("routine")),
    };
    let mut chart = definition(vec![]);
    chart.principal.clear();
    chart.decision_tables = vec![
        table("danger", vec![row(&["%encounter.priority = 'urgent'"])]),
        table("danger", vec![row(&["true"])]),
        table("empty", vec![]),
        table("blank", vec![row(&[" "])]),
    ];
    assert_eq!(
        chart.problems(&catalog()),
        vec![
            Problem::BadDecisionTable("danger".into()),
            Problem::BadDecisionTable("empty".into()),
            Problem::BadDecisionTable("blank".into()),
        ]
    );
}

#[test]
fn the_principal_declares_only_permissions_its_actions_need() {
    let action = open(&[("service_point", text("triage"))]);
    let mut chart = definition(vec![action]);
    chart.principal.push("billing.void".into());
    assert_eq!(
        chart.problems(&catalog()),
        vec![Problem::UnneededPermission("billing.void".into())]
    );
}

#[test]
fn an_invalid_statechart_is_refused_first() {
    let mut chart = definition(vec![]);
    chart.statechart.states[0].transitions[0].targets = vec!["nowhere".into()];
    assert!(matches!(
        chart.problems(&catalog()).as_slice(),
        [Problem::Statechart(DefinitionError::UnknownTarget(_))]
    ));
}

#[test]
fn a_definition_reads_from_its_library_content() {
    let json = r#"{
        "url": "https://health.go.ug/fhir/Library/opd-visit", "version": "1.0.0",
        "statechart": { "states": [
            { "id": "waiting", "kind": "atomic", "transitions": [
                { "events": ["arrive"], "targets": ["done"], "actions": [
                    { "kind": "execute", "action": "patient-flow/open-queue-entry@1",
                      "params": { "service_point": "triage" } } ] } ] },
            { "id": "done", "kind": "final" } ] },
        "dataModel": [{ "name": "urgent", "kind": "bool" }],
        "dataRequirements": [{ "name": "encounter", "resourceType": "Encounter" }],
        "decisionTables": [{ "id": "danger", "rows": [{ "when": ["true"], "then": "refer" }], "default": "routine" }],
        "principal": ["patient-flow.manage"]
    }"#;
    let definition: WorkflowDefinition = serde_json::from_str(json).expect("reads");
    assert_eq!(definition.problems(&catalog()), vec![]);
}

#[test]
fn human_tasks_must_be_well_formed_on_atomic_states() {
    let task = |form: Option<&str>| crate::model::HumanTask {
        role: "nurse".into(),
        form: form.map(str::to_owned),
        worklist: "opd-triage".into(),
        priority: crate::model::Priority::Urgent,
        claim_timeout: crate::model::CLAIM_TIMEOUT,
    };
    let with = |task: crate::model::HumanTask, on_done: bool| {
        let mut definition = definition(vec![]);
        let index = usize::from(on_done) + 1;
        definition.statechart.states[index].task = Some(task);
        definition
    };
    let open = || open(&[("service_point", text("triage"))]);
    let mut good = with(
        task(Some("https://health.go.ug/fhir/Questionnaire/triage|2.0.0")),
        false,
    );
    good.statechart.states[0].transitions[0]
        .actions
        .push(open());
    assert_eq!(good.problems(&catalog()), vec![]);

    let bad = |task, on_done| {
        let mut definition = with(task, on_done);
        definition.statechart.states[0].transitions[0]
            .actions
            .push(open());
        definition.problems(&catalog())
    };
    let queued = vec![Problem::BadTask("queued".into())];
    assert_eq!(bad(task(Some("no-version")), false), queued);
    assert_eq!(
        bad(
            crate::model::HumanTask {
                worklist: " ".into(),
                ..task(None)
            },
            false
        ),
        queued
    );
    assert_eq!(
        bad(
            crate::model::HumanTask {
                claim_timeout: health_time::SignedDuration::ZERO,
                ..task(None)
            },
            false
        ),
        queued
    );
    assert_eq!(
        bad(task(None), true),
        vec![Problem::BadTask("done".into())],
        "a final state is not a human step"
    );
}
