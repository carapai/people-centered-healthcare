//! What the Studio designs, across packages (#855; studio.md § The Studio home): every form,
//! workflow, decision table, code system, value set and program, each read from its package's
//! open draft when it has one and otherwise from its latest published version, with whose draft
//! it is in.

use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use health_configuration::lifecycle::Stage;
use health_configuration::package::PackageKind;
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_pki::{Authority, new_key, request};
use health_publication::designs::DesignKind;
use health_publication::{Files, LocalSigner, NoSandbox, Publications};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{FixedClock, Timestamp};
use serde_json::{Value, json};

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
    let clock = Arc::new(FixedClock::new(at("2026-11-04T09:00:00Z")));
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
    let blobs = FsBlobStore::open(dir.path()).expect("blobs");
    let store = SurrealStore::new(connection, clock.clone());
    Central {
        publications: Publications::new(
            store.clone(),
            blobs,
            clock.clone(),
            Schemas::platform(),
            Arc::new(NoSandbox),
            "central",
        ),
        signer: LocalSigner::from_pem(&key.serialize_pem(), issued.pem()).expect("signer"),
        _blobs: dir,
    }
}

/// A package `name@version` of `kind`, with one Questionnaire at `form_version` holding `items`.
fn package(name: &str, version: &str, kind: &str, form_version: &str, items: Value) -> Files {
    let file = |path: &str, json: Value| (path.to_owned(), json.to_string().into_bytes());
    Files::from([
        file(
            "package.json",
            json!({
                "name": name, "version": version, "fhirVersions": ["5.0.0"],
                "dependencies": { "ug.moh.core": "2.1.0" },
                "kind": kind, "platform": { "minSoftware": "0.3.0", "formatVersion": 1 }
            }),
        ),
        file(
            "Library-manifest.json",
            json!({
                "resourceType": "Library", "id": "manifest", "title": "ANC",
                "url": format!("http://health.go.ug/fhir/Library/{name}-manifest"), "version": version,
                "status": "active", "type": { "text": "asset-collection" },
                "relatedArtifact": [{
                    "type": "depends-on",
                    "resource": "http://health.go.ug/fhir/ValueSet/anc-visit-type|2.1.0"
                }]
            }),
        ),
        file(
            "Questionnaire-anc-visit.json",
            json!({
                "resourceType": "Questionnaire", "id": "anc-visit",
                "url": "http://health.go.ug/fhir/Questionnaire/anc-visit", "version": form_version,
                "status": "active", "item": items
            }),
        ),
    ])
}

fn items() -> Value {
    json!([{ "linkId": "bp", "text": "Blood pressure", "type": "string" }])
}

/// A Workflow Definition Library at `version` with one decision table, `risk`.
fn workflow(version: &str) -> (String, Vec<u8>) {
    let url = "http://health.go.ug/fhir/Library/anc-visit";
    let definition = json!({
        "url": url, "version": version,
        "statechart": { "states": [{ "id": "start", "kind": "final" }], "initial": ["start"] },
        "decisionTables": [{ "id": "risk", "rows": [{ "when": ["true"], "then": "high" }], "default": null }]
    });
    let library = json!({
        "resourceType": "Library", "id": "anc-visit", "url": url, "version": version,
        "title": "ANC visit", "status": "active",
        "type": { "coding": [{ "system": "https://carapai.github.io/people-centered-healthcare/fhir/CodeSystem/platform", "code": "workflow-definition" }] },
        "content": [{ "contentType": "application/vnd.health.workflow+json", "data": STANDARD.encode(definition.to_string()) }]
    });
    (
        "Library-anc-visit.json".to_owned(),
        library.to_string().into_bytes(),
    )
}

#[tokio::test]
async fn designs_are_read_from_the_open_draft_else_the_latest_published_version() {
    let central = central().await;
    let publications = &central.publications;
    let first = "ug.moh.anc@1.0.0";
    publications
        .draft(
            &package("ug.moh.anc", "1.0.0", "program", "1.0.0", items()),
            "ann",
        )
        .await
        .expect("draft");
    publications.submit(first, "ann").await.expect("submit");
    publications.review(first, "rita").await.expect("review");
    publications.approve(first, "bob").await.expect("approve");
    publications.approve(first, "cara").await.expect("approve");
    publications
        .publish(first, "ops", &central.signer)
        .await
        .expect("publish");

    // Published only: its form, read from the published version, and no draft.
    let designs = publications.studio_designs("ann").await.expect("designs");
    assert_eq!(designs.packages.len(), 1);
    let anc = &designs.packages[0];
    assert_eq!(anc.name, "ug.moh.anc");
    assert_eq!(anc.title.as_deref(), Some("ANC"));
    assert_eq!(anc.kind, PackageKind::Program);
    assert_eq!(anc.published.as_deref(), Some(first));
    assert_eq!(anc.draft, None);
    assert_eq!(anc.canonical.as_deref(), Some("http://health.go.ug/fhir"));
    assert!(anc.changed.is_some());
    let kinds: Vec<_> = designs
        .designs
        .iter()
        .map(|d| (d.kind, d.package_id.as_str()))
        .collect();
    assert_eq!(kinds, [(DesignKind::Form, first)]);
    assert_eq!(designs.designs[0].title, "anc-visit");

    // A draft of the next version, with an Age Band Set, a workflow and its decision table, an
    // indicator and a ValueSet: they are read from the draft, which is its holder's.
    let mut files = package("ug.moh.anc", "1.1.0", "program", "1.0.0", items());
    let (path, bytes) = workflow("1.1.0");
    files.insert(path, bytes);
    files.insert(
        "ValueSet-anc-visit-type.json".into(),
        json!({
            "resourceType": "ValueSet", "id": "anc-visit-type", "title": "ANC visit type",
            "url": "http://health.go.ug/fhir/ValueSet/anc-visit-type", "version": "1.1.0",
            "status": "active"
        })
        .to_string()
        .into_bytes(),
    );
    files.insert(
        "Library-age-bands-anc.json".into(),
        json!({
            "resourceType": "Library", "id": "age-bands-anc", "title": "ANC age bands",
            "url": "http://health.go.ug/fhir/Library/age-bands-anc", "version": "1.1.0",
            "status": "active", "type": {"coding": [{"code": "age-band-set"}]},
            "content": [{
                "contentType": "application/vnd.health.age-band-set+json",
                // {"unit":"years","ageAt":"period-end","bands":[{"from":10}]}
                "data": "eyJ1bml0IjoieWVhcnMiLCJhZ2VBdCI6InBlcmlvZC1lbmQiLCJiYW5kcyI6W3siZnJvbSI6MTB9XX0="
            }]
        })
        .to_string()
        .into_bytes(),
    );
    files.insert(
        "Measure-anc-visits.json".into(),
        json!({
            "resourceType": "Measure", "id": "anc-visits", "title": "ANC visits",
            "url": "http://health.go.ug/fhir/Measure/anc-visits", "version": "1.1.0",
            "status": "active", "basis": "Encounter",
            "scoring": {"coding": [{"code": "cohort"}]},
            "extension": [{
                "url": "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-period-date",
                "valueExpression": {"language": "text/fhirpath", "expression": "Encounter.actualPeriod.start"}
            }],
            "group": [{"population": [{
                "code": {"coding": [{"code": "initial-population"}]},
                "criteria": {"language": "text/fhirpath", "expression": "true"}
            }]}]
        })
        .to_string()
        .into_bytes(),
    );
    publications.draft(&files, "ann").await.expect("draft");
    let second = "ug.moh.anc@1.1.0";
    let designs = publications.studio_designs("ann").await.expect("designs");
    let anc = &designs.packages[0];
    assert_eq!(anc.published.as_deref(), Some(first));
    let draft = anc.draft.as_ref().expect("an open draft");
    assert_eq!(
        (draft.id.as_str(), draft.stage, draft.mine),
        (second, Stage::Draft, true)
    );
    let found: Vec<_> = designs
        .designs
        .iter()
        .map(|d| {
            (
                d.kind,
                d.title.as_str(),
                d.table.as_deref(),
                d.package_id.as_str(),
            )
        })
        .collect();
    assert_eq!(
        found,
        [
            (DesignKind::AgeBandSet, "ANC age bands", None, second),
            (DesignKind::Workflow, "ANC visit", None, second),
            (DesignKind::DecisionTable, "risk", Some("risk"), second),
            (DesignKind::Indicator, "ANC visits", None, second),
            (DesignKind::Form, "anc-visit", None, second),
            (DesignKind::ValueSet, "ANC visit type", None, second),
        ]
    );
    // Someone else sees the same draft as not theirs.
    let theirs = publications.studio_designs("bob").await.expect("designs");
    assert!(!theirs.packages[0].draft.as_ref().expect("draft").mine);

    // Submitted, it is still the open draft, past the Draft stage.
    publications.submit(second, "ann").await.expect("submit");
    let designs = publications.studio_designs("ann").await.expect("designs");
    let stage = designs.packages[0].draft.as_ref().map(|d| d.stage);
    assert!(
        matches!(
            stage,
            Some(Stage::Review | Stage::Validation | Stage::Simulation)
        ),
        "{stage:?}"
    );
}

#[tokio::test]
async fn a_package_with_only_withdrawn_drafts_is_left_out() {
    let central = central().await;
    let publications = &central.publications;
    let id = "ug.moh.tb@0.1.0";
    publications
        .draft(
            &package("ug.moh.tb", "0.1.0", "program", "0.1.0", items()),
            "ann",
        )
        .await
        .expect("draft");
    publications.withdraw(id, "ann").await.expect("withdraw");
    let designs = publications.studio_designs("ann").await.expect("designs");
    assert_eq!(designs.packages, []);
    assert_eq!(designs.designs, []);
}

/// A Reporting package's Reports (#556), from `configuration/reports/`, and its standard
/// Dashboards (#879), from `configuration/dashboards/`, each under its title.
#[tokio::test]
async fn a_reporting_package_lists_its_reports_and_dashboards() {
    let central = central().await;
    let publications = &central.publications;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/reference-packages/hmis");
    let mut files = Files::new();
    for path in [
        "package.json",
        "Library-manifest.json",
        "configuration/reports/hmis-105-anc.json",
        "configuration/reports/anc-facility-monitoring.json",
        "configuration/dashboards/anc-overview.json",
    ] {
        files.insert(path.into(), std::fs::read(dir.join(path)).expect("read"));
    }
    publications.draft(&files, "ann").await.expect("draft");
    let designs = publications.studio_designs("ann").await.expect("designs");
    assert_eq!(designs.packages[0].kind, PackageKind::Reporting);
    let found: Vec<_> = designs
        .designs
        .iter()
        .map(|d| (d.kind, d.title.as_str(), d.path.as_str()))
        .collect();
    assert_eq!(
        found,
        [
            (
                DesignKind::Dashboard,
                "ANC overview",
                "configuration/dashboards/anc-overview.json"
            ),
            (
                DesignKind::Report,
                "ANC facility monitoring",
                "configuration/reports/anc-facility-monitoring.json"
            ),
            (
                DesignKind::Report,
                "HMIS 105 · 2 Antenatal care",
                "configuration/reports/hmis-105-anc.json"
            ),
        ]
    );
}
