//! Override points through central's lifecycle (ADR 0026, ADR 0067; #507): an override package
//! is validated against the latest published version of the base it overrides, and a base
//! version that narrows a range below a facility's value cannot be activated there.

use std::sync::Arc;

use health_configuration::activation::Scope;
use health_configuration::lifecycle::Stage;
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_packages::overrides::{OVERRIDE_POINTS_SCHEMA, OVERRIDES_SCHEMA};
use health_pki::{Authority, new_key, request};
use health_publication::{Error, Files, LocalSigner, NoSandbox, Publications};
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
            store,
            blobs,
            clock,
            Schemas::platform(),
            Arc::new(NoSandbox),
            "central",
        ),
        signer: LocalSigner::from_pem(&key.serialize_pem(), issued.pem()).expect("signer"),
        _blobs: dir,
    }
}

fn file(path: &str, value: &Value) -> (String, Vec<u8>) {
    (path.to_owned(), value.to_string().into_bytes())
}

fn package(name: &str, version: &str, kind: &str) -> [(String, Vec<u8>); 2] {
    [
        file(
            "package.json",
            &json!({
                "name": name, "version": version, "fhirVersions": ["5.0.0"],
                "kind": kind, "platform": { "minSoftware": "0.3.0", "formatVersion": 1 }
            }),
        ),
        file(
            "Library-manifest.json",
            &json!({
                "resourceType": "Library", "id": "manifest",
                "url": format!("http://health.go.ug/fhir/Library/{name}-manifest"),
                "version": version, "status": "active", "type": { "text": "asset-collection" }
            }),
        ),
    ]
}

/// `ug.moh.anc@version`, declaring tracing after a missed visit in `minimum`–28 days.
fn anc(version: &str, minimum: i64) -> Files {
    let mut files = Files::from(package("ug.moh.anc", version, "program"));
    files.extend([file(
        "configuration/override-points.json",
        &json!({
            "$schema": OVERRIDE_POINTS_SCHEMA,
            "points": [{
                "key": "tracing.missed-visit-days", "type": "integer", "default": minimum,
                "allowed": { "minimum": minimum, "maximum": 28 }, "class": "clinically-visible"
            }]
        }),
    )]);
    files
}

/// `ug.facility.kam013@version`, setting `key` of `ug.moh.anc` to `value`.
fn kam013(version: &str, key: &str, value: i64) -> Files {
    let mut files = Files::from(package("ug.facility.kam013", version, "override"));
    files.extend([file(
        "configuration/overrides.json",
        &json!({
            "$schema": OVERRIDES_SCHEMA,
            "settings": [{ "package": "ug.moh.anc", "key": key, "value": value }]
        }),
    )]);
    files
}

/// Drafts and submits `files` as `name@version`; the stage it reaches and the records' text.
async fn submit(central: &Central, files: &Files, id: &str) -> (Stage, String) {
    let publications = &central.publications;
    publications.draft(files, "ann").await.expect("draft");
    let submitted = publications.submit(id, "ann").await.expect("submit");
    let (_, records) = publications.lifecycle(id).await.expect("records");
    (
        submitted.stage,
        serde_json::to_string(&records).expect("json"),
    )
}

async fn publish(central: &Central, files: &Files, id: &str) {
    let (stage, records) = submit(central, files, id).await;
    assert_eq!(stage, Stage::Review, "{id}: {records}");
    let publications = &central.publications;
    publications.review(id, "rita").await.expect("review");
    publications.approve(id, "bob").await.expect("approve");
    let lifecycle = publications.lifecycle(id).await.expect("lifecycle").0;
    if lifecycle.approvers.len() < lifecycle.approvals_required() {
        publications.approve(id, "cara").await.expect("approve");
    }
    publications
        .publish(id, "ops", &central.signer)
        .await
        .expect("publish");
}

fn kam() -> Scope {
    Scope::Facility {
        facility: "KAM013".into(),
    }
}

#[tokio::test]
async fn an_override_setting_an_undeclared_key_is_refused() {
    let central = central().await;

    // Nothing of ug.moh.anc is published yet: there is nothing to override.
    let (stage, records) = submit(
        &central,
        &kam013("1.0.0", "tracing.missed-visit-days", 21),
        "ug.facility.kam013@1.0.0",
    )
    .await;
    assert_eq!(stage, Stage::Draft);
    assert!(
        records.contains("no version of ug.moh.anc is published to override"),
        "{records}"
    );

    publish(&central, &anc("2.0.0", 14), "ug.moh.anc@2.0.0").await;
    let (stage, records) = submit(
        &central,
        &kam013("1.0.1", "triage.skip-vitals", 1),
        "ug.facility.kam013@1.0.1",
    )
    .await;
    assert_eq!(stage, Stage::Draft);
    assert!(
        records.contains("ug.moh.anc declares no override point triage.skip-vitals"),
        "{records}"
    );

    let (stage, records) = submit(
        &central,
        &kam013("1.0.2", "tracing.missed-visit-days", 35),
        "ug.facility.kam013@1.0.2",
    )
    .await;
    assert_eq!(stage, Stage::Draft);
    assert!(records.contains("35 is not allowed: 14 to 28"), "{records}");

    // A declared key within its range goes through, with one approval.
    publish(
        &central,
        &kam013("1.1.0", "tracing.missed-visit-days", 21),
        "ug.facility.kam013@1.1.0",
    )
    .await;
}

#[tokio::test]
async fn a_base_that_narrows_a_range_below_a_facilitys_value_cannot_be_activated_there() {
    let central = central().await;
    let publications = &central.publications;
    publish(&central, &anc("2.0.0", 14), "ug.moh.anc@2.0.0").await;
    publish(
        &central,
        &kam013("1.0.0", "tracing.missed-visit-days", 14),
        "ug.facility.kam013@1.0.0",
    )
    .await;
    let v = |text: &str| text.parse().expect("version");
    publications
        .activate(
            "ug.moh.anc",
            &v("2.0.0"),
            Scope::National,
            None,
            "ops",
            None,
        )
        .await
        .expect("base activated");
    publications
        .activate(
            "ug.facility.kam013",
            &v("1.0.0"),
            kam(),
            None,
            "admin",
            None,
        )
        .await
        .expect("override activated");

    // Version 3 allows 21–28 days: KAM013's 14 falls outside.
    publish(&central, &anc("3.0.0", 21), "ug.moh.anc@3.0.0").await;
    for scope in [Scope::National, kam()] {
        let refused = publications
            .activate("ug.moh.anc", &v("3.0.0"), scope.clone(), None, "ops", None)
            .await;
        let Err(Error::Conflict(why)) = refused else {
            panic!("activated in {scope}: {refused:?}");
        };
        assert!(
            why.contains("ug.facility.kam013@1.0.0 in facility KAM013")
                && why.contains("14 is not allowed: 21 to 28"),
            "{why}"
        );
    }
    // Another facility, which overrides nothing, may take it.
    publications
        .activate(
            "ug.moh.anc",
            &v("3.0.0"),
            Scope::Facility {
                facility: "GUL001".into(),
            },
            None,
            "ops",
            None,
        )
        .await
        .expect("activated where nothing conflicts");

    // Once KAM013 changes its value, the new base may be activated there.
    publish(
        &central,
        &kam013("1.1.0", "tracing.missed-visit-days", 21),
        "ug.facility.kam013@1.1.0",
    )
    .await;
    publications
        .activate(
            "ug.facility.kam013",
            &v("1.1.0"),
            kam(),
            None,
            "admin",
            None,
        )
        .await
        .expect("changed override activated");
    publications
        .activate("ug.moh.anc", &v("3.0.0"), kam(), None, "ops", None)
        .await
        .expect("activated once the conflict is gone");
}
