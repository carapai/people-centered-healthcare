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
    ] {
        assert!(document.contains(operation), "{operation}");
    }
}
