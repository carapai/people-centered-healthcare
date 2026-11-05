//! The publication gates of an Extraction Mapping (#93): expressions that evaluate, linkIds the
//! Questionnaire has, rounded arithmetic (ADR 0052), and fixtures that give exactly their Bundle.

use health_fhir_extraction::Mapping;
use health_fhir_extraction::gates::{Fixture, fixture, unknown_items, unparsable, unrounded};
use health_fhir_validation::OctofhirEngine;
use serde_json::{Value, json};

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("json")
}

fn library() -> Value {
    parse(include_str!("fixtures/anc-mapping.json"))
}

/// The fixture mapping with the weight's value expression replaced.
fn with_weight_value(expression: &str) -> Mapping {
    let mut library = library();
    library["contained"][0]["valueQuantity"]["_value"]["extension"][0]["valueString"] =
        json!(expression);
    Mapping::from_library(&library).expect("mapping")
}

#[tokio::test]
async fn the_fixture_mapping_passes_every_gate() {
    let mapping = Mapping::from_library(&library()).expect("mapping");
    let engine = OctofhirEngine::new();
    assert_eq!(unparsable(&engine, &mapping).await, Vec::<String>::new());
    assert_eq!(unrounded(&mapping), Vec::<String>::new());
    let ok = Fixture {
        response: parse(include_str!("fixtures/anc-response.json")),
        expected: parse(include_str!("fixtures/anc-expected.json")),
    };
    assert_eq!(fixture(&engine, &mapping, &ok).await, Vec::<String>::new());
}

#[tokio::test]
async fn expressions_that_do_not_evaluate_are_named() {
    let mapping = with_weight_value("answer.value.(");
    let found = unparsable(&OctofhirEngine::new(), &mapping).await;
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with("template weight: `answer.value.(`"),
        "{found:?}"
    );
}

#[test]
fn arithmetic_is_rounded_as_a_whole() {
    for unrounded_expression in [
        "answer.value * 2",
        "answer.value / 2.2",
        "answer.value + 1",
        "answer.value div 2",
        "answer.value.sqrt()",
        "answer.value.power(2)",
        "answer.value * 2.round(1)",
        "(answer.value * 2).round()",
        "(answer.value * 2).round(x)",
        "answer.value.round(1) * 2",
    ] {
        assert_eq!(
            unrounded(&with_weight_value(unrounded_expression)).len(),
            1,
            "{unrounded_expression}"
        );
    }
    for fine in [
        "answer.value",
        "(answer.value * 2).round(1)",
        "(answer.value / 2.2).round(2)",
        "answer.value.sqrt().round(3)",
        "%resource.item.where(linkId = 'weight-note').answer.value",
        "iif(answer.exists(), @2026-01-01, @2026-01-02)",
        "'a-b' & 'c'",
    ] {
        assert_eq!(
            unrounded(&with_weight_value(fine)),
            Vec::<String>::new(),
            "{fine}"
        );
    }
    // Contexts are not values: they repeat, they write nothing.
    let mut library = library();
    library["contained"][0]["extension"][1]["valueString"] =
        json!("item.where(linkId = 'vitals').item.take(1 + 1)");
    assert_eq!(
        unrounded(&Mapping::from_library(&library).expect("mapping")),
        Vec::<String>::new()
    );
}

#[test]
fn every_named_item_is_in_the_questionnaire() {
    let mapping = Mapping::from_library(&library()).expect("mapping");
    let questionnaire = json!({
        "resourceType": "Questionnaire", "version": "1.4.0",
        "item": [
            { "linkId": "vitals", "item": [{ "linkId": "weight" }, { "linkId": "weight-note" }] },
            { "linkId": "medications", "item": [{ "linkId": "medication", "item": [{ "linkId": "drug" }, { "linkId": "instructions" }] }] },
            { "linkId": "danger-signs" }
        ]
    });
    assert_eq!(
        unknown_items(&mapping, &questionnaire),
        Vec::<String>::new()
    );
    let mut older = questionnaire.clone();
    older["item"][2]["linkId"] = json!("danger");
    older["version"] = json!("1.0.0");
    let found = unknown_items(&mapping, &older);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("linkId 'danger-signs', which version 1.0.0"),
        "{found:?}"
    );
}

#[tokio::test]
async fn a_fixture_that_does_not_give_its_bundle_fails() {
    let mapping = Mapping::from_library(&library()).expect("mapping");
    let engine = OctofhirEngine::new();
    let response = parse(include_str!("fixtures/anc-response.json"));
    let expected = parse(include_str!("fixtures/anc-expected.json"));

    // A value that differs only in scale is a different value (ADR 0052).
    let mut scaled = expected.clone();
    scaled["entry"][0]["resource"]["valueQuantity"]["value"] = parse("61.5");
    let found = fixture(
        &engine,
        &mapping,
        &Fixture {
            response: response.clone(),
            expected: scaled,
        },
    )
    .await;
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("entry 0 differs"), "{found:?}");

    let mut fewer = expected.clone();
    fewer["entry"].as_array_mut().expect("entries").pop();
    assert_eq!(
        fixture(
            &engine,
            &mapping,
            &Fixture {
                response: response.clone(),
                expected: fewer
            }
        )
        .await,
        ["expected 2 entries, got 3"]
    );

    let mut anonymous = response.clone();
    anonymous.as_object_mut().expect("object").remove("id");
    let found = fixture(
        &engine,
        &mapping,
        &Fixture {
            response: anonymous,
            expected,
        },
    )
    .await;
    assert!(
        found[0].starts_with("the response does not extract"),
        "{found:?}"
    );

    // Outputs must be valid FHIR.
    let invalid = with_weight_value("'heavy'");
    let found = fixture(
        &engine,
        &invalid,
        &Fixture {
            response,
            expected: json!({}),
        },
    )
    .await;
    assert!(
        found.iter().any(|p| p.contains("not valid FHIR")),
        "{found:?}"
    );
}
