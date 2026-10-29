//! Tier 2 (core invariants) and the decimal conformance suite for the configured FHIRPath
//! engine (ADR 0052; fhir-r5-model.md § FHIRPath).

use health_fhir_validation::{
    FhirPathEngine, FhirPathItem, IssueCode, OctofhirEngine, validate_invariants,
};
use serde_json::Value;

fn parse(json: &str) -> Value {
    serde_json::from_str(json).expect("json")
}

async fn invariant_issues(json: &str) -> Vec<(IssueCode, String, String)> {
    let engine = OctofhirEngine::new();
    validate_invariants(&engine, &parse(json))
        .await
        .into_iter()
        .map(|i| {
            let key = i.message.split(':').next().unwrap_or_default().to_owned();
            (i.code, i.location, key)
        })
        .collect()
}

fn issue(code: IssueCode, location: &str, key: &str) -> (IssueCode, String, String) {
    (code, location.to_owned(), key.to_owned())
}

#[tokio::test]
async fn valid_resources_hold_their_invariants() {
    for json in [
        r#"{"resourceType":"Patient","contact":[{"name":{"family":"Akello"}}]}"#,
        r#"{"resourceType":"Observation","status":"final","code":{"text":"x"},"effectivePeriod":{"start":"2026-01-01","end":"2026-01-02"}}"#,
        r#"{"resourceType":"Patient","extension":[{"url":"http://e","valueString":"x"}]}"#,
        r##"{"resourceType":"Observation","status":"final","code":{"text":"x"},"contained":[{"resourceType":"Patient","id":"p"}],"subject":{"reference":"#p"}}"##,
    ] {
        assert_eq!(invariant_issues(json).await, Vec::new(), "{json}");
    }
}

#[tokio::test]
async fn violated_invariants_are_reported_with_key_and_location() {
    assert_eq!(
        invariant_issues(r#"{"resourceType":"Patient","contact":[{"gender":"female"}]}"#).await,
        vec![issue(IssueCode::Invariant, "Patient.contact", "pat-1")]
    );
    assert_eq!(
        invariant_issues(
            r#"{"resourceType":"Observation","status":"final","code":{"text":"x"},"effectivePeriod":{"start":"2026-02-01","end":"2026-01-01"}}"#
        )
        .await,
        vec![issue(IssueCode::Invariant, "Observation.effectivePeriod", "per-1")]
    );
    assert_eq!(
        invariant_issues(
            r#"{"resourceType":"Patient","extension":[{"url":"http://e","valueString":"x","extension":[{"url":"http://f","valueString":"y"}]}]}"#
        )
        .await,
        vec![issue(IssueCode::Invariant, "Patient.extension[0]", "ext-1")]
    );
    assert_eq!(
        invariant_issues(
            r#"{"resourceType":"Observation","status":"final","code":{"text":"x"},"contained":[{"resourceType":"Patient","id":"orphan"}]}"#
        )
        .await,
        vec![issue(IssueCode::Invariant, "Observation", "dom-3")]
    );
}

#[tokio::test]
async fn contained_resources_are_validated_as_resources() {
    assert_eq!(
        invariant_issues(
            r##"{"resourceType":"Observation","status":"final","code":{"text":"x"},"subject":{"reference":"#p"},"contained":[{"resourceType":"Patient","id":"p","contact":[{"gender":"male"}]}]}"##
        )
        .await,
        vec![issue(IssueCode::Invariant, "Observation.contained[0].contact", "pat-1")]
    );
}

#[tokio::test]
async fn trace_output_is_discarded() {
    // `ref-1` calls trace('url') on a contained reference; nothing may reach stdout or stderr
    // (ADR 0047). The engine's default would print `TRACE[url][0]: 'p'`; the test runner
    // captures output, so this checks the evaluation still succeeds with tracing silenced.
    let json = r##"{"resourceType":"Observation","status":"final","code":{"text":"x"},"contained":[{"resourceType":"Patient","id":"p"}],"subject":{"reference":"#p"}}"##;
    assert_eq!(invariant_issues(json).await, Vec::new());
}

#[tokio::test(flavor = "multi_thread")]
async fn skeletons_of_every_type_hold_their_invariants() {
    let engine = OctofhirEngine::new();
    for resource_type in health_fhir_validation::resource_types() {
        let skeleton = health_fhir_validation::skeleton(resource_type).expect("skeleton");
        let tier1 = health_fhir_validation::validate_value(&skeleton);
        let tier2 = validate_invariants(&engine, &skeleton).await;
        assert!(
            tier1.is_empty() && tier2.is_empty(),
            "{resource_type}: {tier1:?} {tier2:?}"
        );
    }
}

async fn evaluate(resource: &str, expression: &str) -> Result<Vec<FhirPathItem>, String> {
    let engine = OctofhirEngine::new();
    let mut results = engine
        .evaluate_many(&parse(resource), &[expression.to_owned()])
        .await;
    results.remove(0).map_err(|e| e.to_string())
}

const QUANTITY: &str = r#"{"resourceType":"Observation","status":"final","code":{"text":"x"},"valueQuantity":{"value":VALUE,"unit":"kg","system":"http://unitsofmeasure.org","code":"kg"}}"#;

fn with_value(value: &str) -> String {
    QUANTITY.replace("VALUE", value)
}

async fn value_of(value: &str) -> Vec<FhirPathItem> {
    evaluate(
        &with_value(value),
        "Observation.value.ofType(Quantity).value",
    )
    .await
    .expect("evaluates")
}

async fn truth(value: &str, expression: &str) -> bool {
    match evaluate(&with_value(value), expression).await.as_deref() {
        Ok([FhirPathItem::Boolean(b)]) => *b,
        other => panic!("{expression} on {value}: {other:?}"),
    }
}

/// The decimal conformance suite (ADR 0052).
#[tokio::test]
async fn decimals_are_exact() {
    // Trailing zeros and scale are kept.
    assert_eq!(
        value_of("1.50").await,
        vec![FhirPathItem::Decimal("1.50".into())]
    );
    assert_eq!(
        value_of("0.010").await,
        vec![FhirPathItem::Decimal("0.010".into())]
    );
    // 28 significant digits are exact.
    assert_eq!(
        value_of("1234567890.123456789012345678").await,
        vec![FhirPathItem::Decimal(
            "1234567890.123456789012345678".into()
        )]
    );
    // -0 is zero.
    assert!(truth("-0.0", "Observation.value.ofType(Quantity).value = 0").await);
    // Exponents are read exactly.
    assert!(truth("1.0e3", "Observation.value.ofType(Quantity).value = 1000").await);
    assert!(
        truth(
            "2.5e-3",
            "Observation.value.ofType(Quantity).value = 0.0025"
        )
        .await
    );
    // Arithmetic is decimal, not floating point.
    assert!(
        truth(
            "0.1",
            "(Observation.value.ofType(Quantity).value + 0.2) = 0.3"
        )
        .await
    );
    // Equivalence is at the precision of the less precise operand.
    assert!(truth("1.50", "Observation.value.ofType(Quantity).value ~ 1.5").await);
    assert!(!truth("1.50", "Observation.value.ofType(Quantity).value ~ 1.6").await);
    // UCUM comparisons convert units.
    assert!(truth("1", "Observation.value.ofType(Quantity) = 1000 'g'").await);
}

#[tokio::test]
async fn non_ascii_units_evaluate() {
    // `m²` once panicked inside the UCUM crate (third_party/octofhir-ucum patch 0001).
    let json = r#"{"resourceType":"Observation","status":"final","code":{"text":"x"},"valueQuantity":{"value":2,"unit":"m²"}}"#;
    assert_eq!(invariant_issues(json).await, Vec::new());
    assert!(
        evaluate(json, "Observation.value.ofType(Quantity).value = 2")
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn more_than_28_significant_digits_fails_closed() {
    let found = invariant_issues(&with_value("1.2345678901234567890123456789")).await;
    assert_eq!(
        found,
        vec![issue(
            IssueCode::Processing,
            "Observation.valueQuantity.value",
            "a decimal has more than 28 significant digits, so the invariants cannot be evaluated exactly"
        )]
    );
}

/// The official examples that violate a core invariant, each confirmed by a check that does
/// not use FHIRPath. They are errors in HL7's examples, not in the engine:
///
/// - `que-2`: generated Questionnaires whose linkIds repeat (`…._null`);
/// - `sdf-27`: StructureDefinitions with a `baseDefinition` but no `derivation`.
fn genuinely_violates(key: &str, resource: &Value) -> bool {
    fn all_of(value: &Value, resource_type: &str, out: &mut Vec<Value>) {
        match value {
            Value::Object(object) => {
                if object.get("resourceType").and_then(Value::as_str) == Some(resource_type) {
                    out.push(value.clone());
                }
                object.values().for_each(|v| all_of(v, resource_type, out));
            }
            Value::Array(items) => items.iter().for_each(|v| all_of(v, resource_type, out)),
            _ => {}
        }
    }
    fn link_ids(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Object(object) => {
                for (k, v) in object {
                    match (k.as_str(), v) {
                        ("linkId", Value::String(id)) => out.push(id.clone()),
                        _ => link_ids(v, out),
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|v| link_ids(v, out)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    match key {
        "que-2" => {
            all_of(resource, "Questionnaire", &mut found);
            found.iter().any(|q| {
                let mut ids = Vec::new();
                link_ids(q, &mut ids);
                let unique: std::collections::BTreeSet<_> = ids.iter().collect();
                unique.len() != ids.len()
            })
        }
        "sdf-27" => {
            all_of(resource, "StructureDefinition", &mut found);
            found
                .iter()
                .any(|sd| sd.get("baseDefinition").is_some() && sd.get("derivation").is_none())
        }
        _ => false,
    }
}

/// With `HEALTH_FHIR_R5_EXAMPLES` set: every official example holds its core invariants,
/// except violations that an independent check confirms.
#[tokio::test(flavor = "multi_thread")]
async fn the_official_corpus_holds_its_invariants_when_available() {
    let Some(dir) = std::env::var_os("HEALTH_FHIR_R5_EXAMPLES") else {
        eprintln!("skipped: set HEALTH_FHIR_R5_EXAMPLES (tools/fhir/fetch-r5-examples.sh)");
        return;
    };
    let engine = OctofhirEngine::new();
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("corpus")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    assert!(files.len() >= 2824, "{} files", files.len());
    let (mut failures, mut confirmed) = (Vec::new(), 0);
    for file in &files {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let resource: Value =
            serde_json::from_str(&std::fs::read_to_string(file).expect("read")).expect("json");
        for issue in validate_invariants(&engine, &resource).await {
            let key = issue.message.split(':').next().unwrap_or_default();
            if issue.code == IssueCode::Invariant && genuinely_violates(key, &resource) {
                confirmed += 1;
            } else {
                failures.push(format!(
                    "{name}: {} at {}: {}",
                    issue.code.as_str(),
                    issue.location,
                    issue.message
                ));
            }
        }
    }
    eprintln!(
        "{} examples checked, {confirmed} confirmed example errors",
        files.len()
    );
    assert_eq!(failures, Vec::<String>::new());
    assert_eq!(confirmed, 171, "161 que-2 and 10 sdf-27");
}
