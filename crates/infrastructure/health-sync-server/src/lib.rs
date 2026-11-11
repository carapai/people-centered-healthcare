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
//!   verifies the blob), and `GET /sync/v1/blobs/{hash}/content` serves it on demand, to a node
//!   with a record in its scope that references it
//!   (synchronization.md § Attachments).
//! - `POST /sync/v1/break-glass` takes the facility's break-glass grants, kept with the privacy
//!   officer's review due 72 hours after they arrive (#224).
//! - `POST /sync/v1/transfers` takes the facility's inter-facility transfers, matched once
//!   received; `POST /sync/v1/transfers/inbound` answers those in transit to it (#247).
//! - `POST /sync/v1/activation-reports` takes the node's package activation reports, `[{"id",
//!   "report"}]`, kept per node; a failed activation raises `package-activation-failed` (#89).
//! - `GET /sync/v1/events` is a server-sent event stream of "cursor advanced" hints. The pull is
//!   the truth; the hint only saves the peer a poll.
//!
//! A server built [`with_check`](SyncServer::with_check) holds every pushed group to its
//! [`GroupCheck`] before applying it: central re-derives a completed response's extraction there
//! (ADR 0022).
//!
//! Bodies are NDJSON or JSON, zstd-compressed when the peer says so: a request with
//! `Content-Encoding: zstd` is decompressed before its signature is checked (the signature
//! covers the content), and an answer is compressed for a peer that sends `Accept-Encoding: zstd`.
//! The event stream and attachment bytes are sent as they are.

mod review;
mod seed;

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use health_identifiers::NodeId;
use health_provisioning::{Error as RegistryError, NodeRegistry, SYNC_SCHEME};
use health_surrealdb::{SnapshotPart, SurrealAuthority, SurrealStore, SyncScope};
use health_sync_protocol::{
    Compatibility, DecodeError, GroupResult, Hello, Item, Outcome, PullRequest, Reason,
    StreamEncoder, Verdict, Welcome, ZSTD, compress, decompress, from_ndjson, group, receive,
    to_ndjson,
};
use serde_json::json;

pub use review::{CONFLICT_PERMISSION, review_router};
pub use seed::write_seed;

/// A check a pushed group must pass before it is applied, e.g. re-deriving a completed
/// response's extraction at the authority (ADR 0022). `Some` is the outcome the group gets
/// instead of being applied.
pub trait GroupCheck: Send + Sync {
    /// The outcome for `group`, if the check refuses it.
    fn check<'a>(
        &'a self,
        group: &'a health_sync_protocol::Group,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<Outcome>> + Send + 'a>>;
}

/// The largest request body on the wire: a push batch is at most 4 MiB (synchronization.md
/// § Push), with room for the envelope. A compressed one may expand to
/// [`MAX_DECODED`](health_sync_protocol::MAX_DECODED).
const MAX_BODY: usize = 8 * 1024 * 1024;

/// How often the event stream checks the log, and how often a quiet stream sends a comment.
const HINT_POLL: Duration = Duration::from_secs(2);
const KEEP_ALIVE: Duration = Duration::from_secs(25);

/// What the sync endpoints serve from.
#[derive(Clone)]
pub struct SyncServer {
    inner: Arc<Inner>,
    blobs: Option<health_files::FsBlobStore>,
    events: Option<health_http::Events>,
    check: Option<Arc<dyn GroupCheck>>,
}

struct Inner {
    store: SurrealStore,
    registry: NodeRegistry,
    compatibility: Compatibility,
    scope_version: u64,
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
            }),
            blobs: None,
            events: None,
            check: None,
        }
    }

    /// Holds every pushed group to `check` before it is applied.
    #[must_use]
    pub fn with_check(mut self, check: Arc<dyn GroupCheck>) -> Self {
        self.check = Some(check);
        self
    }

    /// Accepts and serves attachments from `blobs` (synchronization.md § Attachments).
    #[must_use]
    pub fn with_blobs(mut self, blobs: health_files::FsBlobStore) -> Self {
        self.blobs = Some(blobs);
        self
    }

    /// Tells `events`' subscribers when grants arrive for review (#224).
    #[must_use]
    pub fn with_events(mut self, events: health_http::Events) -> Self {
        self.events = Some(events);
        self
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
            .route("/sync/v1/break-glass", post(break_glass))
            .route("/sync/v1/transfers", post(transfers))
            .route("/sync/v1/transfers/inbound", post(inbound_transfers))
            .route("/sync/v1/activation-reports", post(activation_reports))
            .route("/sync/v1/events", get(events))
            .layer(DefaultBodyLimit::max(health_sync_protocol::MAX_DECODED))
            .layer(axum::middleware::from_fn(zstd_bodies))
            .with_state(self)
    }
}

/// Whether the peer takes zstd answers.
fn accepts_zstd(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::ACCEPT_ENCODING)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|token| {
            let mut parts = token.split(';').map(str::trim);
            parts.next() == Some(ZSTD) && parts.all(|q| q != "q=0" && q != "q=0.0")
        })
}

/// zstd on the wire (synchronization.md § Wire format): decompresses a compressed request, at
/// most [`MAX_BODY`] on the wire and [`MAX_DECODED`](health_sync_protocol::MAX_DECODED) once
/// expanded, and compresses the answer for a peer that takes it. Streams and attachment bytes
/// pass as they are.
async fn zstd_bodies(request: Request, next: Next) -> Response {
    let (mut parts, body) = request.into_parts();
    let compress_answer = accepts_zstd(&parts.headers);
    let request = match parts.headers.get(header::CONTENT_ENCODING) {
        // The wire limit holds whatever the encoding.
        None => Request::from_parts(
            parts,
            Body::new(http_body_util::Limited::new(body, MAX_BODY)),
        ),
        Some(encoding) if encoding.as_bytes() == ZSTD.as_bytes() => {
            let Ok(wire) = axum::body::to_bytes(body, MAX_BODY).await else {
                return problem(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    format!("a compressed body is at most {MAX_BODY} bytes"),
                );
            };
            let content = match decompress(&wire) {
                Ok(content) => content,
                Err(DecodeError::TooLarge) => {
                    return problem(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        DecodeError::TooLarge.to_string(),
                    );
                }
                Err(error) => return problem(StatusCode::BAD_REQUEST, error.to_string()),
            };
            parts.headers.remove(header::CONTENT_ENCODING);
            parts.headers.remove(header::CONTENT_LENGTH);
            Request::from_parts(parts, Body::from(content))
        }
        Some(_) => {
            return problem(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "sync bodies are sent as they are or zstd-compressed",
            );
        }
    };
    let response = next.run(request).await;
    let streamed = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|t| t.starts_with("text/event-stream") || t == "application/octet-stream");
    if !compress_answer || streamed || response.headers().contains_key(header::CONTENT_ENCODING) {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let content = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(content) => content,
        Err(error) => return failed(error),
    };
    parts
        .headers
        .insert(header::CONTENT_ENCODING, HeaderValue::from_static(ZSTD));
    parts.headers.remove(header::CONTENT_LENGTH);
    parts
        .headers
        .append(header::VARY, HeaderValue::from_static("accept-encoding"));
    Response::from_parts(parts, Body::from(compress(&content)))
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
                if let Some(check) = &server.check
                    && let Some(outcome) = check.check(group).await
                {
                    tracing::warn!(peer = %peer.id, ?outcome, "a pushed group failed its check");
                    results.push(GroupResult {
                        tx_id: group.tx_id,
                        outcome,
                        replayed: false,
                    });
                    continue;
                }
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
    // Streamed a page at a time (#83): central never holds every head in the peer's scope, nor
    // the whole answer. The cursor comes first, so a failure before it is still a clean answer.
    let mut parts = server.inner.store.snapshot_stream(peer.scope.clone(), node);
    let cursor = match parts.recv().await {
        Some(Ok(SnapshotPart::Cursor(cursor))) => cursor,
        Some(Err(error)) => return failed(error),
        _ => return failed("the snapshot ended before its cursor"),
    };
    let Ok(cursor) = serde_json::to_string(&cursor) else {
        return failed("the cursor does not serialize");
    };
    let zstd = accepts_zstd(&headers);
    let stream = SnapshotBody {
        parts,
        encoder: zstd.then(StreamEncoder::new),
        head: Some(format!(r#"{{"cursor":{cursor},"items":["#)),
        first: true,
        done: false,
    }
    .into_stream();
    let mut response = Response::new(Body::from_stream(stream));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    if zstd {
        headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static(ZSTD));
        headers.append(header::VARY, HeaderValue::from_static("accept-encoding"));
    }
    response
}

/// A snapshot answer as it is written: `{"cursor":…,"items":[…]}`, one page of items at a time,
/// compressed as it goes for a peer that takes zstd. A store failure part way ends the body with
/// an error, so the peer sees a broken answer, never a short one that parses.
struct SnapshotBody {
    parts: tokio::sync::mpsc::Receiver<Result<SnapshotPart, health_persistence::StoreError>>,
    encoder: Option<StreamEncoder>,
    head: Option<String>,
    first: bool,
    done: bool,
}

impl SnapshotBody {
    fn into_stream(self) -> impl futures_util::Stream<Item = Result<Bytes, std::io::Error>> {
        futures_util::stream::unfold(self, |mut body| async move {
            if body.done {
                return None;
            }
            if let Some(head) = body.head.take() {
                let bytes = body.encode(head.as_bytes());
                return Some((Ok(Bytes::from(bytes)), body));
            }
            loop {
                match body.parts.recv().await {
                    Some(Ok(SnapshotPart::Items(page))) => {
                        let mut raw = Vec::new();
                        for item in &page {
                            if !body.first {
                                raw.push(b',');
                            }
                            body.first = false;
                            if serde_json::to_writer(&mut raw, item).is_err() {
                                body.done = true;
                                return Some((Err(std::io::Error::other("an item")), body));
                            }
                        }
                        let bytes = body.encode(&raw);
                        return Some((Ok(Bytes::from(bytes)), body));
                    }
                    Some(Ok(SnapshotPart::Cursor(_))) => {}
                    Some(Err(error)) => {
                        tracing::error!(%error, "snapshot failed part way");
                        body.done = true;
                        return Some((Err(std::io::Error::other(error.to_string())), body));
                    }
                    None => {
                        body.done = true;
                        let mut bytes = body.encode(b"]}");
                        if let Some(encoder) = body.encoder.take() {
                            bytes.extend(encoder.finish());
                        }
                        return Some((Ok(Bytes::from(bytes)), body));
                    }
                }
            }
        })
    }

    fn encode(&mut self, raw: &[u8]) -> Vec<u8> {
        match &mut self.encoder {
            Some(encoder) => encoder.push(raw),
            None => raw.to_vec(),
        }
    }
}

/// The facility's break-glass grants (#224): kept with the privacy officer's review due 72
/// hours after they arrive here, however long ago they were made; an acknowledgement is taken.
/// A grant of another facility is refused.
async fn break_glass(
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
    let Ok(grants) = serde_json::from_slice::<Vec<health_sync_protocol::BreakGlassGrant>>(&body)
    else {
        return problem(StatusCode::BAD_REQUEST, "the body is not a list of grants");
    };
    let facility = match &peer.scope {
        SyncScope::Facility(facility) => Some(facility.as_str()),
        SyncScope::All => None,
    };
    if let Some(stranger) = grants
        .iter()
        .find(|g| facility.is_some_and(|f| f != g.facility))
    {
        return problem(
            StatusCode::FORBIDDEN,
            format!("grant {} is another facility's", stranger.id),
        );
    }
    let arrived = health_time::Timestamp::now();
    for grant in &grants {
        if let Err(error) = server.inner.store.receive_grant(grant, arrived).await {
            return failed(error);
        }
    }
    if !grants.is_empty()
        && let Some(events) = &server.events
    {
        events.publish(health_http::Hint::worklist(
            health_auth::REVIEWS_WORKLIST,
            health_http::Audience {
                facility: None,
                permission: Some(health_auth::REVIEW_PERMISSION.into()),
            },
        ));
    }
    axum::Json(json!({ "received": grants.iter().map(|g| &g.id).collect::<Vec<_>>() }))
        .into_response()
}

/// A node's package activation reports (#89): kept per node, once each; a failed activation is
/// raised at central, where the rollout is run.
async fn activation_reports(
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
    let Ok(reports) = serde_json::from_slice::<Vec<serde_json::Value>>(&body) else {
        return problem(StatusCode::BAD_REQUEST, "the body is not a list of reports");
    };
    let Some(reports) = reports
        .iter()
        .map(|sent| Some((sent["id"].as_str()?, &sent["report"])))
        .collect::<Option<Vec<_>>>()
    else {
        return problem(StatusCode::BAD_REQUEST, "a report has no id");
    };
    let mut tx = match health_persistence::Store::begin(&server.inner.store).await {
        Ok(tx) => tx,
        Err(error) => return failed(error),
    };
    for (id, report) in &reports {
        let (Some(at), Some(outcome)) = (report["at"].as_str(), report["outcome"].as_str()) else {
            return problem(
                StatusCode::BAD_REQUEST,
                format!("report {id} has no time or outcome"),
            );
        };
        if let Err(error) = tx
            .receive_activation_report(&peer.id.to_string(), id, at, outcome, &report.to_string())
            .await
        {
            return match error {
                health_persistence::WriteError::Store(error) => failed(error),
                _ => problem(
                    StatusCode::BAD_REQUEST,
                    format!("report {id} is not a report"),
                ),
            };
        }
        if outcome == "failed" {
            tracing::warn!(
                alert = "package-activation-failed",
                node = %peer.id,
                problems = %report["problems"],
                "a node could not activate its packages and kept its previous set"
            );
        }
    }
    if let Err(error) = health_persistence::Transaction::commit(tx).await {
        return failed(error);
    }
    axum::Json(json!({ "received": reports.iter().map(|(id, _)| id).collect::<Vec<_>>() }))
        .into_response()
}

/// The facility's inter-facility transfers (#247): kept, with the source's lines authoritative,
/// and once received matched against the dispatch. A transfer naming neither end as this
/// facility is refused.
async fn transfers(
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
    let SyncScope::Facility(facility) = &peer.scope else {
        return problem(StatusCode::FORBIDDEN, "transfers come from a facility");
    };
    let Ok(transfers) = serde_json::from_slice::<Vec<health_inventory::transfers::Transfer>>(&body)
    else {
        return problem(
            StatusCode::BAD_REQUEST,
            "the body is not a list of transfers",
        );
    };
    let mut answers = serde_json::Map::new();
    for transfer in transfers {
        let id = transfer.id.clone();
        match server
            .inner
            .store
            .receive_transfer(facility, transfer)
            .await
        {
            Ok(outcome) => {
                answers.insert(id, json!(format!("{outcome:?}")));
            }
            Err(health_surrealdb::TransferRefused::Store(error)) => return failed(error),
            Err(refused) => return problem(StatusCode::FORBIDDEN, refused.to_string()),
        }
    }
    axum::Json(json!({ "received": answers })).into_response()
}

/// The transfers in transit addressed to the facility, for its edge to receive (#247).
async fn inbound_transfers(
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
    let SyncScope::Facility(facility) = &peer.scope else {
        return axum::Json(json!([])).into_response();
    };
    match server.inner.store.inbound_transfers(facility).await {
        Ok(transfers) => axum::Json(transfers).into_response(),
        Err(error) => failed(error),
    }
}

/// The blob store, and the blob a path names.
fn blob_target(
    server: &SyncServer,
    hash: &str,
    size: u64,
) -> Result<(health_files::FsBlobStore, health_files::BlobRef), Box<Response>> {
    let store = server.blobs.clone().ok_or_else(|| {
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
    let peer = match server.authenticate(&method, &uri, &headers, &[]).await {
        Ok(peer) => peer,
        Err(response) => return *response,
    };
    let (store, blob) = match blob_target(&server, &hash, 0) {
        Ok(target) => target,
        Err(response) => return *response,
    };
    // Only a blob that a record in the node's scope references (#84); any other answers as an
    // unknown one does.
    match server
        .inner
        .store
        .blob_admitted(&peer.scope, &blob.hash.to_string())
        .await
    {
        Ok(true) => {}
        Ok(false) => return problem(StatusCode::NOT_FOUND, "no such attachment here"),
        Err(error) => return failed(error),
    }
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
