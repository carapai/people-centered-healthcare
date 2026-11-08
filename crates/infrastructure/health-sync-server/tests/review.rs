//! The Conflict Record review queue over HTTP (#82): only a reviewer with the conflict-resolution
//! permission sees it; a record shows both branches; a decision resolves it, audited.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Extension;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use health_access_policy::{Permissions, Subject};
use health_audit::Auditor;
use health_identifiers::{NodeId, OpId, ResourceId};
use health_persistence::{Precondition, RawJson, Store as _, Transaction as _, WriteRequest};
use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealAuthority, SurrealStore, SyncScope, Target, migrate,
};
use health_sync_protocol::{ContentHash, Group, Hlc, Item, Kind, Outcome, receive};
use health_sync_server::{CONFLICT_PERMISSION, review_router};
use health_time::SystemClock;
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use tower::ServiceExt as _;

async fn central() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "central".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

/// A Patient at central and an edge's edit of a version central never had: a Conflict Record.
async fn conflicted(store: &SurrealStore) -> (String, ResourceId) {
    let id = ResourceId::new();
    let mut tx = store.begin().await.expect("begin");
    tx.write(WriteRequest {
        resource_type: "Patient".parse().expect("type"),
        id,
        json: RawJson::from_validated(
            json!({"resourceType": "Patient", "name": [{"text": "Dina"}]}).to_string(),
        ),
        op_id: OpId::new(),
        precondition: Precondition::MustNotExist,
    })
    .await
    .expect("write");
    tx.commit().await.expect("commit");

    let body =
        json!({"resourceType": "Patient", "id": id.to_string(), "name": [{"text": "Dina A."}]});
    let tx_id = OpId::new();
    let item = Item {
        kind: Kind::Version,
        op_id: OpId::new(),
        tx_id,
        resource_type: "Patient".into(),
        resource_id: id.to_string(),
        parent_version_id: Some(OpId::new()),
        origin_node: NodeId::new(),
        device_id: None,
        author: None,
        hlc: Hlc::default(),
        content_hash: ContentHash::of(&body),
        packages: Vec::new(),
        snapshot_version: 0,
        provisional: false,
        deleted: false,
        blobs: Vec::new(),
        body,
    };
    let authority = SurrealAuthority::new(store, NodeId::new(), SyncScope::All);
    let result = receive(
        &authority,
        &Group {
            tx_id,
            items: vec![item],
        },
    )
    .await
    .expect("receive");
    assert!(matches!(result.outcome, Outcome::Conflict { .. }));
    (tx_id.to_string(), id)
}

fn app(store: &SurrealStore, permissions: &[&str]) -> Router {
    let subject = Subject {
        account: "registry-okello".into(),
        facility: "central".into(),
        permissions: Permissions(permissions.iter().map(|p| (*p).to_owned()).collect()),
        programs: BTreeSet::new(),
        purpose: None,
        break_glass: None,
    };
    review_router(store.clone(), Auditor::new("central", Vec::new())).layer(Extension(subject))
}

async fn call(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
        .expect("request");
    let response = app.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn a_reviewer_sees_both_branches_and_resolves_the_record_audited() {
    let store = central().await;
    let (record, id) = conflicted(&store).await;

    let (status, _) = call(&app(&store, &[]), "GET", "/", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no permission, no queue");

    let reviewer = app(&store, &[CONFLICT_PERMISSION]);
    let (status, queue) = call(&reviewer, "GET", "/", None).await;
    assert_eq!(status, StatusCode::OK);
    let entries = queue["conflicts"].as_array().expect("conflicts");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["class"], "identity");
    assert_eq!(entries[0]["overdue"], false);

    let (_, detail) = call(&reviewer, "GET", &format!("/{record}"), None).await;
    assert_eq!(
        detail["candidates"][0]["body"]["name"][0]["text"],
        "Dina A."
    );
    assert_eq!(
        detail["heads"][format!("Patient/{id}")]["name"][0]["text"],
        "Dina"
    );

    // A decision about a record outside this Conflict Record is refused.
    let (status, _) = call(
        &reviewer,
        "POST",
        &format!("/{record}"),
        Some(
            json!({"decision": "keep-head", "resource": format!("Patient/{}", ResourceId::new())}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let before = store.cursor().await.expect("cursor").seq;
    let (status, resolved) = call(
        &reviewer,
        "POST",
        &format!("/{record}"),
        Some(json!({
            "decision": "write",
            "resource": format!("Patient/{id}"),
            "json": {"resourceType": "Patient", "name": [{"text": "Dina Atim"}]}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert_eq!(resolved["status"], "resolved");
    let (_, queue) = call(&reviewer, "GET", "/", None).await;
    assert!(queue["conflicts"].as_array().expect("conflicts").is_empty());

    // The new version and its AuditEvent were committed together.
    let written = store.changes_after(before, 10).await.expect("log");
    let types: Vec<String> = written
        .iter()
        .map(|c| c.resource_type.to_string())
        .collect();
    assert!(types.contains(&"Patient".to_owned()), "{types:?}");
    assert!(types.contains(&"AuditEvent".to_owned()), "{types:?}");
    assert_eq!(
        written[0].tx_id,
        written[written.len() - 1].tx_id,
        "one transaction"
    );
}
