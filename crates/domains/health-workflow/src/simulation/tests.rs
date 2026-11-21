use super::*;

/// Triage (a nurse's task, opening a queue entry, overdue after an hour) → consultation, unless
/// urgent → done.
fn definition() -> WorkflowDefinition {
    serde_json::from_str(
        r#"{
          "url": "https://health.go.ug/fhir/Library/opd-visit",
          "version": "1.0.0",
          "statechart": {
            "states": [
              {
                "id": "triage", "kind": "atomic",
                "task": { "role": "nurse", "worklist": "opd-triage" },
                "on_entry": [[{ "kind": "execute", "action": "patient-flow/open-queue-entry@1" }]],
                "timers": [{ "after": "PT1H", "event": "triage.overdue" }],
                "transitions": [
                  { "events": ["triage.overdue"], "actions": [{ "kind": "execute", "action": "communication/notify@1" }] },
                  { "events": ["triaged"], "guard": "%urgent", "targets": ["done"] },
                  { "events": ["triaged"], "targets": ["consultation"] }
                ]
              },
              {
                "id": "consultation", "kind": "atomic",
                "task": { "role": "clinician", "worklist": "opd-consult" },
                "transitions": [{ "events": ["seen"], "targets": ["done"] }]
              },
              { "id": "done", "kind": "final" }
            ]
          }
        }"#,
    )
    .expect("a definition")
}

fn scenario(guards: &str, steps: &str) -> Scenario {
    serde_json::from_str(&format!(
        r#"{{ "type": "workflow-scenario", "name": "opd", "definition": "https://health.go.ug/fhir/Library/opd-visit|1.0.0", "guards": {guards}, "steps": {steps} }}"#
    ))
    .expect("a scenario")
}

#[test]
fn a_passing_scenario_has_no_failures() {
    let passing = scenario(
        r#"{ "%urgent": false }"#,
        r#"[
          { "expect": { "configuration": ["triage"], "actions": ["patient-flow/open-queue-entry@1"],
                        "timers": ["triage.overdue"], "tasks": [{ "state": "triage", "status": "ready" }] } },
          { "wait": "PT1H", "expect": { "actions": ["communication/notify@1"], "timers": [], "configuration": ["triage"] } },
          { "event": "triaged", "expect": { "configuration": ["consultation"], "actions": [],
                                            "tasks": [{ "state": "consultation", "status": "ready" }] } },
          { "event": "seen", "expect": { "configuration": ["done"], "running": false, "tasks": [] } }
        ]"#,
    );
    assert_eq!(simulate(&definition(), &passing), Vec::<String>::new());
}

#[test]
fn a_wrong_expectation_names_its_step() {
    let wrong = scenario(
        r#"{ "%urgent": true }"#,
        r#"[
          { "event": "triaged", "expect": { "configuration": ["consultation"], "running": true } }
        ]"#,
    );
    assert_eq!(
        simulate(&definition(), &wrong),
        [
            r#"opd: step 1: expected configuration ["consultation"], got ["done"]"#,
            "opd: step 1: expected running true, got false",
        ]
    );
}

#[test]
fn a_timer_not_yet_due_does_not_fire() {
    let early = scenario(
        r#"{}"#,
        r#"[
          { "wait": "PT59M", "expect": { "actions": [], "timers": ["triage.overdue"] } }
        ]"#,
    );
    assert_eq!(simulate(&definition(), &early), Vec::<String>::new());
}

#[test]
fn unstubbed_guards_and_malformed_steps_fail() {
    let unstubbed = scenario(r#"{}"#, r#"[{ "event": "triaged" }]"#);
    assert_eq!(
        simulate(&definition(), &unstubbed),
        ["opd: guards not stubbed: %urgent"]
    );
    let both = scenario(
        r#"{}"#,
        r#"[{ "expect": {} }, { "event": "triaged", "wait": "PT1M" }]"#,
    );
    assert_eq!(
        simulate(&definition(), &both),
        ["opd: step 2: a step sends one event or waits, not both or neither"]
    );
}

fn gated() -> WorkflowDefinition {
    serde_json::from_str(
        r#"{
          "url": "https://health.go.ug/fhir/Library/lab", "version": "1.0.0",
          "statechart": { "states": [
            { "id": "ordered", "kind": "atomic",
              "transitions": [{ "events": ["sent"], "targets": ["payment"] }] },
            { "id": "payment", "kind": "atomic",
              "gate": "https://health.go.ug/fhir/FinancialGate/before-lab",
              "task": { "role": "cashier", "worklist": "cashier" },
              "transitions": [
                { "events": ["gate.pass"], "targets": ["collect"] },
                { "events": ["gate.defer"], "targets": ["collect"] }
              ] },
            { "id": "collect", "kind": "atomic",
              "transitions": [{ "events": ["collected"], "targets": ["done"] }] },
            { "id": "done", "kind": "final" }
          ] }
        }"#,
    )
    .expect("a definition")
}

fn gate_scenario(outcome: Option<&str>, expect: &str) -> Scenario {
    let gates = outcome.map_or_else(String::new, |o| {
        format!(r#""https://health.go.ug/fhir/FinancialGate/before-lab": "{o}""#)
    });
    serde_json::from_str(&format!(
        r#"{{ "type": "workflow-scenario", "name": "lab", "definition": "https://health.go.ug/fhir/Library/lab|1.0.0",
             "gates": {{ {gates} }}, "steps": [{expect}] }}"#
    ))
    .expect("a scenario")
}

#[test]
fn a_gate_passes_defers_or_blocks_with_its_cashier_task() {
    let through = r#"{ "event": "sent", "expect": { "configuration": ["collect"], "tasks": [] } }"#;
    assert_eq!(
        simulate(&gated(), &gate_scenario(Some("pass"), through)),
        Vec::<String>::new()
    );
    assert_eq!(
        simulate(&gated(), &gate_scenario(None, through)),
        Vec::<String>::new(),
        "an unstubbed gate defers"
    );
    let waits = r#"{ "event": "sent", "expect": { "configuration": ["payment"],
                     "tasks": [{ "state": "payment", "status": "ready" }] } }"#;
    assert_eq!(
        simulate(&gated(), &gate_scenario(Some("block"), waits)),
        Vec::<String>::new()
    );
}

// The Workflow Studio's sandbox (#499).

fn moves(json: &str) -> Vec<Move> {
    serde_json::from_str(json).expect("moves")
}

#[test]
fn the_sandbox_shows_what_holds_at_the_start_with_actions_stubbed() {
    let played = play(&definition(), &Play::default()).expect("plays");
    assert_eq!(played.unanswered, Vec::<String>::new());
    let [start] = played.snapshots.as_slice() else {
        panic!("one snapshot: {played:?}");
    };
    assert_eq!(start.configuration, ["triage"]);
    assert_eq!(start.actions, ["patient-flow/open-queue-entry@1"]);
    assert_eq!(
        start.timers,
        [PendingTimer {
            event: "triage.overdue".into(),
            due_in: SignedDuration::from_hours(1)
        }]
    );
    assert_eq!(
        start.tasks,
        [ExpectedTask {
            state: "triage".into(),
            status: "ready".into()
        }]
    );
    assert_eq!(start.accepts, ["triage.overdue", "triaged"]);
    assert!(start.running);
    assert_eq!(start.elapsed, SignedDuration::ZERO);
}

#[test]
fn an_unanswered_guard_is_reported_and_its_move_not_taken() {
    let unanswered = Play {
        moves: moves(r#"[{ "event": "triaged" }, { "event": "seen" }]"#),
        ..Play::default()
    };
    let played = play(&definition(), &unanswered).expect("plays");
    assert_eq!(played.unanswered, ["%urgent"]);
    assert_eq!(played.snapshots.len(), 1, "only the start is shown");
    assert_eq!(played.snapshots[0].configuration, ["triage"]);
    assert_eq!(played.refused, None);

    let answered = Play {
        guards: BTreeMap::from([("%urgent".to_owned(), false)]),
        ..unanswered
    };
    let played = play(&definition(), &answered).expect("plays");
    assert_eq!(played.unanswered, Vec::<String>::new());
    let configurations: Vec<_> = played
        .snapshots
        .iter()
        .map(|s| s.configuration.clone())
        .collect();
    assert_eq!(configurations, [["triage"], ["consultation"], ["done"]]);
    assert!(!played.snapshots[2].running);
    assert_eq!(played.snapshots[2].accepts, Vec::<String>::new());
}

#[test]
fn waiting_fires_the_timers_that_fall_due_and_a_bad_move_is_refused() {
    let waits = Play {
        moves: moves(
            r#"[{ "wait": "PT30M" }, { "wait": "PT30M" }, { "event": "x", "wait": "PT1M" }]"#,
        ),
        ..Play::default()
    };
    let played = play(&definition(), &waits).expect("plays");
    assert_eq!(played.snapshots.len(), 3);
    assert_eq!(
        played.snapshots[1].timers[0].due_in,
        SignedDuration::from_mins(30)
    );
    assert_eq!(played.snapshots[2].actions, ["communication/notify@1"]);
    assert_eq!(played.snapshots[2].timers, []);
    assert_eq!(played.snapshots[2].elapsed, SignedDuration::from_hours(1));
    assert_eq!(
        played.refused.as_deref(),
        Some("a step sends one event or waits, not both or neither")
    );
}

#[test]
fn a_run_saved_as_a_scenario_passes_unchanged() {
    let run = Play {
        guards: BTreeMap::from([
            ("%urgent".to_owned(), false),
            ("%never-asked".to_owned(), true),
        ]),
        gates: BTreeMap::new(),
        moves: moves(r#"[{ "wait": "PT1H" }, { "event": "triaged" }, { "event": "seen" }]"#),
    };
    let played = play(&definition(), &run).expect("plays");
    let scenario = save(&definition(), &run, &played, "Routine visit");
    assert_eq!(scenario.kind, SCENARIO_TYPE);
    assert_eq!(
        scenario.definition,
        "https://health.go.ug/fhir/Library/opd-visit|1.0.0"
    );
    // Only the answers the run used are kept.
    assert_eq!(
        scenario.guards,
        BTreeMap::from([("%urgent".to_owned(), false)])
    );
    assert_eq!(scenario.steps.len(), 4);
    assert_eq!(scenario.steps[0].event, None);
    assert_eq!(scenario.steps[1].wait, Some(SignedDuration::from_hours(1)));
    assert_eq!(
        scenario.steps[2].expect.configuration.as_deref(),
        Some(&["consultation".to_owned()][..])
    );
    assert_eq!(simulate(&definition(), &scenario), Vec::<String>::new());
    // It survives being written to and read from its file.
    let file: Scenario =
        serde_json::from_str(&serde_json::to_string(&scenario).expect("json")).expect("read");
    assert_eq!(simulate(&definition(), &file), Vec::<String>::new());
}

#[test]
fn a_gate_answered_in_the_sandbox_is_kept_in_the_scenario() {
    let gate = "https://health.go.ug/fhir/FinancialGate/before-lab".to_owned();
    let run = Play {
        guards: BTreeMap::new(),
        gates: BTreeMap::from([(gate.clone(), "block".to_owned())]),
        moves: moves(r#"[{ "event": "sent" }]"#),
    };
    let played = play(&gated(), &run).expect("plays");
    assert_eq!(played.snapshots[1].configuration, ["payment"]);
    assert_eq!(played.gates, BTreeMap::from([(gate, "block".to_owned())]));
    let scenario = save(&gated(), &run, &played, "Blocked at the till");
    assert_eq!(simulate(&gated(), &scenario), Vec::<String>::new());
}

#[test]
fn guards_and_gates_list_each_once() {
    assert_eq!(guards(&definition().statechart), ["%urgent"]);
    assert_eq!(
        gates(&gated().statechart),
        ["https://health.go.ug/fhir/FinancialGate/before-lab"]
    );
}
