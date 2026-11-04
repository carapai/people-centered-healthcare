use health_terminology::{ExpandRequest, Subsumption, Validation};
use serde_json::json;

use super::*;

fn files(resources: &[Value]) -> Files {
    resources
        .iter()
        .enumerate()
        .map(|(i, r)| (format!("resource-{i}.json"), r.to_string().into_bytes()))
        .collect()
}

fn signs() -> Value {
    json!({
        "resourceType": "CodeSystem", "url": "urn:signs", "version": "1.0.0",
        "status": "active", "content": "complete", "caseSensitive": true,
        "property": [{ "code": "inactive", "type": "boolean" }],
        "concept": [{
            "code": "danger", "display": "Danger signs",
            "concept": [
                { "code": "fits", "display": "Convulsions",
                  "designation": [{ "language": "lg", "use": { "code": "synonym" }, "value": "Okukonziba" }] },
                { "code": "fever", "display": "Fever",
                  "property": [{ "code": "inactive", "valueBoolean": true }] }
            ]
        }]
    })
}

#[test]
fn a_package_s_terminology_is_read_and_served() {
    let danger = json!({
        "resourceType": "ValueSet", "url": "urn:danger", "version": "1.0.0", "status": "active",
        "compose": { "inactive": false, "include": [{
            "system": "urn:signs",
            "filter": [{ "property": "concept", "op": "descendent-of", "value": "danger" }]
        }]}
    });
    let pinned = json!({
        "resourceType": "ValueSet", "url": "urn:icd-subset", "version": "1.0.0", "status": "active",
        "compose": { "include": [{ "system": "http://hl7.org/fhir/sid/icd-10" }] },
        "expansion": { "timestamp": "2026-11-04T00:00:00Z", "contains": [
            { "system": "http://hl7.org/fhir/sid/icd-10", "code": "O15", "display": "Eclampsia",
              "contains": [{ "system": "http://hl7.org/fhir/sid/icd-10", "code": "O15.0", "display": "Eclampsia in pregnancy" }] },
            { "abstract": true, "display": "a heading" }
        ]}
    });
    let map = json!({
        "resourceType": "ConceptMap", "url": "urn:map", "version": "1.0.0", "status": "active",
        "group": [{
            "source": "urn:signs|1.0.0", "target": "http://hl7.org/fhir/sid/icd-10",
            "element": [
                { "code": "fits", "target": [{ "code": "O15", "relationship": "source-is-broader-than-target" }] },
                { "code": "fever", "noMap": true }
            ]
        }]
    });
    let other = json!({ "resourceType": "Questionnaire", "url": "urn:form" });
    let mut terminology = Terminology::new();
    let mut package = files(&[signs(), danger, pinned, map, other]);
    package.insert("tests/case.json".into(), b"not json".to_vec());
    load(&mut terminology, &package).expect("load");

    let fits = terminology
        .lookup("urn:signs", None, "fits")
        .expect("lookup");
    assert_eq!(fits.parents, ["danger"]);
    assert_eq!(
        fits.concept.designations[0].use_code.as_deref(),
        Some("synonym")
    );
    assert_eq!(
        terminology.subsumes("urn:signs", None, "danger", "fever"),
        Ok(Subsumption::Subsumes)
    );

    let danger = terminology
        .expand("urn:danger", &ExpandRequest::default())
        .expect("expand");
    let codes: Vec<&str> = danger.contains.iter().map(|c| c.code.as_str()).collect();
    assert_eq!(
        codes,
        ["fits"],
        "the inactive fever is left out by the compose"
    );

    let pinned = terminology
        .expand("urn:icd-subset", &ExpandRequest::default())
        .expect("the pinned expansion, without ICD-10");
    let codes: Vec<&str> = pinned.contains.iter().map(|c| c.code.as_str()).collect();
    assert_eq!(
        codes,
        ["O15", "O15.0"],
        "nested codes flattened, headings skipped"
    );
    assert_eq!(
        terminology.validate_code(
            "urn:icd-subset",
            "http://hl7.org/fhir/sid/icd-10",
            "O15.0",
            None
        ),
        Ok(Validation::Valid {
            display: Some("Eclampsia in pregnancy".into())
        })
    );

    let translated = terminology
        .translate(Some("urn:map"), "urn:signs", "fits", None)
        .expect("translate");
    assert_eq!(translated[0].target.code.as_deref(), Some("O15"));
    let unmapped = terminology
        .translate(Some("urn:map"), "urn:signs", "fever", None)
        .expect("translate");
    assert_eq!(unmapped[0].target.code, None);
}

#[test]
fn malformed_terminology_is_refused_not_skipped() {
    let cases = [
        json!({ "resourceType": "CodeSystem", "content": "complete" }),
        json!({ "resourceType": "CodeSystem", "url": "urn:x", "content": "everything" }),
        json!({ "resourceType": "CodeSystem", "url": "urn:x", "content": "complete", "concept": [{ "display": "no code" }] }),
        json!({ "resourceType": "ValueSet", "url": "urn:x", "compose": { "include": [{
            "system": "urn:signs", "filter": [{ "property": "concept", "op": "descendent-leaf", "value": "danger" }] }] } }),
        json!({ "resourceType": "ValueSet", "url": "urn:x", "compose": { "include": [{
            "system": "urn:signs", "filter": [{ "property": "concept", "op": "is-a" }] }] } }),
        json!({ "resourceType": "ConceptMap", "url": "urn:x", "group": [{ "source": "urn:a" }] }),
        json!({ "resourceType": "ConceptMap", "url": "urn:x", "group": [{ "source": "urn:a", "target": "urn:b",
            "element": [{ "code": "a", "target": [{ "code": "b", "relationship": "wider" }] }] }] }),
    ];
    for case in cases {
        let mut terminology = Terminology::new();
        assert!(
            matches!(
                load(&mut terminology, &files(std::slice::from_ref(&case))),
                Err(Error::Terminology { .. })
            ),
            "{case}"
        );
    }
}

#[test]
fn a_code_system_that_does_not_say_is_case_sensitive() {
    let mut system = signs();
    system
        .as_object_mut()
        .expect("object")
        .remove("caseSensitive");
    let mut terminology = Terminology::new();
    load(&mut terminology, &files(&[system])).expect("load");
    assert!(terminology.lookup("urn:signs", None, "FITS").is_err());
}

#[test]
fn the_core_package_s_template_modes_expand_from_its_code_system() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/fhir-core-package/package");
    let package: Files = std::fs::read_dir(dir)
        .expect("the core package")
        .map(|entry| {
            let path = entry.expect("entry").path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .expect("name")
                .to_owned();
            (name, std::fs::read(&path).expect("read"))
        })
        .collect();
    let mut terminology = Terminology::new();
    load(&mut terminology, &package).expect("load");
    let base = "https://carapai.github.io/people-centered-healthcare/fhir";
    let modes = terminology
        .expand(
            &format!("{base}/ValueSet/template-modes"),
            &ExpandRequest::default(),
        )
        .expect("expand");
    let codes: Vec<&str> = modes.contains.iter().map(|c| c.code.as_str()).collect();
    assert_eq!(codes, ["managed", "seed"]);
    assert_eq!(modes.contains[0].display.as_deref(), Some("Managed"));
    assert_eq!(
        terminology.validate_code(
            &format!("{base}/ValueSet/template-modes"),
            &format!("{base}/CodeSystem/platform"),
            "sometimes",
            None
        ),
        Ok(Validation::Invalid)
    );
}
