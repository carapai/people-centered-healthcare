//! The reference packages in `packages/reference-packages/` publish (#109, #103, #548, #551): each
//! passes validation, its extraction fixtures and its workflow scenarios, and a Workflow Definition Library holds exactly its
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
    submitted_files(&files(&root().join(package)), id).await
}

/// Drafts and submits `files` as `id`, and returns any problems validation or simulation found.
async fn submitted_files(files: &Files, id: &str) -> (Stage, Vec<String>) {
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
    publications.draft(files, "ann").await.expect("draft");
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

/// The ANC package (#548): the *ANC 1st visits* indicator and the test patients *Try* evaluates
/// it over.
#[tokio::test]
async fn the_anc_package_publishes() {
    assert_eq!(
        submitted("anc", "ug.moh.anc@1.0.0").await,
        (Stage::Review, Vec::new())
    );
}

/// An indicator without its period date, or a test patient without a Patient, fails validation.
#[tokio::test]
async fn an_indicator_without_a_period_date_fails_validation() {
    let mut files = files(&root().join("anc"));
    let path = "Measure-anc-first-visits.json";
    let mut measure: Value = serde_json::from_slice(&files[path]).expect("measure");
    measure["extension"]
        .as_array_mut()
        .expect("extensions")
        .retain(|e| {
            !e["url"]
                .as_str()
                .is_some_and(|u| u.ends_with("/indicator-period-date"))
        });
    files.insert(path.into(), measure.to_string().into_bytes());
    let patient = "tests/patient-akello.json";
    let mut file: Value = serde_json::from_slice(&files[patient]).expect("patient");
    file["resources"]
        .as_array_mut()
        .expect("resources")
        .retain(|r| r["resourceType"] != "Patient");
    files.insert(patient.into(), file.to_string().into_bytes());
    assert_eq!(
        submitted_files(&files, "ug.moh.anc@1.0.0").await,
        (
            Stage::Draft,
            vec![
                "Measure-anc-first-visits.json: the indicator has no period date: the declared date that places care in a Report's period".to_owned(),
                "tests/patient-akello.json: the test patient needs one Patient, and it has 0".to_owned(),
            ]
        )
    );
}

/// Two indicators that refer to each other fail validation (#550): each would need the other's
/// membership first.
#[tokio::test]
async fn indicators_that_refer_to_each_other_fail_validation() {
    let mut files = files(&root().join("anc"));
    let refer = |files: &mut Files, path: &str, to: &str| {
        let mut measure: Value = serde_json::from_slice(&files[path]).expect("measure");
        let criteria = &mut measure["group"][0]["population"][0]["criteria"]["expression"];
        *criteria = Value::String(format!(
            "{} and '{to}' in %indicators.numerator",
            criteria.as_str().expect("criteria")
        ));
        files.insert(path.into(), measure.to_string().into_bytes());
    };
    refer(
        &mut files,
        "Measure-anc-first-visits.json",
        "https://health.go.ug/fhir/Measure/anc-visits|1.0.0",
    );
    refer(
        &mut files,
        "Measure-anc-visits.json",
        "https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0",
    );
    assert_eq!(
        submitted_files(&files, "ug.moh.anc@1.0.0").await,
        (
            Stage::Draft,
            vec![
                "Measure-anc-first-visits.json, Measure-anc-visits.json: the indicators https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0, https://health.go.ug/fhir/Measure/anc-visits|1.0.0 refer to each other in a circle".to_owned(),
            ]
        )
    );
}

/// The Workflow Definition Library at `library` holds exactly the readable source `source`.
fn holds_its_source(library: &str, source: &str) {
    let library: Value =
        serde_json::from_slice(&std::fs::read(root().join(library)).expect("library"))
            .expect("json");
    let content = STANDARD
        .decode(library["content"][0]["data"].as_str().expect("data"))
        .expect("base64");
    let held: Value = serde_json::from_slice(&content).expect("definition");
    let source_json: Value =
        serde_json::from_slice(&std::fs::read(root().join(source)).expect("source")).expect("json");
    assert_eq!(
        held, source_json,
        "regenerate the Library's content from {source}"
    );
    assert_eq!(
        (library["url"].clone(), library["version"].clone()),
        (source_json["url"].clone(), source_json["version"].clone())
    );
}

#[tokio::test]
async fn the_core_package_publishes() {
    let (stage, problems) = submitted("core", "ug.moh.core@1.0.0").await;
    assert_eq!(stage, Stage::Review, "{problems:?}");
}

/// The Age Band Set Library at `library` holds exactly the readable content `source`.
fn holds_its_bands(library: &str, source: &str) {
    let library: Value =
        serde_json::from_slice(&std::fs::read(root().join(library)).expect("library"))
            .expect("json");
    let content = STANDARD
        .decode(library["content"][0]["data"].as_str().expect("data"))
        .expect("base64");
    let held: Value = serde_json::from_slice(&content).expect("bands");
    let source_json: Value =
        serde_json::from_slice(&std::fs::read(root().join(source)).expect("source")).expect("json");
    assert_eq!(
        held, source_json,
        "regenerate the Library's content from {source}"
    );
}

#[test]
fn the_core_age_band_sets_are_their_sources() {
    holds_its_bands(
        "core/Library-age-bands-anc.json",
        "sources/age-bands-anc.json",
    );
    holds_its_bands(
        "core/Library-age-bands-outpatient.json",
        "sources/age-bands-outpatient.json",
    );
}

#[test]
fn the_lab_definition_is_its_source() {
    holds_its_source("lab/Library-lab-test.json", "sources/lab-test.json");
}

#[test]
fn the_opd_definition_is_its_source() {
    holds_its_source("opd/Library-opd-visit.json", "sources/opd-visit.json");
}

#[test]
fn the_opd_triage_route_is_its_source() {
    holds_its_source(
        "opd/Library-opd-triage-route.json",
        "sources/opd-triage-route.json",
    );
}
