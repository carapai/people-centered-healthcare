//! Population (#94): each fixture in `tests/fixtures/populate/` gives exactly its expected
//! response (and candidates) from its Questionnaire and launch.

use std::collections::BTreeMap;

use health_fhir_extraction::populate::{Launch, PopulateError, populate};
use health_fhir_validation::OctofhirEngine;
use serde_json::{Value, json};

fn launch(value: &Value) -> Launch {
    Launch {
        contexts: value["contexts"]
            .as_object()
            .map(|c| c.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default(),
        observations: value["observations"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
        now: value["now"].as_str().map(|t| t.parse().expect("time")),
    }
}

#[tokio::test]
async fn every_prefill_fixture_gives_its_expected_response() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/populate");
    let engine = OctofhirEngine::new();
    let mut ran = 0;
    for entry in std::fs::read_dir(dir).expect("fixtures") {
        let path = entry.expect("entry").path();
        let fixture: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
        let populated = populate(
            &engine,
            &fixture["questionnaire"],
            &launch(&fixture["launch"]),
        )
        .await
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            populated.response,
            fixture["expected"],
            "{}",
            path.display()
        );
        let candidates: BTreeMap<String, Vec<Value>> = fixture["candidates"]
            .as_object()
            .map(|c| {
                c.iter()
                    .map(|(k, v)| (k.clone(), v.as_array().cloned().unwrap_or_default()))
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(populated.candidates, candidates, "{}", path.display());
        ran += 1;
    }
    assert!(ran >= 2, "the fixtures ran");
}

#[tokio::test]
async fn a_required_patient_must_be_given_and_a_bad_expression_fails() {
    let engine = OctofhirEngine::new();
    let questionnaire = json!({
        "resourceType": "Questionnaire", "status": "active",
        "extension": [{
            "url": "http://hl7.org/fhir/uv/sdc/StructureDefinition/sdc-questionnaire-launchContext",
            "extension": [{ "url": "name", "valueCoding": { "code": "patient" } }]
        }],
        "item": [{ "linkId": "x", "type": "string", "extension": [{
            "url": "http://hl7.org/fhir/uv/sdc/StructureDefinition/sdc-questionnaire-initialExpression",
            "valueExpression": { "language": "text/fhirpath", "expression": "%patient.name.(" }
        }] }]
    });
    assert_eq!(
        populate(&engine, &questionnaire, &Launch::default()).await,
        Err(PopulateError::MissingContext("patient".into()))
    );
    let with_patient = Launch {
        contexts: BTreeMap::from([(
            "patient".to_owned(),
            json!({ "resourceType": "Patient", "id": "p" }),
        )]),
        ..Launch::default()
    };
    assert!(matches!(
        populate(&engine, &questionnaire, &with_patient).await,
        Err(PopulateError::Expression { .. })
    ));
}
