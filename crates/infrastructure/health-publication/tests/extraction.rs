//! Extraction Mappings at publication (#93): a package's mapping must name Owners that accept its
//! targets, round its arithmetic, name only items of its Questionnaire, and ship fixtures that
//! give exactly their Bundle. Otherwise the draft goes back with the problems.

use std::sync::Arc;

use health_configuration::lifecycle::{Stage, Step};
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_publication::{EXTRACTION_FIXTURE_TYPE, Files, NoSandbox, Publications};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::FixedClock;
use serde_json::{Value, json};

const MAPPING: &str = "https://health.go.ug/fhir/Library/anc-visit-mapping|1.0.0";

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("json")
}

async fn publications() -> (Publications, tempfile::TempDir) {
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
        Arc::new(NoSandbox),
        "central",
    );
    (publications, dir)
}

fn mapping() -> Value {
    let mut library = parse(include_str!(
        "../../../contracts/health-fhir-extraction/tests/fixtures/anc-mapping.json"
    ));
    library["id"] = json!("anc-visit-mapping");
    library
}

fn questionnaire() -> Value {
    json!({
        "resourceType": "Questionnaire", "id": "anc-visit", "status": "active",
        "url": "https://health.go.ug/fhir/Questionnaire/anc-visit", "version": "1.4.0",
        "item": [
            { "linkId": "vitals", "type": "group", "item": [
                { "linkId": "weight", "type": "decimal" },
                { "linkId": "weight-note", "type": "string", "repeats": true } ] },
            { "linkId": "medications", "type": "group", "item": [
                { "linkId": "medication", "type": "group", "repeats": true, "item": [
                    { "linkId": "drug", "type": "coding" }, { "linkId": "instructions", "type": "string" } ] } ] },
            { "linkId": "danger-signs", "type": "boolean" }
        ]
    })
}

fn fixture() -> Value {
    json!({
        "type": EXTRACTION_FIXTURE_TYPE,
        "mapping": MAPPING,
        "response": parse(include_str!("../../../contracts/health-fhir-extraction/tests/fixtures/anc-response.json")),
        "expected": parse(include_str!("../../../contracts/health-fhir-extraction/tests/fixtures/anc-expected.json")),
    })
}

/// A program package `ug.moh.anc@<version>` with the mapping, its Questionnaire and `fixture`.
fn package(
    version: &str,
    mapping: &Value,
    questionnaire: &Value,
    fixture: Option<&Value>,
) -> Files {
    let file = |path: &str, json: &Value| (path.to_owned(), json.to_string().into_bytes());
    let mut files = Files::from([
        file(
            "package.json",
            &json!({
                "name": "ug.moh.anc", "version": version, "fhirVersions": ["5.0.0"],
                "kind": "program", "platform": { "minSoftware": "0.1.0", "formatVersion": 1 }
            }),
        ),
        file(
            "Library-manifest.json",
            &json!({
                "resourceType": "Library", "id": "manifest", "status": "active",
                "url": "https://health.go.ug/fhir/Library/ug.moh.anc-manifest", "version": version,
                "type": { "text": "asset-collection" }
            }),
        ),
        file("Library-anc-visit-mapping.json", mapping),
        file("Questionnaire-anc-visit.json", questionnaire),
    ]);
    if let Some(fixture) = fixture {
        files.insert(
            "tests/anc-visit/extraction.json".into(),
            fixture.to_string().into_bytes(),
        );
    }
    files
}

async fn submit(files: &Files) -> (Stage, Vec<String>) {
    let (publications, _dir) = publications().await;
    let id = "ug.moh.anc@1.0.0";
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
async fn a_mapping_with_passing_fixtures_passes_validation() {
    let (stage, problems) = submit(&package(
        "1.0.0",
        &mapping(),
        &questionnaire(),
        Some(&fixture()),
    ))
    .await;
    assert_eq!((stage, problems), (Stage::Review, Vec::new()));
}

async fn refused(files: &Files, expected: &str) {
    let (stage, problems) = submit(files).await;
    assert_eq!(stage, Stage::Draft, "{problems:?}");
    assert!(
        problems.iter().any(|p| p.contains(expected)),
        "{expected} in {problems:?}"
    );
}

#[tokio::test]
async fn each_gate_sends_the_draft_back() {
    // No fixture.
    refused(
        &package("1.0.0", &mapping(), &questionnaire(), None),
        "no tests/ fixture of type extraction-fixture",
    )
    .await;

    // An Owner that does not take the target.
    let mut wrong_owner = mapping();
    wrong_owner["contained"][1]["extension"][0]["extension"][1]["valueString"] =
        json!("clinical-data.record-observation");
    refused(
        &package("1.0.0", &wrong_owner, &questionnaire(), Some(&fixture())),
        "template medication: no Owner use case clinical-data.record-observation accepts a MedicationRequest",
    )
    .await;

    // Unrounded arithmetic.
    let mut unrounded = mapping();
    unrounded["contained"][0]["valueQuantity"]["_value"]["extension"][0]["valueString"] =
        json!("answer.value * 1.0");
    refused(
        &package("1.0.0", &unrounded, &questionnaire(), Some(&fixture())),
        "must end in an explicit round(n)",
    )
    .await;

    // An item the Questionnaire does not have.
    let mut older = questionnaire();
    older["item"][2]["linkId"] = json!("danger");
    refused(
        &package("1.0.0", &mapping(), &older, Some(&fixture())),
        "names linkId 'danger-signs'",
    )
    .await;

    // A fixture that does not give its Bundle.
    let mut wrong = fixture();
    wrong["expected"]["entry"][0]["resource"]["valueQuantity"]["value"] = parse("61.5");
    refused(
        &package("1.0.0", &mapping(), &questionnaire(), Some(&wrong)),
        "tests/anc-visit/extraction.json: entry 0 differs",
    )
    .await;

    // A fixture naming a mapping the package does not hold.
    let mut stray = fixture();
    stray["mapping"] = json!("https://health.go.ug/fhir/Library/other|1.0.0");
    refused(
        &package("1.0.0", &mapping(), &questionnaire(), Some(&stray)),
        "holds no Extraction Mapping",
    )
    .await;

    // A Library that is not a usable mapping.
    let mut broken = mapping();
    broken["contained"][0]["extension"][0]["extension"]
        .as_array_mut()
        .expect("governing")
        .remove(1);
    refused(
        &package("1.0.0", &broken, &questionnaire(), Some(&fixture())),
        "a managed template names its retraction status",
    )
    .await;
}
