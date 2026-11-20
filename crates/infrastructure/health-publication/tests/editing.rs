//! Editing a draft in the Studio (#494; studio.md § The authoring API, Editing, Starting,
//! Checks): the server keeps the working copy and repacks it on each save, one artefact at a
//! time; a save names the revision it read, and a stale one changes nothing; a draft starts as a
//! copy of a published version with the next version proposed, or blank; *Check now* stores the
//! latest results without moving the draft.

use std::sync::Arc;

use health_configuration::lifecycle::{Stage, Step};
use health_configuration::package::PackageKind;
use health_files::FsBlobStore;
use health_packages::{Packed, Schemas};
use health_pki::{Authority, new_key, request};
use health_publication::editing::Blank;
use health_publication::studio::{CheckKind, StudioAction};
use health_publication::{Error, Files, LocalSigner, Publications, Sandbox};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{FixedClock, Timestamp};
use serde_json::{Value, json};

/// A sandbox in which every package fails one case, so a check has something to show.
struct Failing;

impl Sandbox for Failing {
    fn simulate(&self, _: &Packed) -> Vec<String> {
        vec!["tests/visit.json: expected the visit to end".into()]
    }
}

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
    let clock = Arc::new(FixedClock::new(at("2026-11-20T09:00:00Z")));
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
            Arc::new(Failing),
            "central",
        ),
        signer: LocalSigner::from_pem(&key.serialize_pem(), issued.pem()).expect("signer"),
        _blobs: dir,
    }
}

fn file(path: &str, json: &Value) -> (String, Vec<u8>) {
    (path.to_owned(), json.to_string().into_bytes())
}

fn form(text: &str) -> Value {
    json!({
        "resourceType": "Questionnaire", "id": "anc-visit",
        "url": "http://health.go.ug/fhir/Questionnaire/anc-visit", "version": "2.0.0",
        "status": "active", "item": [{ "linkId": "bp", "text": text, "type": "string" }]
    })
}

/// `ug.moh.anc@version`, a program package with one Questionnaire.
fn anc(version: &str) -> Files {
    Files::from([
        file(
            "package.json",
            &json!({
                "name": "ug.moh.anc", "version": version, "fhirVersions": ["5.0.0"],
                "kind": "program", "platform": { "minSoftware": "0.3.0", "formatVersion": 1 }
            }),
        ),
        file(
            "Library-manifest.json",
            &json!({
                "resourceType": "Library", "id": "manifest", "title": "ANC",
                "url": "http://health.go.ug/fhir/Library/ug.moh.anc-manifest", "version": version,
                "status": "active", "type": { "text": "asset-collection" }
            }),
        ),
        file("Questionnaire-anc-visit.json", &form("Blood pressure")),
    ])
}

/// Publishes `ug.moh.anc@2.0.0` through the whole lifecycle.
async fn published(central: &Central) {
    let publications = &central.publications;
    let id = "ug.moh.anc@2.0.0";
    let lifecycle = publications
        .draft(&anc("2.0.0"), "ann")
        .await
        .expect("draft");
    assert_eq!(lifecycle.stage, Stage::Draft);
    // The failing sandbox would stop submission; publish this one past it by hand.
    let lifecycle = publications.submit(id, "ann").await.expect("submit");
    assert_eq!(
        lifecycle.stage,
        Stage::Draft,
        "the sandbox fails every package"
    );
    let passing = publications
        .clone()
        .with_sandbox(Arc::new(health_publication::NoSandbox));
    passing.submit(id, "ann").await.expect("submit");
    passing.review(id, "rita").await.expect("review");
    passing.approve(id, "bob").await.expect("approve");
    passing.approve(id, "cara").await.expect("approve");
    passing
        .publish(id, "ops", &central.signer)
        .await
        .expect("publish");
}

fn every(_: &str) -> bool {
    true
}

#[tokio::test]
async fn a_copy_of_a_published_version_proposes_the_next_with_identical_artefacts() {
    let central = central().await;
    published(&central).await;
    let publications = &central.publications;

    let lifecycle = publications
        .start_copy("ug.moh.anc@2.0.0", "dan")
        .await
        .expect("copy");
    assert_eq!(lifecycle.version.to_string(), "2.1.0-draft");
    assert_eq!(lifecycle.stage, Stage::Draft);
    let id = "ug.moh.anc@2.1.0-draft";

    let (files, revision) = publications.draft_files(id).await.expect("files");
    assert_eq!(revision, 0);
    let (_, tgz) = publications
        .package("ug.moh.anc@2.0.0")
        .await
        .expect("published");
    let original = health_packages::unpack(&tgz, health_packages::Limits::default())
        .expect("unpack")
        .files;
    for (path, bytes) in &original {
        if path == "package.json" || path == "Library-manifest.json" || path.starts_with('.') {
            continue;
        }
        assert_eq!(files.get(path), Some(bytes), "{path} is copied as it was");
    }
    let manifest: Value =
        serde_json::from_slice(&files["Library-manifest.json"]).expect("manifest");
    assert_eq!(manifest["version"], "2.1.0-draft");

    // Nothing changed against 2.0.0, and the page says so.
    let page = publications
        .studio_package(id, "dan", &every)
        .await
        .expect("page");
    assert!(page.changes.is_empty(), "{:?}", page.changes);
    assert_eq!(
        page.summary.previous.map(|v| v.to_string()).as_deref(),
        Some("2.0.0")
    );
    assert_eq!(page.revision, 0);
    assert!(page.edit.is_none(), "the author may edit it");

    // A second copy of the same version is refused: that draft exists.
    assert!(matches!(
        publications.start_copy("ug.moh.anc@2.0.0", "eve").await,
        Err(Error::Conflict(_))
    ));
    // Only a published version is copied.
    assert!(matches!(
        publications.start_copy(id, "eve").await,
        Err(Error::NotFound(_))
    ));
}

#[tokio::test]
async fn a_stale_save_is_refused_and_overwrites_nothing() {
    let central = central().await;
    let publications = &central.publications;
    publications
        .draft(&anc("2.0.0"), "ann")
        .await
        .expect("draft");
    let id = "ug.moh.anc@2.0.0";
    let (_, read) = publications.draft_files(id).await.expect("files");

    // Two tabs read revision `read`. The first saves.
    let first = form("Systolic blood pressure").to_string().into_bytes();
    let revision = publications
        .save_file(
            id,
            "Questionnaire-anc-visit.json",
            Some(first.clone()),
            "ann",
            read,
        )
        .await
        .expect("first save");
    assert_eq!(revision, read + 1);

    // The second, still on `read`, is refused and changes nothing.
    let second = form("Diastolic blood pressure").to_string().into_bytes();
    let stale = publications
        .save_file(
            id,
            "Questionnaire-anc-visit.json",
            Some(second),
            "ann",
            read,
        )
        .await;
    assert!(
        matches!(stale, Err(Error::Stale { current }) if current == revision),
        "{stale:?}"
    );
    let (files, now) = publications.draft_files(id).await.expect("files");
    assert_eq!(now, revision);
    assert_eq!(files["Questionnaire-anc-visit.json"], first);

    // The saved artefact was repacked into the draft's tarball, and the save is in the history.
    let (lifecycle, history) = publications.lifecycle(id).await.expect("lifecycle");
    assert_eq!(lifecycle.stage, Stage::Draft);
    assert_eq!(history.last().map(|r| r.step.clone()), Some(Step::Revised));

    // A new artefact is added, and removed again, one at a time.
    let added = json!({
        "resourceType": "ValueSet", "id": "anc-risk",
        "url": "http://health.go.ug/fhir/ValueSet/anc-risk", "version": "2.0.0", "status": "active"
    });
    let revision = publications
        .save_file(
            id,
            "ValueSet-anc-risk.json",
            Some(added.to_string().into_bytes()),
            "ann",
            now,
        )
        .await
        .expect("added");
    let page = publications
        .studio_package(id, "ann", &every)
        .await
        .expect("page");
    assert!(
        page.files.iter().any(|f| f.path == "ValueSet-anc-risk.json"
            && f.resource_type.as_deref() == Some("ValueSet"))
    );
    let revision = publications
        .save_file(id, "ValueSet-anc-risk.json", None, "ann", revision)
        .await
        .expect("removed");
    let (files, _) = publications.draft_files(id).await.expect("files");
    assert!(!files.contains_key("ValueSet-anc-risk.json"));

    // A file that does not read, or a path outside the package, is refused before anything is
    // kept; so is renaming the package by hand.
    for (path, bytes) in [
        ("Questionnaire-anc-visit.json", b"{ not json".to_vec()),
        ("../escape.json", b"{}".to_vec()),
        (".index.json", b"{}".to_vec()),
    ] {
        let refused = publications
            .save_file(id, path, Some(bytes), "ann", revision)
            .await;
        assert!(
            matches!(refused, Err(Error::Invalid(_) | Error::Package(_))),
            "{path}: {refused:?}"
        );
    }
    let mut renamed: Value = serde_json::from_slice(&anc("2.0.0")["package.json"]).expect("json");
    renamed["name"] = json!("ug.moh.other");
    let refused = publications
        .save_file(
            id,
            "package.json",
            Some(renamed.to_string().into_bytes()),
            "ann",
            revision,
        )
        .await;
    assert!(matches!(refused, Err(Error::Invalid(_))), "{refused:?}");
    let (_, unchanged) = publications.draft_files(id).await.expect("files");
    assert_eq!(unchanged, revision);
}

#[tokio::test]
async fn check_now_shows_the_results_and_the_draft_stays_a_draft() {
    let central = central().await;
    let publications = &central.publications;
    let mut files = anc("2.0.0");
    // A Questionnaire without a version: a validation problem.
    let mut broken = form("Blood pressure");
    broken.as_object_mut().expect("object").remove("version");
    files.insert(
        "Questionnaire-anc-visit.json".into(),
        broken.to_string().into_bytes(),
    );
    publications.draft(&files, "ann").await.expect("draft");
    let id = "ug.moh.anc@2.0.0";
    let (_, revision) = publications.draft_files(id).await.expect("files");

    let lifecycle = publications.check_now(id, "ann").await.expect("checked");
    assert_eq!(lifecycle.stage, Stage::Draft);
    let page = publications
        .studio_package(id, "ann", &every)
        .await
        .expect("page");
    assert_eq!(page.summary.stage, Stage::Draft);
    let validation = &page.checks[0];
    assert_eq!(validation.check, CheckKind::Validation);
    assert!(!validation.passed);
    assert!(validation.current);
    assert!(
        validation.problems.iter().any(|p| p.contains("anc-visit")),
        "{:?}",
        validation.problems
    );
    let simulation = &page.checks[1];
    assert_eq!(simulation.check, CheckKind::Simulation);
    assert_eq!(
        simulation.problems,
        vec!["tests/visit.json: expected the visit to end".to_owned()]
    );
    assert!(matches!(
        page.history.last().map(|r| &r.step),
        Some(Step::Checked { .. })
    ));
    // A check changes no content, so an open editor's revision still holds.
    let (_, after) = publications.draft_files(id).await.expect("files");
    assert_eq!(after, revision);
    assert!(
        page.summary
            .actions
            .iter()
            .any(|a| a.action == StudioAction::Check && a.refused.is_none())
    );

    // Once a save changes the content, the results are marked as from before.
    publications
        .save_file(
            id,
            "Questionnaire-anc-visit.json",
            Some(form("Blood pressure").to_string().into_bytes()),
            "ann",
            revision,
        )
        .await
        .expect("saved");
    let page = publications
        .studio_package(id, "ann", &every)
        .await
        .expect("page");
    assert!(page.checks.iter().all(|c| !c.current));

    // Without the author permission, Check now is offered to nobody.
    let page = publications
        .studio_package(id, "rita", &|p: &str| p == "metadata.review")
        .await
        .expect("page");
    let check = page
        .summary
        .actions
        .iter()
        .find(|a| a.action == StudioAction::Check)
        .expect("listed");
    assert!(check.refused.is_some());
    assert!(page.edit.is_some());
}

#[tokio::test]
async fn a_draft_changes_version_until_submission_and_a_blank_one_starts_a_new_package() {
    let central = central().await;
    published(&central).await;
    let publications = &central.publications;
    publications
        .start_copy("ug.moh.anc@2.0.0", "dan")
        .await
        .expect("copy");
    let id = "ug.moh.anc@2.1.0-draft";
    let (_, revision) = publications.draft_files(id).await.expect("files");

    let stale = publications
        .change_version(id, "2.1.0", "dan", revision + 7)
        .await;
    assert!(matches!(stale, Err(Error::Stale { .. })), "{stale:?}");
    let (renamed, revision) = publications
        .change_version(id, "2.1.0", "dan", revision)
        .await
        .expect("renumbered");
    assert_eq!(renamed, "ug.moh.anc@2.1.0");
    assert!(matches!(
        publications.lifecycle(id).await,
        Err(Error::NotFound(_))
    ));
    let (files, now) = publications.draft_files(&renamed).await.expect("files");
    assert_eq!(now, revision);
    let package: Value = serde_json::from_slice(&files["package.json"]).expect("json");
    assert_eq!(package["version"], "2.1.0");
    let (_, history) = publications.lifecycle(&renamed).await.expect("lifecycle");
    assert_eq!(history.first().map(|r| r.step.clone()), Some(Step::Created));
    assert!(matches!(
        history.last().map(|r| &r.step),
        Some(Step::VersionChanged { previous }) if previous.to_string() == "2.1.0-draft"
    ));
    // A published version, or a version that is not one, is refused.
    for taken in ["2.0.0", "two"] {
        assert!(
            publications
                .change_version(&renamed, taken, "dan", revision)
                .await
                .is_err(),
            "{taken}"
        );
    }

    let lifecycle = publications
        .start_blank(
            &Blank {
                name: "ug.moh.tb".into(),
                kind: PackageKind::Program,
                title: "Tuberculosis".into(),
                version: "0.1.0".parse().expect("version"),
                canonical: "http://health.go.ug/fhir".into(),
                min_software: "0.3.0".into(),
            },
            "dan",
        )
        .await
        .expect("blank");
    assert_eq!(lifecycle.package, "ug.moh.tb");
    let (files, _) = publications
        .draft_files("ug.moh.tb@0.1.0")
        .await
        .expect("files");
    let manifest: Value =
        serde_json::from_slice(&files["Library-manifest.json"]).expect("manifest");
    assert_eq!(manifest["title"], "Tuberculosis");
    assert_eq!(
        manifest["url"],
        "http://health.go.ug/fhir/Library/ug.moh.tb-manifest"
    );
    assert!(matches!(
        publications
            .start_blank(
                &Blank {
                    name: "ug.moh.tb".into(),
                    kind: PackageKind::Program,
                    title: "Again".into(),
                    version: "0.1.0".parse().expect("version"),
                    canonical: "http://health.go.ug/fhir".into(),
                    min_software: "0.3.0".into(),
                },
                "eve",
            )
            .await,
        Err(Error::Conflict(_))
    ));
}
