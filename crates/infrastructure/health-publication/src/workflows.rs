//! Workflow Definitions at validation (#96; workflow-engine.md § Simulation and publication
//! gates; ADR 0049): every `WorkflowDefinitionLibrary` in a package is read and checked against
//! the actions the release knows, so an unknown action or an invalid parameter fails
//! publication.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use health_packages::Packed;
use health_workflow::definition::WorkflowDefinition;
use health_workflow_actions::ActionDescriptor;
use serde_json::Value;

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

/// Every problem with the package's Workflow Definitions, each prefixed with its file.
pub(crate) fn problems(packed: &Packed, catalog: &[ActionDescriptor]) -> Vec<String> {
    let mut found = Vec::new();
    for artefact in &packed.contents.artefacts {
        if artefact.resource_type != "Library" {
            continue;
        }
        let Some(library) = packed
            .files
            .get(&artefact.path)
            .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok())
        else {
            continue;
        };
        if !is_workflow(&library) {
            continue;
        }
        let path = &artefact.path;
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
        found.extend(
            definition
                .problems(catalog)
                .iter()
                .map(|problem| format!("{path}: {problem}")),
        );
    }
    found
}
