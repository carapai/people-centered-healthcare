//! The reference packages in `packages/reference-packages/` publish (#109, #103): each passes
//! validation, its extraction fixtures and its workflow scenarios, and a Workflow Definition Library holds exactly its
//! readable source in `packages/reference-packages/sources/`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use health_configuration::lifecycle::{Stage, Step};
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_publication::{Files, Publications, WorkflowSandbox};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::FixedClock;
use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packages/reference-packages")
}

fn files(dir: &Path) -> Files {
    fn walk(base: &Path, dir: &Path, out: &mut Files) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                let relative = path
                    .strip_prefix(base)
                    .expect("relative")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(relative, std::fs::read(&path).expect("read"));
            }
        }
    }
    let mut out = Files::new();
    walk(dir, dir, &mut out);
    out
}

/// Drafts and submits the package in directory `package` as `id`, and returns any problems validation or
/// simulation found.
async fn submitted(package: &str, id: &str) -> (Stage, Vec<String>) {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "central".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let clock = Arc::new(FixedClock::new(
        "2026-11-05T09:00:00Z".parse().expect("timestamp"),
    ));
    let dir = tempfile::tempdir().expect("tempdir");
    let publications = Publications::new(
        SurrealStore::new(connection, clock.clone()),
        FsBlobStore::open(dir.path()).expect("blobs"),
        clock,
        Schemas::platform(),
        Arc::new(WorkflowSandbox),
        "central",
    );
    let files = files(&root().join(package));
    publications.draft(&files, "ann").await.expect("draft");
    let lifecycle = publications.submit(id, "ann").await.expect("submit");
    let (_, records) = publications.lifecycle(id).await.expect("records");
    let problems = match records.last().map(|r| &r.step) {
        Some(Step::ValidationFailed { problems } | Step::SimulationFailed { problems }) => {
            problems.clone()
        }
        _ => Vec::new(),
    };
    (lifecycle.stage, problems)
}

#[tokio::test]
async fn the_lab_package_publishes() {
    assert_eq!(
        submitted("lab", "ug.moh.lab@1.0.0").await,
        (Stage::Review, Vec::new())
    );
}

/// The OPD package (#103): the triage Questionnaire and its Extraction Mapping, whose fixtures
/// give the expected vital-sign Observations.
#[tokio::test]
async fn the_opd_package_publishes() {
    assert_eq!(
        submitted("opd", "ug.moh.opd@1.0.0").await,
        (Stage::Review, Vec::new())
    );
}

#[test]
fn the_lab_definition_is_its_source() {
    let library: Value = serde_json::from_slice(
        &std::fs::read(root().join("lab/Library-lab-test.json")).expect("library"),
    )
    .expect("json");
    let content = STANDARD
        .decode(library["content"][0]["data"].as_str().expect("data"))
        .expect("base64");
    let held: Value = serde_json::from_slice(&content).expect("definition");
    let source: Value = serde_json::from_slice(
        &std::fs::read(root().join("sources/lab-test.json")).expect("source"),
    )
    .expect("json");
    assert_eq!(
        held, source,
        "regenerate the Library's content from sources/lab-test.json"
    );
    assert_eq!(
        (library["url"].clone(), library["version"].clone()),
        (source["url"].clone(), source["version"].clone())
    );
}
