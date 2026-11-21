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
        "listStudioDesigns",
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

// The Workflow Studio's sandbox (#499).

/// A draft visit: triage (a nurse's task, opening a queue entry, overdue after an hour), then a
/// consultation unless urgent.
fn visit() -> Value {
    json!({
        "url": "http://health.go.ug/fhir/Library/anc-visit", "version": "1.0.0",
        "statechart": { "states": [
            { "id": "triage", "kind": "atomic",
              "task": { "role": "nurse", "worklist": "opd-triage" },
              "on_entry": [[{ "kind": "execute", "action": "patient-flow/open-queue-entry@1" }]],
              "timers": [{ "after": "PT1H", "event": "triage.overdue" }],
              "transitions": [
                { "events": ["triaged"], "guard": "%urgent", "targets": ["done"] },
                { "events": ["triaged"], "targets": ["consultation"] }
              ] },
            { "id": "consultation", "kind": "atomic",
              "transitions": [{ "events": ["seen"], "targets": ["done"] }] },
            { "id": "done", "kind": "final" }
        ] }
    })
}

#[tokio::test]
async fn the_sandbox_plays_a_draft_step_by_step_and_reports_an_unanswered_guard() {
    let (publications, _dir) = publications().await;
    publications.draft(&package(), "ann").await.expect("draft");
    let author = || subject("ann", &["metadata.author"]);
    let path = format!("/packages/{ID}/workflows/sandbox");

    // Every step comes from the server: the start, with its stubbed action, task and timer.
    let (status, body) = call(
        &publications,
        author(),
        "POST",
        &path,
        Some(json!({ "definition": visit() })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["guards"], json!(["%urgent"]));
    assert_eq!(body["gates"], json!([]));
    let start = &body["steps"][0];
    assert_eq!(start["configuration"], json!(["triage"]));
    assert_eq!(start["actions"], json!(["patient-flow/open-queue-entry@1"]));
    assert_eq!(
        start["timers"],
        json!([{ "event": "triage.overdue", "dueIn": "PT1H" }])
    );
    assert_eq!(
        start["tasks"],
        json!([{ "state": "triage", "status": "ready" }])
    );
    assert_eq!(start["accepts"], json!(["triaged"]));
    assert_eq!(start["elapsed"], "PT0S");

    // An unanswered guard is reported and its move not taken.
    let (status, body) = call(
        &publications,
        author(),
        "POST",
        &path,
        Some(json!({ "definition": visit(), "moves": [{ "event": "triaged" }] })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["unanswered"], json!(["%urgent"]));
    assert_eq!(body["steps"].as_array().expect("steps").len(), 1);

    // Answered, the run goes on; waiting fires the timer.
    let (status, body) = call(
        &publications,
        author(),
        "POST",
        &path,
        Some(json!({
            "definition": visit(), "guards": { "%urgent": false }, "name": "Routine visit",
            "moves": [{ "wait": "PT1H" }, { "event": "triaged" }, { "event": "seen" }]
        })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["unanswered"], json!([]));
    assert_eq!(body["refused"], Value::Null);
    let steps = body["steps"].as_array().expect("steps");
    assert_eq!(steps.len(), 4);
    assert_eq!(steps[1]["timers"], json!([]));
    assert_eq!(steps[1]["elapsed"], "PT1H");
    assert_eq!(steps[2]["configuration"], json!(["consultation"]));
    assert_eq!(steps[3]["running"], false);

    // The run as a scenario: the answers it used and each step's configuration as expected,
    // passing in submission's simulation unchanged.
    let scenario: Value =
        serde_json::from_str(body["scenario"].as_str().expect("the file")).expect("json");
    let scenario = &scenario;
    assert_eq!(scenario["type"], "workflow-scenario");
    assert_eq!(scenario["name"], "Routine visit");
    assert_eq!(
        scenario["definition"],
        "http://health.go.ug/fhir/Library/anc-visit|1.0.0"
    );
    assert_eq!(scenario["guards"], json!({ "%urgent": false }));
    assert_eq!(
        scenario["steps"][2],
        json!({ "event": "triaged", "expect": {
            "configuration": ["consultation"], "actions": [], "timers": [], "tasks": [], "running": true
        } })
    );
    let definition: health_workflow::definition::WorkflowDefinition =
        serde_json::from_value(visit()).expect("definition");
    let scenario = serde_json::from_value(scenario.clone()).expect("scenario");
    assert_eq!(
        health_workflow::simulation::simulate(&definition, &scenario),
        Vec::<String>::new()
    );

    // A malformed wait or statechart is refused; like every Studio call it needs a permission.
    let (status, _) = call(
        &publications,
        author(),
        "POST",
        &path,
        Some(json!({ "definition": visit(), "moves": [{ "wait": "an hour" }] })),
    )
    .await;
    assert_eq!(status, 422);
    let mut broken = visit();
    broken["statechart"]["initial"] = json!(["nowhere"]);
    let (status, _) = call(
        &publications,
        author(),
        "POST",
        &path,
        Some(json!({ "definition": broken })),
    )
    .await;
    assert_eq!(status, 422);
    let (status, _) = call(
        &publications,
        subject("cy", &["queues.work"]),
        "POST",
        &path,
        Some(json!({ "definition": visit() })),
    )
    .await;
    assert_eq!(status, 403);
}

// Editing a local CodeSystem's concepts (#524).

#[tokio::test]
async fn a_local_code_system_is_edited_code_by_code_and_its_version_follows() {
    let (publications, _dir) = publications().await;
    let mut published_signs = signs("1.0.0", &[]);
    published_signs["concept"][0]["concept"][1]["definition"] = json!("Fits of any kind");
    let under_bleeding = json!({ "include": [{ "system": SIGNS,
        "filter": [{ "property": "concept", "op": "is-a", "value": "bleeding" }] }] });
    publish(
        &publications,
        &terminology_package(
            "ug.moh.anc",
            "1.0.0",
            "program",
            &[
                published_signs,
                danger("1.0.0", under_bleeding),
                form("1.0.0"),
            ],
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
    let path = "CodeSystem-anc-signs.json";
    let change_of = || format!("{base}/code-system-change?path={path}");
    let (_, mut tag, _) = send(
        &publications,
        ann(),
        "GET",
        &format!("/packages/{draft}"),
        None,
        None,
    )
    .await;
    let codes = |list: &Value| -> Vec<String> {
        list.as_array()
            .expect("codes")
            .iter()
            .map(|c| c["code"].as_str().expect("code").to_owned())
            .collect()
    };

    // As copied, nothing changed.
    let (status, change) = call(&publications, ann(), "GET", &change_of(), None).await;
    assert_eq!(status, 200, "{change}");
    assert_eq!(
        (&change["published"], &change["next"], &change["bump"]),
        (&json!("1.0.0"), &json!("1.0.0"), &json!("none"))
    );
    assert_eq!(change["reaches"][0]["valueSet"]["url"], DANGER);
    assert_eq!(change["reaches"][0]["gained"], json!([]));

    // An edit needs If-Match, like any save.
    let edit = format!("{base}/edit");
    let spotting = json!({ "path": path, "edit": "add", "code": "spotting", "display": "Spotting",
        "parent": "bleeding", "note": "IMCI 2024 adds this sign" });
    let (status, _, _) = send(
        &publications,
        ann(),
        "POST",
        &edit,
        None,
        Some(spotting.clone()),
    )
    .await;
    assert_eq!(status, 412);

    // Adding a code under bleeding: minor, and the is-a ValueSet on bleeding gains it.
    let (status, next, page) = send(
        &publications,
        ann(),
        "POST",
        &edit,
        tag.as_deref(),
        Some(spotting),
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert_ne!(next, tag, "the revision moves on");
    tag = next;
    let note = page["history"]
        .as_array()
        .expect("history")
        .iter()
        .find(|r| r["step"] == "commented")
        .expect("the note is in the discussion");
    assert_eq!(
        note["text"],
        "Added code spotting under bleeding: IMCI 2024 adds this sign"
    );
    assert_eq!(note["about"], path);
    let (_, change) = call(&publications, ann(), "GET", &change_of(), None).await;
    assert_eq!(
        (&change["version"], &change["next"], &change["bump"]),
        (&json!("1.1.0"), &json!("1.1.0"), &json!("minor"))
    );
    assert_eq!(change["changes"][0]["summary"], "Code spotting added");
    let reach = &change["reaches"][0];
    assert_eq!(reach["valueSet"]["url"], DANGER);
    assert_eq!(codes(&reach["gained"]), ["spotting"]);
    assert_eq!(reach["boundBy"][0]["linkId"], "danger");

    // A published code is fixed: not deleted, nor its code changed.
    let (_, fits) = call(
        &publications,
        ann(),
        "GET",
        &format!("{base}/concept?system={SIGNS}&code=fits"),
        None,
    )
    .await;
    assert_eq!(fits["fixed"], true);
    let (_, new) = call(
        &publications,
        ann(),
        "GET",
        &format!("{base}/concept?system={SIGNS}&code=spotting"),
        None,
    )
    .await;
    assert_eq!(new["fixed"], false);
    for refused in [
        json!({ "path": path, "edit": "delete", "code": "fits" }),
        json!({ "path": path, "edit": "change", "code": "fits", "newCode": "seizures",
                "display": "Convulsions", "definition": "Fits of any kind", "parent": "danger" }),
    ] {
        let (status, _, problem) = send(
            &publications,
            ann(),
            "POST",
            &edit,
            tag.as_deref(),
            Some(refused),
        )
        .await;
        assert_eq!(status, 422, "{problem}");
        assert!(
            problem["detail"]
                .as_str()
                .expect("detail")
                .contains("published")
        );
    }

    // Retiring one: kept, so records still resolve, but new forms no longer get it.
    let (status, next, page) = send(
        &publications,
        ann(),
        "POST",
        &edit,
        tag.as_deref(),
        Some(json!({ "path": path, "edit": "retire", "code": "heavy-bleeding" })),
    )
    .await;
    assert_eq!(status, 200, "{page}");
    tag = next;
    let (status, retired) = call(
        &publications,
        ann(),
        "GET",
        &format!("{base}/concept?system={SIGNS}&code=heavy-bleeding"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{retired}");
    assert_eq!(retired["inactive"], true);
    let (_, change) = call(&publications, ann(), "GET", &change_of(), None).await;
    assert_eq!(change["bump"], "minor");
    assert_eq!(codes(&change["reaches"][0]["lost"]), ["heavy-bleeding"]);
    assert_eq!(codes(&change["reaches"][0]["gained"]), ["spotting"]);

    // A new code is deleted freely.
    let (status, next, page) = send(
        &publications,
        ann(),
        "POST",
        &edit,
        tag.as_deref(),
        Some(json!({ "path": path, "edit": "delete", "code": "spotting" })),
    )
    .await;
    assert_eq!(status, 200, "{page}");
    tag = next;

    // Redefining a published code: major, with the reason.
    let (status, _, page) = send(
        &publications,
        ann(),
        "POST",
        &edit,
        tag.as_deref(),
        Some(
            json!({ "path": path, "edit": "change", "code": "fits", "display": "Convulsions",
                     "definition": "Generalised fits only", "parent": "danger" }),
        ),
    )
    .await;
    assert_eq!(status, 200, "{page}");
    let (_, change) = call(&publications, ann(), "GET", &change_of(), None).await;
    assert_eq!(
        (&change["version"], &change["next"], &change["bump"]),
        (&json!("2.0.0"), &json!("2.0.0"), &json!("major"))
    );
    let reason = change["changes"]
        .as_array()
        .expect("changes")
        .iter()
        .find(|c| c["bump"] == "major")
        .expect("a major change");
    assert_eq!(reason["summary"], "Code fits: definition changed");
    assert_eq!(reason["at"], "concept[fits].definition");
}

// The form designer's Import FHIR JSON (#497, #853): central converts R4 with health-fhir-mapping.

#[tokio::test]
async fn importing_an_r4_questionnaire_converts_it_on_the_server_with_its_report() {
    let (publications, _dir) = publications().await;
    publications.draft(&package(), "ann").await.expect("draft");
    let author = || subject("ann", &["metadata.author"]);
    let path = format!("/packages/{ID}/questionnaires/import");
    let r4 = json!({
        "resourceType": "Questionnaire", "status": "active", "title": "ANC contact",
        "item": [
            { "linkId": "danger", "type": "choice", "text": "Danger signs" },
            { "linkId": "other", "type": "open-choice", "text": "Other" }
        ]
    });

    // Converted, with the server's report, and nothing blocking.
    let (status, body) = call(
        &publications,
        author(),
        "POST",
        &path,
        Some(json!({ "text": r4.to_string(), "version": "R4" })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["refusal"], Value::Null);
    assert_eq!(body["blocked"], false);
    assert_eq!(body["questionnaire"]["title"], "ANC contact");
    assert_eq!(body["questionnaire"]["item"][0]["type"], "coding");
    assert_eq!(
        body["questionnaire"]["item"][0]["answerConstraint"],
        "optionsOnly"
    );
    assert_eq!(
        body["questionnaire"]["item"][1]["answerConstraint"],
        "optionsOrString"
    );
    assert_eq!(
        body["report"],
        json!([
            { "outcome": "converted", "code": "choice-to-coding", "at": "danger", "element": null, "type": null },
            { "outcome": "converted", "code": "open-choice-to-coding", "at": "other", "element": null, "type": null }
        ])
    );

    // What cannot be converted is reported and blocks the import.
    let odd = json!({ "resourceType": "Questionnaire", "status": "active", "somethingElse": 1,
        "item": [{ "linkId": "a", "type": "question" }] });
    let (status, body) = call(
        &publications,
        author(),
        "POST",
        &path,
        Some(json!({ "text": odd.to_string(), "version": "R4" })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["blocked"], true);
    assert_eq!(
        body["report"],
        json!([
            { "outcome": "unconverted", "code": "unknown-element", "at": "Questionnaire", "element": "somethingElse", "type": null },
            { "outcome": "unconverted", "code": "unknown-type", "at": "a", "element": null, "type": "question" }
        ])
    );

    // Refusals: read as R5, R4 choice types; text that is not JSON.
    for (text, version, refusal) in [
        (r4.to_string(), "R5", "looks-r4"),
        ("{".to_owned(), "R5", "not-json"),
        (
            r#"{"resourceType":"Patient"}"#.to_owned(),
            "R4",
            "not-questionnaire",
        ),
    ] {
        let (status, body) = call(
            &publications,
            author(),
            "POST",
            &path,
            Some(json!({ "text": text, "version": version })),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["refusal"], refusal);
        assert_eq!(body["questionnaire"], Value::Null);
        assert_eq!(body["blocked"], true);
    }

    // Only a Studio author, and only on a package version that exists.
    let (status, _) = call(
        &publications,
        subject("bob", &["patient.read"]),
        "POST",
        &path,
        Some(json!({ "text": r4.to_string(), "version": "R4" })),
    )
    .await;
    assert_eq!(status, 403);
    let (status, _) = call(
        &publications,
        author(),
        "POST",
        "/packages/ug.moh.none@1.0.0/questionnaires/import",
        Some(json!({ "text": r4.to_string(), "version": "R4" })),
    )
    .await;
    assert_eq!(status, 404);
}

// The Studio home and its sections (#855).

#[tokio::test]
async fn the_designs_say_which_package_version_each_opens_in_and_whose_draft_it_is() {
    let (publications, _dir) = publications().await;
    let author = || subject("ann", &["metadata.author"]);
    publish(&publications, &package()).await;

    // Published only: the form opens in the published version, and nobody holds a draft.
    let (status, body) = call(&publications, author(), "GET", "/designs", None).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["packages"][0]["name"], "ug.moh.anc");
    assert_eq!(body["packages"][0]["published"], ID);
    assert_eq!(body["packages"][0]["draft"], Value::Null);
    assert_eq!(body["packages"][0]["canonical"], "http://health.go.ug/fhir");
    assert_eq!(body["designs"][0]["kind"], "form");
    assert_eq!(body["designs"][0]["packageId"], ID);
    assert_eq!(body["designs"][0]["path"], "Questionnaire-anc-visit.json");

    // *Edit* starts the next version: the form now opens in the draft, which is ann's.
    let (status, _) = call(
        &publications,
        author(),
        "POST",
        "/drafts",
        Some(json!({ "copyOf": ID })),
    )
    .await;
    assert_eq!(status, 201);
    let (_, body) = call(&publications, author(), "GET", "/designs?kind=form", None).await;
    let draft = &body["packages"][0]["draft"];
    assert_eq!(draft["id"], "ug.moh.anc@1.1.0-draft");
    assert_eq!(draft["stage"], "draft");
    assert_eq!(draft["mine"], true);
    assert_eq!(body["designs"][0]["packageId"], "ug.moh.anc@1.1.0-draft");
    let (_, theirs) = call(
        &publications,
        subject("rita", &["metadata.review"]),
        "GET",
        "/designs",
        None,
    )
    .await;
    assert_eq!(theirs["packages"][0]["draft"]["mine"], false);

    // Another kind keeps none of them; a kind that is not one is refused.
    let (_, body) = call(
        &publications,
        author(),
        "GET",
        "/designs?kind=workflow",
        None,
    )
    .await;
    assert_eq!(body["designs"], json!([]));
    assert_eq!(body["packages"].as_array().map(Vec::len), Some(1));
    let (status, _) = call(&publications, author(), "GET", "/designs?kind=poem", None).await;
    assert_eq!(status, 400);
    let (status, _) = call(&publications, subject("x", &[]), "GET", "/designs", None).await;
    assert_eq!(status, 403);
}

/// The ANC reference package's files: *ANC 1st visits* and its test patients (#548).
fn anc_package() -> Files {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/reference-packages/anc");
    let mut files = Files::new();
    for sub in ["", "tests"] {
        for entry in std::fs::read_dir(dir.join(sub)).expect("dir") {
            let path = entry.expect("entry").path();
            if path.is_file() {
                let name = path.file_name().expect("name").to_string_lossy();
                let key = if sub.is_empty() {
                    name.into_owned()
                } else {
                    format!("{sub}/{name}")
                };
                files.insert(key, std::fs::read(&path).expect("read"));
            }
        }
    }
    files
}

#[tokio::test]
async fn try_calculates_a_percentage_and_a_median_with_what_roll_up_needs() {
    let (publications, _dir) = publications().await;
    let files = anc_package();
    publications.draft(&files, "ann").await.expect("draft");
    let author = || subject("ann", &["metadata.author"]);
    let try_path = format!("/packages/{ID}/indicators/try");
    let measure = |file: &str| -> Value { serde_json::from_slice(&files[file]).expect("measure") };

    let input = json!({
        "measure": measure("Measure-anc-first-visit-before-12-weeks.json"),
        "periodType": "monthly",
        "period": "2026-03",
    });
    let (status, body) = call(&publications, author(), "POST", &try_path, Some(input)).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["calculation"], "percentage");
    assert_eq!(body["value"], 75);
    assert_eq!(
        (&body["count"], &body["denominator"]),
        (&json!(3), &json!(4))
    );
    assert_eq!(body["errors"], json!([]));
    let akello = body["patients"]
        .as_array()
        .expect("patients")
        .iter()
        .find(|p| p["patient"] == "Patient/akello")
        .expect("akello");
    assert_eq!(
        (&akello["count"], &akello["denominator"]),
        (&json!(0), &json!(1))
    );
    assert_eq!(akello["records"][0]["reason"], "denominator-only");

    let input = json!({
        "measure": measure("Measure-anc-gestational-age-at-first-visit.json"),
        "periodType": "monthly",
        "period": "2026-03",
    });
    let (status, body) = call(&publications, author(), "POST", &try_path, Some(input)).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["calculation"], "median");
    assert_eq!(body["value"], 9.5);
    assert_eq!(
        body["values"],
        json!({ "count": 4, "sum": 43, "minimum": 8, "maximum": 16 })
    );
    let nansubuga = body["patients"]
        .as_array()
        .expect("patients")
        .iter()
        .find(|p| p["patient"] == "Patient/nansubuga")
        .expect("nansubuga");
    assert_eq!(
        nansubuga["records"]
            .as_array()
            .expect("records")
            .iter()
            .map(|r| (r["value"].clone(), r["picked"].clone()))
            .collect::<Vec<_>>(),
        [(json!("8"), json!(true)), (json!("11"), json!(false))]
    );
}

#[tokio::test]
async fn try_counts_a_draft_indicator_over_the_test_patients_and_says_why() {
    let (publications, _dir) = publications().await;
    let files = anc_package();
    let measure: Value =
        serde_json::from_slice(&files["Measure-anc-first-visits.json"]).expect("measure");
    publications.draft(&files, "ann").await.expect("draft");
    let author = || subject("ann", &["metadata.author"]);
    let try_path = format!("/packages/{ID}/indicators/try");

    let (status, body) = call(
        &publications,
        author(),
        "POST",
        &try_path,
        Some(json!({ "measure": measure, "periodType": "monthly", "period": "2026-03" })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body["period"],
        json!({ "periodType": "monthly", "id": "2026-03", "start": "2026-03-01", "end": "2026-03-31" })
    );
    assert_eq!(body["calculation"], "count");
    assert_eq!(body["value"], 4);
    assert_eq!(body["count"], 4);
    assert_eq!(body["denominator"], Value::Null);
    assert_eq!(body["errors"], json!([]));
    let patients = body["patients"].as_array().expect("patients");
    assert_eq!(patients.len(), 11);
    let counted: Vec<&str> = patients
        .iter()
        .filter(|p| p["counted"] == true)
        .map(|p| p["patient"].as_str().expect("patient"))
        .collect();
    assert_eq!(
        counted,
        [
            "Patient/akello",
            "Patient/nakato",
            "Patient/namubiru",
            "Patient/nansubuga"
        ]
    );
    let nakato = patients
        .iter()
        .find(|p| p["patient"] == "Patient/nakato")
        .expect("nakato");
    assert_eq!(nakato["path"], "tests/patient-nakato.json");
    assert_eq!(nakato["display"], "Annet Nakato");
    assert_eq!(
        nakato["records"],
        json!([
            { "reference": "Encounter/nakato-visit-1", "date": "2026-03-04", "reason": "meets", "error": null, "value": null, "picked": false },
            { "reference": "Encounter/nakato-visit-2", "date": "2026-03-28", "reason": "does-not-meet", "error": null, "value": null, "picked": false }
        ])
    );
    let auma = patients
        .iter()
        .find(|p| p["patient"] == "Patient/auma")
        .expect("auma");
    assert_eq!(auma["counted"], false);
    assert_eq!(auma["records"][0]["reason"], "outside-period");

    // An unsaved edit is what is tried: without the sex condition, Opio counts too.
    let mut edited = measure.clone();
    edited["group"][0]["population"][0]["criteria"]["expression"] =
        json!("Encounter.type.coding.where(code = 'first').exists()");
    let (status, body) = call(
        &publications,
        author(),
        "POST",
        &try_path,
        Some(json!({ "measure": edited, "periodType": "quarterly", "period": "2026Q1" })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["value"], 7);

    // A Measure without a period date, or a period not of its type, is refused.
    let mut undated = measure.clone();
    undated["extension"] = json!([]);
    for (input, why) in [
        (
            json!({ "measure": undated, "periodType": "monthly", "period": "2026-03" }),
            "has no period date",
        ),
        (
            json!({ "measure": measure, "periodType": "monthly", "period": "2026Q1" }),
            "is not a Monthly period",
        ),
        (
            json!({ "measure": { "resourceType": "Measure" }, "periodType": "annual", "period": "2026" }),
            "is not a usable indicator",
        ),
    ] {
        let (status, body) = call(&publications, author(), "POST", &try_path, Some(input)).await;
        assert_eq!(status, 422, "{body}");
        assert!(
            body["detail"].as_str().expect("detail").contains(why),
            "{body}"
        );
    }
    // Counting visits, Nansubuga's two first-visit records count twice.
    let mut visits = measure.clone();
    visits["extension"][1]["valueCode"] = json!("visits");
    let (status, body) = call(
        &publications,
        author(),
        "POST",
        &try_path,
        Some(json!({ "measure": visits, "periodType": "monthly", "period": "2026-03" })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["value"], 5);

    // It needs a Studio permission, and a package version that exists.
    let input = json!({ "measure": measure, "periodType": "monthly", "period": "2026-03" });
    let (status, _) = call(
        &publications,
        subject("x", &[]),
        "POST",
        &try_path,
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, 403);
    let (status, _) = call(
        &publications,
        author(),
        "POST",
        "/packages/ug.moh.anc@9.9.9/indicators/try",
        Some(input),
    )
    .await;
    assert_eq!(status, 404);
}
