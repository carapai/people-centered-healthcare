use std::collections::BTreeMap;

use super::*;
use crate::definition::Row;

fn text(value: &str) -> Value {
    Value::Text(value.into())
}

fn row(name: &str, when: &str, then: &str) -> Row {
    Row {
        name: Some(name.into()),
        when: vec![when.into()],
        then: text(then),
    }
}

/// Route after triage: emergencies bypass, routine patients pay first.
fn table(default: Option<Value>) -> DecisionTable {
    DecisionTable {
        id: "triage-route".into(),
        rows: vec![
            row("Emergency", "emergency", "bypass"),
            row("Routine", "routine", "normal"),
        ],
        default,
    }
}

#[test]
fn the_first_row_that_holds_decides_else_the_default() {
    let route = table(Some(text("normal")));
    let holds = |category: &'static str| move |_: usize, c: &str| c == category;
    assert_eq!(
        route.decide(holds("emergency")),
        Decision {
            outcome: Some(text("bypass")),
            row: Some(0)
        }
    );
    assert_eq!(
        route.decide(holds("routine")),
        Decision {
            outcome: Some(text("normal")),
            row: Some(1)
        }
    );
    assert_eq!(
        route.decide(holds("urgent")),
        Decision {
            outcome: Some(text("normal")),
            row: None
        }
    );
    assert_eq!(table(None).decide(holds("urgent")), Decision::default());
}

#[test]
fn a_row_holds_only_when_all_its_conditions_do() {
    let mut route = table(None);
    route.rows[0].when.push("adult".into());
    let decided = route.decide(|_, c| c == "emergency");
    assert_eq!(decided.row, None);
}

#[test]
fn rows_keep_their_names_and_rows_without_one_read_as_before() {
    let with: Row = serde_json::from_value(serde_json::json!({
        "name": "Sick child", "when": ["%patient.age < 5"], "then": "bypass"
    }))
    .expect("row");
    assert_eq!(with.name.as_deref(), Some("Sick child"));
    let without: Row =
        serde_json::from_value(serde_json::json!({ "when": ["true"], "then": "refer" }))
            .expect("row");
    assert_eq!(without.name, None);
    assert_eq!(
        serde_json::to_value(&without).expect("json"),
        serde_json::json!({ "when": ["true"], "then": "refer" })
    );
}

#[test]
fn inputs_are_the_declared_paths_as_fhirpath_reads_them() {
    let requirement = DataRequirement {
        name: "patient".into(),
        resource_type: "Patient".into(),
        paths: vec![
            "Patient.age".into(),
            "pregnant".into(),
            "Patient".into(),
            "Patient.name.where(use = 'official')".into(),
        ],
    };
    assert_eq!(requirement.inputs(), ["%patient.age", "%patient.pregnant"]);
    assert_eq!(
        input("%triage.danger.signs"),
        Some(("triage", vec!["danger", "signs"]))
    );
    assert_eq!(input("%triage"), None);
    assert_eq!(input("triage.category"), None);
    assert_eq!(input("%triage.category()"), None);
}

#[test]
fn combinations_cover_every_sample_value_of_every_input_up_to_a_limit() {
    let samples = BTreeMap::from([
        (
            "%a.x".to_owned(),
            vec![Value::Bool(true), Value::Bool(false)],
        ),
        (
            "%a.y".to_owned(),
            vec![Value::Int(4), Value::Int(5), Value::Int(6)],
        ),
    ]);
    let all = combinations(&samples, 6).expect("six");
    assert_eq!(all.len(), 6);
    assert!(all.contains(&Inputs::from([
        ("%a.x".to_owned(), Value::Bool(false)),
        ("%a.y".to_owned(), Value::Int(6)),
    ])));
    assert_eq!(combinations(&samples, 5), None);
    assert_eq!(combinations(&BTreeMap::new(), 1), Some(vec![Inputs::new()]));
}

#[test]
fn coverage_finds_the_gaps_and_the_rules_that_never_fire() {
    let route = table(None);
    let case = |category: &str| Inputs::from([("%triage.category".to_owned(), text(category))]);
    let decided: Vec<(Inputs, Decision)> = ["emergency", "urgent"]
        .into_iter()
        .map(|category| {
            let inputs = case(category);
            let decision = route.decide(|_, c| c == category);
            (inputs, decision)
        })
        .collect();
    assert_eq!(
        coverage(&route, &decided),
        Coverage {
            gaps: vec![case("urgent")],
            never_fires: vec![1],
        }
    );
}

#[test]
fn a_test_case_reads_from_its_file() {
    let test: TableTest = serde_json::from_value(serde_json::json!({
        "type": TEST_TYPE, "name": "Sick child",
        "definition": "https://health.go.ug/fhir/Library/opd-visit|1.0.0",
        "table": "triage-route",
        "inputs": { "%patient.age": 3, "%triage.danger": true, "%triage.category": "routine" },
        "expect": "bypass"
    }))
    .expect("test");
    assert_eq!(test.inputs["%patient.age"], Value::Int(3));
    assert_eq!(test.inputs["%triage.danger"], Value::Bool(true));
    assert_eq!(test.expect, text("bypass"));
}
