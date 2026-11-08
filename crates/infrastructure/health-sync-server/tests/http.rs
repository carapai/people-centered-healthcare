//! The `/sync/v1` endpoints over HTTP (#81; ADR 0062), in process: only a request signed by an
//! active node, for that very request and on time, is served; a node is held to its handshake's
//! verdict; and a compatible edge pushes and pulls in its facility's scope.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{Request, StatusCode};
use health_identifiers::{NodeId, OpId, ResourceId};
use health_persistence::{Precondition, RawJson, Store as _, Transaction as _, WriteRequest};
use health_pki::{Authority, IssuingCa, KeyPair, new_key, request};
use health_provisioning::{NodeRegistry, SYNC_SCHEME, sync_jws};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_sync_protocol::{
    Compatibility, GroupResult, Hello, Lane, Outcome, PROTOCOL, PullRequest, PullResponse, Reason,
    Verdict, Welcome, from_ndjson, to_ndjson,
};
use health_sync_server::SyncServer;
use health_time::{Clock as _, FixedClock, SignedDuration, Timestamp};
use http_body_util::BodyExt as _;
use p256::ecdsa::SigningKey;
use p256::pkcs8::DecodePrivateKey as _;
use serde_json::{Value, json};
use tower::ServiceExt as _;

const FACILITY: &str = "ug-kla-mulago";

async fn store(namespace: &str, clock: Arc<FixedClock>) -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: namespace.into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, clock)
}

struct Setup {
    app: Router,
    central: SurrealStore,
    clock: Arc<FixedClock>,
    node: NodeId,
    key: SigningKey,
    edge: SurrealStore,
    _files: tempfile::TempDir,
}

async fn setup() -> Setup {
    let clock = Arc::new(FixedClock::new(
        "2026-11-08T09:00:00Z".parse().expect("timestamp"),
    ));
    let central = store("central", clock.clone()).await;
    let registry = NodeRegistry::new(central.clone(), clock.clone(), "central");

    // A provisioned edge: its node id and its facility intermediate's key.
    let at: Timestamp = "2026-01-01T00:00:00Z".parse().expect("timestamp");
    let root = Authority::root("Root", at).expect("root");
    let ca_key = new_key().expect("key");
    let issued = root
        .issue_issuing_ca(
            &request(&ca_key, "issuing").expect("csr"),
            &IssuingCa {
                common_name: "Issuing".into(),
                dns_suffixes: Vec::new(),
            },
            at,
        )
        .expect("issuing");
    let issuer = Authority::from_pem(&issued.pem(), &ca_key.serialize_pem()).expect("issuing");
    let (code, _) = registry
        .issue_code(FACILITY, Vec::new(), None, "ops:auma")
        .await
        .expect("code");
    let edge_key: KeyPair = new_key().expect("key");
    let node = registry
        .redeem(
            &code,
            &request(&edge_key, "edge").expect("csr"),
            true,
            &issuer,
        )
        .await
        .expect("provisioned")
        .node;
    let key = SigningKey::from_pkcs8_pem(&edge_key.serialize_pem()).expect("p256 key");

    let compatibility = Compatibility {
        protocol: PROTOCOL,
        software: "1.4.0".parse().expect("version"),
        approved: BTreeSet::new(),
    };
    let central_id = central.node_id().await.expect("central id");
    let files = tempfile::tempdir().expect("files");
    let app = SyncServer::new(central.clone(), registry, compatibility)
        .with_blobs(health_files::FsBlobStore::open(files.path()).expect("blob store"))
        .router();
    let edge = store("edge", clock.clone()).await;
    edge.add_upstream(&central_id.to_string())
        .await
        .expect("upstream");
    Setup {
        app,
        central,
        clock,
        node: node.id.to_string().parse().expect("node id"),
        key,
        edge,
        _files: files,
    }
}

impl Setup {
    /// Sends `body` to `path`, signed by the edge at `at` (now by default).
    async fn send_at(
        &self,
        method: &str,
        path: &str,
        body: Vec<u8>,
        at: Timestamp,
    ) -> (StatusCode, Bytes) {
        let token = sync_jws(&self.node.to_string(), method, path, &body, &self.key, at);
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", format!("{SYNC_SCHEME} {token}"))
            .body(Body::from(body))
            .expect("request");
        let response = self.app.clone().oneshot(request).await.expect("response");
        let status = response.status();
        (
            status,
            response
                .into_body()
                .collect()
                .await
                .expect("body")
                .to_bytes(),
        )
    }

    async fn send(&self, method: &str, path: &str, body: Vec<u8>) -> (StatusCode, Bytes) {
        self.send_at(method, path, body, self.clock.now()).await
    }

    async fn hello(&self, software: &str) -> Welcome {
        let hello = Hello {
            protocol: PROTOCOL,
            software: software.parse().expect("version"),
            node: self.node,
            epoch: "e1".into(),
            packages: Vec::new(),
            snapshot_version: 0,
            cursor: None,
        };
        let (status, body) = self
            .send(
                "POST",
                "/sync/v1/hello",
                serde_json::to_vec(&hello).expect("json"),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        serde_json::from_slice(&body).expect("welcome")
    }

    /// Writes an observation at the edge and returns the items queued for central.
    async fn observation_from_edge(&self, facility: &str, central: NodeId) -> Vec<u8> {
        let body = json!({
            "resourceType": "Observation",
            "status": "final",
            "meta": {"tag": [{"system": "urn:health:facility", "code": facility}]}
        });
        let mut tx = self.edge.begin().await.expect("begin");
        tx.write(WriteRequest {
            resource_type: "Observation".parse().expect("type"),
            id: ResourceId::new(),
            json: RawJson::from_validated(body.to_string()),
            op_id: OpId::new(),
            precondition: Precondition::MustNotExist,
        })
        .await
        .expect("write");
        tx.commit().await.expect("commit");
        let outgoing = self
            .edge
            .outbox(&central.to_string(), self.node, 50)
            .await
            .expect("outbox");
        let items: Vec<_> = outgoing.into_iter().flat_map(|o| o.group.items).collect();
        to_ndjson(&items).expect("ndjson")
    }
}

#[tokio::test]
async fn only_a_request_signed_by_the_node_for_itself_and_on_time_is_served() {
    let s = setup().await;
    let unsigned = Request::builder()
        .method("POST")
        .uri("/sync/v1/hello")
        .body(Body::empty())
        .expect("request");
    let response = s.app.clone().oneshot(unsigned).await.expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Signed for another body, or another path: refused.
    let token = sync_jws(
        &s.node.to_string(),
        "POST",
        "/sync/v1/pull",
        b"{}",
        &s.key,
        s.clock.now(),
    );
    let replayed = Request::builder()
        .method("POST")
        .uri("/sync/v1/push")
        .header("authorization", format!("{SYNC_SCHEME} {token}"))
        .body(Body::from("{}"))
        .expect("request");
    let response = s.app.clone().oneshot(replayed).await.expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Signed ten minutes off central's clock: refused, naming the skew.
    let late = s.clock.now() + SignedDuration::from_mins(10);
    let (status, body) = s
        .send_at("POST", "/sync/v1/pull", b"{}".to_vec(), late)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let detail: Value = serde_json::from_slice(&body).expect("problem");
    assert!(
        detail["detail"].as_str().expect("detail").contains("clock"),
        "{detail}"
    );

    // Signed properly but before any handshake: told to say hello first.
    let (status, _) = s.send("POST", "/sync/v1/pull", b"{}".to_vec()).await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
}

#[tokio::test]
async fn a_compatible_edge_pushes_and_pulls_in_its_facility_scope() {
    let s = setup().await;
    let welcome = s.hello("1.3.2").await;
    assert_eq!(welcome.verdict, Verdict::Compatible);
    let central = welcome.node;
    assert_eq!(central, s.central.node_id().await.expect("id"));

    // Its own facility's observation is accepted; one tagged for another facility is refused.
    let ours = s.observation_from_edge(FACILITY, central).await;
    let (status, body) = s.send("POST", "/sync/v1/push", ours.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let results: Vec<GroupResult> = from_ndjson(&body).expect("results");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].outcome, Outcome::Accepted);
    // A resend after a lost response is answered from the receipts.
    let (_, body) = s.send("POST", "/sync/v1/push", ours).await;
    let again: Vec<GroupResult> = from_ndjson(&body).expect("results");
    assert!(again[0].replayed);

    let pending = s
        .edge
        .outbox(&central.to_string(), s.node, 50)
        .await
        .expect("outbox");
    let answers: Vec<Outcome> = results.iter().map(|r| r.outcome.clone()).collect();
    s.edge
        .record_push(&central.to_string(), &pending, &answers)
        .await
        .expect("record");
    let theirs = s.observation_from_edge("ug-gul-regional", central).await;
    let (_, body) = s.send("POST", "/sync/v1/push", theirs).await;
    let results: Vec<GroupResult> = from_ndjson(&body).expect("results");
    assert!(
        matches!(
            results[0].outcome,
            Outcome::Rejected {
                reason: Reason::Unauthorized,
                ..
            }
        ),
        "{results:?}"
    );

    // The pull sends nothing back the edge sent itself.
    let request = PullRequest {
        cursor: None,
        lane: Lane::Clinical,
        limit: 100,
    };
    let (status, body) = s
        .send(
            "POST",
            "/sync/v1/pull",
            serde_json::to_vec(&request).expect("json"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let PullResponse::Items { items, cursor, .. } = serde_json::from_slice(&body).expect("pull")
    else {
        panic!("items")
    };
    assert!(items.is_empty(), "{items:?}");
    assert_eq!(cursor.node, central);
}

#[tokio::test]
async fn an_out_of_range_edge_is_quarantined_and_sent_nothing() {
    let s = setup().await;
    let welcome = s.hello("1.1.0").await;
    assert!(
        matches!(welcome.verdict, Verdict::Quarantined { .. }),
        "{:?}",
        welcome.verdict
    );
    let items = s.observation_from_edge(FACILITY, welcome.node).await;
    let before = s.central.cursor().await.expect("cursor").seq;
    let (status, body) = s.send("POST", "/sync/v1/push", items).await;
    assert_eq!(status, StatusCode::OK);
    let results: Vec<GroupResult> = from_ndjson(&body).expect("results");
    assert!(
        matches!(
            results[0].outcome,
            Outcome::Rejected {
                reason: Reason::Quarantined,
                ..
            }
        ),
        "{results:?}"
    );
    assert_eq!(s.central.quarantined().await.expect("held").len(), 1);
    assert_eq!(
        s.central.cursor().await.expect("cursor").seq,
        before,
        "nothing applied"
    );

    let request = PullRequest {
        cursor: None,
        lane: Lane::Clinical,
        limit: 100,
    };
    let (status, _) = s
        .send(
            "POST",
            "/sync/v1/pull",
            serde_json::to_vec(&request).expect("json"),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_interrupted_attachment_upload_resumes_where_it_stopped() {
    use health_sync_protocol::{BlobStatus, CHUNK_SIZE, ContentHash};
    let s = setup().await;
    let size = 2 * CHUNK_SIZE + CHUNK_SIZE / 2;
    let content: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
    let hash = ContentHash::of_bytes(&content);
    let base = format!("/sync/v1/blobs/{hash}/{size}");
    let chunk = |offset: u64| {
        let end = (offset + CHUNK_SIZE).min(size);
        content[usize::try_from(offset).expect("offset")..usize::try_from(end).expect("end")]
            .to_vec()
    };
    let status = |body: &Bytes| serde_json::from_slice::<BlobStatus>(body).expect("status");

    let (code, body) = s.send("GET", &base, Vec::new()).await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(status(&body).received, 0);
    // The first chunk arrives, then the connection drops.
    let (code, body) = s.send("PUT", &format!("{base}/0"), chunk(0)).await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(status(&body).received, CHUNK_SIZE);

    // Resuming: central says where it stopped; a chunk from elsewhere is refused with that offset.
    let (_, body) = s.send("GET", &base, Vec::new()).await;
    let resume = status(&body);
    assert_eq!(resume.next_chunk(), Some((CHUNK_SIZE, CHUNK_SIZE)));
    let (code, body) = s
        .send(
            "PUT",
            &format!("{base}/{}", 2 * CHUNK_SIZE),
            chunk(2 * CHUNK_SIZE),
        )
        .await;
    assert_eq!(code, StatusCode::CONFLICT);
    assert_eq!(status(&body).received, CHUNK_SIZE);
    let mut at = resume;
    while let Some((offset, _)) = at.next_chunk() {
        let (code, body) = s
            .send("PUT", &format!("{base}/{offset}"), chunk(offset))
            .await;
        assert_eq!(code, StatusCode::OK);
        at = status(&body);
    }
    assert!(at.complete());

    // Stored and verified: it is served whole, and a second upload has nothing to send.
    let (code, body) = s
        .send("GET", &format!("/sync/v1/blobs/{hash}/content"), Vec::new())
        .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(body.as_ref(), content.as_slice());
    let (_, body) = s.send("GET", &base, Vec::new()).await;
    assert!(status(&body).complete(), "deduplicated");
}
