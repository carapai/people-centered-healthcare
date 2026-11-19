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
    let app = router(publications.clone(), None).layer(Extension(who));
    let request = Request::builder().method(method).uri(path);
    let request = match body {
        Some(body) => request
            .header("content-type", "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .expect("request");
    let response = app.oneshot(request).await.expect("response");
    let status = response.status().as_u16();
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
    (status, json)
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
        &format!("{path}/withdraw"),
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

const REVIEW: &str = health_publication::studio::REVIEW_PERMISSION;
const APPROVE: &str = health_publication::studio::APPROVE_PERMISSION;
const PUBLISH: &str = health_publication::studio::PUBLISH_PERMISSION;
const ACTIVATE: &str = health_publication::studio::ACTIVATE_PERMISSION;

#[test]
fn the_openapi_document_describes_the_studio() {
    let document = crate::openapi().to_json().expect("json");
    for operation in ["listStudioPackages", "getStudioPackage", "takeStudioAction"] {
        assert!(document.contains(operation), "{operation}");
    }
}
