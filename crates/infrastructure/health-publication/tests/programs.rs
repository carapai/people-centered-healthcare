//! Program definitions at publication (#116): a program's regimens must cite one of its approved
//! protocol sources, and a clinical protocol must be a usable program definition; otherwise the
//! draft goes back with the problems.

use std::sync::Arc;

use health_configuration::lifecycle::{Stage, Step};
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_publication::{Files, NoSandbox, Publications};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::FixedClock;
use serde_json::{Value, json};

const SOURCE: &str = "https://health.go.ug/protocols/consolidated-hiv-2024";
const REGIMEN: &str = "https://health.go.ug/fhir/ActivityDefinition/tdf-3tc-dtg";

async fn submit(files: &Files) -> (Stage, Vec<String>) {
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
    let id = "ug.moh.art@1.0.0";
    publications.draft(files, "ann").await.expect("draft");
    let lifecycle = publications.submit(id, "ann").await.expect("submit");
    let (_, records) = publications.lifecycle(id).await.expect("records");
    let problems = match records.last().map(|r| &r.step) {
        Some(Step::ValidationFailed { problems }) => problems.clone(),
        _ => Vec::new(),
    };
    (lifecycle.stage, problems)
}

fn program() -> Value {
    json!({
        "resourceType": "PlanDefinition", "id": "art-program", "status": "active",
        "url": "https://health.go.ug/fhir/PlanDefinition/art-program", "version": "1.0.0", "name": "ArtProgram",
        "type": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/plan-definition-type", "code": "clinical-protocol" }] },
        "extension": [{ "url": "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/program-episode-type", "valueCodeableConcept": { "text": "HIV care" } }],
        "relatedArtifact": [
            { "type": "citation", "resource": SOURCE },
            { "type": "composed-of", "resource": format!("{REGIMEN}|1.0.0") }
        ]
    })
}

fn regimen(cites: &str) -> Value {
    json!({
        "resourceType": "ActivityDefinition", "id": "tdf-3tc-dtg", "status": "active",
        "url": REGIMEN, "version": "1.0.0", "name": "TdfThreeTcDtg", "kind": "MedicationRequest",
        "relatedArtifact": [{ "type": "derived-from", "resource": cites }]
    })
}

fn package(program: &Value, regimen: &Value) -> Files {
    let file = |path: &str, json: &Value| (path.to_owned(), json.to_string().into_bytes());
    Files::from([
        file(
            "package.json",
            &json!({ "name": "ug.moh.art", "version": "1.0.0", "fhirVersions": ["5.0.0"], "kind": "program",
                     "platform": { "minSoftware": "0.1.0", "formatVersion": 1 } }),
        ),
        file(
            "Library-manifest.json",
            &json!({ "resourceType": "Library", "id": "manifest", "status": "active",
                     "url": "https://health.go.ug/fhir/Library/ug.moh.art-manifest", "version": "1.0.0", "type": { "text": "asset-collection" } }),
        ),
        file("PlanDefinition-art-program.json", program),
        file("ActivityDefinition-tdf-3tc-dtg.json", regimen),
    ])
}

#[tokio::test]
async fn a_regimen_from_an_approved_source_passes() {
    assert_eq!(
        submit(&package(&program(), &regimen(SOURCE))).await,
        (Stage::Review, Vec::new())
    );
}

#[tokio::test]
async fn a_regimen_from_elsewhere_or_an_unusable_program_is_sent_back() {
    let (stage, problems) = submit(&package(
        &program(),
        &regimen("https://example.org/a-blog-post"),
    ))
    .await;
    assert_eq!(stage, Stage::Draft);
    assert_eq!(
        problems,
        [format!(
            "PlanDefinition-art-program.json: the regimen {REGIMEN}|1.0.0 cites none of the program's approved protocol sources"
        )]
    );

    let mut untyped = program();
    untyped["extension"] = json!([]);
    let (stage, problems) = submit(&package(&untyped, &regimen(SOURCE))).await;
    assert_eq!(stage, Stage::Draft);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("no program-episode-type")),
        "{problems:?}"
    );
}
