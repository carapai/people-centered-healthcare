//! Writes the cases for the HL7 Java validator differential (#166; ADR 0051, ADR 0053):
//!
//! ```text
//! cargo run -p health-fhir-validation --example differential_cases -- <R5 examples dir> <out dir>
//! ```
//!
//! - `<out>/corpus/`: the official clinical examples (conformance and knowledge resources are
//!   left out, as in `tests/performance.rs`; the Java validator also fails on some of them
//!   with terminology off).
//! - `<out>/tier1/`: invalid variants of the first official example of each clinical resource
//!   type: an
//!   unknown element, a missing required element, a malformed `id`, a wrong JSON type, and an
//!   empty string.
//! - `<out>/tier3/`: resources that claim the vital-signs or custom profiles in `meta.profile`,
//!   valid and invalid, covering every slicing rule and discriminator.
//!
//! `tests/java_differential.rs` then compares the Java validator's verdict on each file with
//! ours.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(corpus), Some(out)) = (args.next(), args.next()) else {
        eprintln!("usage: differential_cases <R5 examples dir> <out dir>");
        std::process::exit(2);
    };
    let out = PathBuf::from(out);
    let clinical = clinical_examples(Path::new(&corpus));
    let tier1 = tier1_cases(Path::new(&corpus));
    let tier3 = tier3_cases();
    write_all(&out.join("corpus"), &clinical);
    write_all(&out.join("tier1"), &tier1);
    write_all(&out.join("tier3"), &tier3);
    eprintln!(
        "{} clinical examples, {} tier 1 cases, {} tier 3 cases in {}",
        clinical.len(),
        tier1.len(),
        tier3.len(),
        out.display()
    );
}

fn write_all(dir: &Path, cases: &BTreeMap<String, Value>) {
    std::fs::create_dir_all(dir).expect("create the case directory");
    for (name, resource) in cases {
        let text = serde_json::to_string_pretty(resource).expect("serialize");
        std::fs::write(dir.join(format!("{name}.json")), text).expect("write a case");
    }
}

/// Conformance and knowledge resources: authored as packages, not written by clinical users.
/// The same list as `tests/performance.rs`.
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

fn clinical_examples(corpus: &Path) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(corpus)
        .expect("the examples directory")
        .flatten()
    {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let Ok(resource) =
            serde_json::from_str::<Value>(&std::fs::read_to_string(&path).expect("read"))
        else {
            continue;
        };
        let resource_type = resource["resourceType"].as_str().unwrap_or_default();
        if !NOT_CLINICAL.contains(&resource_type) {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_owned();
            out.insert(stem, resource);
        }
    }
    out
}

/// The first official example of each resource type, by file name.
fn first_examples(corpus: &Path) -> BTreeMap<String, (String, Map<String, Value>)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(corpus)
        .expect("the examples directory")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    let mut first = BTreeMap::new();
    for file in files {
        let text = std::fs::read_to_string(&file).expect("read");
        let Ok(Value::Object(resource)) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(resource_type) = resource.get("resourceType").and_then(Value::as_str) else {
            continue;
        };
        if NOT_CLINICAL.contains(&resource_type) {
            continue;
        }
        // Valid examples only: a variant must fail for the one reason it introduces.
        if !health_fhir_validation::validate_value(&Value::Object(resource.clone())).is_empty() {
            continue;
        }
        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();
        first
            .entry(resource_type.to_owned())
            .or_insert((stem, resource));
    }
    first
}

fn tier1_cases(corpus: &Path) -> BTreeMap<String, Value> {
    let mut cases = BTreeMap::new();
    for (resource_type, (stem, resource)) in first_examples(corpus) {
        let mut add = |variant: &str, changed: Map<String, Value>| {
            cases.insert(format!("{stem}.{variant}"), Value::Object(changed));
        };

        let mut unknown = resource.clone();
        unknown.insert("notAnElement".into(), json!("x"));
        add("unknown-element", unknown);

        // The skeleton holds exactly the required top-level elements.
        if let Some(Value::Object(skeleton)) = health_fhir_validation::skeleton(&resource_type)
            && let Some(required) = skeleton
                .keys()
                .find(|k| *k != "resourceType" && resource.contains_key(*k))
        {
            let mut missing = resource.clone();
            missing.remove(required);
            add("missing-required", missing);
        }

        let mut bad_id = resource.clone();
        bad_id.insert("id".into(), json!("not a valid id!"));
        add("bad-id", bad_id);

        let mut wrong_type = resource.clone();
        wrong_type.insert("language".into(), json!(42));
        add("wrong-json-type", wrong_type);

        let mut empty = resource;
        empty.insert("implicitRules".into(), json!(""));
        add("empty-string", empty);
    }
    cases
}

const VITALS: &str = "http://hl7.org/fhir/StructureDefinition/vitalsigns";
const HEART_RATE: &str = "http://hl7.org/fhir/StructureDefinition/heartrate";
const BP: &str = "http://hl7.org/fhir/StructureDefinition/bp";
const UG_PATIENT: &str = "http://example.org/fhir/StructureDefinition/ug-patient";
const UG_TRIBE: &str = "http://example.org/fhir/StructureDefinition/ug-tribe";

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/profile-examples")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("json")
}

fn claiming(mut resource: Value, profiles: &[&str]) -> Value {
    resource["meta"] = json!({ "profile": profiles });
    resource
}

fn patient(extra: Value) -> Value {
    let mut base = json!({
        "resourceType": "Patient",
        "identifier": [{ "system": "urn:ug:nin", "value": "CM90" }],
        "name": [{ "use": "official", "family": "Akello" }, { "use": "usual", "given": ["Grace"] }],
    });
    for (k, v) in extra.as_object().expect("object") {
        if v.is_null() {
            base.as_object_mut().expect("object").remove(k);
        } else {
            base[k] = v.clone();
        }
    }
    claiming(base, &[UG_PATIENT])
}

/// The tier 3 scenarios of `tests/profiles.rs`, as files that claim their profiles.
fn tier3_cases() -> BTreeMap<String, Value> {
    let mut cases = BTreeMap::new();
    let heart_rate = claiming(
        fixture("observation-example-heart-rate.json"),
        &[VITALS, HEART_RATE],
    );
    let bp = claiming(
        fixture("observation-example-bloodpressure.json"),
        &[VITALS, BP],
    );
    let tribe = |value: Value| {
        json!({ "url": UG_TRIBE })
            .as_object()
            .cloned()
            .map(|mut o| {
                o.extend(value.as_object().expect("object").clone());
                Value::Object(o)
            })
            .expect("extension")
    };

    cases.insert("heart-rate.valid".into(), heart_rate.clone());
    let mut c = heart_rate.clone();
    c["category"] = json!([{ "text": "not a vital sign" }]);
    cases.insert("heart-rate.missing-category".into(), c);
    let mut c = heart_rate.clone();
    c["code"]["coding"][0]["code"] = json!("8310-5");
    cases.insert("heart-rate.wrong-loinc".into(), c);
    let mut c = heart_rate;
    c["valueQuantity"]["code"] = json!("kg");
    cases.insert("heart-rate.wrong-unit".into(), c);

    cases.insert("bp.valid".into(), bp.clone());
    let mut c = bp;
    c["component"][0]["code"]["coding"][0]["code"] = json!("0000-0");
    cases.insert("bp.missing-systolic".into(), c);

    cases.insert("ug-patient.valid".into(), patient(json!({
        "maritalStatus": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/v3-MaritalStatus", "code": "M", "display": "Married" }] },
        "deceasedBoolean": false,
        "telecom": [{ "system": "phone", "value": "0772" }],
        "generalPractitioner": [{ "reference": "Practitioner/p1" }],
        "extension": [tribe(json!({ "valueString": "Langi" }))],
        "multipleBirthBoolean": false,
    })));
    cases.insert("ug-patient.closed-slicing".into(), patient(json!({ "identifier": [{ "system": "urn:ug:nin", "value": "CM90" }, { "system": "urn:other", "value": "x" }] })));
    cases.insert(
        "ug-patient.slice-cardinality".into(),
        patient(json!({ "identifier": [{ "system": "urn:ug:mrn", "value": "M1" }] })),
    );
    cases.insert("ug-patient.out-of-order".into(), patient(json!({ "name": [{ "use": "usual", "given": ["Grace"] }, { "use": "official", "family": "Akello" }] })));
    cases.insert("ug-patient.open-at-end".into(), patient(json!({ "name": [{ "use": "nickname", "given": ["G"] }, { "use": "official", "family": "Akello" }] })));
    cases.insert("ug-patient.exists-discriminator".into(), patient(json!({ "telecom": [{ "system": "phone", "value": "1" }, { "system": "phone", "value": "2" }] })));
    cases.insert(
        "ug-patient.type-discriminator".into(),
        patient(json!({ "deceasedDateTime": "2026-01-01" })),
    );
    cases.insert("ug-patient.wrong-pattern".into(), patient(json!({ "maritalStatus": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/v3-MaritalStatus", "code": "S" }] } })));
    cases.insert(
        "ug-patient.wrong-type".into(),
        patient(json!({ "multipleBirthInteger": 2 })),
    );
    cases.insert(
        "ug-patient.wrong-target".into(),
        patient(json!({ "generalPractitioner": [{ "reference": "Organization/o1" }] })),
    );
    cases.insert(
        "ug-patient.extension-value".into(),
        patient(json!({ "extension": [tribe(json!({ "valueInteger": 3 }))] })),
    );
    cases.insert("ug-patient.extension-context".into(), patient(json!({ "name": [{ "use": "official", "family": "A", "extension": [tribe(json!({ "valueString": "Langi" }))] }] })));
    cases.insert(
        "ug-patient.invariant".into(),
        patient(json!({ "name": null })),
    );
    cases.insert(
        "ug-patient.profile-discriminator-valid".into(),
        patient(json!({ "address": [{ "city": "Lira", "district": "Lira" }] })),
    );
    cases.insert(
        "ug-patient.profile-discriminator".into(),
        patient(json!({ "address": [{ "city": "Lira" }] })),
    );
    cases
}
