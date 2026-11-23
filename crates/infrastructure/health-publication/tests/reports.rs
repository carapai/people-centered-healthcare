//! Reports through the publication lifecycle (ADR 0070, ADR 0072; analytics.md § Reports; #556),
//! starting from the reference packages: the national core's Age Band Sets (`ug.moh.core`) and
//! the ANC indicators (`ug.moh.anc`) published, then the HMIS Reporting package
//! (`packages/reference-packages/hmis`, `ug.moh.hmis`) submitted:
//! - the reference Reports pass Validation;
//! - a Report with a median row that runs at district level fails, with the reason;
//! - a Report with a line list that runs at national level fails;
//! - a Report pins its indicators and Age Band Sets to `url|version`, and keeps version 1 of a
//!   set after version 2 is published;
//! - a Report belongs in a Reporting package;
//! - standard Dashboards (#603): the reference *ANC overview* passes Validation, one whose
//!   Visualization references an unknown indicator fails, and once the package is published
//!   its newest version's Dashboards are listed.
//! - an indicator's HMIS mapping (#864): the rows of Reports laying it out, in the newest
//!   published version and the open draft.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use health_configuration::lifecycle::{Stage, Step};
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_pki::{Authority, new_key, request};
use health_publication::{Files, LocalSigner, NoSandbox, Publications};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{FixedClock, Timestamp};
use serde_json::{Value, json};

const HMIS_105: &str = "configuration/reports/hmis-105-anc.json";
const MONITORING: &str = "configuration/reports/anc-facility-monitoring.json";
const BANDS: &str = "https://health.go.ug/fhir/Library/age-bands-anc";
const OVERVIEW: &str = "configuration/dashboards/anc-overview.json";

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

/// `files` as package version `version`, its manifest following.
fn versioned(mut files: Files, version: &str) -> Files {
    for path in ["package.json", "Library-manifest.json"] {
        let mut value = json_of(&files, path);
        value["version"] = json!(version);
        put(&mut files, path, &value);
    }
    files
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
        Some(Step::ValidationFailed { problems } | Step::SimulationFailed { problems }) => {
            problems.clone()
        }
        _ => Vec::new(),
    };
    (id, submitted.stage, problems)
}

/// Drafts, submits, reviews, approves twice and publishes `files`.
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

/// Central with the national core and the ANC indicators published.
async fn with_indicators() -> Central {
    let central = central().await;
    publish(&central, &package("core")).await;
    publish(&central, &package("anc")).await;
    central
}

/// The HMIS package, the HMIS 105 Report edited by `edit`.
fn hmis_105(edit: impl FnOnce(&mut Value)) -> Files {
    let mut files = package("hmis");
    let mut report = json_of(&files, HMIS_105);
    edit(&mut report);
    put(&mut files, HMIS_105, &report);
    files
}

#[tokio::test]
async fn the_reference_reports_pass_validation() {
    let central = with_indicators().await;
    let (_, stage, problems) = submit(&central, &package("hmis")).await;
    assert_eq!((stage, problems), (Stage::Review, Vec::new()));
}

#[tokio::test]
async fn a_report_with_a_median_row_at_district_level_fails_with_the_reason() {
    let central = with_indicators().await;
    let files = hmis_105(|report| {
        report["sections"][1]["rows"]
            .as_array_mut()
            .expect("rows")
            .push(json!({
                "id": "median-ga",
                "indicator": "https://health.go.ug/fhir/Measure/anc-gestational-age-at-first-visit|1.0.0",
                "label": "Median gestational age at the 1st visit"
            }));
        report["levels"] = json!(["facility", "district"]);
    });
    let (_, stage, problems) = submit(&central, &files).await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        [format!(
            "{HMIS_105}: section 2 (2.2 Timing of the 1st visit)'s row 3 (Median gestational age at the 1st visit) is a median, which cannot be rebuilt from facility summaries because patient-level data never leaves the facility; the Report runs at district level: run it at facility level only, or move the row to a facility Report"
        )]
    );
}

#[tokio::test]
async fn a_report_with_a_line_list_at_national_level_fails() {
    let central = with_indicators().await;
    let mut files = package("hmis");
    let mut report = json_of(&files, MONITORING);
    report["sections"] = json!([report["sections"][1].clone()]);
    report["levels"] = json!(["facility", "national"]);
    put(&mut files, MONITORING, &report);
    let (_, stage, problems) = submit(&central, &files).await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        [format!(
            "{MONITORING}: section 1 (ANC clients this month) is a line list, which shows patient-level records, and they never leave the facility; the Report runs at national level: run it at facility level only, or move the line list to a facility Report"
        )]
    );
}

#[tokio::test]
async fn a_report_pins_indicator_and_age_band_set_versions() {
    let central = with_indicators().await;
    // Version 2 of the ANC age bands is published; the Report keeps version 1.
    let mut core = versioned(package("core"), "2.0.0");
    let mut set = json_of(&core, "Library-age-bands-anc.json");
    set["version"] = json!("2.0.0");
    put(&mut core, "Library-age-bands-anc.json", &set);
    publish(&central, &core).await;
    let (_, stage, problems) = submit(&central, &package("hmis")).await;
    assert_eq!((stage, problems), (Stage::Review, Vec::new()));

    let unpinned = hmis_105(|report| {
        let section = &mut report["sections"][0];
        section["disaggregations"][0]["ageBandSet"] = json!(BANDS);
        section["rows"][0]["indicator"] =
            json!("https://health.go.ug/fhir/Measure/anc-first-visits");
        section["rows"][1]["indicator"] =
            json!("https://health.go.ug/fhir/Measure/anc-visits|3.0.0");
    });
    let (_, stage, problems) = submit(&central, &versioned(unpinned, "1.0.1")).await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        [
            format!(
                "{HMIS_105}: section 1 (2.1 ANC attendance)'s disaggregation 1: the Age Band Set {BANDS} is not pinned to a version (url|version)"
            ),
            format!(
                "{HMIS_105}: section 1 (2.1 ANC attendance)'s row 1 (ANC 1st visit for women): the indicator https://health.go.ug/fhir/Measure/anc-first-visits is not pinned to a version (url|version)"
            ),
            format!(
                "{HMIS_105}: section 1 (2.1 ANC attendance)'s row 2 (ANC visits (all)): no published indicator is https://health.go.ug/fhir/Measure/anc-visits|3.0.0"
            ),
        ]
    );
}

#[tokio::test]
async fn a_report_belongs_in_a_reporting_package() {
    let central = with_indicators().await;
    let mut files = package("hmis");
    // The standard Dashboard's own case is below.
    files.remove(OVERVIEW);
    let mut manifest = json_of(&files, "package.json");
    manifest["kind"] = json!("program");
    put(&mut files, "package.json", &manifest);
    let (_, stage, problems) = submit(&central, &files).await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        [
            format!("{MONITORING}: a Report belongs in a Reporting package, not a program package"),
            format!("{HMIS_105}: a Report belongs in a Reporting package, not a program package"),
        ]
    );
}

#[tokio::test]
async fn a_report_lives_at_its_id() {
    let central = with_indicators().await;
    let files = hmis_105(|report| report["id"] = json!("hmis-105"));
    let (_, stage, problems) = submit(&central, &files).await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        [format!(
            "{HMIS_105}: the Report hmis-105 belongs at configuration/reports/hmis-105.json"
        )]
    );
}

#[tokio::test]
async fn a_visualization_referencing_an_unknown_indicator_fails_validation() {
    let central = with_indicators().await;
    let mut files = package("hmis");
    let mut overview = json_of(&files, OVERVIEW);
    overview["items"][0]["visualization"]["question"]["indicators"] =
        json!(["https://health.go.ug/fhir/Measure/anc-fourth-visits|1.0.0"]);
    put(&mut files, OVERVIEW, &overview);
    let (_, stage, problems) = submit(&central, &files).await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        [format!(
            "{OVERVIEW}: Visualization 1 (ANC 1st visits by area): no published indicator is https://health.go.ug/fhir/Measure/anc-fourth-visits|1.0.0"
        )]
    );
}

#[tokio::test]
async fn a_standard_dashboard_belongs_in_a_reporting_package_at_its_id() {
    let central = with_indicators().await;
    let mut files = package("hmis");
    files.remove(HMIS_105);
    files.remove(MONITORING);
    let mut overview = json_of(&files, OVERVIEW);
    overview["id"] = json!("anc");
    put(&mut files, OVERVIEW, &overview);
    let mut manifest = json_of(&files, "package.json");
    manifest["kind"] = json!("program");
    put(&mut files, "package.json", &manifest);
    let (_, stage, problems) = submit(&central, &files).await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        [
            format!(
                "{OVERVIEW}: a standard Dashboard belongs in a Reporting package, not a program package"
            ),
            format!("{OVERVIEW}: the Dashboard anc belongs at configuration/dashboards/anc.json"),
        ]
    );
}

#[tokio::test]
async fn a_published_reporting_package_lists_its_newest_versions_dashboards() {
    let central = with_indicators().await;
    assert_eq!(
        central
            .publications
            .published_dashboards()
            .await
            .expect("none yet"),
        []
    );
    publish(&central, &package("hmis")).await;
    let mut renamed = versioned(package("hmis"), "1.1.0");
    let mut overview = json_of(&renamed, OVERVIEW);
    overview["title"] = json!("Antenatal care overview");
    put(&mut renamed, OVERVIEW, &overview);
    let listed = central
        .publications
        .published_dashboards()
        .await
        .expect("published");
    assert_eq!(
        listed
            .iter()
            .map(|d| (
                d.package.as_str(),
                d.version.as_str(),
                d.dashboard.title.as_str()
            ))
            .collect::<Vec<_>>(),
        [("ug.moh.hmis", "1.0.0", "ANC overview")]
    );
    assert_eq!(listed[0].dashboard.items.len(), 4);
    publish(&central, &renamed).await;
    let listed = central
        .publications
        .published_dashboards()
        .await
        .expect("published");
    assert_eq!(
        listed
            .iter()
            .map(|d| (d.version.as_str(), d.dashboard.title.as_str()))
            .collect::<Vec<_>>(),
        [("1.1.0", "Antenatal care overview")]
    );
}

#[tokio::test]
async fn an_indicators_mapping_lists_the_report_rows_laying_it_out() {
    let central = with_indicators().await;
    let first_visits = "https://health.go.ug/fhir/Measure/anc-first-visits";
    let rows = |found: &[health_publication::IndicatorMapping]| {
        found
            .iter()
            .map(|m| {
                (
                    m.package.clone(),
                    m.published,
                    m.title.clone(),
                    m.section.clone(),
                    m.label.clone(),
                    m.indicator.clone(),
                    m.sent_to_dhis2,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        central
            .publications
            .indicator_mappings(first_visits)
            .await
            .expect("none yet"),
        []
    );
    publish(&central, &package("hmis")).await;
    let row = |package: &str, published: bool, label: &str| {
        (
            package.to_owned(),
            published,
            "HMIS 105 · 2 Antenatal care".to_owned(),
            "2.1 ANC attendance".to_owned(),
            label.to_owned(),
            format!("{first_visits}|1.0.0"),
            true,
        )
    };
    let found = central
        .publications
        .indicator_mappings(first_visits)
        .await
        .expect("mapped");
    assert_eq!(
        rows(&found),
        [row("ug.moh.hmis@1.0.0", true, "ANC 1st visit for women")]
    );
    // An open draft that relabels the row is listed beside the published version.
    let mut relabelled = versioned(package("hmis"), "1.1.0");
    let mut report = json_of(&relabelled, HMIS_105);
    report["sections"][0]["rows"][0]["label"] = json!("ANC 1st visits");
    put(&mut relabelled, HMIS_105, &report);
    central
        .publications
        .draft(&relabelled, "ann")
        .await
        .expect("draft");
    let found = central
        .publications
        .indicator_mappings(first_visits)
        .await
        .expect("mapped");
    assert_eq!(
        rows(&found),
        [
            row("ug.moh.hmis@1.0.0", true, "ANC 1st visit for women"),
            row("ug.moh.hmis@1.1.0", false, "ANC 1st visits"),
        ]
    );
    // An indicator no Report lays out has no mapping.
    assert_eq!(
        central
            .publications
            .indicator_mappings("https://health.go.ug/fhir/Measure/anc-median-gestational-age")
            .await
            .expect("none"),
        []
    );
}
