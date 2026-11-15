//! An attachment whose blob has not arrived reads as pending (ADR 0021; #84).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use health_fhir_search::Unrestricted;
use health_fhir_server::FhirServer;
use health_identifiers::{OpId, ResourceId};
use health_persistence::{Precondition, RawJson, Store, Transaction, WriteRequest};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_sync_protocol::{ContentHash, PENDING_EXTENSION, blob_url};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn get(
    router: &axum::Router,
    uri: &str,
    etag: Option<&str>,
) -> (StatusCode, Option<String>, Value) {
    let mut request = Request::get(uri);
    if let Some(etag) = etag {
        request = request.header("if-none-match", etag);
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    let status = response.status();
    let cache = response
        .headers()
        .get("cache-control")
        .map(|v| v.to_str().expect("ascii").to_owned());
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("json")
    };
    (status, cache, body)
}

#[tokio::test]
async fn an_attachment_reads_as_pending_until_its_blob_arrives() {
    let settings = Settings {
        target: Target::Memory,
        namespace: "test".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock));
    let dir = tempfile::tempdir().expect("files");
    let blobs = health_files::FsBlobStore::open(dir.path()).expect("blob store");
    let router = FhirServer::new(store.clone(), "http://test/fhir", Arc::new(Unrestricted))
        .with_attachments(blobs.clone())
        .router();

    let scan = b"a referral letter".to_vec();
    let hash: ContentHash = health_files::BlobHash::of(&scan)
        .to_string()
        .parse()
        .expect("hash");
    let id = ResourceId::new();
    let op = OpId::new();
    let mut tx = store.begin().await.expect("begin");
    tx.write(WriteRequest {
        resource_type: "DocumentReference".parse().expect("type"),
        id,
        json: RawJson::from_validated(
            json!({
                "resourceType": "DocumentReference",
                "status": "current",
                "content": [{"attachment": {
                    "url": blob_url(&hash),
                    "size": scan.len(),
                    "contentType": "application/pdf"
                }}]
            })
            .to_string(),
        ),
        op_id: op,
        precondition: Precondition::MustNotExist,
    })
    .await
    .expect("write");
    tx.commit().await.expect("commit");
    let uri = format!("/DocumentReference/{id}");
    let etag = format!("W/\"{op}\"");

    // Accepted before its blob: pending, not to be cached, and not "not modified".
    let (status, cache, body) = get(&router, &uri, Some(&etag)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("no-store"));
    let attachment = &body["content"][0]["attachment"];
    assert_eq!(
        attachment["extension"],
        json!([{"url": PENDING_EXTENSION, "valueBoolean": true}])
    );
    let (_, _, versioned) = get(&router, &format!("{uri}/_history/{op}"), None).await;
    assert_eq!(
        versioned["content"][0]["attachment"]["extension"],
        attachment["extension"]
    );

    // Once the blob is here it reads as stored.
    blobs.put(scan.as_slice()).expect("put");
    let (status, cache, body) = get(&router, &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache, None);
    assert!(body["content"][0]["attachment"].get("extension").is_none());
    let (status, _, _) = get(&router, &uri, Some(&etag)).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
}
