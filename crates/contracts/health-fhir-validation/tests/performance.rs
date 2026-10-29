//! The validation budget (ADR 0053): tiers 1–3 take p95 ≤ 2 ms per clinical resource on
//! reference edge hardware.
//!
//! With `HEALTH_FHIR_R5_EXAMPLES` set, in a release build, this times tiers 1–3 as the write
//! path runs them over every official clinical example. The vital-signs profiles
//! (`tests/fixtures/profiles/`) are active: an example is checked against the profiles it
//! claims, and every vital-signs Observation against `vitalsigns`. Set
//! `HEALTH_VALIDATION_P95_MS` to test a different budget.
//!
//! `cargo test --release -p health-fhir-validation --test performance -- --nocapture`

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use health_fhir_validation::{
    OctofhirEngine, ProfileSet, ProfileValidator, SnapshotValidator, validate_invariants,
    validate_value,
};
use serde_json::Value;

/// Conformance and knowledge resources: authored as packages, not written by clinical users.
const NOT_CLINICAL: &[&str] = &[
    "Bundle",
    "StructureDefinition",
    "ValueSet",
    "CodeSystem",
    "CapabilityStatement",
    "SearchParameter",
    "OperationDefinition",
    "ImplementationGuide",
    "ConceptMap",
    "NamingSystem",
    "StructureMap",
    "GraphDefinition",
    "CompartmentDefinition",
    "MessageDefinition",
    "TerminologyCapabilities",
    "Questionnaire",
    "PlanDefinition",
    "ActivityDefinition",
    "Library",
    "Measure",
    "TestScript",
    "ExampleScenario",
    "EvidenceReport",
    "Citation",
    "Evidence",
    "EvidenceVariable",
    "ArtifactAssessment",
];

const VITALS: &str = "http://hl7.org/fhir/StructureDefinition/vitalsigns";

fn profiles() -> ProfileSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/profiles");
    let definitions: Vec<Value> = std::fs::read_dir(dir)
        .expect("profiles")
        .map(|e| {
            let text = std::fs::read_to_string(e.expect("entry").path()).expect("read");
            serde_json::from_str(&text).expect("json")
        })
        .collect();
    ProfileSet::compile(&definitions).expect("the profiles compile")
}

/// The active profiles that apply to `resource`.
fn applicable(set: &ProfileSet, resource: &Value) -> Vec<String> {
    let mut urls: Vec<String> = resource["meta"]["profile"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|url| set.urls().any(|u| u == *url))
        .map(str::to_owned)
        .collect();
    let vital_sign = resource["category"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|c| c["coding"].as_array().into_iter().flatten())
        .any(|c| c["code"] == "vital-signs");
    if resource["resourceType"] == "Observation" && vital_sign && !urls.iter().any(|u| u == VITALS)
    {
        urls.push(VITALS.to_owned());
    }
    urls
}

#[tokio::test(flavor = "current_thread")]
async fn tiers_1_to_3_meet_the_budget_when_available() {
    let Some(dir) = std::env::var_os("HEALTH_FHIR_R5_EXAMPLES") else {
        eprintln!("skipped: set HEALTH_FHIR_R5_EXAMPLES (tools/fhir/fetch-r5-examples.sh)");
        return;
    };
    if cfg!(debug_assertions) {
        eprintln!("skipped: the budget applies to release builds (cargo test --release)");
        return;
    }
    let budget: f64 = std::env::var("HEALTH_VALIDATION_P95_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2.0);
    let engine = Arc::new(OctofhirEngine::new());
    let set = Arc::new(profiles());
    let validator = SnapshotValidator::new(set.clone(), engine.clone());
    let mut resources = Vec::new();
    for entry in std::fs::read_dir(dir).expect("corpus").flatten() {
        if entry.path().extension().is_none_or(|e| e != "json") {
            continue;
        }
        let text = std::fs::read_to_string(entry.path()).expect("read");
        let resource: Value = serde_json::from_str(&text).expect("json");
        let resource_type = resource["resourceType"].as_str().unwrap_or_default();
        if !NOT_CLINICAL.contains(&resource_type) {
            let profiles = applicable(&set, &resource);
            resources.push((resource, profiles));
        }
    }
    let profiled = resources.iter().filter(|(_, p)| !p.is_empty()).count();
    let (mut durations, mut profiled_durations) = (Vec::new(), Vec::new());
    // The first pass warms the engine's plan cache, as a running node's would be.
    for _ in 0..2 {
        durations.clear();
        profiled_durations.clear();
        for (resource, profiles) in &resources {
            let started = Instant::now();
            let _ = validate_value(resource);
            let _ = validate_invariants(engine.as_ref(), resource).await;
            if !profiles.is_empty() {
                let _ = validator.validate(resource, profiles).await;
            }
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            durations.push(elapsed);
            if !profiles.is_empty() {
                profiled_durations.push(elapsed);
            }
        }
    }
    profiled_durations.sort_by(f64::total_cmp);
    durations.sort_by(f64::total_cmp);
    let at = |q: f64| durations[((durations.len() as f64 - 1.0) * q) as usize];
    eprintln!(
        "tiers 1-3: {} clinical resources ({profiled} profiled): p50 {:.3} ms, p95 {:.3} ms, p99 {:.3} ms, max {:.3} ms",
        durations.len(),
        at(0.50),
        at(0.95),
        at(0.99),
        at(1.0)
    );
    eprintln!(
        "profiled: p50 {:.3} ms, max {:.3} ms",
        profiled_durations[profiled_durations.len() / 2],
        profiled_durations[profiled_durations.len() - 1]
    );
    assert!(
        durations.len() >= 700,
        "{} clinical resources",
        durations.len()
    );
    assert!(
        at(0.95) <= budget,
        "p95 {:.3} ms is over the {budget} ms budget",
        at(0.95)
    );
}
