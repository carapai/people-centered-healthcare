use health_fhir_validation::OctofhirEngine;
use health_workflow::definition::{DataRequirement, Row};
use health_workflow::{Kind, State, Statechart};

use super::*;

/// The rule the Studio's tree any(all(age < 5, danger signs), all(pregnant, urgent)) saves as:
/// one FHIRPath expression, as the client's `compile` writes it (apps/client test
/// `decision-tables.test.ts` pins the same text).
const SICK_CHILD_OR_URGENT_PREGNANCY: &str = "(%patient.age < 5 and %triage.danger = true) or (%patient.pregnant = true and %triage.category = 'urgent')";

fn text(value: &str) -> Sample {
    Sample::Text(value.into())
}

fn row(name: &str, when: &str, then: &str) -> Row {
    Row {
        name: Some(name.into()),
        when: vec![when.into()],
        then: text(then),
    }
}

fn definition(rows: Vec<Row>) -> WorkflowDefinition {
    WorkflowDefinition {
        url: "https://health.go.ug/fhir/Library/opd-visit".into(),
        version: "1.0.0".into(),
        statechart: Statechart {
            states: vec![State::new("done", Kind::Final)],
            initial: vec![],
        },
        data_model: vec![],
        data_requirements: vec![
            DataRequirement {
                name: "patient".into(),
                resource_type: "Patient".into(),
                paths: vec!["Patient.age".into(), "Patient.pregnant".into()],
            },
            DataRequirement {
                name: "triage".into(),
                resource_type: "Observation".into(),
                paths: vec!["Observation.category".into(), "Observation.danger".into()],
            },
        ],
        decision_tables: vec![DecisionTable {
            id: "route".into(),
            rows,
            default: Some(text("normal")),
        }],
        principal: vec![],
    }
}

fn patient(age: i64, danger: bool, pregnant: bool, category: &str) -> Inputs {
    Inputs::from([
        ("%patient.age".to_owned(), Sample::Int(age)),
        ("%patient.pregnant".to_owned(), Sample::Bool(pregnant)),
        ("%triage.category".to_owned(), text(category)),
        ("%triage.danger".to_owned(), Sample::Bool(danger)),
    ])
}

#[tokio::test]
async fn a_condition_tree_saved_as_one_expression_decides_on_the_server() {
    let engine = OctofhirEngine::new();
    let definition = definition(vec![row(
        "Sick child or urgent pregnancy",
        SICK_CHILD_OR_URGENT_PREGNANCY,
        "bypass",
    )]);
    let table = &definition.decision_tables[0];
    let nodes = [
        "%patient.age < 5 and %triage.danger = true".to_owned(),
        "%patient.pregnant = true and %triage.category = 'urgent'".to_owned(),
        "%patient.age < 5".to_owned(),
    ];
    let cases = [
        (
            patient(3, true, false, "routine"),
            Some(0),
            [true, false, true],
        ),
        (
            patient(30, false, true, "urgent"),
            Some(0),
            [false, true, false],
        ),
        (
            patient(3, false, false, "urgent"),
            None,
            [false, false, true],
        ),
        (
            patient(30, true, true, "routine"),
            None,
            [false, false, false],
        ),
    ];
    for (inputs, row, held) in cases {
        let tried = try_table(&engine, &definition, table, &inputs, &nodes)
            .await
            .expect("tried");
        assert_eq!(tried.decision.row, row, "{inputs:?}");
        let outcome = if row.is_some() { "bypass" } else { "normal" };
        assert_eq!(tried.decision.outcome, Some(text(outcome)));
        let marks: Vec<bool> = tried.nodes.iter().map(|(_, o)| o.holds()).collect();
        assert_eq!(marks, held, "{inputs:?}");
    }
}

#[tokio::test]
async fn an_input_of_no_declared_requirement_is_refused() {
    let engine = OctofhirEngine::new();
    let definition = definition(vec![row("Any", "true", "bypass")]);
    let inputs = Inputs::from([("%visit.kind".to_owned(), text("new"))]);
    let refused = try_table(
        &engine,
        &definition,
        &definition.decision_tables[0],
        &inputs,
        &[],
    )
    .await;
    assert_eq!(
        refused,
        Err("%visit.kind reads visit, which is not a declared data requirement".into())
    );
}

#[tokio::test]
async fn a_rule_shadowed_by_one_above_never_fires_and_gaps_show_without_a_default() {
    let engine = OctofhirEngine::new();
    let mut definition = definition(vec![
        row("Routine", "%triage.category = 'routine'", "normal"),
        row(
            "Routine infant",
            "%triage.category = 'routine' and %patient.age < 1",
            "bypass",
        ),
        row("Emergency", "%triage.category = 'emergency'", "bypass"),
    ]);
    definition.decision_tables[0].default = None;
    let samples = BTreeMap::from([
        (
            "%triage.category".to_owned(),
            vec![text("routine"), text("emergency"), text("urgent")],
        ),
        (
            "%patient.age".to_owned(),
            vec![Sample::Int(0), Sample::Int(1), Sample::Int(2)],
        ),
    ]);
    let checked = check(
        &engine,
        &definition,
        &definition.decision_tables[0],
        &samples,
    )
    .await
    .expect("checked");
    assert_eq!(checked.combinations, 9);
    assert_eq!(checked.errors, Vec::<String>::new());
    let coverage = checked.coverage.expect("coverage");
    assert_eq!(coverage.never_fires, [1]);
    assert_eq!(coverage.gaps.len(), 3);
    assert!(
        coverage
            .gaps
            .iter()
            .all(|g| g["%triage.category"] == text("urgent"))
    );
}

#[tokio::test]
async fn too_many_combinations_are_not_checked() {
    let engine = OctofhirEngine::new();
    let definition = definition(vec![row("Any", "true", "bypass")]);
    let many: Vec<Sample> = (0..40).map(Sample::Int).collect();
    let samples = BTreeMap::from([
        ("%patient.age".to_owned(), many.clone()),
        ("%triage.category".to_owned(), many),
    ]);
    let checked = check(
        &engine,
        &definition,
        &definition.decision_tables[0],
        &samples,
    )
    .await
    .expect("checked");
    assert_eq!(checked.coverage, None);
}
