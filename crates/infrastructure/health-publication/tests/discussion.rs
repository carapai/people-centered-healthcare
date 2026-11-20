//! One discussion per draft (#496; studio.md § The authoring API, Discussion): the history and
//! the comments in one thread, oldest first. A comment may name an artefact of the draft; it is
//! a record the lifecycle writes, audited, and never removed. Commenting moves nothing, and does
//! not change the draft's revision, so no editor goes stale.

use std::sync::Arc;

use health_configuration::lifecycle::{Refused, Stage, Step};
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_publication::studio::RefusalCode;
use health_publication::{Error, Files, NoSandbox, Publications};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{FixedClock, Timestamp};
use serde_json::{Value, json};

const ID: &str = "ug.moh.anc@1.0.0";

struct Central {
    publications: Publications,
    connection: Connection,
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
    let dir = tempfile::tempdir().expect("tempdir");
    Central {
        publications: Publications::new(
            SurrealStore::new(connection.clone(), clock.clone()),
            FsBlobStore::open(dir.path()).expect("blobs"),
            clock,
            Schemas::platform(),
            Arc::new(NoSandbox),
            "central",
        ),
        connection,
        _blobs: dir,
    }
}

fn package(text: &str) -> Files {
    let file = |path: &str, json: Value| (path.to_owned(), json.to_string().into_bytes());
    Files::from([
        file(
            "package.json",
            json!({
                "name": "ug.moh.anc", "version": "1.0.0", "fhirVersions": ["5.0.0"],
                "dependencies": {}, "kind": "program",
                "platform": { "minSoftware": "0.3.0", "formatVersion": 1 }
            }),
        ),
        file(
            "Library-manifest.json",
            json!({
                "resourceType": "Library", "id": "manifest", "title": "ANC",
                "url": "http://health.go.ug/fhir/Library/ug.moh.anc-manifest", "version": "1.0.0",
                "status": "active", "type": { "text": "asset-collection" }
            }),
        ),
        file(
            "Questionnaire-anc-visit.json",
            json!({
                "resourceType": "Questionnaire", "id": "anc-visit",
                "url": "http://health.go.ug/fhir/Questionnaire/anc-visit", "version": "1.0.0",
                "status": "active", "item": [{ "linkId": "bp", "text": text, "type": "string" }]
            }),
        ),
    ])
}

fn every(_: &str) -> bool {
    true
}

fn refused(error: Error) -> Refused {
    match error {
        Error::Refused(refused) => refused,
        other => panic!("expected a refusal, got {other}"),
    }
}

/// Everything central's database holds, as text: the audit is in it.
async fn dump(central: &Central) -> String {
    String::from_utf8(central.connection.export_sql().await.expect("export")).expect("utf-8")
}

#[tokio::test]
async fn comments_join_the_history_in_one_thread_and_are_audited() {
    let central = central().await;
    let publications = &central.publications;
    publications
        .draft(&package("BP"), "ann")
        .await
        .expect("draft");
    let (_, revision) = publications.draft_files(ID).await.expect("files");

    // A reviewer, who does not hold the draft, comments on the Questionnaire.
    publications
        .comment(
            ID,
            "rita",
            "Should BP be required?",
            Some("Questionnaire-anc-visit.json"),
        )
        .await
        .expect("commented");
    // The holder answers; then a change, then another comment.
    publications
        .comment(ID, "ann", "Yes, done next", None)
        .await
        .expect("commented");
    publications
        .draft(&package("Blood pressure"), "ann")
        .await
        .expect("revised");

    let page = publications
        .studio_package(ID, "rita", &every)
        .await
        .expect("page");
    let steps: Vec<&Step> = page.history.iter().map(|r| &r.step).collect();
    assert_eq!(
        steps,
        [
            &Step::Created,
            &Step::Commented {
                text: "Should BP be required?".into(),
                about: Some("Questionnaire-anc-visit.json".into()),
            },
            &Step::Commented {
                text: "Yes, done next".into(),
                about: None,
            },
            &Step::Revised,
        ]
    );
    assert_eq!(page.history[1].by, "rita");
    assert_eq!(page.summary.stage, Stage::Draft);
    assert_eq!(page.comment, None, "anyone in the Studio may comment");
    // Comments did not move the revision: only the save did.
    assert_eq!(page.revision, revision + 1);

    let audited = dump(&central).await;
    assert!(audited.contains("commented on Questionnaire-anc-visit.json"));
}

#[tokio::test]
async fn a_comment_names_only_an_artefact_of_the_draft_and_ends_with_it() {
    let central = central().await;
    let publications = &central.publications;
    publications
        .draft(&package("BP"), "ann")
        .await
        .expect("draft");

    let error = publications
        .comment(ID, "rita", "?", Some("Questionnaire-gone.json"))
        .await
        .expect_err("no such artefact");
    assert!(matches!(error, Error::Invalid(_)), "{error}");
    let error = publications
        .comment(ID, "rita", "  ", None)
        .await
        .expect_err("blank");
    assert_eq!(refused(error), Refused::Incomplete);
    assert!(matches!(
        publications
            .comment("ug.moh.anc@9.9.9", "rita", "hi", None)
            .await,
        Err(Error::NotFound(_))
    ));

    let (kept, _) = publications.withdraw(ID, "ann").await.expect("withdrawn");
    let error = publications
        .comment(&kept, "rita", "too late", None)
        .await
        .expect_err("ended");
    assert_eq!(refused(error), Refused::Stage(Stage::Withdrawn));
    let page = publications
        .studio_package(&kept, "rita", &every)
        .await
        .expect("page");
    assert_eq!(
        page.comment.map(|refusal| refusal.code),
        Some(RefusalCode::Stage)
    );
    // Nothing was removed: the history still begins where it began.
    assert_eq!(page.history.first().map(|r| &r.step), Some(&Step::Created));
}
