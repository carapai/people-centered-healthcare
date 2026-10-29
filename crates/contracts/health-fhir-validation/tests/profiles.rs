//! Tier 3 (ADR 0053): the official vital-signs profiles, and our slicing fixtures
//! (`tests/fixtures/custom-profiles/`) covering every supported construct.

use std::path::Path;
use std::sync::Arc;

use health_fhir_validation::{
    IssueCode, OctofhirEngine, ProfileSet, ProfileValidator, SnapshotValidator,
    validate_invariants, validate_value,
};
use serde_json::{Value, json};

fn read_dir(dir: &str) -> Vec<Value> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(dir);
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("dir")
        .map(|e| e.expect("entry").path())
        .collect();
    files.sort();
    files
        .iter()
        .map(|f| serde_json::from_str(&std::fs::read_to_string(f).expect("read")).expect("json"))
        .collect()
}

fn example(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/profile-examples")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("json")
}

fn validator() -> SnapshotValidator {
    let mut definitions = read_dir("profiles");
    definitions.extend(read_dir("custom-profiles"));
    let set = ProfileSet::compile(&definitions).expect("the profiles compile");
    SnapshotValidator::new(Arc::new(set), Arc::new(OctofhirEngine::new()))
}

const VITALS: &str = "http://hl7.org/fhir/StructureDefinition/vitalsigns";
const HEART_RATE: &str = "http://hl7.org/fhir/StructureDefinition/heartrate";
const BP: &str = "http://hl7.org/fhir/StructureDefinition/bp";
const UG_PATIENT: &str = "http://example.org/fhir/StructureDefinition/ug-patient";

/// Tiers 1–3 together, as the write path runs them; the issues as (code, location).
async fn issues(resource: &Value, profiles: &[&str]) -> Vec<(IssueCode, String)> {
    let mut all = validate_value(resource);
    assert!(all.is_empty(), "tier 1: {all:?}");
    all.extend(validate_invariants(&OctofhirEngine::new(), resource).await);
    let profiles: Vec<String> = profiles.iter().map(|p| (*p).to_owned()).collect();
    all.extend(validator().validate(resource, &profiles).await);
    all.into_iter().map(|i| (i.code, i.location)).collect()
}

fn one(code: IssueCode, location: &str) -> Vec<(IssueCode, String)> {
    vec![(code, location.to_owned())]
}

#[tokio::test]
async fn the_official_heart_rate_example_passes() {
    let heart_rate = example("observation-example-heart-rate.json");
    assert_eq!(issues(&heart_rate, &[VITALS, HEART_RATE]).await, Vec::new());
}

#[tokio::test]
async fn a_missing_vital_signs_category_fails() {
    let mut heart_rate = example("observation-example-heart-rate.json");
    heart_rate["category"] = json!([{ "text": "not a vital sign" }]);
    let found = issues(&heart_rate, &[VITALS, HEART_RATE]).await;
    assert!(
        found.contains(&(IssueCode::Required, "Observation.category".to_owned())),
        "{found:?}"
    );
}

#[tokio::test]
async fn a_wrong_loinc_code_fails() {
    let mut heart_rate = example("observation-example-heart-rate.json");
    heart_rate["code"]["coding"][0]["code"] = json!("8310-5");
    let found = issues(&heart_rate, &[HEART_RATE]).await;
    assert!(
        found.contains(&(IssueCode::Required, "Observation.code.coding".to_owned())),
        "{found:?}"
    );
}

#[tokio::test]
async fn a_wrong_fixed_unit_fails() {
    let mut heart_rate = example("observation-example-heart-rate.json");
    heart_rate["valueQuantity"]["code"] = json!("kg");
    let found = issues(&heart_rate, &[HEART_RATE]).await;
    assert!(
        found.contains(&(
            IssueCode::Value,
            "Observation.valueQuantity.code".to_owned()
        )),
        "{found:?}"
    );
}

#[tokio::test]
async fn blood_pressure_components_are_sliced_by_nested_codings() {
    let bp = example("observation-example-bloodpressure.json");
    assert_eq!(issues(&bp, &[VITALS, BP]).await, Vec::new());
    let mut swapped = bp.clone();
    swapped["component"][0]["code"]["coding"][0]["code"] = json!("0000-0");
    let found = issues(&swapped, &[BP]).await;
    assert!(
        found.contains(&(IssueCode::Required, "Observation.component".to_owned())),
        "the systolic slice is missing: {found:?}"
    );
}

fn patient(extra: Value) -> Value {
    let mut base = json!({
        "resourceType": "Patient",
        "identifier": [{ "system": "urn:ug:nin", "value": "CM90" }],
        "name": [{ "use": "official", "family": "Akello" }, { "use": "usual", "given": ["Grace"] }],
    });
    for (k, v) in extra.as_object().expect("object") {
        base[k] = v.clone();
    }
    base
}

#[tokio::test]
async fn a_conforming_patient_passes() {
    let valid = patient(json!({
        "maritalStatus": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/v3-MaritalStatus", "code": "M", "display": "Married" }] },
        "deceasedBoolean": false,
        "telecom": [{ "system": "phone", "value": "0772" }],
        "generalPractitioner": [{ "reference": "Practitioner/p1" }],
        "extension": [{ "url": "http://example.org/fhir/StructureDefinition/ug-tribe", "valueString": "Langi" }],
        "multipleBirthBoolean": false,
    }));
    assert_eq!(issues(&valid, &[UG_PATIENT]).await, Vec::new());
}

#[tokio::test]
async fn closed_slicing_and_slice_cardinality() {
    let unknown = patient(
        json!({ "identifier": [{ "system": "urn:ug:nin", "value": "CM90" }, { "system": "urn:other", "value": "x" }] }),
    );
    assert_eq!(
        issues(&unknown, &[UG_PATIENT]).await,
        one(IssueCode::Structure, "Patient.identifier[1]")
    );
    let missing = patient(json!({ "identifier": [{ "system": "urn:ug:mrn", "value": "M1" }] }));
    assert_eq!(
        issues(&missing, &[UG_PATIENT]).await,
        one(IssueCode::Required, "Patient.identifier")
    );
}

#[tokio::test]
async fn ordered_and_open_at_end_slicing() {
    let out_of_order = patient(
        json!({ "name": [{ "use": "usual", "given": ["Grace"] }, { "use": "official", "family": "Akello" }] }),
    );
    assert_eq!(
        issues(&out_of_order, &[UG_PATIENT]).await,
        one(IssueCode::Structure, "Patient.name")
    );
    let unsliced_first = patient(
        json!({ "name": [{ "use": "nickname", "given": ["G"] }, { "use": "official", "family": "Akello" }] }),
    );
    assert_eq!(
        issues(&unsliced_first, &[UG_PATIENT]).await,
        one(IssueCode::Structure, "Patient.name[0]")
    );
}

#[tokio::test]
async fn exists_and_type_discriminators() {
    let two_current = patient(
        json!({ "telecom": [{ "system": "phone", "value": "1" }, { "system": "phone", "value": "2" }] }),
    );
    assert_eq!(
        issues(&two_current, &[UG_PATIENT]).await,
        one(IssueCode::Structure, "Patient.telecom")
    );
    let date = patient(json!({ "deceasedDateTime": "2026-01-01" }));
    assert_eq!(
        issues(&date, &[UG_PATIENT]).await,
        vec![
            (IssueCode::Structure, "Patient.deceasedDateTime".to_owned()),
            (IssueCode::Structure, "Patient.deceasedDateTime".to_owned()),
        ],
        "dateTime is narrowed out of deceased[x], and its slice allows no value"
    );
}

#[tokio::test]
async fn profile_discriminator() {
    // Patient.address is closed, sliced by profile: every address must conform to ug-address.
    let district = patient(json!({ "address": [{ "city": "Lira", "district": "Lira" }] }));
    assert_eq!(issues(&district, &[UG_PATIENT]).await, Vec::new());
    let no_district = patient(json!({ "address": [{ "city": "Lira" }] }));
    assert_eq!(
        issues(&no_district, &[UG_PATIENT]).await,
        one(IssueCode::Structure, "Patient.address[0]"),
        "an address without a district is not a ug-address"
    );
}

#[tokio::test]
async fn patterns_types_targets_extensions_and_invariants() {
    let wrong_pattern = patient(
        json!({ "maritalStatus": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/v3-MaritalStatus", "code": "S" }] } }),
    );
    assert_eq!(
        issues(&wrong_pattern, &[UG_PATIENT]).await,
        one(IssueCode::Value, "Patient.maritalStatus")
    );
    let wrong_type = patient(json!({ "multipleBirthInteger": 2 }));
    assert_eq!(
        issues(&wrong_type, &[UG_PATIENT]).await,
        one(IssueCode::Structure, "Patient.multipleBirthInteger")
    );
    let wrong_target =
        patient(json!({ "generalPractitioner": [{ "reference": "Organization/o1" }] }));
    assert_eq!(
        issues(&wrong_target, &[UG_PATIENT]).await,
        one(IssueCode::Structure, "Patient.generalPractitioner[0]")
    );
    let wrong_value = patient(
        json!({ "extension": [{ "url": "http://example.org/fhir/StructureDefinition/ug-tribe", "valueInteger": 3 }] }),
    );
    assert_eq!(
        issues(&wrong_value, &[UG_PATIENT]).await,
        one(IssueCode::Structure, "Patient.extension[0].valueInteger")
    );
    let wrong_context = patient(
        json!({ "name": [{ "use": "official", "family": "A", "extension": [{ "url": "http://example.org/fhir/StructureDefinition/ug-tribe", "valueString": "Langi" }] }] }),
    );
    assert_eq!(
        issues(&wrong_context, &[UG_PATIENT]).await,
        one(IssueCode::Structure, "Patient.name[0].extension[0]")
    );
    let mut nameless = patient(json!({}));
    nameless.as_object_mut().expect("object").remove("name");
    assert_eq!(
        issues(&nameless, &[UG_PATIENT]).await,
        one(IssueCode::Invariant, "Patient")
    );
}

#[tokio::test]
async fn profiles_must_be_active_and_for_the_type() {
    let heart_rate = example("observation-example-heart-rate.json");
    assert_eq!(
        issues(&heart_rate, &["http://example.org/not-active"]).await,
        one(IssueCode::Processing, "Observation")
    );
    assert_eq!(
        issues(&heart_rate, &[UG_PATIENT]).await,
        one(IssueCode::Structure, "Observation")
    );
}

#[test]
fn differential_only_and_unsupported_profiles_fail_to_compile() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/diff-only-profile.json");
    let differential: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("json");
    let errors = ProfileSet::compile(&[differential]).expect_err("refused");
    assert!(errors[0].reason.contains("no snapshot"), "{errors:?}");

    // Without ug-address in the set, the profile discriminator cannot be checked.
    let mut definitions: Vec<Value> = read_dir("custom-profiles")
        .into_iter()
        .filter(|d| d["id"] != "ug-address")
        .collect();
    let errors = ProfileSet::compile(&definitions).expect_err("refused");
    assert!(
        errors.iter().any(|e| e.reason.contains("ug-address")),
        "{errors:?}"
    );

    // A choice element is sliced by type only.
    let mut patient = read_dir("custom-profiles")
        .into_iter()
        .find(|d| d["id"] == "ug-patient")
        .expect("ug-patient");
    patient["snapshot"]["element"]
        .as_array_mut()
        .expect("elements")
        .iter_mut()
        .find(|e| e["id"] == "Patient.deceased[x]")
        .expect("deceased[x]")["slicing"]["discriminator"] =
        json!([{ "type": "value", "path": "$this" }]);
    let errors = ProfileSet::compile(&[patient]).expect_err("refused");
    assert!(
        errors.iter().any(|e| e.reason.contains("choice element")),
        "{errors:?}"
    );

    // A resolve() discriminator is out of v1.
    definitions = read_dir("profiles");
    let mut resolving = definitions[0].clone();
    resolving["url"] = json!("http://example.org/resolving");
    let elements = resolving["snapshot"]["element"]
        .as_array_mut()
        .expect("elements");
    let sliced = elements
        .iter_mut()
        .find(|e| e.get("slicing").is_some())
        .expect("a sliced element");
    sliced["slicing"]["discriminator"] =
        json!([{ "type": "value", "path": "subject.resolve().id" }]);
    definitions.push(resolving);
    let errors = ProfileSet::compile(&definitions).expect_err("refused");
    assert!(
        errors.iter().any(|e| e.reason.contains("resolve()")),
        "{errors:?}"
    );
}

/// With `HEALTH_FHIR_R5_EXAMPLES` set: every official example that claims one of the fixture
/// profiles in `meta.profile` conforms to it.
#[tokio::test(flavor = "multi_thread")]
async fn official_examples_conform_to_the_profiles_they_claim_when_available() {
    let Some(dir) = std::env::var_os("HEALTH_FHIR_R5_EXAMPLES") else {
        eprintln!("skipped: set HEALTH_FHIR_R5_EXAMPLES (tools/fhir/fetch-r5-examples.sh)");
        return;
    };
    let validator = validator();
    let set = ProfileSet::compile(&read_dir("profiles")).expect("the profiles compile");
    let (mut checked, mut failures) = (0, Vec::new());
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("corpus")
        .map(|e| e.expect("entry").path())
        .collect();
    files.sort();
    for file in files
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
    {
        let resource: Value =
            serde_json::from_str(&std::fs::read_to_string(file).expect("read")).expect("json");
        let claimed: Vec<String> = resource["meta"]["profile"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|url| set.urls().any(|u| u == *url))
            .map(str::to_owned)
            .collect();
        if claimed.is_empty() {
            continue;
        }
        checked += 1;
        for issue in validator.validate(&resource, &claimed).await {
            failures.push(format!(
                "{}: {:?} at {}: {}",
                file.display(),
                issue.code,
                issue.location,
                issue.message
            ));
        }
    }
    eprintln!("{checked} examples claim a fixture profile");
    assert!(checked >= 12, "{checked}");
    assert_eq!(failures, Vec::<String>::new());
}
