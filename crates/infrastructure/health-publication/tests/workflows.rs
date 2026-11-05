//! Workflow Definitions at publication (#96): a package's `WorkflowDefinitionLibrary` is checked
//! against the actions the release knows, so an unknown action or an invalid parameter sends the
//! draft back. Its `tests/` scenarios run in the simulation sandbox (#101), and a definition with
//! an unreachable state cannot be published.

use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use health_configuration::lifecycle::{Stage, Step};
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_publication::{
    ActionDescriptor, Files, NoSandbox, Publications, Sandbox, WorkflowSandbox,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::FixedClock;
use health_workflow_actions::{Compensation, ParamKind, ParamSpec};
use serde_json::{Value, json};

const URL: &str = "https://health.go.ug/fhir/Library/opd-visit";

fn actions() -> Vec<ActionDescriptor> {
    vec![ActionDescriptor {
        id: "patient-flow/open-queue-entry",
        major: 1,
        permission: "patient-flow.manage",
        compensation: Compensation::Action("patient-flow/close-queue-entry"),
        params: vec![ParamSpec {
            name: "service_point",
            kind: ParamKind::Text,
            required: true,
        }],
    }]
}

async fn publications() -> (Publications, tempfile::TempDir) {
    publications_with(Arc::new(NoSandbox)).await
}

async fn publications_with(sandbox: Arc<dyn Sandbox>) -> (Publications, tempfile::TempDir) {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "central".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let clock = Arc::new(FixedClock::new(
        "2026-11-04T09:00:00Z".parse().expect("timestamp"),
    ));
    let dir = tempfile::tempdir().expect("tempdir");
    let publications = Publications::new(
        SurrealStore::new(connection, clock.clone()),
        FsBlobStore::open(dir.path()).expect("blobs"),
        clock,
        Schemas::platform(),
        sandbox,
        "central",
    )
    .with_actions(actions());
    (publications, dir)
}

/// A program package `ug.moh.opd@<version>` whose one Library holds `definition`.
fn package(version: &str, definition: &Value, content_type: &str) -> Files {
    let file = |path: &str, json: Value| (path.to_owned(), json.to_string().into_bytes());
    Files::from([
        file(
            "package.json",
            json!({
                "name": "ug.moh.opd", "version": version, "fhirVersions": ["5.0.0"],
                "kind": "program", "platform": { "minSoftware": "0.1.0", "formatVersion": 1 }
            }),
        ),
        file(
            "Library-manifest.json",
            json!({
                "resourceType": "Library", "id": "manifest", "status": "active",
                "url": "https://health.go.ug/fhir/Library/ug.moh.opd-manifest", "version": version,
                "type": { "text": "asset-collection" }
            }),
        ),
        file(
            "Library-opd-visit.json",
            json!({
                "resourceType": "Library", "id": "opd-visit", "status": "active",
                "url": URL, "version": "1.0.0", "name": "OpdVisit",
                "type": { "coding": [{
                    "system": "https://carapai.github.io/people-centered-healthcare/fhir/CodeSystem/platform",
                    "code": "workflow-definition"
                }] },
                "content": [{
                    "contentType": content_type,
                    "data": STANDARD.encode(definition.to_string())
                }]
            }),
        ),
    ])
}

fn definition(action: &str, params: Value) -> Value {
    json!({
        "url": URL, "version": "1.0.0",
        "statechart": { "states": [
            { "id": "waiting", "kind": "atomic", "transitions": [
                { "events": ["arrive"], "targets": ["done"], "actions": [
                    { "kind": "execute", "action": action, "params": params } ] } ] },
            { "id": "done", "kind": "final" } ] },
        "principal": ["patient-flow.manage"]
    })
}

/// Drafts and submits `files`, returning the stage reached and any validation problems.
async fn submit(publications: &Publications, files: &Files, id: &str) -> (Stage, Vec<String>) {
    publications.draft(files, "ann").await.expect("draft");
    let lifecycle = publications.submit(id, "ann").await.expect("submit");
    let (_, records) = publications.lifecycle(id).await.expect("records");
    let problems = match records.last().map(|r| &r.step) {
        Some(Step::ValidationFailed { problems }) => problems.clone(),
        _ => Vec::new(),
    };
    (lifecycle.stage, problems)
}

#[tokio::test]
async fn a_valid_workflow_definition_passes_validation() {
    let (publications, _dir) = publications().await;
    let valid = definition(
        "patient-flow/open-queue-entry@1",
        json!({ "service_point": "triage" }),
    );
    let (stage, problems) = submit(
        &publications,
        &package("1.0.0", &valid, "application/vnd.health.workflow+json"),
        "ug.moh.opd@1.0.0",
    )
    .await;
    assert_eq!((stage, problems), (Stage::Review, Vec::new()));
}

#[tokio::test]
async fn unknown_actions_and_invalid_parameters_fail_publication() {
    let (publications, _dir) = publications().await;
    let cases = [
        (
            "1.0.1",
            definition("billing/charge@1", json!({})),
            "unknown action billing/charge@1",
        ),
        (
            "1.0.2",
            definition(
                "patient-flow/open-queue-entry@1",
                json!({ "service_point": 7 }),
            ),
            "service_point is not a valid value",
        ),
        (
            "1.0.3",
            definition("patient-flow/open-queue-entry@1", json!({})),
            "needs service_point",
        ),
    ];
    for (version, definition, expected) in cases {
        let id = format!("ug.moh.opd@{version}");
        let (stage, problems) = submit(
            &publications,
            &package(version, &definition, "application/vnd.health.workflow+json"),
            &id,
        )
        .await;
        assert_eq!(stage, Stage::Draft, "{id}");
        assert!(
            problems
                .iter()
                .any(|p| p.starts_with("Library-opd-visit.json: ") && p.contains(expected)),
            "{id}: {problems:?}"
        );
    }
}

#[tokio::test]
async fn the_content_must_be_a_definition_of_its_own_library() {
    let (publications, _dir) = publications().await;
    let valid = definition(
        "patient-flow/open-queue-entry@1",
        json!({ "service_point": "triage" }),
    );
    let (_, problems) = submit(
        &publications,
        &package("2.0.0", &valid, "application/json"),
        "ug.moh.opd@2.0.0",
    )
    .await;
    assert!(
        problems
            .iter()
            .any(|p| p.contains("is not application/vnd.health.workflow+json")),
        "{problems:?}"
    );

    let mut other = valid;
    other["version"] = json!("9.9.9");
    let (_, problems) = submit(
        &publications,
        &package("2.0.1", &other, "application/vnd.health.workflow+json"),
        "ug.moh.opd@2.0.1",
    )
    .await;
    assert!(
        problems
            .iter()
            .any(|p| p.contains("not its Library's url and version")),
        "{problems:?}"
    );

    let (_, problems) = submit(
        &publications,
        &package(
            "2.0.2",
            &json!({ "states": [] }),
            "application/vnd.health.workflow+json",
        ),
        "ug.moh.opd@2.0.2",
    )
    .await;
    assert!(
        problems
            .iter()
            .any(|p| p.contains("not a Workflow Definition")),
        "{problems:?}"
    );
}

/// The OPD definition: triage, then consultation, then done.
fn opd() -> Value {
    json!({
        "url": URL, "version": "1.0.0",
        "statechart": { "states": [
            { "id": "triage", "kind": "atomic",
              "task": { "role": "nurse", "worklist": "opd-triage" },
              "transitions": [{ "events": ["triaged"], "targets": ["consultation"] }] },
            { "id": "consultation", "kind": "atomic",
              "transitions": [{ "events": ["seen"], "targets": ["done"] }] },
            { "id": "done", "kind": "final" } ] }
    })
}

/// A `tests/` scenario: after `event`, the instance is in `state`.
fn scenario(event: &str, state: &str) -> Vec<u8> {
    json!({
        "type": "workflow-scenario",
        "name": format!("{event} leads to {state}"),
        "definition": format!("{URL}|1.0.0"),
        "steps": [
            { "expect": { "configuration": ["triage"], "tasks": [{ "state": "triage", "status": "ready" }] } },
            { "event": event, "expect": { "configuration": [state] } }
        ]
    })
    .to_string()
    .into_bytes()
}

/// Drafts and submits `files`, returning the stage reached and any simulation failures.
async fn simulated(publications: &Publications, files: &Files, id: &str) -> (Stage, Vec<String>) {
    publications.draft(files, "ann").await.expect("draft");
    let lifecycle = publications.submit(id, "ann").await.expect("submit");
    let (_, records) = publications.lifecycle(id).await.expect("records");
    let failures = match records.last().map(|r| &r.step) {
        Some(Step::SimulationFailed { problems }) => problems.clone(),
        _ => Vec::new(),
    };
    (lifecycle.stage, failures)
}

#[tokio::test]
async fn a_definition_with_an_unreachable_state_cannot_be_published() {
    let (publications, _dir) = publications_with(Arc::new(WorkflowSandbox)).await;
    let mut unreachable = opd();
    unreachable["statechart"]["states"]
        .as_array_mut()
        .expect("states")
        .push(json!({ "id": "orphan", "kind": "atomic",
                      "transitions": [{ "events": ["x"], "targets": ["done"] }] }));
    let (stage, problems) = submit(
        &publications,
        &package(
            "1.0.0",
            &unreachable,
            "application/vnd.health.workflow+json",
        ),
        "ug.moh.opd@1.0.0",
    )
    .await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        ["Library-opd-visit.json: state orphan cannot be reached"]
    );
}

#[tokio::test]
async fn passing_scenarios_let_the_package_through_and_a_failing_one_sends_it_back() {
    let (publications, _dir) = publications_with(Arc::new(WorkflowSandbox)).await;
    let mut passing = package("1.0.0", &opd(), "application/vnd.health.workflow+json");
    passing.insert(
        "tests/opd-triaged.json".into(),
        scenario("triaged", "consultation"),
    );
    // Other `tests/` files, such as indicator cases, are not workflow scenarios.
    passing.insert(
        "tests/indicator.json".into(),
        json!({ "type": "indicator-case" })
            .to_string()
            .into_bytes(),
    );
    let (stage, failures) = simulated(&publications, &passing, "ug.moh.opd@1.0.0").await;
    assert_eq!((stage, failures), (Stage::Review, Vec::new()));

    let mut failing = package("1.0.1", &opd(), "application/vnd.health.workflow+json");
    failing.insert("tests/opd-triaged.json".into(), scenario("triaged", "done"));
    let (stage, failures) = simulated(&publications, &failing, "ug.moh.opd@1.0.1").await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        failures,
        [
            r#"tests/opd-triaged.json: triaged leads to done: step 2: expected configuration ["done"], got ["consultation"]"#
        ]
    );

    let mut elsewhere = package("1.0.2", &opd(), "application/vnd.health.workflow+json");
    let mut stray: Value =
        serde_json::from_slice(&scenario("triaged", "consultation")).expect("json");
    stray["definition"] = json!("https://health.go.ug/fhir/Library/anc|1.0.0");
    elsewhere.insert("tests/anc.json".into(), stray.to_string().into_bytes());
    let (stage, failures) = simulated(&publications, &elsewhere, "ug.moh.opd@1.0.2").await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        failures,
        [
            "tests/anc.json: the package holds no definition https://health.go.ug/fhir/Library/anc|1.0.0"
        ]
    );
}
