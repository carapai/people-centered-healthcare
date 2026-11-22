//! Age Band Sets through the publication lifecycle (ADR 0071; analytics.md § Age Band Sets and
//! Population Estimates; #551), starting from the reference national core package
//! `packages/reference-packages/core` (`ug.moh.core`):
//! - a set with a gap or an overlap between its bands fails Validation;
//! - changing a band boundary needs a major version of the set, and renaming it a patch;
//! - an indicator pinned to version 1 keeps version 1's bands after version 2 is published.

use std::path::Path;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use health_analytics::{AgeBand, pinned};
use health_analytics_app::measure;
use health_configuration::lifecycle::{Stage, Step};
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_pki::{Authority, new_key, request};
use health_publication::{Files, LocalSigner, NoSandbox, Publications};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{FixedClock, Timestamp};
use serde_json::{Value, json};

const ANC_BANDS: &str = "Library-age-bands-anc.json";
const ANC_URL: &str = "https://health.go.ug/fhir/Library/age-bands-anc";

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
    let clock = Arc::new(FixedClock::new(at("2026-11-22T09:00:00Z")));
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

/// The reference national core package.
fn core() -> Files {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packages/reference-packages/core");
    std::fs::read_dir(&dir)
        .expect("the core package")
        .map(|entry| {
            let path = entry.expect("entry").path();
            let name = path.file_name().and_then(|n| n.to_str()).expect("name");
            (name.to_owned(), std::fs::read(&path).expect("read"))
        })
        .collect()
}

fn json_of(files: &Files, path: &str) -> Value {
    serde_json::from_slice(&files[path]).expect("json")
}

fn put(files: &mut Files, path: &str, value: &Value) {
    files.insert(path.to_owned(), value.to_string().into_bytes());
}

/// `files` as package version `version`, its manifest following.
fn versioned(mut files: Files, version: &str) -> Files {
    for path in ["package.json", "Library-manifest.json"] {
        let mut value = json_of(&files, path);
        value["version"] = json!(version);
        put(&mut files, path, &value);
    }
    files
}

/// `files` with the ANC Age Band Set at `version`, titled `title`, with `bands`.
fn anc_bands(mut files: Files, version: &str, title: &str, bands: Value) -> Files {
    let mut library = json_of(&files, ANC_BANDS);
    library["version"] = json!(version);
    library["title"] = json!(title);
    let content = json!({"unit": "years", "ageAt": "period-end", "bands": bands});
    library["content"][0]["data"] = json!(STANDARD.encode(content.to_string()));
    put(&mut files, ANC_BANDS, &library);
    files
}

fn moh_bands() -> Value {
    json!([{"from": 10, "to": 14}, {"from": 15, "to": 19}, {"from": 20, "to": 24}, {"from": 25}])
}

/// Drafts and submits `files`, returning the stage it reached and any validation problems.
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
        Some(Step::ValidationFailed { problems }) => problems.clone(),
        _ => Vec::new(),
    };
    (id, submitted.stage, problems)
}

/// Drafts, submits, reviews, approves twice (national core) and publishes `files`.
async fn publish(central: &Central, files: &Files) {
    let (id, stage, problems) = submit(central, files).await;
    assert_eq!(stage, Stage::Review, "{problems:?}");
    let publications = &central.publications;
    publications.review(&id, "rita").await.expect("review");
    publications.approve(&id, "bob").await.expect("approve");
    publications.approve(&id, "carol").await.expect("approve");
    publications
        .publish(&id, "ops", &central.signer)
        .await
        .expect("publish");
}

#[tokio::test]
async fn a_set_with_a_gap_or_an_overlap_fails_validation() {
    let central = central().await;
    let gap = anc_bands(
        core(),
        "1.0.0",
        "MoH ANC age bands",
        json!([{"from": 10, "to": 14}, {"from": 20}]),
    );
    let (_, stage, problems) = submit(&central, &gap).await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        [
            "Library-age-bands-anc.json: there is a gap between the bands 10–14 years and 20+ years: ages 15 to 19 (years) are in neither"
        ]
    );

    let overlap = anc_bands(
        versioned(core(), "1.0.1"),
        "1.0.0",
        "MoH ANC age bands",
        json!([{"from": 10, "to": 15}, {"from": 15}]),
    );
    let (_, stage, problems) = submit(&central, &overlap).await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        [
            "Library-age-bands-anc.json: the bands 10–15 years and 15+ years overlap: age 15 (years) is in both"
        ]
    );
}

#[tokio::test]
async fn a_band_change_needs_a_major_version_and_a_rename_a_patch() {
    let central = central().await;
    publish(&central, &core()).await;

    // A boundary moved, released as a minor version of the set: refused.
    let moved = json!([{"from": 10, "to": 15}, {"from": 16, "to": 19}, {"from": 20, "to": 24}, {"from": 25}]);
    let minor = anc_bands(
        versioned(core(), "1.1.0"),
        "1.1.0",
        "MoH ANC age bands",
        moved.clone(),
    );
    let (_, stage, problems) = submit(&central, &minor).await;
    assert_eq!(stage, Stage::Draft);
    assert!(
        problems.iter().any(|p| p.starts_with(&format!(
            "{ANC_URL} went from 1.0.0 to 1.1.0, a minor bump, but its changes need a major bump: bands (changed, major)"
        ))),
        "{problems:?}"
    );

    // As a major version of the set and the package, it passes.
    let major = anc_bands(
        versioned(core(), "2.0.0"),
        "2.0.0",
        "MoH ANC age bands",
        moved,
    );
    let (_, stage, problems) = submit(&central, &major).await;
    assert_eq!(stage, Stage::Review, "{problems:?}");

    // Renamed, with the same bands: a patch is enough.
    let renamed = anc_bands(
        versioned(core(), "1.0.1"),
        "1.0.1",
        "MoH antenatal care age bands",
        moh_bands(),
    );
    let (_, stage, problems) = submit(&central, &renamed).await;
    assert_eq!(stage, Stage::Review, "{problems:?}");
}

#[tokio::test]
async fn an_indicator_pinned_to_version_1_keeps_its_bands_after_version_2_is_published() {
    let central = central().await;
    publish(&central, &core()).await;
    let wider = json!([{"from": 10, "to": 19}, {"from": 20}]);
    publish(
        &central,
        &anc_bands(
            versioned(core(), "2.0.0"),
            "2.0.0",
            "MoH ANC age bands",
            wider,
        ),
    )
    .await;

    let indicator = measure(&json!({
        "resourceType": "Measure",
        "url": "https://health.go.ug/fhir/Measure/anc-first-visits",
        "version": "1.0.0",
        "title": "ANC 1st visits",
        "scoring": {"coding": [{"code": "cohort"}]},
        "basis": "Encounter",
        "extension": [{
            "url": health_analytics_app::PERIOD_DATE,
            "valueExpression": {"language": "text/fhirpath", "expression": "Encounter.actualPeriod.start"}
        }],
        "group": [{
            "population": [{
                "code": {"coding": [{"code": "initial-population"}]},
                "criteria": {"language": "text/fhirpath", "expression": "true"}
            }],
            "stratifier": [{
                "extension": [{
                    "url": health_analytics_app::AGE_BAND_SET,
                    "valueCanonical": format!("{ANC_URL}|1.0.0")
                }]
            }]
        }]
    }))
    .indicator()
    .expect("an indicator");

    let published = central.publications.age_band_sets().await.expect("sets");
    let versions: Vec<String> = published
        .iter()
        .filter(|p| p.set.url == ANC_URL)
        .map(|p| format!("{} in {}", p.set.version, p.package))
        .collect();
    assert_eq!(
        versions,
        ["1.0.0 in ug.moh.core@1.0.0", "2.0.0 in ug.moh.core@2.0.0"]
    );
    let set = pinned(
        indicator.age_band_sets()[0],
        published.iter().map(|p| &p.set),
    )
    .expect("the pinned set");
    let codes: Vec<String> = set.bands.iter().map(|b: &AgeBand| b.code()).collect();
    assert_eq!(codes, ["10-14", "15-19", "20-24", "25+"]);
}
