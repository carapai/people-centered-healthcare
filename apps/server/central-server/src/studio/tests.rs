//! The Studio's API over the real lifecycle (#492): the actions are listed for the signed-in
//! account with their reasons, and every step is checked again when it is taken.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::Request;
use health_access_policy::Permissions;
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_publication::{Files, NoSandbox};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::FixedClock;
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use tower::ServiceExt as _;

use super::*;

const ID: &str = "ug.moh.anc@1.0.0";

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
        "2026-11-19T09:00:00Z".parse().expect("time"),
    ));
    let dir = tempfile::tempdir().expect("dir");
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

fn package() -> Files {
    let file = |path: &str, json: Value| (path.to_owned(), json.to_string().into_bytes());
    Files::from([
        file(
            "package.json",
            json!({
                "name": "ug.moh.anc", "version": "1.0.0", "fhirVersions": ["5.0.0"],
                "dependencies": { "ug.moh.core": "2.1.0" },
                "kind": "program", "platform": { "minSoftware": "0.3.0", "formatVersion": 1 }
            }),
        ),
        file(
            "Library-manifest.json",
            json!({
                "resourceType": "Library", "id": "manifest", "title": "ANC",
                "url": "http://health.go.ug/fhir/Library/ug.moh.anc-manifest", "version": "1.0.0",
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
                "url": "http://health.go.ug/fhir/Questionnaire/anc-visit", "version": "1.0.0",
                "status": "active", "item": [{ "linkId": "bp", "text": "Blood pressure", "type": "string" }]
            }),
        ),
    ])
}

/// Accounts named by username: each person's account id is their name, and `nobody` has none.
fn usernames() -> Accounts {
    Arc::new(|username: String| {
        Box::pin(async move { Ok((username != "nobody").then_some(username)) })
    })
}

fn subject(account: &str, permissions: &[&str]) -> Subject {
    Subject {
        account: account.into(),
        facility: "central".into(),
        permissions: Permissions(permissions.iter().map(|p| (*p).to_owned()).collect()),
        programs: BTreeSet::new(),
        purpose: None,
        break_glass: None,
    }
}

async fn call(
    publications: &Publications,
    who: Subject,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (u16, Value) {
    let (status, _, json) = send(publications, who, method, path, None, body).await;
    (status, json)
}

/// Sends a request with `if_match`, answering its status, `ETag` and body.
async fn send(
    publications: &Publications,
    who: Subject,
    method: &str,
    path: &str,
    if_match: Option<&str>,
    body: Option<Value>,
) -> (u16, Option<String>, Value) {
    let app = router(publications.clone(), None, usernames()).layer(Extension(who));
    let mut request = Request::builder().method(method).uri(path);
    if let Some(tag) = if_match {
        request = request.header("if-match", tag);
    }
    let request = match body {
        Some(body) => request
            .header("content-type", "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .expect("request");
    let response = app.oneshot(request).await.expect("response");
    let status = response.status().as_u16();
    let etag = response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("json")
    };
    (status, etag, json)
}

fn action<'a>(package: &'a Value, name: &str) -> &'a Value {
    package["actions"]
        .as_array()
        .expect("actions")
        .iter()
        .find(|a| a["action"] == name)
        .expect("listed")
}

#[tokio::test]
async fn an_author_who_reviews_sees_approve_refused_and_is_refused_it_when_they_try() {
    let (publications, _dir) = publications().await;
    publications.draft(&package(), "ann").await.expect("draft");
    let path = format!("/packages/{ID}");

    // Without a Studio permission there is no Studio.
    let (status, _) = call(&publications, subject("eve", &[]), "GET", "/packages", None).await;
    assert_eq!(status, 403);

    let ann = || subject("ann", &PERMISSIONS);
    let (status, list) = call(&publications, ann(), "GET", "/packages", None).await;
    assert_eq!(status, 200, "{list}");
    assert_eq!(list["packages"][0]["id"], ID);
    assert_eq!(list["packages"][0]["next"], "submit");
    assert_eq!(list["packages"][0]["stage"], "draft");

    let (status, page) = call(
        &publications,
        ann(),
        "POST",
        &format!("{path}/submit"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(page["package"]["stage"], "review", "{}", page["checks"]);
    assert_eq!(page["checks"][0]["check"], "validation");
    assert_eq!(page["checks"][0]["passed"], true);
    let rita = subject("rita", &[REVIEW]);
    let (status, page) = call(&publications, rita, "POST", &format!("{path}/review"), None).await;
    assert_eq!(status, 200, "{page}");

    // The author sees Approve disabled, with the reason, and is refused it all the same.
    let (_, page) = call(&publications, ann(), "GET", &path, None).await;
    let approve = action(&page["package"], "approve");
    assert_eq!(approve["allowed"], false);
    assert_eq!(approve["refusal"]["code"], "author");
    assert_eq!(
        approve["refusal"]["message"],
        "You wrote this draft, so you cannot review or approve it"
    );
    let (status, refused) = call(
        &publications,
        ann(),
        "POST",
        &format!("{path}/approve"),
        None,
    )
    .await;
    assert_eq!(status, 409);
    assert_eq!(refused["refusal"]["code"], "author");

    // A step whose permission the roles do not carry is refused before the lifecycle is asked.
    let bob = || subject("bob", &[APPROVE]);
    let (status, refused) = call(
        &publications,
        bob(),
        "POST",
        &format!("{path}/request-changes"),
        Some(json!({ "reason": "why" })),
    )
    .await;
    assert_eq!(status, 403);
    assert_eq!(refused["refusal"]["permission"], "metadata.review");

    let (status, page) = call(
        &publications,
        bob(),
        "POST",
        &format!("{path}/approve"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(page["approvers"], json!(["bob"]));
    assert_eq!(page["approvalsRequired"], 2);
    let publish = action(&page["package"], "publish");
    assert_eq!(publish["refusal"]["code"], "permission");

    // A reason is needed to send it back; activating waits for publication.
    let (status, _) = call(
        &publications,
        subject("rita", &[REVIEW]),
        "POST",
        &format!("{path}/request-changes"),
        Some(json!({ "reason": " " })),
    )
    .await;
    assert_eq!(status, 422);
    let (status, refused) = call(
        &publications,
        subject("ops", &[ACTIVATE]),
        "POST",
        &format!("{path}/activate"),
        Some(json!({ "scope": "national" })),
    )
    .await;
    assert_eq!(status, 409);
    assert_eq!(refused["refusal"]["code"], "not-published");

    // Two approvals in, publishing needs the signing key central was not given.
    let (status, _) = call(
        &publications,
        subject("cara", &[APPROVE]),
        "POST",
        &format!("{path}/approve"),
        None,
    )
    .await;
    assert_eq!(status, 200);
    let (status, _) = call(
        &publications,
        subject("ops", &[PUBLISH]),
        "POST",
        &format!("{path}/publish"),
        None,
    )
    .await;
    assert_eq!(status, 503);

    let (status, _) = call(
        &publications,
        ann(),
        "POST",
        &format!("{path}/delete"),
        None,
    )
    .await;
    assert_eq!(status, 404);
    let (status, _) = call(
        &publications,
        ann(),
        "GET",
        "/packages/ug.moh.none@1.0.0",
        None,
    )
    .await;
    assert_eq!(status, 404);
}

const AUTHOR: &str = health_publication::studio::AUTHOR_PERMISSION;
const REVIEW: &str = health_publication::studio::REVIEW_PERMISSION;
const APPROVE: &str = health_publication::studio::APPROVE_PERMISSION;
const PUBLISH: &str = health_publication::studio::PUBLISH_PERMISSION;
const ACTIVATE: &str = health_publication::studio::ACTIVATE_PERMISSION;
const MANAGE: &str = health_publication::studio::MANAGE_PERMISSION;

#[tokio::test]
async fn two_tabs_saving_the_same_draft_the_second_gets_412_and_nothing_is_overwritten() {
    let (publications, _dir) = publications().await;
    publications.draft(&package(), "ann").await.expect("draft");
    let ann = || subject("ann", &[AUTHOR]);
    let file = format!("/packages/{ID}/files/Questionnaire-anc-visit.json");

    // Both tabs open the same artefact, at the same revision.
    let (status, tag, opened) = send(&publications, ann(), "GET", &file, None, None).await;
    assert_eq!(status, 200, "{opened}");
    let tag = tag.expect("an ETag");
    assert_eq!(tag, format!("\"{}\"", opened["revision"]));
    let edited = |text: &str| {
        json!({ "content": json!({
            "resourceType": "Questionnaire", "id": "anc-visit",
            "url": "http://health.go.ug/fhir/Questionnaire/anc-visit", "version": "1.0.0",
            "status": "active", "item": [{ "linkId": "bp", "text": text, "type": "string" }]
        }).to_string() })
    };

    // The first tab saves; the page comes back with the next revision.
    let (status, next, page) = send(
        &publications,
        ann(),
        "PUT",
        &file,
        Some(&tag),
        Some(edited("Systolic")),
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert_ne!(next.as_deref(), Some(tag.as_str()));
    assert_eq!(page["package"]["stage"], "draft");

    // The second tab, on the old revision, is refused, as is a save that names none.
    let (status, _, refused) = send(
        &publications,
        ann(),
        "PUT",
        &file,
        Some(&tag),
        Some(edited("Diastolic")),
    )
    .await;
    assert_eq!(status, 412, "{refused}");
    let (status, _, _) = send(&publications, ann(), "PUT", &file, None, Some(edited("x"))).await;
    assert_eq!(status, 412);
    let (_, _, now) = send(&publications, ann(), "GET", &file, None, None).await;
    assert!(
        now["content"].as_str().expect("text").contains("Systolic"),
        "{now}"
    );

    // Saving needs the author permission.
    let (status, _, _) = send(
        &publications,
        subject("rita", &[REVIEW]),
        "PUT",
        &file,
        next.as_deref(),
        Some(edited("x")),
    )
    .await;
    assert_eq!(status, 403);
    // A file that is not JSON is refused, and kept nowhere.
    let (status, _, _) = send(
        &publications,
        ann(),
        "PUT",
        &file,
        next.as_deref(),
        Some(json!({ "content": "{ nope" })),
    )
    .await;
    assert_eq!(status, 422);
}

// #495: the holder keeps one change note on the draft, under the same If-Match as a save, and the
// page gives it back; each changed artefact carries its changed elements.
#[tokio::test]
async fn the_holder_keeps_one_change_note_and_the_page_gives_it_back() {
    let (publications, _dir) = publications().await;
    publications.draft(&package(), "ann").await.expect("draft");
    let note = format!("/packages/{ID}/note");
    let body = Some(json!({ "note": "Reworded the BP question" }));
    let (status, _, _) = send(
        &publications,
        subject("ann", &[AUTHOR]),
        "PUT",
        &note,
        None,
        body.clone(),
    )
    .await;
    assert_eq!(status, 412, "a note needs If-Match");
    let (status, tag, page) = send(
        &publications,
        subject("ann", &[AUTHOR]),
        "PUT",
        &note,
        Some("\"0\""),
        body.clone(),
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(page["note"], "Reworded the BP question");
    assert_eq!(tag.as_deref(), Some("\"1\""));
    assert!(
        page["changes"]
            .as_array()
            .expect("changes")
            .iter()
            .all(|c| c["change"] == "added" && c["elements"] == json!([]))
    );
    // Someone else, even with the permission, may not.
    let (status, _, _) = send(
        &publications,
        subject("dan", &[AUTHOR]),
        "PUT",
        &note,
        tag.as_deref(),
        body,
    )
    .await;
    assert_eq!(status, 409);
}

#[tokio::test]
async fn check_now_shows_results_on_the_page_and_the_draft_stays_a_draft() {
    let (publications, _dir) = publications().await;
    publications.draft(&package(), "ann").await.expect("draft");
    let (status, tag, page) = send(
        &publications,
        subject("ann", &[AUTHOR]),
        "POST",
        &format!("/packages/{ID}/check"),
        None,
        None,
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(page["package"]["stage"], "draft");
    assert_eq!(page["checks"][0]["check"], "validation");
    assert_eq!(page["checks"][1]["check"], "simulation");
    assert_eq!(page["checks"][0]["current"], true);
    assert_eq!(
        page["history"]
            .as_array()
            .expect("history")
            .last()
            .expect("entry")["step"],
        "checked"
    );
    // Check now is never anyone's next step: the author's is still to submit.
    assert_eq!(page["package"]["next"], "submit");
    assert_eq!(page["editable"], true);
    // A check changes no revision.
    assert_eq!(tag, Some("\"0\"".to_owned()));
    let (status, _) = call(
        &publications,
        subject("rita", &[REVIEW]),
        "POST",
        &format!("/packages/{ID}/check"),
        None,
    )
    .await;
    assert_eq!(status, 403);
}

// One discussion per draft (#496): a reviewer comments on the Questionnaire, the comment lands
// in the history with the lifecycle's records, and a client cannot write a history entry.
#[tokio::test]
async fn a_comment_names_its_artefact_and_only_the_lifecycle_writes_the_history() {
    let (publications, _dir) = publications().await;
    publications.draft(&package(), "ann").await.expect("draft");
    let comments = format!("/packages/{ID}/comments");
    let (status, tag, page) = send(
        &publications,
        subject("rita", &[REVIEW]),
        "POST",
        &comments,
        None,
        Some(json!({
            "text": "Should BP be required?",
            "about": "Questionnaire-anc-visit.json",
            // A client may not choose what the entry is, who wrote it or the stage.
            "step": "approved", "by": "mo", "stage": "published"
        })),
    )
    .await;
    assert_eq!(status, 200, "{page}");
    // A comment changes no revision.
    assert_eq!(tag, Some("\"0\"".to_owned()));
    let history = page["history"].as_array().expect("history");
    assert_eq!(history.len(), 2);
    assert_eq!(history[0]["step"], "created");
    let comment = &history[1];
    assert_eq!(comment["step"], "commented");
    assert_eq!(comment["by"], "rita");
    assert_eq!(comment["stage"], "draft");
    assert_eq!(comment["text"], "Should BP be required?");
    assert_eq!(comment["about"], "Questionnaire-anc-visit.json");
    assert_eq!(page["package"]["stage"], "draft");
    assert_eq!(page["commentable"], true);

    // It names only an artefact of the draft, and says something.
    for (body, expected) in [
        (
            json!({ "text": "?", "about": "Questionnaire-gone.json" }),
            422,
        ),
        (json!({ "text": "   " }), 409),
    ] {
        let (status, _) = call(
            &publications,
            subject("rita", &[REVIEW]),
            "POST",
            &comments,
            Some(body),
        )
        .await;
        assert_eq!(status, expected);
    }
    // Only someone the Studio lets in.
    let (status, _) = call(
        &publications,
        subject("zed", &[]),
        "POST",
        &comments,
        Some(json!({ "text": "hello" })),
    )
    .await;
    assert_eq!(status, 403);
    // No step is a comment, and no route removes one.
    let (status, _) = call(
        &publications,
        subject("ann", &[AUTHOR]),
        "POST",
        &format!("/packages/{ID}/commented"),
        Some(json!({ "reason": "fake" })),
    )
    .await;
    assert!(status >= 400, "{status}");
    let (status, _) = call(
        &publications,
        subject("ann", &[AUTHOR]),
        "DELETE",
        &comments,
        None,
    )
    .await;
    assert_eq!(status, 405);
}

#[tokio::test]
async fn a_draft_starts_blank_and_takes_another_version() {
    let (publications, _dir) = publications().await;
    let dan = || subject("dan", &[AUTHOR]);
    let (status, tag, page) = send(
        &publications,
        dan(),
        "POST",
        "/drafts",
        None,
        Some(json!({
            "name": "ug.moh.tb", "kind": "program", "title": "Tuberculosis",
            "version": "0.1.0-draft", "canonical": "http://health.go.ug/fhir"
        })),
    )
    .await;
    assert_eq!(status, 201, "{page}");
    assert_eq!(page["package"]["id"], "ug.moh.tb@0.1.0-draft");
    assert!(
        page["files"]
            .as_array()
            .expect("files")
            .iter()
            .any(|f| f["path"] == "Library-manifest.json")
    );
    let (status, _, page) = send(
        &publications,
        dan(),
        "PUT",
        "/packages/ug.moh.tb@0.1.0-draft/version",
        tag.as_deref(),
        Some(json!({ "version": "0.1.0" })),
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(page["package"]["id"], "ug.moh.tb@0.1.0");
    let history = page["history"].as_array().expect("history");
    assert_eq!(history.last().expect("entry")["previous"], "0.1.0-draft");

    // Copying needs a published version; a blank draft needs its fields.
    let (status, _) = call(
        &publications,
        dan(),
        "POST",
        "/drafts",
        Some(json!({ "copyOf": "ug.moh.tb@0.1.0" })),
    )
    .await;
    assert_eq!(status, 404);
    let (status, _) = call(
        &publications,
        dan(),
        "POST",
        "/drafts",
        Some(json!({ "name": "ug.moh.hiv" })),
    )
    .await;
    assert_eq!(status, 422);
}

// #493: the holder hands the draft over by username; until it is accepted they keep it, and once
// it is, the former holder may not approve it. A manager reassigns with a reason, and the holder
// withdraws, freeing its name@version, the withdrawn one still readable.
#[tokio::test]
async fn a_draft_is_handed_over_reassigned_and_withdrawn_by_its_holder() {
    let (publications, _dir) = publications().await;
    publications.draft(&package(), "ann").await.expect("draft");
    let path = format!("/packages/{ID}");
    let ann = || subject("ann", &[AUTHOR, APPROVE]);
    let dan = || subject("dan", &[AUTHOR]);

    let (status, refused) = call(
        &publications,
        ann(),
        "POST",
        &format!("{path}/hand-over"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, 422, "{refused}");
    let (status, refused) = call(
        &publications,
        ann(),
        "POST",
        &format!("{path}/hand-over"),
        Some(json!({ "to": "nobody" })),
    )
    .await;
    assert_eq!(status, 422, "{refused}");
    let (status, refused) = call(
        &publications,
        dan(),
        "POST",
        &format!("{path}/hand-over"),
        Some(json!({ "to": "eve" })),
    )
    .await;
    assert_eq!(status, 409, "{refused}");
    assert_eq!(refused["refusal"]["code"], "not-holder");
    let (status, page) = call(
        &publications,
        ann(),
        "POST",
        &format!("{path}/hand-over"),
        Some(json!({ "to": "dan" })),
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(page["package"]["holder"], "ann");
    assert_eq!(page["package"]["offeredTo"], "dan");
    let history = page["history"].as_array().expect("history");
    assert_eq!(history.last().expect("entry")["step"], "handover-offered");
    assert_eq!(history.last().expect("entry")["person"], "dan");

    // Not accepted yet: dan may not edit, and his next step is to accept.
    let (_, page) = call(&publications, dan(), "GET", &path, None).await;
    assert_eq!(page["editable"], false);
    assert_eq!(page["editRefusal"]["code"], "not-holder");
    assert_eq!(page["package"]["next"], "accept-handover");
    let (status, page) = call(
        &publications,
        dan(),
        "POST",
        &format!("{path}/accept-handover"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(page["package"]["holder"], "dan");
    assert_eq!(page["editable"], true);

    // dan submits; ann, a former holder, may not approve.
    let (status, page) = call(
        &publications,
        dan(),
        "POST",
        &format!("{path}/submit"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{page}");
    let (status, _) = call(
        &publications,
        subject("rita", &[REVIEW]),
        "POST",
        &format!("{path}/review"),
        None,
    )
    .await;
    assert_eq!(status, 200);
    let (status, refused) = call(
        &publications,
        ann(),
        "POST",
        &format!("{path}/approve"),
        None,
    )
    .await;
    assert_eq!(status, 409, "{refused}");
    assert_eq!(refused["refusal"]["code"], "author");

    // dan withdraws it; it is kept under another id, and the version is free.
    let (status, page) = call(
        &publications,
        dan(),
        "POST",
        &format!("{path}/withdraw"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(page["package"]["stage"], "withdrawn");
    let kept = page["package"]["id"].as_str().expect("id").to_owned();
    assert_ne!(kept, ID);
    let (status, page) = call(
        &publications,
        ann(),
        "GET",
        &format!("/packages/{kept}"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(
        page["history"]
            .as_array()
            .expect("history")
            .last()
            .expect("entry")["step"],
        "withdrawn"
    );
    let (status, _) = call(&publications, ann(), "GET", &path, None).await;
    assert_eq!(status, 404);
    publications
        .draft(&package(), "ann")
        .await
        .expect("drafted again");

    // Reassigning needs metadata.manage and a reason.
    let (status, _) = call(
        &publications,
        ann(),
        "POST",
        &format!("{path}/reassign"),
        Some(json!({ "to": "dan", "reason": "leave" })),
    )
    .await;
    assert_eq!(status, 403);
    let mo = || subject("mo", &[MANAGE]);
    let (status, _) = call(
        &publications,
        mo(),
        "POST",
        &format!("{path}/reassign"),
        Some(json!({ "to": "dan" })),
    )
    .await;
    assert_eq!(status, 422);
    let (status, page) = call(
        &publications,
        mo(),
        "POST",
        &format!("{path}/reassign"),
        Some(json!({ "to": "dan", "reason": "ann is on leave" })),
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(page["package"]["holder"], "dan");
    let last = page["history"]
        .as_array()
        .expect("history")
        .last()
        .cloned()
        .expect("entry");
    assert_eq!(last["step"], "reassigned");
    assert_eq!(last["reason"], "ann is on leave");
    assert_eq!(last["by"], "mo");
}

#[test]
fn the_openapi_document_describes_the_studio() {
    let document = crate::openapi().to_json().expect("json");
    for operation in [
        "listStudioPackages",
        "getStudioPackage",
        "takeStudioAction",
        "startStudioDraft",
        "getStudioFile",
        "saveStudioFile",
        "removeStudioFile",
        "setStudioVersion",
        "commentStudioPackage",
        "listStudioWorkflowActions",
        "tryStudioDecisionTable",
        "checkStudioDecisionTable",
    ] {
        assert!(document.contains(operation), "{operation}");
    }
}

// The terminology browser (#523).

const ICD11: &str = "http://id.who.int/icd/release/11/mms";
const SIGNS: &str = "http://health.go.ug/fhir/CodeSystem/anc-signs";
const DANGER: &str = "http://health.go.ug/fhir/ValueSet/danger-signs";

fn signer() -> health_publication::LocalSigner {
    let at = |text: &str| text.parse().expect("time");
    let root = health_pki::Authority::root("Uganda Health Root", at("2026-01-01T00:00:00Z"))
        .expect("root");
    let key = health_pki::new_key().expect("key");
    let issued = root
        .issue_publication(
            &health_pki::request(&key, "MoH publication signing").expect("request"),
            "MoH publication signing",
            at("2026-06-01T00:00:00Z"),
        )
        .expect("certificate");
    health_publication::LocalSigner::from_pem(&key.serialize_pem(), issued.pem()).expect("signer")
}

/// `name@version` of `kind` holding `resources`, each in `<resourceType>-<id>.json`.
fn terminology_package(name: &str, version: &str, kind: &str, resources: &[Value]) -> Files {
    let file = |path: String, json: &Value| (path, json.to_string().into_bytes());
    let mut files = Files::from([
        file(
            "package.json".into(),
            &json!({
                "name": name, "version": version, "fhirVersions": ["5.0.0"],
                "kind": kind, "platform": { "minSoftware": "0.3.0", "formatVersion": 1 }
            }),
        ),
        file(
            "Library-manifest.json".into(),
            &json!({
                "resourceType": "Library", "id": "manifest", "title": name,
                "url": format!("http://health.go.ug/fhir/Library/{name}-manifest"), "version": version,
                "status": "active", "type": { "text": "asset-collection" }
            }),
        ),
    ]);
    for resource in resources {
        let path = format!(
            "{}-{}.json",
            resource["resourceType"].as_str().expect("type"),
            resource["id"].as_str().expect("id")
        );
        files.insert(path, resource.to_string().into_bytes());
    }
    files
}

async fn publish(publications: &Publications, files: &Files) {
    let lifecycle = publications.draft(files, "ann").await.expect("draft");
    let id = health_publication::lifecycle_id(&lifecycle);
    let submitted = publications.submit(&id, "ann").await.expect("submit");
    assert_eq!(
        submitted.stage,
        health_configuration::lifecycle::Stage::Review,
        "{id} must validate"
    );
    publications.review(&id, "rita").await.expect("review");
    publications.approve(&id, "bob").await.expect("approve");
    publications.approve(&id, "cara").await.expect("approve");
    publications
        .publish(&id, "ops", &signer())
        .await
        .expect("publish");
}

/// A slice of ICD-11: a chapter with 300 intestinal infections under one block.
fn icd11() -> Value {
    let infections: Vec<Value> = (0..300)
        .map(|n| json!({ "code": format!("1A{n:03}"), "display": format!("Intestinal infection type {n}") }))
        .collect();
    json!({
        "resourceType": "CodeSystem", "id": "icd11", "url": ICD11, "version": "2024-01",
        "name": "ICD11MMS", "title": "ICD-11 MMS", "status": "active", "content": "complete",
        "caseSensitive": false,
        "concept": [
            { "code": "01", "display": "Certain infectious or parasitic diseases",
              "concept": [{ "code": "1A0", "display": "Bacterial intestinal infections", "concept": infections }] },
            { "code": "12", "display": "Diseases of the respiratory system",
              "concept": [{ "code": "CA40", "display": "Pneumonia" }] }
        ]
    })
}

fn signs(version: &str, extra: &[Value]) -> Value {
    let mut bleeding = vec![json!({ "code": "heavy-bleeding", "display": "Heavy bleeding" })];
    bleeding.extend(extra.iter().cloned());
    json!({
        "resourceType": "CodeSystem", "id": "anc-signs", "url": SIGNS, "version": version,
        "name": "AncSigns", "status": "active", "content": "complete",
        "concept": [{ "code": "danger", "display": "Danger signs", "concept": [
            { "code": "bleeding", "display": "Vaginal bleeding", "concept": bleeding },
            { "code": "fits", "display": "Convulsions" }
        ]}]
    })
}

fn danger(version: &str, compose: Value) -> Value {
    json!({
        "resourceType": "ValueSet", "id": "danger-signs", "url": DANGER, "version": version,
        "name": "DangerSigns", "title": "Danger signs", "status": "active", "compose": compose
    })
}

fn form(version: &str) -> Value {
    json!({
        "resourceType": "Questionnaire", "id": "anc-visit", "title": "ANC visit",
        "url": "http://health.go.ug/fhir/Questionnaire/anc-visit", "version": version, "status": "active",
        "item": [{ "linkId": "danger", "text": "Danger signs", "type": "coding",
                   "answerValueSet": format!("{DANGER}|{version}") }]
    })
}

#[tokio::test]
async fn the_terminology_browser_searches_adds_with_is_a_and_flags_removals() {
    let (publications, _dir) = publications().await;
    publish(
        &publications,
        &terminology_package("who.icd11", "2024.1.0", "terminology", &[icd11()]),
    )
    .await;
    let listed = json!({ "include": [{ "system": SIGNS, "concept": [{ "code": "bleeding" }, { "code": "fits" }] }] });
    publish(
        &publications,
        &terminology_package(
            "ug.moh.anc",
            "1.0.0",
            "program",
            &[signs("1.0.0", &[]), danger("1.0.0", listed), form("1.0.0")],
        ),
    )
    .await;
    let ann = || subject("ann", &[AUTHOR]);
    let (status, page) = call(
        &publications,
        ann(),
        "POST",
        "/drafts",
        Some(json!({ "copyOf": "ug.moh.anc@1.0.0" })),
    )
    .await;
    assert_eq!(status, 201, "{page}");
    let draft = page["package"]["id"].as_str().expect("id").to_owned();
    let base = format!("/packages/{draft}/terminology");

    // The code systems: the draft's own, local; ICD-11 from its terminology package, external.
    let (status, systems) = call(
        &publications,
        ann(),
        "GET",
        &format!("{base}/code-systems"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{systems}");
    let systems = systems["codeSystems"].as_array().expect("list");
    let own = systems.iter().find(|s| s["url"] == SIGNS).expect("own");
    assert_eq!(
        (own["own"].clone(), own["external"].clone()),
        (json!(true), json!(false))
    );
    assert_eq!(own["concepts"], 4);
    assert_eq!(own["path"], "CodeSystem-anc-signs.json");
    let icd = systems.iter().find(|s| s["url"] == ICD11).expect("icd");
    assert_eq!(
        (icd["external"].clone(), icd["concepts"].clone()),
        (json!(true), json!(304))
    );
    assert_eq!(icd["package"], "who.icd11@2024.1.0");

    // Searching ICD-11 by words: the server answers one page, never the whole system.
    let (status, found) = call(
        &publications,
        ann(),
        "GET",
        &format!("{base}/concepts?system={ICD11}&text=intestinal%20TYPE&count=20"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{found}");
    assert_eq!(found["total"], 300);
    assert_eq!(found["concepts"].as_array().expect("page").len(), 20);
    assert_eq!(found["concepts"][0]["code"], "1A000");
    // The hierarchy, one level at a time.
    let (_, top) = call(
        &publications,
        ann(),
        "GET",
        &format!("{base}/concepts?system={ICD11}"),
        None,
    )
    .await;
    assert_eq!(top["concepts"][0]["code"], "01");
    assert_eq!(top["concepts"][0]["children"], 1);
    let (_, under) = call(
        &publications,
        ann(),
        "GET",
        &format!("{base}/concepts?system={SIGNS}&parent=danger"),
        None,
    )
    .await;
    let codes: Vec<&str> = under["concepts"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|c| c["code"].as_str().expect("code"))
        .collect();
    assert_eq!(codes, ["bleeding", "fits"]);
    let (status, _) = call(
        &publications,
        ann(),
        "GET",
        &format!("{base}/concepts?system=urn:none"),
        None,
    )
    .await;
    assert_eq!(status, 404);

    // A concept shows the ValueSets that contain it.
    let (status, concept) = call(
        &publications,
        ann(),
        "GET",
        &format!("{base}/concept?system={SIGNS}&code=fits"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{concept}");
    assert_eq!(concept["parents"], json!(["danger"]));
    assert_eq!(concept["valueSets"][0]["url"], DANGER);

    // Editing the draft ValueSet, unsaved: bleeding with everything under it, and fits removed.
    let edited = danger(
        "1.1.0-draft",
        json!({ "include": [{ "system": SIGNS,
        "filter": [{ "property": "concept", "op": "is-a", "value": "bleeding" }] }] }),
    );
    let (status, expansion) = call(
        &publications,
        ann(),
        "POST",
        &format!("{base}/expand"),
        Some(json!({ "valueSet": edited })),
    )
    .await;
    assert_eq!(status, 200, "{expansion}");
    let codes = |e: &Value, key: &str| -> Vec<String> {
        e[key]
            .as_array()
            .expect("codes")
            .iter()
            .map(|c| c["code"].as_str().expect("code").to_owned())
            .collect()
    };
    assert_eq!(
        codes(&expansion, "contains"),
        ["bleeding", "heavy-bleeding"]
    );
    let change = &expansion["change"];
    assert_eq!(change["published"], format!("{DANGER}|1.0.0"));
    assert_eq!(codes(change, "added"), ["heavy-bleeding"]);
    assert_eq!(codes(change, "removed"), ["fits"], "the removal is flagged");
    assert_eq!(change["removedTotal"], 1);
    assert_eq!(expansion["boundBy"][0]["linkId"], "danger");

    // Saved, the is-a filter is kept, and a child code added later is in the expansion.
    let save = |path: &str, resource: &Value| {
        (
            format!("/packages/{draft}/files/{path}"),
            json!({ "content": resource.to_string() }),
        )
    };
    let (_, tag, _) = send(
        &publications,
        ann(),
        "GET",
        &format!("/packages/{draft}"),
        None,
        None,
    )
    .await;
    let (path, body) = save("ValueSet-danger-signs.json", &edited);
    let (status, tag, page) = send(
        &publications,
        ann(),
        "PUT",
        &path,
        tag.as_deref(),
        Some(body),
    )
    .await;
    assert_eq!(status, 200, "{page}");
    let (_, saved) = call(&publications, ann(), "GET", &path, None).await;
    assert!(
        saved["content"]
            .as_str()
            .expect("text")
            .contains("\"is-a\"")
    );
    let spotting = json!({ "code": "spotting", "display": "Spotting" });
    let (path, body) = save(
        "CodeSystem-anc-signs.json",
        &signs("1.1.0-draft", &[spotting]),
    );
    let (status, _, page) = send(
        &publications,
        ann(),
        "PUT",
        &path,
        tag.as_deref(),
        Some(body),
    )
    .await;
    assert_eq!(status, 200, "{page}");
    let (status, expansion) = call(
        &publications,
        ann(),
        "POST",
        &format!("{base}/expand"),
        Some(json!({ "url": DANGER })),
    )
    .await;
    assert_eq!(status, 200, "{expansion}");
    assert_eq!(
        codes(&expansion, "contains"),
        ["bleeding", "heavy-bleeding", "spotting"]
    );
    assert_eq!(codes(&expansion["change"], "removed"), ["fits"]);

    // The ValueSets a form may bind: the draft's own, searched.
    let (status, sets) = call(
        &publications,
        ann(),
        "GET",
        &format!("{base}/value-sets?text=danger"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{sets}");
    assert_eq!(sets["valueSets"][0]["url"], DANGER);
    assert_eq!(sets["valueSets"][0]["own"], true);
    let (_, none) = call(
        &publications,
        ann(),
        "GET",
        &format!("{base}/value-sets?text=pneumonia"),
        None,
    )
    .await;
    assert_eq!(none["valueSets"], json!([]));

    // Without a Studio permission there is nothing to browse.
    let (status, _) = call(
        &publications,
        subject("eve", &[]),
        "GET",
        &format!("{base}/code-systems"),
        None,
    )
    .await;
    assert_eq!(status, 403);
}

// The Workflow Studio's action catalogue (#498).

#[tokio::test]
async fn the_studio_lists_the_actions_publication_checks_against_with_their_schemas() {
    use health_publication::ActionDescriptor;
    use health_workflow_actions::{Compensation, ParamKind, ParamSpec};

    let (publications, _dir) = publications().await;
    let publications = publications.with_actions(vec![
        ActionDescriptor {
            id: "patient-flow/open-queue-entry",
            major: 1,
            permission: "queues.work",
            compensation: Compensation::Action("patient-flow/close-queue-entry"),
            params: vec![
                ParamSpec {
                    name: "service_point",
                    kind: ParamKind::Text,
                    required: true,
                },
                ParamSpec {
                    name: "priority",
                    kind: ParamKind::Code(&["routine", "urgent"]),
                    required: false,
                },
            ],
        },
        ActionDescriptor {
            id: "laboratory/collect-specimen",
            major: 2,
            permission: "laboratory.work",
            compensation: Compensation::None,
            params: vec![],
        },
    ]);
    let (status, body) = call(
        &publications,
        subject("ann", &["metadata.author"]),
        "GET",
        "/workflow-actions",
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let actions = body["actions"].as_array().expect("actions");
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0]["reference"], "laboratory/collect-specimen@2");
    assert_eq!(
        actions[0]["compensation"],
        json!({ "kind": "none", "action": null })
    );
    let queue = &actions[1];
    assert_eq!(queue["reference"], "patient-flow/open-queue-entry@1");
    assert_eq!(queue["permission"], "queues.work");
    assert_eq!(
        queue["compensation"],
        json!({ "kind": "action", "action": "patient-flow/close-queue-entry" })
    );
    // The schema served is the contract's own, field for field.
    let descriptors = publications.actions();
    let queue_descriptor = descriptors
        .iter()
        .find(|d| d.id == "patient-flow/open-queue-entry")
        .expect("descriptor");
    assert_eq!(
        queue["parameterSchema"],
        queue_descriptor.parameter_schema()
    );
    assert_eq!(
        queue["parameterSchema"]["required"],
        json!(["service_point"])
    );

    // Like every Studio read, it needs a Studio permission.
    let (status, _) = call(
        &publications,
        subject("cy", &["queues.work"]),
        "GET",
        "/workflow-actions",
        None,
    )
    .await;
    assert_eq!(status, 403);
}

#[test]
fn the_release_catalogue_lists_each_action_once() {
    // No Bounded Context declares an action yet; when one does, it is listed once.
    let catalogue = super::actions::catalogue();
    let mut references: Vec<String> = catalogue
        .iter()
        .map(health_publication::ActionDescriptor::reference)
        .collect();
    references.sort();
    references.dedup();
    assert_eq!(references.len(), catalogue.len());
}

// The decision-table editor (#516).

/// A draft Workflow Definition routing after triage: a sick child or an urgent pregnancy
/// bypasses payment, and a routine patient pays first, shadowing the routine infant below it.
fn routing() -> Value {
    json!({
        "url": "http://health.go.ug/fhir/Library/anc-visit", "version": "1.0.0",
        "statechart": { "states": [{ "id": "done", "kind": "final" }] },
        "dataRequirements": [
            { "name": "patient", "resourceType": "Patient", "paths": ["Patient.age", "Patient.pregnant"] },
            { "name": "triage", "resourceType": "Observation", "paths": ["Observation.category", "Observation.danger"] }
        ],
        "decisionTables": [{
            "id": "route",
            "rows": [
                { "name": "Sick child or urgent pregnancy",
                  "when": ["(%patient.age < 5 and %triage.danger = true) or (%patient.pregnant = true and %triage.category = 'urgent')"],
                  "then": "bypass" },
                { "name": "Routine", "when": ["%triage.category = 'routine'"], "then": "normal" },
                { "name": "Routine infant", "when": ["%triage.category = 'routine' and %patient.age < 1"], "then": "bypass" }
            ]
        }]
    })
}

#[tokio::test]
async fn the_decision_table_editor_tries_a_patient_and_checks_the_draft_table() {
    let (publications, _dir) = publications().await;
    let mut files = package();
    let test = |name: &str, age: i64, expect: &str| {
        json!({
            "type": "decision-table-test", "name": name,
            "definition": "http://health.go.ug/fhir/Library/anc-visit|1.0.0", "table": "route",
            "inputs": { "%patient.age": age, "%triage.danger": true, "%triage.category": "routine", "%patient.pregnant": false },
            "expect": expect
        })
        .to_string()
        .into_bytes()
    };
    files.insert(
        "tests/sick-child.json".into(),
        test("Sick child", 3, "bypass"),
    );
    files.insert(
        "tests/well-adult.json".into(),
        test("Adult with danger signs", 30, "bypass"),
    );
    publications.draft(&files, "ann").await.expect("draft");
    let author = || subject("ann", &["metadata.author"]);

    // Try a patient: the outcome, the rule that fired, and each part of its tree.
    let (status, body) = call(
        &publications,
        author(),
        "POST",
        &format!("/packages/{ID}/decision-tables/try"),
        Some(json!({
            "definition": routing(), "table": "route",
            "inputs": { "%patient.age": 3, "%triage.danger": true, "%triage.category": "urgent", "%patient.pregnant": false },
            "nodes": ["%patient.age < 5 and %triage.danger = true", "%patient.pregnant = true", "%patient.age <"]
        })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["outcome"], "bypass");
    assert_eq!(body["row"], 0);
    assert_eq!(body["nodes"][0]["holds"], true);
    assert_eq!(body["nodes"][1]["holds"], false);
    assert_eq!(body["nodes"][2]["holds"], false);
    assert!(body["nodes"][2]["error"].is_string(), "{body}");

    // The checks: the shadowed rule never fires, gaps show without a default, and the saved
    // test cases run against the draft.
    let samples = json!({
        "%patient.age": [0, 1, 4, 5, 6],
        "%patient.pregnant": [true, false],
        "%triage.category": ["routine", "urgent"],
        "%triage.danger": [true, false]
    });
    let (status, body) = call(
        &publications,
        author(),
        "POST",
        &format!("/packages/{ID}/decision-tables/check"),
        Some(json!({ "definition": routing(), "table": "route", "samples": samples })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["covered"], true);
    assert_eq!(body["combinations"], 40);
    assert_eq!(body["neverFires"], json!([2]));
    assert!(body["gapCount"].as_u64().expect("gaps") > 0);
    assert!(
        body["gaps"]
            .as_array()
            .expect("gaps")
            .iter()
            .all(|g| g["%triage.category"] == "urgent")
    );
    let tests = body["tests"].as_array().expect("tests");
    assert_eq!(tests.len(), 2);
    assert_eq!(tests[0]["name"], "Sick child");
    assert_eq!(tests[0]["passes"], true);
    assert_eq!(tests[1]["path"], "tests/well-adult.json");
    assert_eq!(tests[1]["outcome"], "normal");
    assert_eq!(tests[1]["passes"], false);

    // A table the definition does not have, or an input of no declared requirement, is refused.
    let (status, _) = call(
        &publications,
        author(),
        "POST",
        &format!("/packages/{ID}/decision-tables/try"),
        Some(json!({ "definition": routing(), "table": "other", "inputs": {} })),
    )
    .await;
    assert_eq!(status, 422);
    let (status, body) = call(
        &publications,
        author(),
        "POST",
        &format!("/packages/{ID}/decision-tables/try"),
        Some(json!({ "definition": routing(), "table": "route", "inputs": { "%visit.kind": "new" } })),
    )
    .await;
    assert_eq!(status, 422);
    assert_eq!(
        body["detail"],
        "%visit.kind reads visit, which is not a declared data requirement"
    );

    // Like every Studio read, it needs a Studio permission.
    let (status, _) = call(
        &publications,
        subject("cy", &["queues.work"]),
        "POST",
        &format!("/packages/{ID}/decision-tables/check"),
        Some(json!({ "definition": routing(), "table": "route" })),
    )
    .await;
    assert_eq!(status, 403);
}
