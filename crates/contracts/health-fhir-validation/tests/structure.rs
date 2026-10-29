//! Tier 1 structural validation (ADR 0051): what it rejects, with locations, and the corpus
//! check that it agrees with the model crate.

use health_fhir_validation::{IssueCode, resource_types, skeleton, validate_structure};

fn issues(json: &str) -> Vec<(IssueCode, String)> {
    validate_structure(json)
        .into_iter()
        .map(|i| (i.code, i.location))
        .collect()
}

fn one(json: &str, code: IssueCode, location: &str) {
    assert_eq!(issues(json), vec![(code, location.to_owned())], "{json}");
}

#[test]
fn valid_resources_pass() {
    for json in [
        r#"{"resourceType":"Patient"}"#,
        r#"{"resourceType":"Patient","id":"p1","name":[{"family":"Akello","given":["Grace"]}],"deceasedBoolean":false,"birthDate":"1990"}"#,
        r#"{"resourceType":"Observation","status":"final","code":{"text":"x"},"valueQuantity":{"value":1.50,"unit":"kg"},"component":[{"code":{"text":"y"},"valueInteger":3}]}"#,
        // A required primitive may carry only an extension (`_name`).
        r#"{"resourceType":"QuestionnaireResponse","status":"completed","_questionnaire":{"extension":[{"url":"http://hl7.org/fhir/StructureDefinition/display","valueString":"x"}]}}"#,
        // Primitive arrays align with `_name`, and nulls pair with extensions.
        r#"{"resourceType":"Patient","name":[{"given":["a",null],"_given":[null,{"extension":[{"url":"http://e","valueCode":"CL"}]}]}]}"#,
        // contentReference: Questionnaire.item.item is a Questionnaire.item.
        r#"{"resourceType":"Questionnaire","status":"draft","item":[{"linkId":"1","type":"group","item":[{"linkId":"1.1","type":"string"}]}]}"#,
        // Contained resources and bundle entries are resources.
        r#"{"resourceType":"Bundle","type":"collection","entry":[{"resource":{"resourceType":"Patient","contained":[{"resourceType":"Organization","name":"x"}]}}]}"#,
        // Decimals keep any precision; integers are bounded.
        r#"{"resourceType":"Observation","status":"final","code":{"text":"x"},"valueQuantity":{"value":123456789012.345678901234567890123456}}"#,
    ] {
        assert_eq!(issues(json), Vec::new(), "{json}");
    }
}

#[test]
fn unknown_elements_are_errors() {
    one(
        r#"{"resourceType":"Patient","nickname":"x"}"#,
        IssueCode::Structure,
        "Patient.nickname",
    );
    one(
        r#"{"resourceType":"Patient","name":[{"middle":"x"}]}"#,
        IssueCode::Structure,
        "Patient.name[0].middle",
    );
    one(
        r#"{"resourceType":"Observation","status":"final","code":{"text":"x"},"valueInteger64":"1"}"#,
        IssueCode::Structure,
        "Observation.valueInteger64",
    );
    one(
        r#"{"resourceType":"Patient","_nickname":{}}"#,
        IssueCode::Structure,
        "Patient.nickname",
    );
}

#[test]
fn resource_types_must_be_known_and_concrete() {
    one(r#"{"resourceType":"NotAType"}"#, IssueCode::Structure, "");
    one(
        r#"{"resourceType":"DomainResource"}"#,
        IssueCode::Structure,
        "",
    );
    one(r#"{"id":"x"}"#, IssueCode::Structure, "");
    one("[1]", IssueCode::Structure, "");
    one("{not json", IssueCode::Structure, "");
    one(
        r#"{"resourceType":"Patient","contained":[{"resourceType":"Nope"}]}"#,
        IssueCode::Structure,
        "Patient.contained[0]",
    );
    assert!(resource_types().count() >= 157);
}

#[test]
fn cardinality_is_enforced() {
    one(
        r#"{"resourceType":"Observation","code":{"text":"x"}}"#,
        IssueCode::Required,
        "Observation.status",
    );
    one(
        r#"{"resourceType":"Patient","name":{"family":"x"}}"#,
        IssueCode::Structure,
        "Patient.name",
    );
    one(
        r#"{"resourceType":"Patient","gender":["female"]}"#,
        IssueCode::Structure,
        "Patient.gender",
    );
    one(
        r#"{"resourceType":"Patient","name":[]}"#,
        IssueCode::Structure,
        "Patient.name",
    );
    one(
        r#"{"resourceType":"Questionnaire","status":"draft","item":[{"type":"group"}]}"#,
        IssueCode::Required,
        "Questionnaire.item[0].linkId",
    );
}

#[test]
fn primitive_formats_are_enforced() {
    one(
        r#"{"resourceType":"Patient","birthDate":"1990-13-01"}"#,
        IssueCode::Value,
        "Patient.birthDate",
    );
    one(
        r#"{"resourceType":"Patient","active":"true"}"#,
        IssueCode::Structure,
        "Patient.active",
    );
    one(
        r#"{"resourceType":"Patient","id":"has spaces"}"#,
        IssueCode::Value,
        "Patient.id",
    );
    one(
        r#"{"resourceType":"Patient","name":[{"family":""}]}"#,
        IssueCode::Value,
        "Patient.name[0].family",
    );
    one(
        r#"{"resourceType":"Patient","multipleBirthInteger":1.5}"#,
        IssueCode::Value,
        "Patient.multipleBirthInteger",
    );
    one(
        r#"{"resourceType":"Patient","multipleBirthInteger":3000000000}"#,
        IssueCode::Value,
        "Patient.multipleBirthInteger",
    );
    one(
        r#"{"resourceType":"Observation","status":"final","code":{"text":"x"},"valueQuantity":{"value":"1.5"}}"#,
        IssueCode::Structure,
        "Observation.valueQuantity.value",
    );
}

#[test]
fn shape_rules_are_enforced() {
    one(
        r#"{"resourceType":"Patient","deceasedBoolean":true,"deceasedDateTime":"2020"}"#,
        IssueCode::Structure,
        "Patient",
    );
    one(
        r#"{"resourceType":"Patient","maritalStatus":{}}"#,
        IssueCode::Structure,
        "Patient.maritalStatus",
    );
    one(
        r#"{"resourceType":"Patient","name":[{"given":["a",null]}]}"#,
        IssueCode::Structure,
        "Patient.name[0].given[1]",
    );
    one(
        r#"{"resourceType":"Patient","gender":null}"#,
        IssueCode::Structure,
        "Patient.gender",
    );
    assert_eq!(
        issues(r#"{"resourceType":"Patient","name":[{"given":["a"],"_given":[null,null]}]}"#),
        vec![
            (IssueCode::Structure, "Patient.name[0]._given".to_owned()),
            (IssueCode::Structure, "Patient.name[0]._given[1]".to_owned()),
        ],
        "a length mismatch, and an entry with neither value nor extension"
    );
}

#[test]
fn every_issue_is_reported_with_its_location() {
    let found = issues(r#"{"resourceType":"Observation","code":{"text":""},"nickname":1}"#);
    assert_eq!(found.len(), 3, "{found:?}");
}

#[test]
fn skeletons_of_every_type_pass() {
    let mut count = 0;
    for resource_type in resource_types() {
        let skeleton = skeleton(resource_type).expect("skeleton");
        let found = issues(&skeleton.to_string());
        assert_eq!(found, Vec::new(), "{resource_type}: {skeleton}");
        count += 1;
    }
    assert!(count >= 157);
    assert!(skeleton("DomainResource").is_none());
}

/// Examples that pass tier 1 but that `fhir-r5` cannot parse: the model crate is stricter than
/// R5 here, so these are recorded gaps of the crate, not invalid data (ADR 0051: such a
/// mismatch is reported and alerted).
const MODEL_GAPS: &[&str] = health_fhir_models::KNOWN_UNPARSEABLE_EXAMPLES;

/// Set `HEALTH_FHIR_R5_EXAMPLES` to the unpacked official corpus
/// (`tools/fhir/fetch-r5-examples.sh`): every example must pass tier 1, and every one that
/// passes must parse into the typed model, apart from the recorded model gaps.
#[test]
fn the_official_corpus_passes_and_parses_when_available() {
    let Some(dir) = std::env::var_os("HEALTH_FHIR_R5_EXAMPLES") else {
        eprintln!("skipped: set HEALTH_FHIR_R5_EXAMPLES (tools/fhir/fetch-r5-examples.sh)");
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("corpus")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    assert!(files.len() >= 2824, "{} files", files.len());
    let mut failures = Vec::new();
    for file in &files {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let text = std::fs::read_to_string(file).expect("read");
        let found = validate_structure(&text);
        if let Some(first) = found.first() {
            failures.push(format!(
                "{name}: tier 1 {} at {}: {}",
                first.code.as_str(),
                first.location,
                first.message
            ));
            continue;
        }
        let parsed = health_fhir_models::parse_resource(&text).is_ok();
        match (parsed, MODEL_GAPS.contains(&name)) {
            (true, false) | (false, true) => {}
            (false, false) => {
                failures.push(format!("{name}: passes tier 1 but fails typed parsing"))
            }
            (true, true) => {
                failures.push(format!("{name}: parses now; remove it from the model gaps"))
            }
        }
    }
    eprintln!("{} examples checked", files.len());
    assert_eq!(failures, Vec::<String>::new());
}
