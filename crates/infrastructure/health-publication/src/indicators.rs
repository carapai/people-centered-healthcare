//! Indicators at validation (analytics.md; studio.md § Indicators; ADR 0068; #548): every
//! `Measure` in a package is an indicator, and must be a usable one: a period date that places
//! care in a Report's period, a scoring with the populations it needs, a basis, and FHIRPath
//! criteria. Every test patient in `tests/` (`type` `test-patient`) must hold one Patient and
//! their records, as *Try* evaluates indicators over them. Indicators that refer to each other in
//! a circle (#550) are refused: each would need the other's membership first.

use std::collections::{BTreeMap, BTreeSet};

use health_analytics::circles;
use health_analytics_app::{is_test_patient, measure, test_patient};
use health_packages::Packed;
use serde_json::Value;

/// Every problem with the package's indicators and test patients, each prefixed with its file.
pub(crate) fn problems(packed: &Packed) -> Vec<String> {
    let mut problems = Vec::new();
    let mut refers = BTreeMap::new();
    let mut paths = BTreeMap::new();
    for artefact in &packed.contents.artefacts {
        if artefact.resource_type != "Measure" {
            continue;
        }
        let Some(value) = packed
            .files
            .get(&artefact.path)
            .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok())
        else {
            continue;
        };
        let definition = measure(&value);
        if let Ok(indicator) = definition.indicator() {
            refers.insert(
                indicator.canonical(),
                indicator
                    .references
                    .iter()
                    .map(|r| r.measure.clone())
                    .collect::<BTreeSet<_>>(),
            );
            paths.insert(indicator.canonical(), artefact.path.clone());
        }
        problems.extend(
            definition
                .problems()
                .into_iter()
                .map(|problem| format!("{}: {problem}", artefact.path)),
        );
    }
    for circle in circles(&refers) {
        // One refers to itself is already a problem of its own.
        if circle.len() < 2 {
            continue;
        }
        let files: Vec<&str> = circle.iter().map(|c| paths[c].as_str()).collect();
        problems.push(format!(
            "{}: the indicators {} refer to each other in a circle",
            files.join(", "),
            circle.join(", ")
        ));
    }
    for (path, bytes) in &packed.files {
        if !path.starts_with("tests/") || !path.ends_with(".json") {
            continue;
        }
        let Ok(file) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        if is_test_patient(&file)
            && let Err(problem) = test_patient(&file)
        {
            problems.push(format!("{path}: {problem}"));
        }
    }
    problems
}
