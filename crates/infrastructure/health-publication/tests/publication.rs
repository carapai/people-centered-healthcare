//! The publication lifecycle at central (#88; configuration.md § Lifecycle; ADR 0025), over
//! SurrealDB in memory, a blob store in a temporary directory and a real publication-signing
//! certificate: a package goes from draft to a signed publication that verifies under the root,
//! no author reviews or approves, validation and the semver check send drafts back, and a
//! published version never changes.

use std::sync::Arc;

use health_configuration::lifecycle::{Refused, Stage, Step};
use health_files::FsBlobStore;
use health_packages::{Limits, Schemas, verify};
use health_pki::{Authority, Trust, new_key, request};
use health_publication::{Error, Files, LocalSigner, NoSandbox, Publications};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{Clock as _, FixedClock, Timestamp};
use serde_json::{Value, json};

struct Central {
    publications: Publications,
    store: SurrealStore,
    clock: Arc<FixedClock>,
    signer: LocalSigner,
    trust: Trust,
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
        store,
        clock,
        signer: LocalSigner::from_pem(&key.serialize_pem(), issued.pem()).expect("signer"),
        trust: Trust::new(&root.certificate_pem()).expect("trust"),
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
                "resourceType": "Library", "id": "manifest",
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
    json!([
        { "linkId": "bp", "text": "Blood pressure", "type": "string" },
        { "linkId": "pulse", "text": "Pulse", "type": "integer" }
    ])
}

/// Takes `id` from submission to Approval, reviewed by `rita`.
async fn to_approval(central: &Central, id: &str) {
    let submitted = central
        .publications
        .submit(id, "ann")
        .await
        .expect("submit");
    assert_eq!(submitted.stage, Stage::Review);
    central
        .publications
        .review(id, "rita")
        .await
        .expect("review");
}

#[tokio::test]
async fn a_clinical_package_publishes_after_review_and_two_approvals_by_non_authors() {
    let central = central().await;
    let files = package("ug.moh.anc", "1.0.0", "program", "1.0.0", items());
    let draft = central
        .publications
        .draft(&files, "ann")
        .await
        .expect("draft");
    assert_eq!(draft.stage, Stage::Draft);
    let id = "ug.moh.anc@1.0.0";

    let submitted = central
        .publications
        .submit(id, "ann")
        .await
        .expect("submit");
    assert_eq!(submitted.stage, Stage::Review);
    assert!(matches!(
        central.publications.review(id, "ann").await,
        Err(Error::Refused(Refused::Author))
    ));
    central
        .publications
        .review(id, "rita")
        .await
        .expect("review");
    assert!(matches!(
        central.publications.approve(id, "ann").await,
        Err(Error::Refused(Refused::Author))
    ));
    central
        .publications
        .approve(id, "bob")
        .await
        .expect("first approval");
    assert!(matches!(
        central
            .publications
            .publish(id, "ops", &central.signer)
            .await,
        Err(Error::Refused(Refused::NotApproved { have: 1, need: 2 }))
    ));
    central
        .publications
        .approve(id, "cara")
        .await
        .expect("second approval");
    let published = central
        .publications
        .publish(id, "ops", &central.signer)
        .await
        .expect("publish");
    assert_eq!(published.digest, draft.content);
    assert_eq!(published.approvers, ["bob", "cara"]);
    assert_eq!(published.reviewer.as_deref(), Some("rita"));

    // What central distributes verifies as a node would check it.
    let (stored, tgz) = central.publications.package(id).await.expect("package");
    assert_eq!(stored, published);
    let digest = health_packages::Digest::of(&tgz);
    assert_eq!(digest.to_string(), published.digest);
    let verified = verify(
        &tgz,
        &published.signature,
        Some(&digest),
        &central.trust,
        &Schemas::platform(),
        Limits::default(),
        central.clock.now(),
    )
    .expect("verifies under the root");
    assert_eq!(verified.contents.id(), id);

    let (lifecycle, records) = central.publications.lifecycle(id).await.expect("lifecycle");
    assert_eq!(lifecycle.stage, Stage::Published);
    let steps: Vec<&Step> = records.iter().map(|record| &record.step).collect();
    assert_eq!(
        steps,
        [
            &Step::Created,
            &Step::Submitted,
            &Step::Validated,
            &Step::Simulated,
            &Step::Reviewed,
            &Step::Approved,
            &Step::Approved,
            &Step::Published,
        ]
    );
    assert!(central.publications.open().await.expect("open").is_empty());

    // A published version never changes, nor publishes twice.
    assert!(matches!(
        central.publications.draft(&files, "ann").await,
        Err(Error::Conflict(_))
    ));
    assert!(matches!(
        central
            .publications
            .publish(id, "ops", &central.signer)
            .await,
        Err(Error::Refused(Refused::Stage(Stage::Published)))
    ));
}

#[tokio::test]
async fn validation_sends_a_broken_draft_back_with_its_problems() {
    let central = central().await;
    let mut files = package(
        "ug.moh.settings",
        "1.0.0",
        "country-configuration",
        "1.0.0",
        items(),
    );
    let mut json: Value = serde_json::from_slice(&files["package.json"]).expect("package.json");
    json["dependencies"]["ug.moh.core"] = json!("^2.1.0");
    files.insert("package.json".into(), json.to_string().into_bytes());
    central
        .publications
        .draft(&files, "ann")
        .await
        .expect("draft");

    let id = "ug.moh.settings@1.0.0";
    let lifecycle = central
        .publications
        .submit(id, "ann")
        .await
        .expect("submit");
    assert_eq!(lifecycle.stage, Stage::Draft);
    let (_, records) = central.publications.lifecycle(id).await.expect("records");
    let Some(Step::ValidationFailed { problems }) = records.last().map(|r| &r.step) else {
        panic!("{records:?}");
    };
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("not pinned")),
        "{problems:?}"
    );

    // Only a draft's content may change; revising it by someone else makes them an author.
    json["dependencies"]["ug.moh.core"] = json!("2.1.0");
    files.insert("package.json".into(), json.to_string().into_bytes());
    let revised = central
        .publications
        .draft(&files, "dan")
        .await
        .expect("revise");
    assert_ne!(revised.content, lifecycle.content);
    assert!(revised.authors.contains("dan"));
    to_approval(&central, id).await;
    assert!(matches!(
        central.publications.draft(&files, "ann").await,
        Err(Error::Refused(Refused::Stage(Stage::Approval)))
    ));
    assert!(matches!(
        central.publications.approve(id, "dan").await,
        Err(Error::Refused(Refused::Author))
    ));

    // Changes requested clear the approval; configuration needs one approver.
    central
        .publications
        .approve(id, "bob")
        .await
        .expect("approve");
    let back = central
        .publications
        .request_changes(id, "cara", "name the deployment")
        .await
        .expect("changes");
    assert_eq!(back.stage, Stage::Draft);
    assert!(back.approvers.is_empty());
    to_approval(&central, id).await;
    central
        .publications
        .approve(id, "bob")
        .await
        .expect("approve");
    central
        .publications
        .publish(id, "ops", &central.signer)
        .await
        .expect("one approval is enough for configuration");
}

#[tokio::test]
async fn the_semver_check_runs_against_the_latest_published_version() {
    let central = central().await;
    let publish = |files: Files| {
        let central = &central;
        async move {
            let lifecycle = central
                .publications
                .draft(&files, "ann")
                .await
                .expect("draft");
            let id = format!("{}@{}", lifecycle.package, lifecycle.version);
            to_approval(central, &id).await;
            central
                .publications
                .approve(&id, "bob")
                .await
                .expect("approve");
            central
                .publications
                .publish(&id, "ops", &central.signer)
                .await
                .expect("publish");
        }
    };
    let kind = "country-configuration";
    publish(package("ug.moh.forms", "1.0.0", kind, "1.0.0", items())).await;

    // The pulse item is removed, but the form claims a minor bump.
    let fewer = json!([{ "linkId": "bp", "text": "Blood pressure", "type": "string" }]);
    let files = package("ug.moh.forms", "1.1.0", kind, "1.1.0", fewer.clone());
    central
        .publications
        .draft(&files, "ann")
        .await
        .expect("draft");
    let lifecycle = central
        .publications
        .submit("ug.moh.forms@1.1.0", "ann")
        .await
        .expect("submit");
    assert_eq!(lifecycle.stage, Stage::Draft);
    let (_, records) = central
        .publications
        .lifecycle("ug.moh.forms@1.1.0")
        .await
        .expect("records");
    let Some(Step::ValidationFailed { problems }) = records.last().map(|r| &r.step) else {
        panic!("{records:?}");
    };
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("Questionnaire/anc-visit went from 1.0.0 to 1.1.0")
            && problems[0].contains("item[pulse] (removed, major)"),
        "{problems:?}"
    );

    // As a major version of the form and the package, it passes.
    let files = package("ug.moh.forms", "2.0.0", kind, "2.0.0", fewer);
    central
        .publications
        .draft(&files, "ann")
        .await
        .expect("draft");
    let lifecycle = central
        .publications
        .submit("ug.moh.forms@2.0.0", "ann")
        .await
        .expect("submit");
    assert_eq!(lifecycle.stage, Stage::Review);

    // A form changed under the same version is refused.
    let reworded = json!([
        { "linkId": "bp", "text": "Blood pressure (mmHg)", "type": "string" },
        { "linkId": "pulse", "text": "Pulse", "type": "integer" }
    ]);
    let files = package("ug.moh.forms", "1.0.1", kind, "1.0.0", reworded);
    central
        .publications
        .draft(&files, "ann")
        .await
        .expect("draft");
    let lifecycle = central
        .publications
        .submit("ug.moh.forms@1.0.1", "ann")
        .await
        .expect("submit");
    assert_eq!(lifecycle.stage, Stage::Draft);

    let open = central.publications.open().await.expect("open");
    let ids: Vec<String> = open
        .iter()
        .map(|l| format!("{}@{}", l.package, l.version))
        .collect();
    assert_eq!(
        ids,
        [
            "ug.moh.forms@1.0.1",
            "ug.moh.forms@1.1.0",
            "ug.moh.forms@2.0.0"
        ]
    );
}

#[tokio::test]
async fn a_stale_revision_is_refused() {
    let central = central().await;
    let files = package("ug.moh.anc", "1.0.0", "program", "1.0.0", items());
    central
        .publications
        .draft(&files, "ann")
        .await
        .expect("draft");
    let mut tx = health_persistence::Store::begin(&central.store)
        .await
        .expect("begin");
    let (data, revision) = tx
        .package_lifecycle("ug.moh.anc@1.0.0")
        .await
        .expect("read")
        .expect("exists");
    assert!(
        tx.update_package_lifecycle("ug.moh.anc@1.0.0", "draft", &data, revision)
            .await
            .expect("update")
    );
    assert!(
        !tx.update_package_lifecycle("ug.moh.anc@1.0.0", "draft", &data, revision)
            .await
            .expect("update")
    );
}

#[tokio::test]
async fn unknown_packages_are_not_found() {
    let central = central().await;
    assert!(matches!(
        central
            .publications
            .submit("ug.moh.none@1.0.0", "ann")
            .await,
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        central.publications.package("ug.moh.none@1.0.0").await,
        Err(Error::NotFound(_))
    ));
}
