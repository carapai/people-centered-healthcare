//! The Draft Holder (#493; studio.md § The authoring API; GLOSSARY.md): one holder per draft,
//! who alone edits, checks, submits, hands over or withdraws it; a handover takes effect when
//! accepted; a manager reassigns with a reason the history and the audit keep; every past holder
//! is an author for the separation of duties (ADR 0025); and a withdrawn draft is kept and frees
//! its `name@version`.

use std::sync::Arc;

use health_configuration::lifecycle::{Refused, Stage, Step};
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_publication::studio::{AUTHOR_PERMISSION, MANAGE_PERMISSION, RefusalCode, StudioAction};
use health_publication::{CLI_AGENT, Error, Files, NoSandbox, Publications};
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

fn author(permission: &str) -> bool {
    permission == AUTHOR_PERMISSION
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
async fn an_unaccepted_handover_leaves_the_draft_with_its_holder() {
    let central = central().await;
    let publications = &central.publications;
    publications
        .draft(&package("BP"), "ann")
        .await
        .expect("draft");

    // Only the holder hands it over.
    let error = publications
        .hand_over(ID, "dan", "eve")
        .await
        .expect_err("not the holder");
    assert_eq!(refused(error), Refused::NotHolder);
    let lifecycle = publications
        .hand_over(ID, "ann", "dan")
        .await
        .expect("offered");
    assert_eq!(lifecycle.holder, "ann");
    assert_eq!(lifecycle.offered_to.as_deref(), Some("dan"));

    // dan has not accepted: he may not save, and ann still may.
    let (_, revision) = publications.draft_files(ID).await.expect("files");
    let error = publications
        .save_file(
            ID,
            "Questionnaire-anc-visit.json",
            Some(b"{}".to_vec()),
            "dan",
            revision,
        )
        .await
        .expect_err("dan does not hold it yet");
    assert_eq!(refused(error), Refused::NotHolder);
    let page = publications
        .studio_package(ID, "dan", &every)
        .await
        .expect("page");
    assert_eq!(page.summary.holder, "ann");
    assert_eq!(page.summary.offered_to.as_deref(), Some("dan"));
    assert_eq!(
        page.edit.as_ref().map(|r| r.code),
        Some(RefusalCode::NotHolder)
    );
    // Accepting is dan's next step, and nobody else's.
    assert_eq!(page.summary.next, Some(StudioAction::AcceptHandover));
    let ann = publications
        .studio_package(ID, "ann", &every)
        .await
        .expect("page");
    assert!(ann.edit.is_none());
    let accept = ann
        .summary
        .actions
        .iter()
        .find(|a| a.action == StudioAction::AcceptHandover)
        .expect("listed");
    assert_eq!(
        accept.refused.as_ref().map(|r| r.code),
        Some(RefusalCode::NotOffered)
    );
    assert_eq!(ann.summary.next, Some(StudioAction::Submit));
    publications
        .draft(&package("Blood pressure"), "ann")
        .await
        .expect("ann still revises it");

    // Called off, the draft stays with ann, and dan cannot accept any more.
    publications
        .decline_handover(ID, "dan")
        .await
        .expect("declined");
    let error = publications
        .accept_handover(ID, "dan")
        .await
        .expect_err("nothing offered");
    assert_eq!(refused(error), Refused::NotOffered);
    let (lifecycle, history) = publications.lifecycle(ID).await.expect("lifecycle");
    assert_eq!(lifecycle.holder, "ann");
    assert!(
        history
            .iter()
            .any(|r| r.step == Step::HandoverOffered { to: "dan".into() })
    );
    assert!(
        history
            .iter()
            .any(|r| r.step == Step::HandoverDeclined { to: "dan".into() })
    );
}

#[tokio::test]
async fn a_former_holder_cannot_approve_the_draft() {
    let central = central().await;
    let publications = &central.publications;
    publications
        .draft(&package("BP"), "ann")
        .await
        .expect("draft");
    publications
        .hand_over(ID, "ann", "dan")
        .await
        .expect("offered");
    let lifecycle = publications
        .accept_handover(ID, "dan")
        .await
        .expect("accepted");
    assert_eq!(lifecycle.holder, "dan");

    // ann no longer holds it: she neither saves nor submits.
    let error = publications
        .draft(&package("Pulse"), "ann")
        .await
        .expect_err("not holder");
    assert_eq!(refused(error), Refused::NotHolder);
    let error = publications
        .submit(ID, "ann")
        .await
        .expect_err("not holder");
    assert_eq!(refused(error), Refused::NotHolder);
    publications.submit(ID, "dan").await.expect("dan submits");

    // Both are authors: neither reviews, and ann, the former holder, does not approve.
    let error = publications.review(ID, "ann").await.expect_err("an author");
    assert_eq!(refused(error), Refused::Author);
    publications.review(ID, "rita").await.expect("reviewed");
    let page = publications
        .studio_package(ID, "ann", &every)
        .await
        .expect("page");
    let approve = page
        .summary
        .actions
        .iter()
        .find(|a| a.action == StudioAction::Approve)
        .expect("listed");
    assert_eq!(
        approve.refused.as_ref().map(|r| r.code),
        Some(RefusalCode::Author)
    );
    let error = publications
        .approve(ID, "ann")
        .await
        .expect_err("a former holder");
    assert_eq!(refused(error), Refused::Author);
    publications.approve(ID, "bob").await.expect("approved");
    let (lifecycle, _) = publications.lifecycle(ID).await.expect("lifecycle");
    assert_eq!(
        lifecycle.authors.into_iter().collect::<Vec<_>>(),
        ["ann", "dan"]
    );
}

#[tokio::test]
async fn a_manager_reassigns_with_a_reason_in_the_history_and_the_audit() {
    let central = central().await;
    let publications = &central.publications;
    publications
        .draft(&package("BP"), "ann")
        .await
        .expect("draft");

    // The permission is metadata.manage; an author alone is refused it in the list.
    let page = publications
        .studio_package(ID, "mo", &author)
        .await
        .expect("page");
    let reassign = page
        .summary
        .actions
        .iter()
        .find(|a| a.action == StudioAction::Reassign)
        .expect("listed");
    let refusal = reassign.refused.as_ref().expect("refused");
    assert_eq!(refusal.code, RefusalCode::Permission);
    assert_eq!(refusal.permission.as_deref(), Some(MANAGE_PERMISSION));

    let error = publications
        .reassign(ID, "mo", "fay", " ")
        .await
        .expect_err("no reason");
    assert_eq!(refused(error), Refused::Incomplete);
    let lifecycle = publications
        .reassign(ID, "mo", "fay", "ann is on leave until March")
        .await
        .expect("reassigned");
    assert_eq!(lifecycle.holder, "fay");
    let (_, history) = publications.lifecycle(ID).await.expect("lifecycle");
    let last = history.last().expect("a record");
    assert_eq!(last.by, "mo");
    assert_eq!(
        last.step,
        Step::Reassigned {
            from: "ann".into(),
            to: "fay".into(),
            reason: "ann is on leave until March".into(),
        }
    );
    let audited = dump(&central).await;
    assert!(audited.contains("reassigned from ann to fay: ann is on leave until March"));
}

#[tokio::test]
async fn a_withdrawn_draft_frees_its_version_and_is_still_readable() {
    let central = central().await;
    let publications = &central.publications;
    publications
        .draft(&package("BP"), "ann")
        .await
        .expect("draft");
    let error = publications
        .withdraw(ID, "dan")
        .await
        .expect_err("not holder");
    assert_eq!(refused(error), Refused::NotHolder);

    let (kept, lifecycle) = publications.withdraw(ID, "ann").await.expect("withdrawn");
    assert_eq!(lifecycle.stage, Stage::Withdrawn);
    assert!(kept.starts_with("ug.moh.anc@1.0.0~withdrawn-"), "{kept}");
    assert!(matches!(
        publications.lifecycle(ID).await,
        Err(Error::NotFound(_))
    ));

    // The withdrawn one is still readable, with its history, and nothing more happens to it.
    let page = publications
        .studio_package(&kept, "ann", &every)
        .await
        .expect("page");
    assert_eq!(page.summary.id, kept);
    assert_eq!(page.summary.stage, Stage::Withdrawn);
    assert!(page.history.iter().any(|r| r.step == Step::Created));
    assert_eq!(page.history.last().map(|r| &r.step), Some(&Step::Withdrawn));
    assert!(page.summary.actions.iter().all(|a| a.refused.is_some()));
    assert_eq!(page.summary.next, None);
    assert!(page.edit.is_some());
    // It is not open.
    assert!(publications.open().await.expect("open").is_empty());
    // The list still shows it while the package has nothing else.
    let rows = publications
        .studio_packages("ann", &every)
        .await
        .expect("list");
    assert_eq!(
        rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        [kept.as_str()]
    );

    // Its name@version can be drafted again, by anyone, and the list shows the new draft.
    let again = publications
        .draft(&package("Pulse"), "dan")
        .await
        .expect("drafted again");
    assert_eq!((again.stage, again.holder.as_str()), (Stage::Draft, "dan"));
    let rows = publications
        .studio_packages("dan", &every)
        .await
        .expect("list");
    assert_eq!(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), [ID]);
    let (_, history) = publications.lifecycle(ID).await.expect("lifecycle");
    assert_eq!(history.len(), 1, "the new draft starts its own history");
    let old = publications
        .studio_package(&kept, "ann", &every)
        .await
        .expect("still there");
    assert_eq!(old.summary.holder, "ann");
}

#[tokio::test]
async fn the_cli_is_audited_as_a_cli_action_by_its_by_name() {
    let central = central().await;
    let cli = central.publications.clone().from_cli();
    cli.draft(&package("BP"), "ann").await.expect("draft");
    let audited = dump(&central).await;
    assert!(audited.contains(CLI_AGENT), "the CLI agent is in the audit");

    // The Studio's own steps are not.
    let central = self::central().await;
    central
        .publications
        .draft(&package("BP"), "ann")
        .await
        .expect("draft");
    assert!(!dump(&central).await.contains(CLI_AGENT));
}
