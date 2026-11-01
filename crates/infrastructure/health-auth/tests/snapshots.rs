//! Permission snapshots at an edge (ADR 0011): no login without a valid, central-signed snapshot
//! for this facility, at a clock that has not gone back.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use health_auth::jose::b64;
use health_auth::{
    AuthConfig, AuthServer, Client, Credentials, MemoryStore, SnapshotGate, SnapshotSigner,
    SnapshotVerifier, TokenSigner,
};
use health_identity::snapshot::{
    PermissionSnapshot, RoleAssignment, RolePermissions, SnapshotPolicy,
};
use health_identity::{AccountId, AccountStatus, Lockout, LoginAccount};
use health_time::{Clock, FixedClock, SignedDuration, Timestamp};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

const ISSUER: &str = "http://edge.test/auth";
const CLIENT: &str = "health-client";
const REDIRECT: &str = "http://edge.test/callback";
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const FACILITY: &str = "KAM013";

struct Edge {
    app: Router,
    server: AuthServer<MemoryStore>,
    clock: Arc<FixedClock>,
    account: AccountId,
    central: SnapshotSigner,
    issued: Timestamp,
}

fn roles() -> RolePermissions {
    RolePermissions(BTreeMap::from([(
        "clinician".into(),
        BTreeSet::from(["record.read".into()]),
    )]))
}

fn edge() -> Edge {
    let issued = Timestamp::from_second(1_800_000_000).expect("time");
    let clock = Arc::new(FixedClock::new(
        issued
            .checked_add(SignedDuration::from_hours(1))
            .expect("time"),
    ));
    let credentials = Credentials::new(Vec::new()).expect("credentials");
    let store = MemoryStore::default();
    let account = AccountId(Uuid::now_v7());
    store.put_account(LoginAccount {
        id: account,
        username: "okello".into(),
        credential: credentials.hash("correct horse battery").expect("hash"),
        status: AccountStatus::Active,
        lockout: Lockout::default(),
    });
    let central = SnapshotSigner::generate();
    let mut config = AuthConfig::new(ISSUER, "offline-local", false);
    config.clients.push(Client {
        id: CLIENT.into(),
        redirect_uris: vec![REDIRECT.into()],
        patient: false,
    });
    config.snapshots = Some(SnapshotGate {
        facility: Some(FACILITY.into()),
        verifier: SnapshotVerifier::new(&[central.public()]).expect("verifier"),
        policy: SnapshotPolicy::default(),
    });
    let server = AuthServer::new(
        config,
        TokenSigner::generate(),
        credentials,
        store,
        clock.clone(),
    );
    Edge {
        app: Router::new().nest("/auth", server.router()),
        server,
        clock,
        account,
        central,
        issued,
    }
}

impl Edge {
    fn snapshot(&self, facility: &str, version: u64) -> PermissionSnapshot {
        PermissionSnapshot {
            account: self.account,
            facility: facility.into(),
            roles: vec![RoleAssignment {
                role: "clinician".into(),
                locations: vec![],
            }],
            programs: vec![],
            issued_at: self.issued,
            not_after: SnapshotPolicy::default().not_after(self.issued),
            snapshot_version: version,
        }
    }

    fn provision(&self, snapshot: &PermissionSnapshot) {
        let jws = self.central.sign(snapshot, &roles()).expect("sign");
        self.server
            .store()
            .put_snapshot(self.account, FACILITY, jws);
    }

    async fn send(&self, request: Request<Body>) -> (StatusCode, String, Option<String>) {
        let response = self.app.clone().oneshot(request).await.expect("response");
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        (
            status,
            String::from_utf8_lossy(&body).into_owned(),
            location,
        )
    }

    async fn post(
        &self,
        path: &str,
        fields: &[(&str, &str)],
    ) -> (StatusCode, String, Option<String>) {
        let body = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(fields)
            .finish();
        self.send(
            Request::post(path)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
    }

    /// Signs in; `Some(code)` on success.
    async fn login(&self) -> Option<String> {
        let (status, _, location) = self
            .post(
                "/auth/authorize",
                &[
                    ("response_type", "code"),
                    ("client_id", CLIENT),
                    ("redirect_uri", REDIRECT),
                    ("scope", "user/Patient.rs"),
                    (
                        "code_challenge",
                        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
                    ),
                    ("code_challenge_method", "S256"),
                    ("username", "okello"),
                    ("password", "correct horse battery"),
                ],
            )
            .await;
        if status != StatusCode::SEE_OTHER {
            assert_eq!(status, StatusCode::UNAUTHORIZED);
            return None;
        }
        let location = location.expect("location");
        form_urlencoded::parse(location.split_once('?').expect("query").1.as_bytes())
            .find(|(k, _)| k == "code")
            .map(|(_, v)| v.into_owned())
    }

    async fn token(&self, fields: &[(&str, &str)]) -> (StatusCode, Value) {
        let (status, body, _) = self.post("/auth/token", fields).await;
        (status, serde_json::from_str(&body).expect("json"))
    }

    async fn redeem(&self, code: &str) -> (StatusCode, Value) {
        self.token(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT),
            ("client_id", CLIENT),
            ("code_verifier", VERIFIER),
        ])
        .await
    }

    fn advance(&self, by: SignedDuration) {
        self.clock.advance(by);
    }
}

fn claims(token: &str) -> Value {
    use base64::Engine;
    let part = token.split('.').nth(1).expect("claims");
    serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(part)
            .expect("base64"),
    )
    .expect("json")
}

#[tokio::test]
async fn a_valid_snapshot_lets_a_user_in_and_its_claims_reach_the_token() {
    let edge = edge();
    assert_eq!(edge.login().await, None, "no snapshot, no login");
    edge.provision(&edge.snapshot(FACILITY, 4));
    let (status, tokens) = edge.redeem(&edge.login().await.expect("login")).await;
    assert_eq!(status, StatusCode::OK, "{tokens}");
    let access = claims(tokens["access_token"].as_str().expect("access"));
    assert_eq!(access["facility"], FACILITY);
    assert_eq!(access["snapshot_version"], 4);
    assert_eq!(access["acr"], "offline-local");
    assert_eq!(
        tokens["snapshot_not_after"],
        SnapshotPolicy::default().not_after(edge.issued).as_second()
    );
}

#[tokio::test]
async fn an_expired_snapshot_blocks_login_and_ends_refresh() {
    let edge = edge();
    // Central issued it nearly 14 days ago: two hours remain, well inside one session.
    let mut snapshot = edge.snapshot(FACILITY, 1);
    snapshot.issued_at = edge
        .clock
        .now()
        .checked_sub(SignedDuration::from_hours(14 * 24 - 2))
        .expect("time");
    snapshot.not_after = SnapshotPolicy::default().not_after(snapshot.issued_at);
    edge.provision(&snapshot);
    let (_, tokens) = edge.redeem(&edge.login().await.expect("login")).await;
    let refresh = tokens["refresh_token"]
        .as_str()
        .expect("refresh")
        .to_owned();

    // The last access token before expiry stops at `not_after`, not 15 minutes later.
    let five_before = SignedDuration::from_secs(
        snapshot.not_after.as_second() - edge.clock.now().as_second() - 300,
    );
    edge.advance(five_before);
    let (status, rotated) = edge
        .token(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &refresh),
            ("client_id", CLIENT),
        ])
        .await;
    assert_eq!(status, StatusCode::OK, "{rotated}");
    assert_eq!(rotated["expires_in"], 300);
    let refresh = rotated["refresh_token"]
        .as_str()
        .expect("refresh")
        .to_owned();

    edge.advance(SignedDuration::from_secs(300));
    assert_eq!(edge.login().await, None, "an expired snapshot blocks login");
    let (status, _) = edge
        .token(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &refresh),
            ("client_id", CLIENT),
        ])
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "and ends the session");

    // A renewed snapshot from central lets the user back in.
    let mut renewed = edge.snapshot(FACILITY, 2);
    renewed.issued_at = edge.clock.now();
    renewed.not_after = SnapshotPolicy::default().not_after(renewed.issued_at);
    edge.provision(&renewed);
    assert!(edge.login().await.is_some());
}

#[tokio::test]
async fn a_tampered_snapshot_is_detected() {
    let edge = edge();
    let jws = edge
        .central
        .sign(&edge.snapshot(FACILITY, 1), &roles())
        .expect("sign");
    // The payload is changed to add a program assignment; the signature is central's original.
    let mut forged = edge.snapshot(FACILITY, 1);
    forged.programs.push("hiv".into());
    let mut parts: Vec<String> = jws.split('.').map(str::to_owned).collect();
    parts[1] = b64(serde_json::to_string(&forged).expect("json").as_bytes());
    edge.server
        .store()
        .put_snapshot(edge.account, FACILITY, parts.join("."));
    assert_eq!(edge.login().await, None);

    // Signed by anyone but central: refused too.
    let impostor = SnapshotSigner::generate()
        .sign(&edge.snapshot(FACILITY, 1), &roles())
        .expect("sign");
    edge.server
        .store()
        .put_snapshot(edge.account, FACILITY, impostor);
    assert_eq!(edge.login().await, None);
}

#[tokio::test]
async fn a_snapshot_for_another_facility_is_refused() {
    let edge = edge();
    let jws = edge
        .central
        .sign(&edge.snapshot("OTHER1", 1), &roles())
        .expect("sign");
    edge.server
        .store()
        .put_snapshot(edge.account, FACILITY, jws);
    assert_eq!(edge.login().await, None);
}

#[tokio::test]
async fn setting_the_clock_back_does_not_revive_an_expired_snapshot() {
    let edge = edge();
    edge.provision(&edge.snapshot(FACILITY, 1));
    assert!(edge.login().await.is_some());
    // A later snapshot advanced the floor (central time seen in a sync).
    let mut later = edge.snapshot(FACILITY, 2);
    later.issued_at = edge
        .issued
        .checked_add(SignedDuration::from_hours(48))
        .expect("time");
    later.not_after = SnapshotPolicy::default().not_after(later.issued_at);
    edge.advance(SignedDuration::from_hours(48));
    edge.provision(&later);
    assert!(edge.login().await.is_some());
    // Someone winds the clock back a day: the edge refuses logins rather than trust it.
    edge.advance(SignedDuration::from_hours(-24));
    edge.provision(&edge.snapshot(FACILITY, 3));
    assert_eq!(edge.login().await, None);
}
