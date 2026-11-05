//! The reference packages in `packages/reference-packages/` publish (#109): each passes
//! validation and its workflow scenarios, and a Workflow Definition Library holds exactly its
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

#[tokio::test]
async fn the_lab_package_publishes() {
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
    let lab = files(&root().join("lab"));
    publications.draft(&lab, "ann").await.expect("draft");
    let lifecycle = publications
        .submit("ug.moh.lab@1.0.0", "ann")
        .await
        .expect("submit");
    let (_, records) = publications
        .lifecycle("ug.moh.lab@1.0.0")
        .await
        .expect("records");
    let problems = match records.last().map(|r| &r.step) {
        Some(Step::ValidationFailed { problems } | Step::SimulationFailed { problems }) => {
            problems.clone()
        }
        _ => Vec::new(),
    };
    assert_eq!((lifecycle.stage, problems), (Stage::Review, Vec::new()));
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
