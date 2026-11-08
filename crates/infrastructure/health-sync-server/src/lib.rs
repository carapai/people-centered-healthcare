//! The `/sync/v1` endpoints a node serves its downstream peers: central for edges and standalone
//! desktops (synchronization.md; ADRs 0019, 0062).
//!
//! Every request carries `Authorization: Health-Node <jws>`, signed with the calling node's
//! facility-intermediate key over the request's method, path and body. Central checks it against
//! the node registry, and the node's facility, from the registry, sets its Sync Scope.
//!
//! - `POST /sync/v1/hello` takes a [`Hello`] and answers a [`Welcome`]. The verdict is kept,
//!   and every later push and pull of the node is held to it.
//! - `POST /sync/v1/push` takes NDJSON items, groups them by transaction and answers NDJSON
//!   [`GroupResult`]s. A quarantined peer's items are held for review and answered
//!   `quarantined`.
//! - `POST /sync/v1/pull` takes a [`PullRequest`] and answers a [`PullResponse`](health_sync_protocol::PullResponse). A quarantined
//!   or refused peer is sent nothing.
//! - `POST /sync/v1/snapshot` answers every head in the peer's scope with the cursor at that
//!   instant, for a node told to re-seed.
//! - `GET /sync/v1/blobs/{hash}/{size}` says how much of an attachment this node holds,
//!   `PUT /sync/v1/blobs/{hash}/{size}/{offset}` appends a 1 MiB chunk there (the completing chunk
//!   verifies the blob), and `GET /sync/v1/blobs/{hash}/content` serves it on demand
//!   (synchronization.md § Attachments).
//! - `GET /sync/v1/events` is a server-sent event stream of "cursor advanced" hints. The pull is
//!   the truth; the hint only saves the peer a poll.
//!
//! Bodies are uncompressed NDJSON or JSON; zstd belongs to a later transport change.

mod review;

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use health_identifiers::NodeId;
use health_provisioning::{Error as RegistryError, NodeRegistry, SYNC_SCHEME};
use health_surrealdb::{SurrealAuthority, SurrealStore, SyncScope};
use health_sync_protocol::{
    Compatibility, GroupResult, Hello, Item, Outcome, PullRequest, Reason, Verdict, Welcome,
    from_ndjson, group, receive, to_ndjson,
};
use serde_json::json;

pub use review::{CONFLICT_PERMISSION, review_router};

/// The largest request body: a push batch is at most 4 MiB (synchronization.md § Push), with
/// room for the envelope.
const MAX_BODY: usize = 8 * 1024 * 1024;

/// How often the event stream checks the log, and how often a quiet stream sends a comment.
const HINT_POLL: Duration = Duration::from_secs(2);
const KEEP_ALIVE: Duration = Duration::from_secs(25);

/// What the sync endpoints serve from.
#[derive(Clone)]
pub struct SyncServer {
    inner: Arc<Inner>,
}

struct Inner {
    store: SurrealStore,
    registry: NodeRegistry,
    compatibility: Compatibility,
    scope_version: u64,
    blobs: Option<health_files::FsBlobStore>,
}

impl SyncServer {
    /// Serves `store`'s log, under the node id the store keeps, authenticating peers against
    /// `registry` and judging them by `compatibility`.
    pub fn new(store: SurrealStore, registry: NodeRegistry, compatibility: Compatibility) -> Self {
        Self {
            inner: Arc::new(Inner {
                store,
                registry,
                compatibility,
                scope_version: 1,
                blobs: None,
            }),
        }
    }

    /// Accepts and serves attachments from `blobs` (synchronization.md § Attachments).
    #[must_use]
    pub fn with_blobs(self, blobs: health_files::FsBlobStore) -> Self {
        let inner = Arc::try_unwrap(self.inner).unwrap_or_else(|shared| Inner {
            store: shared.store.clone(),
            registry: shared.registry.clone(),
            compatibility: shared.compatibility.clone(),
            scope_version: shared.scope_version,
            blobs: shared.blobs.clone(),
        });
        Self {
            inner: Arc::new(Inner {
                blobs: Some(blobs),
                ..inner
            }),
        }
    }

    /// The `/sync/v1` routes.
    pub fn router(self) -> Router {
        Router::new()
            .route("/sync/v1/hello", post(hello))
            .route("/sync/v1/push", post(push))
            .route("/sync/v1/pull", post(pull))
            .route("/sync/v1/snapshot", post(snapshot))
            .route("/sync/v1/blobs/{hash}/{size}", get(blob_status))
            .route(
                "/sync/v1/blobs/{hash}/{size}/{offset}",
                axum::routing::put(blob_chunk),
            )
            .route("/sync/v1/blobs/{hash}/content", get(blob_content))
            .route("/sync/v1/events", get(events))
            .layer(DefaultBodyLimit::max(MAX_BODY))
            .with_state(self)
    }
}

/// An authenticated peer.
struct Peer {
    id: NodeId,
    scope: SyncScope,
}

fn problem(status: StatusCode, detail: impl Into<String>) -> Response {
    (
        status,
        axum::Json(json!({ "error": status.canonical_reason(), "detail": detail.into() })),
    )
        .into_response()
}

fn failed(error: impl std::fmt::Display) -> Response {
    tracing::error!(%error, "sync request failed");
    problem(StatusCode::SERVICE_UNAVAILABLE, "the store is unavailable")
}

impl SyncServer {
    /// The node that signed this request (ADR 0062).
    async fn authenticate(
        &self,
        method: &Method,
        uri: &Uri,
        headers: &HeaderMap,
        body: &[u8],
    ) -> Result<Peer, Box<Response>> {
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix(SYNC_SCHEME))
            .map(str::trim)
            .ok_or_else(|| {
                Box::new(problem(
                    StatusCode::UNAUTHORIZED,
                    format!("a {SYNC_SCHEME} signature is required"),
                ))
            })?;
        let node = self
            .inner
            .registry
            .authenticate_sync(token, method.as_str(), uri.path(), body)
            .await
            .map_err(|error| {
                Box::new(match error {
                    RegistryError::Unauthorized(why) | RegistryError::Malformed(why) => {
                        problem(StatusCode::UNAUTHORIZED, why)
                    }
                    other => failed(other),
                })
            })?;
        let id = node
            .id
            .to_string()
            .parse()
            .map_err(|_| Box::new(failed("a registry node id is not a node id")))?;
        Ok(Peer {
            id,
            scope: SyncScope::Facility(node.facility),
        })
    }

    /// The verdict a peer's pushes and pulls are held to.
    async fn verdict(&self, peer: &Peer) -> Result<Verdict, Box<Response>> {
        self.inner
            .store
            .peer_verdict(peer.id)
            .await
            .map_err(|error| Box::new(failed(error)))?
            .ok_or_else(|| {
                Box::new(problem(
                    StatusCode::PRECONDITION_REQUIRED,
                    "send /sync/v1/hello first",
                ))
            })
    }
}

async fn hello(
    State(server): State<SyncServer>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let peer = match server.authenticate(&method, &uri, &headers, &body).await {
        Ok(peer) => peer,
        Err(response) => return *response,
    };
    let Ok(hello) = serde_json::from_slice::<Hello>(&body) else {
        return problem(StatusCode::BAD_REQUEST, "the body is not a Hello");
    };
    if hello.node != peer.id {
        return problem(StatusCode::FORBIDDEN, "the Hello names another node");
    }
    let node = match server.inner.store.node_id().await {
        Ok(node) => node,
        Err(error) => return failed(error),
    };
    let verdict = server.inner.compatibility.judge(&hello);
    if let Err(error) = server
        .inner
        .store
        .record_peer(peer.id, &hello, &verdict)
        .await
    {
        return failed(error);
    }
    if verdict != Verdict::Compatible {
        tracing::warn!(peer = %peer.id, ?verdict, "a sync peer is out of range");
    }
    axum::Json(Welcome {
        node,
        verdict,
        scope_version: server.inner.scope_version,
        snapshot_version: 0,
        expected: None,
    })
    .into_response()
}

fn ndjson(results: &[GroupResult]) -> Response {
    match to_ndjson(results) {
        Ok(body) => (
            [(header::CONTENT_TYPE, "application/x-ndjson")],
            Body::from(body),
        )
            .into_response(),
        Err(error) => failed(error),
    }
}

async fn push(
    State(server): State<SyncServer>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let peer = match server.authenticate(&method, &uri, &headers, &body).await {
        Ok(peer) => peer,
        Err(response) => return *response,
    };
    let verdict = match server.verdict(&peer).await {
        Ok(verdict) => verdict,
        Err(response) => return *response,
    };
    let items: Vec<Item> = match from_ndjson(&body) {
        Ok(items) => items,
        Err(error) => return problem(StatusCode::BAD_REQUEST, error.to_string()),
    };
    let groups = match group(items) {
        Ok(groups) => groups,
        Err(error) => return problem(StatusCode::BAD_REQUEST, error.to_string()),
    };
    match verdict {
        Verdict::Refused { .. } => problem(
            StatusCode::FORBIDDEN,
            "the protocol versions are incompatible",
        ),
        Verdict::Quarantined { .. } => {
            let items: Vec<Item> = groups.iter().flat_map(|g| g.items.clone()).collect();
            if let Err(error) = server.inner.store.quarantine(peer.id, &items).await {
                return failed(error);
            }
            let held = groups
                .iter()
                .map(|g| GroupResult {
                    tx_id: g.tx_id,
                    outcome: Outcome::Rejected {
                        reason: Reason::Quarantined,
                        detail: "held for review until the node upgrades".into(),
                    },
                    replayed: false,
                })
                .collect::<Vec<_>>();
            ndjson(&held)
        }
        Verdict::Compatible => {
            let authority = SurrealAuthority::new(&server.inner.store, peer.id, peer.scope);
            let mut results = Vec::with_capacity(groups.len());
            for group in &groups {
                match receive(&authority, group).await {
                    Ok(result) => results.push(result),
                    // Results so far stand; the node resends the rest.
                    Err(error) => {
                        tracing::warn!(%error, peer = %peer.id, "a pushed group was not applied");
                        break;
                    }
                }
            }
            ndjson(&results)
        }
    }
}

async fn pull(
    State(server): State<SyncServer>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let peer = match server.authenticate(&method, &uri, &headers, &body).await {
        Ok(peer) => peer,
        Err(response) => return *response,
    };
    match server.verdict(&peer).await {
        Ok(Verdict::Compatible) => {}
        Ok(_) => {
            return problem(
                StatusCode::FORBIDDEN,
                "an out-of-range node is sent no data until it upgrades",
            );
        }
        Err(response) => return *response,
    }
    let Ok(request) = serde_json::from_slice::<PullRequest>(&body) else {
        return problem(StatusCode::BAD_REQUEST, "the body is not a PullRequest");
    };
    let node = match server.inner.store.node_id().await {
        Ok(node) => node,
        Err(error) => return failed(error),
    };
    match server
        .inner
        .store
        .pull(&peer.scope, node, peer.id, &request)
        .await
    {
        Ok(response) => axum::Json(response).into_response(),
        Err(error) => failed(error),
    }
}

/// A re-seed: every head in the peer's scope with central's cursor at that instant, as JSON
/// `{"cursor": …, "items": […]}` (synchronization.md § Seeding).
async fn snapshot(
    State(server): State<SyncServer>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let peer = match server.authenticate(&method, &uri, &headers, &body).await {
        Ok(peer) => peer,
        Err(response) => return *response,
    };
    match server.verdict(&peer).await {
        Ok(Verdict::Compatible) => {}
        Ok(_) => {
            return problem(
                StatusCode::FORBIDDEN,
                "an out-of-range node is sent no data until it upgrades",
            );
        }
        Err(response) => return *response,
    }
    let node = match server.inner.store.node_id().await {
        Ok(node) => node,
        Err(error) => return failed(error),
    };
    match server.inner.store.snapshot(&peer.scope, node).await {
        Ok((items, cursor)) => {
            axum::Json(json!({ "cursor": cursor, "items": items })).into_response()
        }
        Err(error) => failed(error),
    }
}

/// The blob store, and the blob a path names.
fn blob_target(
    server: &SyncServer,
    hash: &str,
    size: u64,
) -> Result<(health_files::FsBlobStore, health_files::BlobRef), Box<Response>> {
    let store = server.inner.blobs.clone().ok_or_else(|| {
        Box::new(problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "this node keeps no attachments",
        ))
    })?;
    let hash = hash
        .parse()
        .map_err(|_| Box::new(problem(StatusCode::BAD_REQUEST, "not a sha-256 hash")))?;
    if size > health_sync_protocol::MAX_BLOB_SIZE {
        return Err(Box::new(problem(
            StatusCode::PAYLOAD_TOO_LARGE,
            "an attachment is at most 50 MiB",
        )));
    }
    Ok((store, health_files::BlobRef { hash, size }))
}

fn blob_answer(result: Result<u64, health_files::BlobError>, size: u64) -> Response {
    match result {
        Ok(received) => {
            axum::Json(health_sync_protocol::BlobStatus { received, size }).into_response()
        }
        Err(health_files::BlobError::OutOfOrder { received, .. }) => (
            StatusCode::CONFLICT,
            axum::Json(health_sync_protocol::BlobStatus { received, size }),
        )
            .into_response(),
        Err(
            error @ (health_files::BlobError::Mismatch { .. }
            | health_files::BlobError::TooLarge { .. }),
        ) => problem(StatusCode::UNPROCESSABLE_ENTITY, error.to_string()),
        Err(error) => failed(error),
    }
}

/// How much of a blob this node holds: where an upload resumes.
async fn blob_status(
    State(server): State<SyncServer>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    Path((hash, size)): Path<(String, u64)>,
) -> Response {
    if let Err(response) = server.authenticate(&method, &uri, &headers, &[]).await {
        return *response;
    }
    let (store, blob) = match blob_target(&server, &hash, size) {
        Ok(target) => target,
        Err(response) => return *response,
    };
    let result = tokio::task::spawn_blocking(move || store.received(&blob))
        .await
        .unwrap_or_else(|e| Err(health_files::BlobError::Io(std::io::Error::other(e))));
    blob_answer(result, size)
}

/// One chunk of an upload, at the offset this node reported; the completing chunk verifies and
/// stores the blob. An out-of-order chunk is answered 409 with where to resume.
async fn blob_chunk(
    State(server): State<SyncServer>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    Path((hash, size, offset)): Path<(String, u64, u64)>,
    body: Bytes,
) -> Response {
    if let Err(response) = server.authenticate(&method, &uri, &headers, &body).await {
        return *response;
    }
    let (store, blob) = match blob_target(&server, &hash, size) {
        Ok(target) => target,
        Err(response) => return *response,
    };
    let result = tokio::task::spawn_blocking(move || {
        store.write_chunk(&blob, offset, &body)?;
        store.received(&blob)
    })
    .await
    .unwrap_or_else(|e| Err(health_files::BlobError::Io(std::io::Error::other(e))));
    blob_answer(result, size)
}

/// A whole blob, verified on read: the on-demand download.
async fn blob_content(
    State(server): State<SyncServer>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    Path(hash): Path<String>,
) -> Response {
    if let Err(response) = server.authenticate(&method, &uri, &headers, &[]).await {
        return *response;
    }
    let (store, blob) = match blob_target(&server, &hash, 0) {
        Ok(target) => target,
        Err(response) => return *response,
    };
    match tokio::task::spawn_blocking(move || store.read(&blob.hash)).await {
        Ok(Ok(bytes)) => (
            [(header::CONTENT_TYPE, "application/octet-stream")],
            Body::from(bytes),
        )
            .into_response(),
        Ok(Err(health_files::BlobError::NotFound(_))) => {
            problem(StatusCode::NOT_FOUND, "no such attachment here")
        }
        Ok(Err(error)) => failed(error),
        Err(error) => failed(error),
    }
}

async fn events(
    State(server): State<SyncServer>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = server.authenticate(&method, &uri, &headers, &[]).await {
        return *response;
    }
    let store = server.inner.store.clone();
    let start = store.cursor().await.map(|c| c.seq).unwrap_or(0);
    let stream = futures_util::stream::unfold(
        (store, start, Duration::ZERO),
        |(store, seen, quiet)| async move {
            let mut quiet = quiet;
            loop {
                tokio::time::sleep(HINT_POLL).await;
                quiet += HINT_POLL;
                let seq = store.cursor().await.map(|c| c.seq).unwrap_or(seen);
                let chunk = if seq > seen {
                    format!("event: cursor\ndata: {{\"seq\":{seq}}}\n\n")
                } else if quiet >= KEEP_ALIVE {
                    ": keep-alive\n\n".to_owned()
                } else {
                    continue;
                };
                return Some((
                    Ok::<Bytes, Infallible>(Bytes::from(chunk)),
                    (store, seq.max(seen), Duration::ZERO),
                ));
            }
        },
    );
    let mut response = Response::new(Body::from_stream(stream));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
