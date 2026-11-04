//! The platform core package's profiles (#87; ADR 0049) under tier 3 (ADR 0053): the committed
//! snapshots in `packages/fhir-core-package/package/` compile, each profile's example passes
//! tiers 1–3, and the constraints each profile adds are enforced.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use health_fhir_validation::{
    Issue, OctofhirEngine, ProfileSet, ProfileValidator, SnapshotValidator, skeleton,
    validate_invariants, validate_value,
};
use serde_json::{Value, json};

const BASE: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition";

fn core_package() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packages/fhir-core-package")
}

fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("json")
}

fn validator() -> SnapshotValidator {
    let mut definitions = Vec::new();
    for entry in std::fs::read_dir(core_package().join("package")).expect("package") {
        let path = entry.expect("entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name.starts_with("StructureDefinition-") {
            definitions.push(read(&path));
        }
    }
    assert_eq!(definitions.len(), 24, "the package's StructureDefinitions");
    // From its dependency hl7.fhir.r5.core, as activation (#89) will load it: R5's own
    // snapshots name SimpleQuantity for quantities.
    definitions
        .push(read(&Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "tests/fixtures/profiles/StructureDefinition-SimpleQuantity.json",
        )));
    let set = ProfileSet::compile(&definitions).expect("the core profiles compile");
    SnapshotValidator::new(Arc::new(set), Arc::new(OctofhirEngine::new()))
}

fn example(name: &str) -> Value {
    read(&core_package().join("examples").join(name))
}

/// Tiers 1–3, as the write path runs them.
async fn issues(resource: &Value, profile: &str) -> Vec<Issue> {
    let mut all = validate_value(resource);
    all.extend(validate_invariants(&OctofhirEngine::new(), resource).await);
    all.extend(
        validator()
            .validate(resource, &[format!("{BASE}/{profile}")])
            .await,
    );
    all
}

async fn fails_at(resource: &Value, profile: &str, location: &str) {
    let found = issues(resource, profile).await;
    assert!(
        found.iter().any(|issue| issue.location == location),
        "expected an issue at {location}: {found:?}"
    );
}

#[tokio::test]
async fn every_example_passes_its_profile() {
    for (file, profile) in [
        ("Library-anc-visit-mapping.json", "ExtractionMappingLibrary"),
        (
            "Library-patient-matching-rules.json",
            "MatchingRulesLibrary",
        ),
        (
            "Library-opd-visit-workflow.json",
            "WorkflowDefinitionLibrary",
        ),
        ("PlanDefinition-art-program.json", "ProgramPlanDefinition"),
    ] {
        let found = issues(&example(file), profile).await;
        assert!(found.is_empty(), "{file}: {found:?}");
    }
}

#[tokio::test]
async fn a_definition_needs_its_canonical_identity() {
    let mut rules = example("Library-patient-matching-rules.json");
    rules.as_object_mut().expect("object").remove("version");
    fails_at(&rules, "MatchingRulesLibrary", "Library.version").await;
}

#[tokio::test]
async fn each_library_has_its_platform_type_and_content_type() {
    let mut rules = example("Library-patient-matching-rules.json");
    rules["type"]["coding"][0]["code"] = json!("workflow-definition");
    fails_at(&rules, "MatchingRulesLibrary", "Library.type").await;

    let mut workflow = example("Library-opd-visit-workflow.json");
    workflow["content"][0]["contentType"] = json!("application/json");
    fails_at(
        &workflow,
        "WorkflowDefinitionLibrary",
        "Library.content[0].contentType",
    )
    .await;

    workflow.as_object_mut().expect("object").remove("content");
    fails_at(&workflow, "WorkflowDefinitionLibrary", "Library.content").await;
}

#[tokio::test]
async fn an_extraction_mapping_names_its_questionnaire_and_governs_each_template() {
    let mapping = example("Library-anc-visit-mapping.json");

    let mut untargeted = mapping.clone();
    untargeted["extension"]
        .as_array_mut()
        .expect("extensions")
        .retain(|e| {
            !e["url"]
                .as_str()
                .unwrap_or_default()
                .ends_with("target-versions")
        });
    let found = issues(&untargeted, "ExtractionMappingLibrary").await;
    assert!(!found.is_empty(), "a mapping without its versions passed");

    let mut ungoverned = mapping.clone();
    ungoverned["contained"][0]
        .as_object_mut()
        .expect("template")
        .remove("extension");
    let found = issues(&ungoverned, "ExtractionMappingLibrary").await;
    assert!(
        found
            .iter()
            .any(|issue| issue.message.contains("hp-template-1")),
        "a template without its mode passed: {found:?}"
    );

    // A template must state its mode. (That the mode is managed or seed is the required
    // binding, checked by tier 4 with #91.)
    let mut modeless = mapping;
    modeless["contained"][0]["extension"][0]["extension"]
        .as_array_mut()
        .expect("template extensions")
        .retain(|e| e["url"] != "mode");
    let found = issues(&modeless, "ExtractionMappingLibrary").await;
    assert!(!found.is_empty(), "a template without its mode passed");
}

#[tokio::test]
async fn a_program_is_a_clinical_protocol_with_its_episode_type() {
    let program = example("PlanDefinition-art-program.json");

    let mut order_set = program.clone();
    order_set["type"]["coding"][0]["code"] = json!("order-set");
    fails_at(&order_set, "ProgramPlanDefinition", "PlanDefinition.type").await;

    let mut no_episode = program;
    no_episode["extension"]
        .as_array_mut()
        .expect("extensions")
        .retain(|e| {
            !e["url"]
                .as_str()
                .unwrap_or_default()
                .ends_with("program-episode-type")
        });
    let found = issues(&no_episode, "ProgramPlanDefinition").await;
    assert!(
        !found.is_empty(),
        "a program without its episode type passed"
    );
}

/// The financial (ADR 0030) and inventory (ADR 0034) resources the platform pins.
const PINNED: [&str; 15] = [
    "ChargeItem",
    "Account",
    "Invoice",
    "Coverage",
    "CoverageEligibilityRequest",
    "CoverageEligibilityResponse",
    "Claim",
    "ClaimResponse",
    "ExplanationOfBenefit",
    "PaymentReconciliation",
    "PaymentNotice",
    "InventoryItem",
    "SupplyRequest",
    "SupplyDelivery",
    "InventoryReport",
];

#[tokio::test]
async fn each_pinned_profile_accepts_a_valid_resource_of_its_type_and_no_other() {
    for resource_type in PINNED {
        let resource = skeleton(resource_type).expect("a skeleton");
        let profile = format!("Platform{resource_type}");
        let found = issues(&resource, &profile).await;
        assert!(found.is_empty(), "{resource_type}: {found:?}");
    }
    let account = skeleton("Account").expect("a skeleton");
    let found = issues(&account, "PlatformInvoice").await;
    assert!(!found.is_empty(), "an Account passed as an Invoice");
}

#[tokio::test]
async fn each_pinned_profile_keeps_r5_cardinalities() {
    let mut claim = skeleton("Claim").expect("a skeleton");
    claim.as_object_mut().expect("object").remove("status");
    fails_at(&claim, "PlatformClaim", "Claim.status").await;
}
