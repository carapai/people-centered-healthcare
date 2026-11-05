//! The shared FHIRPath conformance suite (ADRs 0038, 0054; #92), against the server's engine
//! (`octofhir-fhirpath`). The same fixtures, in `packages/fhirpath-conformance/fixtures`, run
//! against fhirpath.js in lossless precise mode in that package's tests: an expression a published
//! Questionnaire uses must evaluate identically in both.

use std::path::PathBuf;

use health_fhir_validation::{FhirPathEngine, FhirPathItem, OctofhirEngine};
use serde_json::{Value, json};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/fhirpath-conformance/fixtures")
}

/// An item as the fixtures write it.
fn normal(item: &FhirPathItem) -> Value {
    match item {
        FhirPathItem::Boolean(value) => json!({ "boolean": value }),
        // fhirpath.js returns every number as its precise decimal type, so numbers compare as
        // their exact text.
        FhirPathItem::Integer(value) => json!({ "number": value.to_string() }),
        FhirPathItem::Decimal(value) => json!({ "number": value }),
        FhirPathItem::String(value) | FhirPathItem::Temporal(value) => json!({ "text": value }),
        FhirPathItem::Quantity { value, unit } => {
            json!({ "quantity": { "value": value, "unit": unit } })
        }
        FhirPathItem::Element(value) => json!({ "element": value }),
    }
}

#[tokio::test]
async fn every_fixture_passes_on_the_server_engine() {
    let engine = OctofhirEngine::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(fixtures())
        .expect("the fixtures directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no fixtures found");
    let mut failures = Vec::new();
    let mut cases = 0;
    for file in files {
        let text = std::fs::read_to_string(&file).expect("a fixture");
        let fixture: Value = serde_json::from_str(&text).expect("fixture JSON");
        let group = fixture["group"].as_str().unwrap_or_default().to_owned();
        let cases_in = fixture["cases"].as_array().cloned().unwrap_or_default();
        let expressions: Vec<String> = cases_in
            .iter()
            .map(|c| c["expression"].as_str().unwrap_or_default().to_owned())
            .collect();
        let results = engine
            .evaluate_many(&fixture["resource"], &expressions)
            .await;
        for (case, result) in cases_in.iter().zip(results) {
            cases += 1;
            let actual = match result {
                Ok(items) => Value::Array(items.iter().map(normal).collect()),
                Err(error) => json!({ "error": error.0 }),
            };
            if actual != case["expected"] {
                failures.push(format!(
                    "{group} / {}: {} gave {actual}, expected {}",
                    case["name"].as_str().unwrap_or_default(),
                    case["expression"].as_str().unwrap_or_default(),
                    case["expected"]
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{cases} cases, failing:\n{}",
        failures.join("\n")
    );
}
