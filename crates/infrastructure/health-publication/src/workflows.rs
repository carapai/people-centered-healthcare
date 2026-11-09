//! Workflow Definitions at validation (#96; workflow-engine.md § Simulation and publication
//! gates; ADR 0049): every `WorkflowDefinitionLibrary` in a package is read and checked against
//! the actions the release knows, so an unknown action or an invalid parameter fails
//! publication, and [`WorkflowSandbox`] runs their simulation scenarios (#101).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use health_packages::{Files, Packed};
use health_workflow::definition::WorkflowDefinition;
use health_workflow::simulation::{SCENARIO_TYPE, Scenario, simulate};
use health_workflow_actions::ActionDescriptor;
use serde_json::Value;

use crate::Sandbox;

/// The content type of a Workflow Definition (ADR 0049).
pub(crate) const CONTENT_TYPE: &str = "application/vnd.health.workflow+json";

/// Whether `library` is a Workflow Definition: its type is the platform's `workflow-definition`.
fn is_workflow(library: &Value) -> bool {
    library["resourceType"] == "Library"
        && library["type"]["coding"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|c| c["code"] == "workflow-definition")
}

/// The package's Workflow Definitions, each with its file, and every problem reading them.
fn read(packed: &Packed) -> (Vec<(String, WorkflowDefinition)>, Vec<String>) {
    workflow_definitions(&packed.files)
}

/// The Workflow Definitions among a package's top-level `files` (each a `Library` of type
/// `workflow-definition` with its definition inline, ADR 0049), each with its file, and every
/// problem reading them. Activation (#89) reads an active package's with it.
pub fn workflow_definitions(files: &Files) -> (Vec<(String, WorkflowDefinition)>, Vec<String>) {
    let mut definitions = Vec::new();
    let mut found = Vec::new();
    for (path, bytes) in files {
        if path.contains('/') || path.starts_with('.') || !path.ends_with(".json") {
            continue;
        }
        let Ok(library) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        if !is_workflow(&library) {
            continue;
        }
        let content = &library["content"][0];
        if content["contentType"] != CONTENT_TYPE {
            found.push(format!("{path}: the content is not {CONTENT_TYPE}"));
            continue;
        }
        // Content beyond 1 MiB would be a blob (ADR 0049); none is that large yet.
        let Some(data) = content["data"].as_str() else {
            found.push(format!("{path}: the definition must be inline content"));
            continue;
        };
        let definition = match STANDARD
            .decode(data)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                serde_json::from_slice::<WorkflowDefinition>(&bytes).map_err(|e| e.to_string())
            }) {
            Ok(definition) => definition,
            Err(error) => {
                found.push(format!("{path}: not a Workflow Definition: {error}"));
                continue;
            }
        };
        if Some(definition.url.as_str()) != library["url"].as_str()
            || Some(definition.version.as_str()) != library["version"].as_str()
        {
            found.push(format!(
                "{path}: the definition is {}|{}, not its Library's url and version",
                definition.url, definition.version
            ));
        }
        definitions.push((path.clone(), definition));
    }
    (definitions, found)
}

/// Every problem with the package's Workflow Definitions, each prefixed with its file.
pub(crate) fn problems(packed: &Packed, catalog: &[ActionDescriptor]) -> Vec<String> {
    let (definitions, mut found) = read(packed);
    for (path, definition) in &definitions {
        found.extend(
            definition
                .problems(catalog)
                .iter()
                .map(|problem| format!("{path}: {problem}")),
        );
    }
    found
}

/// Runs a package's workflow scenarios (#101; workflow-engine.md § Simulation and publication
/// gates): every `tests/` JSON file whose `type` is `workflow-scenario` runs against the
/// package's definition it names, on the same interpreter, with stubbed actions and guards.
/// Every scenario must pass; one naming a definition the package does not hold fails.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkflowSandbox;

impl Sandbox for WorkflowSandbox {
    fn simulate(&self, package: &Packed) -> Vec<String> {
        let (definitions, _) = read(package);
        let mut failed = Vec::new();
        for (path, bytes) in &package.files {
            if !path.starts_with("tests/") || !path.ends_with(".json") {
                continue;
            }
            let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
                continue;
            };
            if value["type"] != SCENARIO_TYPE {
                continue;
            }
            let scenario = match serde_json::from_value::<Scenario>(value) {
                Ok(scenario) => scenario,
                Err(error) => {
                    failed.push(format!("{path}: not a workflow scenario: {error}"));
                    continue;
                }
            };
            let Some((_, definition)) = definitions
                .iter()
                .find(|(_, d)| format!("{}|{}", d.url, d.version) == scenario.definition)
            else {
                failed.push(format!(
                    "{path}: the package holds no definition {}",
                    scenario.definition
                ));
                continue;
            };
            failed.extend(
                simulate(definition, &scenario)
                    .into_iter()
                    .map(|problem| format!("{path}: {problem}")),
            );
        }
        failed
    }
}
