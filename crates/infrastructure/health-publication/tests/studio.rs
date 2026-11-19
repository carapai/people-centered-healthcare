//! What the Studio shows of the lifecycle (#492; studio.md § The package workspace, § The
//! authoring API): the package list with each package's next step and active scopes, one
//! package read from top to bottom, and the actions the signed-in person may take, each refused
//! one with the reason the lifecycle's own rules give.

use std::sync::Arc;

use health_configuration::activation::Scope;
use health_configuration::lifecycle::Stage;
use health_configuration::lifecycle::semver::Bump;
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_pki::{Authority, new_key, request};
use health_publication::studio::{
    APPROVE_PERMISSION, ArtefactChangeKind, CheckKind, PERMISSIONS, RefusalCode, StudioAction,
};
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
    json!([
        { "linkId": "bp", "text": "Blood pressure", "type": "string" },
        { "linkId": "pulse", "text": "Pulse", "type": "integer" }
    ])
}

/// Every Studio permission.
fn every(_: &str) -> bool {
    true
}

/// Only `metadata.approve`.
fn approver(permission: &str) -> bool {
    permission == APPROVE_PERMISSION
}

fn refused(
    actions: &[health_publication::studio::Allowed],
    action: StudioAction,
) -> Option<&health_publication::studio::Refusal> {
    actions
        .iter()
        .find(|a| a.action == action)
        .expect("listed")
        .refused
        .as_ref()
}

#[tokio::test]
async fn an_author_sees_review_and_approval_refused_with_the_reason() {
    let central = central().await;
    let publications = &central.publications;
    let id = "ug.moh.anc@1.0.0";
    publications
        .draft(
            &package("ug.moh.anc", "1.0.0", "program", "1.0.0", items()),
            "ann",
        )
        .await
        .expect("draft");

    // In Draft, the author's next step is to submit; nobody else may.
    let rows = publications
        .studio_packages("ann", &every)
        .await
        .expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, id);
    assert_eq!(rows[0].title.as_deref(), Some("ANC"));
    assert_eq!(rows[0].stage, Stage::Draft);
    assert_eq!(rows[0].authors, ["ann"]);
    assert_eq!((rows[0].bump, rows[0].previous.as_ref()), (None, None));
    assert_eq!(rows[0].next, Some(StudioAction::Submit));
    assert_eq!(rows[0].actions.len(), StudioAction::ALL.len());
    let rows = publications
        .studio_packages("rita", &every)
        .await
        .expect("list");
    assert_eq!(rows[0].next, None);
    assert_eq!(
        refused(&rows[0].actions, StudioAction::Submit).map(|r| r.code),
        Some(RefusalCode::NotAuthor)
    );

    publications.submit(id, "ann").await.expect("submit");
    publications.review(id, "rita").await.expect("review");

    // At Approval, the author who also reviews and approves sees Approve refused, and why.
    let page = publications
        .studio_package(id, "ann", &every)
        .await
        .expect("page");
    let approve = refused(&page.summary.actions, StudioAction::Approve).expect("refused");
    assert_eq!(approve.code, RefusalCode::Author);
    assert_eq!(
        approve.message,
        "You wrote this draft, so you cannot review or approve it"
    );
    assert_eq!(
        refused(&page.summary.actions, StudioAction::RequestChanges).map(|r| r.code),
        Some(RefusalCode::Author)
    );
    assert_eq!(page.summary.next, None);
    assert_eq!(page.approvals_required, 2);
    assert_eq!(page.reviewer.as_deref(), Some("rita"));

    // Another approver may approve; without the permission it is refused for that.
    let page = publications
        .studio_package(id, "bob", &approver)
        .await
        .expect("page");
    assert_eq!(page.summary.next, Some(StudioAction::Approve));
    let publish = refused(&page.summary.actions, StudioAction::Publish).expect("refused");
    assert_eq!(publish.code, RefusalCode::Permission);
    assert_eq!(publish.permission.as_deref(), Some("metadata.publish"));
    let page = publications
        .studio_package(id, "bob", &|_| false)
        .await
        .expect("page");
    assert!(
        page.summary
            .actions
            .iter()
            .all(|a| a.refused.as_ref().map(|r| r.code) == Some(RefusalCode::Permission))
    );

    // Publication waits for the approvals still missing.
    let page = publications
        .studio_package(id, "ops", &every)
        .await
        .expect("page");
    let publish = refused(&page.summary.actions, StudioAction::Publish).expect("refused");
    assert_eq!(
        (publish.code, publish.missing),
        (RefusalCode::ApprovalsNeeded, Some(2))
    );
    publications.approve(id, "bob").await.expect("approve");
    let page = publications
        .studio_package(id, "ops", &every)
        .await
        .expect("page");
    let publish = refused(&page.summary.actions, StudioAction::Publish).expect("refused");
    assert_eq!(publish.message, "1 more approval needed");
    let page = publications
        .studio_package(id, "bob", &every)
        .await
        .expect("page");
    assert_eq!(
        refused(&page.summary.actions, StudioAction::Approve).map(|r| r.code),
        Some(RefusalCode::AlreadyApproved)
    );
    assert_eq!(
        refused(&page.summary.actions, StudioAction::Activate).map(|r| r.code),
        Some(RefusalCode::NotPublished)
    );
    publications.approve(id, "cara").await.expect("approve");
    // Every approval in, a publisher's next step is to publish.
    let publisher = |p: &str| p == health_publication::studio::PUBLISH_PERMISSION;
    let page = publications
        .studio_package(id, "ops", &publisher)
        .await
        .expect("page");
    assert_eq!(page.summary.next, Some(StudioAction::Publish));
    assert_eq!(page.approvers, ["bob", "cara"]);
    // What was refused there is refused by the lifecycle too: the list is advisory.
    assert!(publications.approve(id, "ann").await.is_err());
}

#[tokio::test]
async fn the_list_shows_the_newest_version_its_bump_and_where_it_is_active() {
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
    let version = "1.0.0".parse().expect("version");
    publications
        .activate("ug.moh.anc", &version, Scope::National, None, "ops", None)
        .await
        .expect("activate");
    let rows = publications
        .studio_packages("ops", &every)
        .await
        .expect("list");
    assert_eq!(rows[0].stage, Stage::Published);
    assert_eq!(rows[0].next, Some(StudioAction::Activate));
    assert_eq!(rows[0].active.len(), 1);
    assert_eq!(rows[0].active[0].scope, Scope::National);

    // A minor version: the form is reworded under the same version, so validation fails.
    let reworded = json!([
        { "linkId": "bp", "text": "Blood pressure (mmHg)", "type": "string" },
        { "linkId": "pulse", "text": "Pulse", "type": "integer" }
    ]);
    let mut files = package("ug.moh.anc", "1.1.0", "program", "1.0.0", reworded.clone());
    files.insert(
        "ValueSet-anc-visit-type.json".into(),
        json!({
            "resourceType": "ValueSet", "id": "anc-visit-type",
            "url": "http://health.go.ug/fhir/ValueSet/anc-visit-type", "version": "1.1.0",
            "status": "active"
        })
        .to_string()
        .into_bytes(),
    );
    publications.draft(&files, "ann").await.expect("draft");
    let second = "ug.moh.anc@1.1.0";
    publications.submit(second, "ann").await.expect("submit");

    let rows = publications
        .studio_packages("ann", &every)
        .await
        .expect("list");
    assert_eq!(rows.len(), 1, "one row per package");
    assert_eq!(rows[0].id, second);
    assert_eq!(rows[0].bump, Some(Bump::Minor));
    assert_eq!(
        rows[0]
            .previous
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("1.0.0")
    );
    assert_eq!(rows[0].active[0].version.to_string(), "1.0.0");

    let page = publications
        .studio_package(second, "ann", &every)
        .await
        .expect("page");
    let changes: Vec<_> = page
        .changes
        .iter()
        .map(|c| {
            (
                c.change,
                c.resource_type.as_str(),
                c.before.as_deref(),
                c.after.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        changes,
        [
            (
                ArtefactChangeKind::Changed,
                "Questionnaire",
                Some("1.0.0"),
                Some("1.0.0")
            ),
            (ArtefactChangeKind::Added, "ValueSet", None, Some("1.1.0")),
        ]
    );
    assert_eq!(page.checks.len(), 1);
    assert_eq!(page.checks[0].check, CheckKind::Validation);
    assert!(!page.checks[0].passed && page.checks[0].current);
    assert!(!page.checks[0].problems.is_empty());
    assert!(page.history.len() >= 3);

    // Once revised, the stored result is about content that is no longer the draft's.
    let files = package("ug.moh.anc", "1.1.0", "program", "1.1.0", reworded);
    publications.draft(&files, "ann").await.expect("revise");
    let page = publications
        .studio_package(second, "ann", &every)
        .await
        .expect("page");
    assert!(!page.checks[0].current);
    assert_eq!(page.changes.len(), 1);
    assert_eq!(page.changes[0].change, ArtefactChangeKind::Changed);
    assert_eq!(page.changes[0].after.as_deref(), Some("1.1.0"));

    // The previous version's page compares it with nothing: everything was added.
    let page = publications
        .studio_package(first, "ann", &every)
        .await
        .expect("page");
    assert!(
        page.changes
            .iter()
            .all(|c| c.change == ArtefactChangeKind::Added)
    );
    assert_eq!(page.checks.len(), 2);
    assert_eq!(PERMISSIONS.len(), 6);
}
