//! Population packages through the publication lifecycle (ADR 0071; analytics.md § Age Band
//! Sets and Population Estimates; #553), from the reference package
//! `packages/reference-packages/population` (`ug.population.2026`):
//! - it passes Validation and needs one approval (the HMIS lead's);
//! - the package is named for its year and holds its estimates at
//!   `configuration/population.json`;
//! - a derived type with an undeclared base, or figures of a derived type, fail;
//! - estimates belong in a Population package.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use health_analytics::Estimates;
use health_configuration::lifecycle::{Stage, Step};
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_pki::{Authority, new_key, request};
use health_publication::{Files, LocalSigner, NoSandbox, Publications, estimates_in};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{FixedClock, Timestamp};
use serde_json::{Value, json};

const ESTIMATES: &str = "configuration/population.json";

struct Central {
    publications: Publications,
    signer: LocalSigner,
    _blobs: tempfile::TempDir,
}

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

async fn central() -> Central {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "central".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let clock = Arc::new(FixedClock::new(at("2026-11-24T09:00:00Z")));
    let root = Authority::root("Uganda Health Root", at("2026-01-01T00:00:00Z")).expect("root");
    let key = new_key().expect("key");
    let issued = root
        .issue_publication(
            &request(&key, "MoH publication signing").expect("request"),
            "MoH publication signing",
            at("2026-06-01T00:00:00Z"),
        )
        .expect("publication certificate");
    let dir = tempfile::tempdir().expect("tempdir");
    Central {
        publications: Publications::new(
            SurrealStore::new(connection, clock.clone()),
            FsBlobStore::open(dir.path()).expect("blobs"),
            clock,
            Schemas::platform(),
            Arc::new(NoSandbox),
            "central",
        ),
        signer: LocalSigner::from_pem(&key.serialize_pem(), issued.pem()).expect("signer"),
        _blobs: dir,
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packages/reference-packages")
}

/// The reference package in directory `package`.
fn package(package: &str) -> Files {
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
    let dir = root().join(package);
    let mut out = Files::new();
    walk(&dir, &dir, &mut out);
    out
}

fn json_of(files: &Files, path: &str) -> Value {
    serde_json::from_slice(&files[path]).expect("json")
}

fn put(files: &mut Files, path: &str, value: &Value) {
    files.insert(path.to_owned(), value.to_string().into_bytes());
}

/// The reference Population package, its estimates edited by `edit`.
fn population(edit: impl FnOnce(&mut Value)) -> Files {
    let mut files = package("population");
    let mut estimates = json_of(&files, ESTIMATES);
    edit(&mut estimates);
    put(&mut files, ESTIMATES, &estimates);
    files
}

/// Drafts and submits `files`, returning its id, the stage it reached and any validation
/// problems.
async fn submit(central: &Central, files: &Files) -> (String, Stage, Vec<String>) {
    let lifecycle = central
        .publications
        .draft(files, "ann")
        .await
        .expect("draft");
    let id = format!("{}@{}", lifecycle.package, lifecycle.version);
    let submitted = central
        .publications
        .submit(&id, "ann")
        .await
        .expect("submit");
    let (_, records) = central.publications.lifecycle(&id).await.expect("records");
    let problems = match records.last().map(|r| &r.step) {
        Some(Step::ValidationFailed { problems } | Step::SimulationFailed { problems }) => {
            problems.clone()
        }
        _ => Vec::new(),
    };
    (id, submitted.stage, problems)
}

#[tokio::test]
async fn the_reference_population_package_publishes_with_the_hmis_lead_s_approval() {
    let central = central().await;
    let (id, stage, problems) = submit(&central, &package("population")).await;
    assert_eq!((stage, problems), (Stage::Review, Vec::<String>::new()));
    let publications = &central.publications;
    publications.review(&id, "rita").await.expect("review");
    // One approval, the HMIS lead's, is enough to publish.
    publications
        .approve(&id, "hmis-lead")
        .await
        .expect("approve");
    publications
        .publish(&id, "ops", &central.signer)
        .await
        .expect("publish");

    // Kampala's 1,800,000 people expect 90,000 pregnancies a year, 7,500 a month.
    let found = estimates_in(&package("population"));
    let [(path, Ok(definition))] = found.as_slice() else {
        panic!("one estimates file: {found:?}");
    };
    assert_eq!(path, ESTIMATES);
    let estimates = Estimates::new(definition.clone(), |_| None);
    assert_eq!(
        estimates
            .annual("district-kampala", "expected-pregnancies")
            .map(|e| e.normalize().to_string()),
        Some("90000".to_owned())
    );
}

#[tokio::test]
async fn a_population_package_is_named_for_its_year_and_holds_its_estimates_where_expected() {
    let central = central().await;
    let (_, stage, problems) = submit(
        &central,
        &population(|estimates| estimates["year"] = json!(2027)),
    )
    .await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        vec![
            "configuration/population.json: the estimates are for 2027, so the package's name ends in .2027 (one Population package per year), not ug.population.2026"
        ]
    );

    let mut moved = package("population");
    let bytes = moved.remove(ESTIMATES).expect("estimates");
    moved.insert("configuration/estimates.json".into(), bytes);
    let (_, stage, problems) = submit(&central, &moved).await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        vec![
            "configuration/estimates.json: a Population package holds its estimates at configuration/population.json"
        ]
    );
}

#[tokio::test]
async fn derived_types_need_a_declared_base_and_take_no_figures() {
    let central = central().await;
    let files = population(|estimates| {
        estimates["types"][1]["derivedFrom"] = json!("women-of-reproductive-age");
        estimates["figures"].as_array_mut().expect("figures").push(
            json!({ "place": "district-kampala", "type": "expected-pregnancies", "value": 90000 }),
        );
    });
    let (_, stage, problems) = submit(&central, &files).await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        vec![
            "configuration/population.json: the estimate type expected-pregnancies is derived from women-of-reproductive-age, which is not declared",
            "configuration/population.json: district-kampala has a figure for expected-pregnancies, which is derived, not given",
        ]
    );
}

#[tokio::test]
async fn estimates_belong_in_a_population_package() {
    let central = central().await;
    let mut files = package("population");
    let mut manifest = json_of(&files, "package.json");
    manifest["kind"] = json!("country-configuration");
    put(&mut files, "package.json", &manifest);
    let (_, stage, problems) = submit(&central, &files).await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        vec![
            "configuration/population.json: Population Estimates belong in a Population package, not a country-configuration package"
        ]
    );
}
