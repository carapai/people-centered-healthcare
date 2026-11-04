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
