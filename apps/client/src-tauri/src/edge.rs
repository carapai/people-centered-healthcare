//! Under an edge (#133; desktop-client.md § Online and offline under an edge): commands pass
//! through to the edge, the authority, while it can be reached. After 30 s without it the
//! desktop works offline, answering locally, and once the edge is back it drains its outbox
//! before passing through again ([`health_desktop_runtime::link`]). The UI never sees the switch.
//!
//! Each request to the edge carries a DPoP proof by the desktop's enrolled device key, and the
//! signed-in user's edge token, which is bound to that key and never leaves Rust.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use health_authority_leases::{Lease, LeaseState};
use health_desktop_runtime::ipc::{Envelope, Reply};
use health_desktop_runtime::keystore::Keystore;
use health_desktop_runtime::link::{Link, Mode, Route};
use health_surrealdb::{LeaseRole, SurrealStore};
use health_sync_client::Sync;
use health_sync_client::edge::{Answer, Edge, Request, Unreachable};

use crate::auth::{self, OAuthClient, SignInError, Transport};
use crate::enroll;

/// The edge's client registration for staff sign-in (`health_edge_runtime`'s).
const EDGE_CLIENT_ID: &str = "health-edge";
/// Its redirect path, under the edge's origin.
const EDGE_REDIRECT_PATH: &str = "/callback";
/// How long one command may wait for the edge.
const TIMEOUT: Duration = Duration::from_secs(15);
/// How often the edge is tried while it is missed.
const PROBE: Duration = Duration::from_secs(5);
/// How often the edge's staff are taken for sign-in while it cannot be reached.
const ACCOUNTS_EVERY: Duration = Duration::from_secs(3600);

/// What to do with the edge's answer to `POST /api/desktop/accounts`: import it.
pub(crate) type Accounts = std::sync::Arc<
    dyn Fn(serde_json::Value) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + std::marker::Sync,
>;

/// The desktop's link to its edge.
pub(crate) struct EdgeLink {
    edge: Edge,
    /// The device's public key, as a JWK: what the edge encrypts its staff's hashes to.
    jwk: serde_json::Value,
    link: Mutex<Link>,
    /// The signed-in user's token at the edge.
    session: tokio::sync::Mutex<Option<String>>,
}

impl EdgeLink {
    /// The link, once this desktop is enrolled with an edge; `None` before.
    pub(crate) fn open(data_dir: &Path, keys: &dyn Keystore) -> Result<Option<Self>, String> {
        let Some(upstream) = enroll::upstream(data_dir) else {
            return Ok(None);
        };
        let key = enroll::device_key(keys)?
            .ok_or("enrolled, but the device key is not in the keystore")?;
        let jwk = serde_json::to_value(health_auth::jose::EcJwk::of(key.verifying_key()))
            .map_err(|e| e.to_string())?;
        let edge = Edge::new(&upstream.origin, upstream.ca_pem.as_deref(), key, TIMEOUT)?;
        Ok(Some(Self::with_link(edge, jwk, Link::default())))
    }

    fn with_link(edge: Edge, jwk: serde_json::Value, link: Link) -> Self {
        Self {
            edge,
            jwk,
            link: Mutex::new(link),
            session: tokio::sync::Mutex::new(None),
        }
    }

    fn link(&self) -> std::sync::MutexGuard<'_, Link> {
        self.link.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Where the link stands.
    pub(crate) fn mode(&self) -> Mode {
        self.link().mode()
    }

    /// Passes `envelope` through to the edge, as the signed-in user, while the link passes
    /// through. `None` when it is to be answered locally: the desktop is offline or draining, or
    /// the edge has just been missed for long enough.
    pub(crate) async fn forward(&self, envelope: &Envelope) -> Option<Reply> {
        if self.link().route() != Route::Edge {
            return None;
        }
        let token = self.session.lock().await.clone();
        match self.edge.send(request(envelope), token.as_deref()).await {
            Ok(answer) => {
                self.link().reached(Instant::now());
                Some(reply(answer))
            }
            Err(Unreachable(why)) => {
                let mut link = self.link();
                link.unreachable(Instant::now());
                if link.route() == Route::Local {
                    eprintln!("health-desktop: the edge is unreachable ({why}): working offline");
                    None
                } else {
                    // Inside the grace: the client retries. A write that may have reached the
                    // edge is not also made here.
                    Some(unavailable())
                }
            }
        }
    }

    /// Signs `username` in at the edge, while it can be reached. `None` when the desktop is
    /// working offline, for local sign-in instead.
    pub(crate) async fn sign_in(
        &self,
        username: &str,
        password: &str,
    ) -> Option<Result<(), SignInError>> {
        if self.link().route() != Route::Edge {
            return None;
        }
        let redirect = format!("{}{EDGE_REDIRECT_PATH}", self.edge.origin());
        let client = OAuthClient {
            id: EDGE_CLIENT_ID,
            redirect: &redirect,
        };
        Some(
            match auth::sign_in_with(client, self, username, password).await {
                Ok(token) => {
                    *self.session.lock().await = Some(token);
                    Ok(())
                }
                Err(refused) => Err(refused),
            },
        )
    }

    /// Ends the session at the edge here.
    pub(crate) async fn sign_out(&self) {
        *self.session.lock().await = None;
    }

    /// The edge's staff for this desktop's location, for sign-in while the edge cannot be
    /// reached: its answer to `POST /api/desktop/accounts`.
    pub(crate) async fn accounts(&self) -> Result<serde_json::Value, String> {
        let answer = self
            .edge
            .send(
                Request {
                    method: "POST".into(),
                    path: "/api/desktop/accounts".into(),
                    query: None,
                    headers: vec![("content-type".into(), "application/json".into())],
                    body: serde_json::json!({ "jwk": self.jwk })
                        .to_string()
                        .into_bytes(),
                },
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        if answer.status != 200 {
            return Err(format!(
                "the edge answered {}: {}",
                answer.status,
                String::from_utf8_lossy(&answer.body)
            ));
        }
        serde_json::from_slice(&answer.body).map_err(|e| e.to_string())
    }

    /// Syncs with the edge until the process ends: each round that reaches it tells the link the
    /// edge is there and the outbox has gone up; each that fails, that it was missed. While the
    /// edge is missed it is tried every 5 seconds, otherwise every `interval`. Hourly, after a
    /// round that reached it, the edge's staff are taken and handed to `accounts`. After every
    /// round that reached it, the desktop's Authority Leases are brought up to date in `store`.
    pub(crate) async fn sync(
        &self,
        sync: Sync,
        store: SurrealStore,
        interval: Duration,
        accounts: Accounts,
    ) {
        let mut taken: Option<Instant> = None;
        loop {
            let reached = match sync.round().await {
                Ok(_) => {
                    let mut link = self.link();
                    link.reached(Instant::now());
                    link.drained();
                    true
                }
                Err(why) => {
                    let mut link = self.link();
                    link.unreachable(Instant::now());
                    if link.mode() != Mode::PassThrough {
                        eprintln!("health-desktop: the edge is unreachable: {why}");
                    }
                    false
                }
            };
            if reached && let Err(e) = self.keep_leases(&store).await {
                eprintln!("health-desktop: the leases were not brought up to date: {e}");
            }
            if reached && taken.is_none_or(|at| at.elapsed() >= ACCOUNTS_EVERY) {
                match self.accounts().await {
                    Ok(answer) => {
                        accounts(answer).await;
                        taken = Some(Instant::now());
                    }
                    Err(e) => eprintln!("health-desktop: the edge's staff were not taken: {e}"),
                }
            }
            let wait = if self.mode() == Mode::PassThrough {
                interval
            } else {
                PROBE
            };
            tokio::time::sleep(wait).await;
        }
    }
}

impl EdgeLink {
    /// Sends `method` to the edge's `path` with `body`, as the device; the JSON answer, or why
    /// there is none.
    async fn device_call(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let answer = self
            .edge
            .send(
                Request {
                    method: method.into(),
                    path: path.into(),
                    query: None,
                    headers: vec![("content-type".into(), "application/json".into())],
                    body: body.map(|b| b.to_string().into_bytes()).unwrap_or_default(),
                },
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        if answer.status != 200 {
            return Err(format!(
                "the edge answered {}: {}",
                answer.status,
                String::from_utf8_lossy(&answer.body)
            ));
        }
        serde_json::from_slice(&answer.body).map_err(|e| e.to_string())
    }

    /// Brings the desktop's leases up to date after a round that reached the edge (#135). A
    /// return the holder asked for completes now: the round just drained its outbox, and it has
    /// written nothing in the scope since asking. Then the edge's view of its leases is kept.
    pub(crate) async fn keep_leases(&self, store: &SurrealStore) -> Result<(), String> {
        for id in returning(
            &store
                .leases(LeaseRole::Held)
                .await
                .map_err(|e| e.to_string())?,
        ) {
            let lease: Lease = serde_json::from_value(
                self.device_call(
                    "POST",
                    &format!("/api/device/leases/{id}/return"),
                    Some(serde_json::json!({ "drained": true })),
                )
                .await?,
            )
            .map_err(|e| e.to_string())?;
            store
                .put_lease(LeaseRole::Held, &lease)
                .await
                .map_err(|e| e.to_string())?;
        }
        let held: Vec<Lease> =
            serde_json::from_value(self.device_call("GET", "/api/device/leases", None).await?)
                .map_err(|e| e.to_string())?;
        for lease in &held {
            store
                .put_lease(LeaseRole::Held, lease)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Asks the edge to take lease `id` back: the desktop writes no more in its scope, and the
    /// return completes after the next round that drains its outbox.
    pub(crate) async fn give_back(&self, store: &SurrealStore, id: &str) -> Result<Lease, String> {
        let lease: Lease = serde_json::from_value(
            self.device_call(
                "POST",
                &format!("/api/device/leases/{id}/return"),
                Some(serde_json::json!({ "drained": false })),
            )
            .await?,
        )
        .map_err(|e| e.to_string())?;
        store
            .put_lease(LeaseRole::Held, &lease)
            .await
            .map_err(|e| e.to_string())?;
        Ok(lease)
    }
}

/// The leases whose return the holder asked for and the edge has yet to acknowledge.
fn returning(leases: &[Lease]) -> Vec<String> {
    leases
        .iter()
        .filter(|lease| matches!(lease.state, LeaseState::Returning { .. }))
        .map(|lease| lease.id.to_string())
        .collect()
}

/// The desktop's lease routes, answered here (the webview's `/api/desktop/…`):
/// `GET /api/desktop/leases` lists the leases it holds, and
/// `POST /api/desktop/leases/{id}/return` asks the edge to take one back.
pub(crate) fn lease_routes(link: std::sync::Arc<EdgeLink>, store: SurrealStore) -> axum::Router {
    use axum::extract::{Path, State};
    use axum::http::StatusCode;
    use axum::response::IntoResponse as _;
    use axum::routing::{get, post};

    type Leases = (std::sync::Arc<EdgeLink>, SurrealStore);
    axum::Router::new()
        .route(
            "/api/desktop/leases",
            get(|State((_, store)): State<Leases>| async move {
                match store.leases(LeaseRole::Held).await {
                    Ok(leases) => axum::Json(leases).into_response(),
                    Err(e) => (StatusCode::SERVICE_UNAVAILABLE, e.to_string()).into_response(),
                }
            }),
        )
        .route(
            "/api/desktop/leases/{id}/return",
            post(
                |State((link, store)): State<Leases>, Path(id): Path<String>| async move {
                    match link.give_back(&store, &id).await {
                        Ok(lease) => axum::Json(lease).into_response(),
                        Err(e) => (StatusCode::BAD_GATEWAY, e).into_response(),
                    }
                },
            ),
        )
        .with_state((link, store))
}

/// Sign-in's requests to the edge: proved by the device key, with no user token yet.
impl Transport for EdgeLink {
    async fn send(&self, envelope: Envelope) -> Reply {
        match self.edge.send(request(&envelope), None).await {
            Ok(answer) => reply(answer),
            Err(_) => unavailable(),
        }
    }
}

/// The edge's request for `envelope`. Any credential the webview put in it is dropped: the
/// edge client adds the desktop's own.
fn request(envelope: &Envelope) -> Request {
    let query = (!envelope.query.is_empty()).then(|| {
        form_urlencoded::Serializer::new(String::new())
            .extend_pairs(&envelope.query)
            .finish()
    });
    Request {
        method: envelope.method.clone(),
        path: envelope.path.clone(),
        query,
        headers: envelope
            .headers
            .iter()
            .filter(|(name, _)| {
                !name.eq_ignore_ascii_case("authorization") && !name.eq_ignore_ascii_case("dpop")
            })
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect(),
        body: envelope.body.clone().unwrap_or_default().into_bytes(),
    }
}

fn reply(answer: Answer) -> Reply {
    Reply {
        status: answer.status,
        headers: answer.headers.into_iter().collect(),
        body: String::from_utf8_lossy(&answer.body).into_owned(),
    }
}

/// The edge was missed, inside the grace: 503, for the client to retry.
fn unavailable() -> Reply {
    Reply {
        status: 503,
        headers: [
            (
                "content-type".to_owned(),
                "application/problem+json".to_owned(),
            ),
            ("retry-after".to_owned(), "5".to_owned()),
        ]
        .into(),
        body: serde_json::json!({
            "type": "about:blank",
            "title": "The edge is unreachable",
            "status": 503,
            "detail": "the edge did not answer; try again",
        })
        .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_crosses_as_the_same_request_without_the_webviews_credentials() {
        let envelope = Envelope {
            method: "GET".into(),
            path: "/api/fhir/Patient".into(),
            query: vec![
                ("name".into(), "Auma Grace".into()),
                ("_count".into(), "5".into()),
            ],
            headers: [
                ("accept".to_owned(), "application/fhir+json".to_owned()),
                ("Authorization".to_owned(), "Bearer stolen".to_owned()),
                ("DPoP".to_owned(), "forged".to_owned()),
            ]
            .into(),
            body: None,
        };
        let sent = request(&envelope);
        assert_eq!(sent.path, "/api/fhir/Patient");
        assert_eq!(sent.query.as_deref(), Some("name=Auma+Grace&_count=5"));
        assert_eq!(
            sent.headers,
            vec![("accept".to_owned(), "application/fhir+json".to_owned())]
        );
        assert!(sent.body.is_empty());
    }

    fn unreachable_edge(grace: Duration) -> EdgeLink {
        let key = p256::ecdsa::SigningKey::from_bytes(&[3; 32].into()).expect("key");
        let edge =
            Edge::new("https://localhost:1", None, key, Duration::from_secs(5)).expect("edge");
        EdgeLink::with_link(edge, serde_json::Value::Null, Link::new(grace))
    }

    fn command() -> Envelope {
        Envelope {
            method: "POST".into(),
            path: "/api/reception/check-in".into(),
            query: Vec::new(),
            headers: Default::default(),
            body: Some("{}".into()),
        }
    }

    #[tokio::test]
    async fn a_missed_edge_is_retried_inside_the_grace_and_answered_locally_after_it() {
        let inside = unreachable_edge(Duration::from_secs(30));
        let answer = inside.forward(&command()).await.expect("answered");
        assert_eq!(
            answer.status, 503,
            "the client retries; nothing is written here"
        );
        assert_eq!(inside.mode(), Mode::PassThrough);

        let after = unreachable_edge(Duration::ZERO);
        assert!(
            after.forward(&command()).await.is_none(),
            "answered locally"
        );
        assert_eq!(after.mode(), Mode::Offline);
        // Offline: commands stay local without trying the edge, and so does sign-in.
        assert!(after.forward(&command()).await.is_none());
        assert!(after.sign_in("auma", "secret").await.is_none());
    }

    #[test]
    fn only_leases_being_returned_are_acknowledged_after_a_drained_round() {
        let lease = |state: LeaseState| Lease {
            id: uuid::Uuid::now_v7(),
            holder: "desktop".into(),
            stores: ["Location/outreach-bag".to_owned()].into(),
            granted_by: "admin".into(),
            granted_at: health_time::Timestamp::UNIX_EPOCH,
            expires_at: health_time::Timestamp::UNIX_EPOCH,
            state,
        };
        let asked = lease(LeaseState::Returning {
            requested_at: health_time::Timestamp::UNIX_EPOCH,
        });
        let leases = [
            lease(LeaseState::Active),
            asked.clone(),
            lease(LeaseState::Returned {
                at: health_time::Timestamp::UNIX_EPOCH,
            }),
        ];
        assert_eq!(returning(&leases), vec![asked.id.to_string()]);
    }

    #[test]
    fn inside_the_grace_the_client_is_told_to_retry() {
        let answer = unavailable();
        assert_eq!(answer.status, 503);
        assert_eq!(
            answer.headers.get("retry-after").map(String::as_str),
            Some("5")
        );
    }
}
