//! Program definitions at validation (#116; program-designer.md § Rules; MASTER_PROMPT §44):
//! every ProgramPlanDefinition (`PlanDefinition` of type `clinical-protocol`) in a package must
//! be a usable program definition, and every regimen it uses that the package holds (an
//! `ActivityDefinition` it names as `composed-of`) must cite one of the program's approved
//! protocol sources. Regimens are never hardcoded, and never from an unapproved source.
//!
//! A regimen the package does not hold comes from a dependency, whose own publication checked it.

use health_packages::Packed;
use health_programs::unapproved_regimens;
use health_programs_app::program;
use serde_json::Value;

fn unversioned(canonical: &str) -> &str {
    canonical.split('|').next().unwrap_or(canonical)
}

/// What a regimen cites: its `citation`, `documentation`, `justification` and `derived-from`
/// artifacts.
fn cited(regimen: &Value) -> Vec<String> {
    regimen["relatedArtifact"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| {
            matches!(
                a["type"].as_str(),
                Some("citation" | "documentation" | "justification" | "derived-from")
            )
        })
        .filter_map(|a| {
            a["resource"]
                .as_str()
                .or_else(|| a["document"]["url"].as_str())
                .or_else(|| a["citation"].as_str())
                .map(str::to_owned)
        })
        .collect()
}

fn is_program(plan: &Value) -> bool {
    plan["type"]["coding"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|c| c["code"] == "clinical-protocol")
}

/// Every problem with the package's program definitions, each prefixed with its file.
pub(crate) fn problems(packed: &Packed) -> Vec<String> {
    let documents = |resource_type: &str| -> Vec<(String, Value)> {
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
    };
    let regimens = documents("ActivityDefinition");
    let mut problems = Vec::new();
    for (path, plan) in documents("PlanDefinition") {
        if !is_program(&plan) {
            continue;
        }
        let defined = match program(&plan) {
            Ok(defined) => defined,
            Err(error) => {
                problems.push(format!("{path}: {error}"));
                continue;
            }
        };
        let held: Vec<(String, Vec<String>)> = defined
            .regimens
            .iter()
            .filter_map(|wanted| {
                regimens
                    .iter()
                    .find(|(_, r)| r["url"].as_str() == Some(unversioned(wanted)))
                    .map(|(_, r)| (wanted.clone(), cited(r)))
            })
            .collect();
        for regimen in unapproved_regimens(&defined, &held) {
            problems.push(format!(
                "{path}: the regimen {regimen} cites none of the program's approved protocol sources"
            ));
        }
    }
    problems
}
