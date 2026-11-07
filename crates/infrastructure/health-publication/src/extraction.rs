//! Extraction Mappings at validation (#93; questionnaire-extraction.md § Publication gates; ADR
//! 0052): every `ExtractionMappingLibrary` in a package must
//! 1. be a usable mapping, every expression of which evaluates;
//! 2. name, in each template, an Owner use case that accepts what the template produces;
//! 3. name only items the package's target Questionnaire versions have;
//! 4. round the whole result of every value expression that computes;
//! 5. ship at least one `tests/` fixture (`type` `extraction-fixture`), each of which gives
//!    exactly its expected Bundle, every output valid FHIR.
//!
//! A fixture naming a mapping the package does not hold fails too.

use health_clinical_documentation_app::{OPT_INS, in_range};
use health_fhir_extraction::Mapping;
use health_fhir_extraction::gates::{
    Fixture, fixture, mistyped, unknown_items, unparsable, unrounded,
};
use health_fhir_validation::OctofhirEngine;
use health_packages::Packed;
use serde_json::Value;

/// The `type` of an extraction fixture in `tests/`.
pub const FIXTURE_TYPE: &str = "extraction-fixture";

fn is_mapping(library: &Value) -> bool {
    library["resourceType"] == "Library"
        && library["type"]["coding"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|c| c["code"] == "extraction-mapping")
}

/// The package's resources of `resource_type`, each with its file.
fn artefacts(packed: &Packed, resource_type: &str) -> Vec<(String, Value)> {
    packed
        .contents
        .artefacts
        .iter()
        .filter(|a| a.resource_type == resource_type)
        .filter_map(|a| {
            let value = serde_json::from_slice::<Value>(packed.files.get(&a.path)?).ok()?;
            Some((a.path.clone(), value))
        })
        .collect()
}

/// The package's extraction fixtures: each file, the mapping it names, and the fixture.
fn fixtures(packed: &Packed, problems: &mut Vec<String>) -> Vec<(String, String, Fixture)> {
    let mut found = Vec::new();
    for (path, bytes) in &packed.files {
        if !path.starts_with("tests/") || !path.ends_with(".json") {
            continue;
        }
        let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        if value["type"] != FIXTURE_TYPE {
            continue;
        }
        let Some(mapping) = value["mapping"].as_str() else {
            problems.push(format!("{path}: a fixture names its mapping (url|version)"));
            continue;
        };
        if !value["response"].is_object() || !value["expected"].is_object() {
            problems.push(format!(
                "{path}: a fixture has a response and an expected Bundle"
            ));
            continue;
        }
        found.push((
            path.clone(),
            mapping.to_owned(),
            Fixture {
                response: value["response"].clone(),
                expected: value["expected"].clone(),
            },
        ));
    }
    found
}

/// Every problem with the package's Extraction Mappings, each prefixed with its file.
pub(crate) async fn problems(packed: &Packed) -> Vec<String> {
    let engine = OctofhirEngine::new();
    let mut problems = Vec::new();
    let mut mappings = Vec::new();
    for (path, library) in artefacts(packed, "Library") {
        if !is_mapping(&library) {
            continue;
        }
        match Mapping::from_library(&library) {
            Ok(mapping) => mappings.push((path, mapping)),
            Err(error) => problems.push(format!("{path}: {error}")),
        }
    }
    let questionnaires = artefacts(packed, "Questionnaire");
    let fixtures = fixtures(packed, &mut problems);
    for (path, mapping) in &mappings {
        let mut found = Vec::new();
        for template in &mapping.templates {
            let accepted = OPT_INS.iter().any(|o| {
                o.use_case == template.owner_use_case && o.resource_type == template.resource_type
            });
            if !accepted {
                found.push(format!(
                    "template {}: no Owner use case {} accepts a {}",
                    template.id, template.owner_use_case, template.resource_type
                ));
            }
        }
        found.extend(unparsable(&engine, mapping).await);
        found.extend(unrounded(mapping));
        for (_, questionnaire) in &questionnaires {
            let version = questionnaire["version"].as_str().unwrap_or_default();
            if questionnaire["url"] == mapping.questionnaire.as_str()
                && in_range(&mapping.versions, version)
            {
                found.extend(unknown_items(mapping, questionnaire));
                found.extend(mistyped(&engine, mapping, questionnaire).await);
            }
        }
        let canonical = mapping.canonical();
        let own: Vec<_> = fixtures
            .iter()
            .filter(|(_, m, _)| *m == canonical)
            .collect();
        if own.is_empty() {
            found.push(format!(
                "no tests/ fixture of type {FIXTURE_TYPE} names {canonical}"
            ));
        }
        for (fixture_path, _, case) in own {
            found.extend(
                fixture(&engine, mapping, case)
                    .await
                    .into_iter()
                    .map(|p| format!("{fixture_path}: {p}")),
            );
        }
        problems.extend(found.into_iter().map(|p| format!("{path}: {p}")));
    }
    for (path, named, _) in &fixtures {
        if !mappings.iter().any(|(_, m)| m.canonical() == *named) {
            problems.push(format!(
                "{path}: the package holds no Extraction Mapping {named}"
            ));
        }
    }
    problems
}
