//! The authorization server over SurrealDB (memory engine): the end-to-end flow, a restart in
//! the middle of a session, and concurrent redemption of one code.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::routing::get;
use health_auth::jose::{EcJwk, sha256_b64, sign};
use health_auth::{
    AccessVerifier, AuthConfig, AuthServer, Client, Credentials, SurrealAuthStore, TokenSigner,
};
use health_identity::{AccountId, AccountStatus, Lockout, LoginAccount, SessionPolicy};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{Clock, FixedClock, SignedDuration, SystemClock, Timestamp};
use http_body_util::BodyExt;
use p256::ecdsa::SigningKey;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

const ISSUER: &str = "http://central.test/auth";
const RESOURCE: &str = "http://central.test/fhir/Patient";
const CLIENT: &str = "health-client";
const REDIRECT: &str = "http://central.test/callback";
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

async fn protected(
    State(verifier): State<AccessVerifier<SurrealAuthStore>>,
    headers: HeaderMap,
) -> Result<String, health_auth::AccessError> {
    verifier
        .verify(&headers, "GET", RESOURCE)
        .await
        .map(|p| p.account.to_string())
}

struct Node {
    store: SurrealAuthStore,
    signer: TokenSigner,
    clock: Arc<FixedClock>,
}

/// One running server over the node's database: building another one is a restart.
fn serve(node: &Node) -> Router {
    let mut config = AuthConfig::new(ISSUER, "central", false);
    config.clients.push(Client {
        id: CLIENT.into(),
        redirect_uris: vec![REDIRECT.into()],
    });
    let server = AuthServer::new(
        config,
        node.signer.clone(),
        Credentials::new(b"pepper".to_vec()).expect("credentials"),
        node.store.clone(),
        node.clock.clone(),
    );
    Router::new().nest("/auth", server.router()).route(
        "/fhir/Patient",
        get(protected).with_state(server.verifier()),
    )
}

async fn node() -> Node {
    let settings = Settings {
        target: Target::Memory,
        namespace: "auth".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealAuthStore::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        SessionPolicy::default().hard_cap,
    );
    let credentials = Credentials::new(b"pepper".to_vec()).expect("credentials");
    store
        .put_account(&LoginAccount {
            id: AccountId(Uuid::now_v7()),
            username: "nakato".into(),
            credential: credentials.hash("correct horse battery").expect("hash"),
            status: AccountStatus::Active,
            lockout: Lockout::default(),
        })
        .await
        .expect("account");
    let clock = Arc::new(FixedClock::new(
        Timestamp::from_second(1_800_000_000).expect("time"),
    ));
    Node {
        store,
        signer: TokenSigner::generate(),
        clock,
    }
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, HeaderMap, String) {
    let response = app.clone().oneshot(request).await.expect("response");
    let (parts, body) = response.into_parts();
    let bytes = body.collect().await.expect("body").to_bytes();
    (
        parts.status,
        parts.headers,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

fn form(path: &str, fields: &[(&str, &str)], dpop: Option<String>) -> Request<Body> {
    let body = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields)
        .finish();
    let mut request =
        Request::post(path).header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some(proof) = dpop {
        request = request.header("dpop", proof);
    }
    request.body(Body::from(body)).expect("request")
}

fn proof(key: &SigningKey, method: &str, uri: &str, iat: i64, token: Option<&str>) -> String {
    let mut claims =
        json!({ "jti": Uuid::now_v7().to_string(), "htm": method, "htu": uri, "iat": iat });
    if let Some(token) = token {
        claims["ath"] = json!(sha256_b64(token));
    }
    sign(
        key,
        &json!({ "typ": "dpop+jwt", "alg": "ES256", "jwk": EcJwk::of(key.verifying_key()) }),
        &claims,
    )
}

async fn login(app: &Router, password: &str) -> Option<String> {
    let fields = [
        ("response_type", "code"),
        ("client_id", CLIENT),
        ("redirect_uri", REDIRECT),
        ("scope", "openid user/Patient.rs"),
        ("state", "s"),
        (
            "code_challenge",
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        ),
        ("code_challenge_method", "S256"),
        ("username", "nakato"),
        ("password", password),
    ];
    let (status, headers, _) = send(app, form("/auth/authorize", &fields, None)).await;
    if status != StatusCode::SEE_OTHER {
        return None;
    }
    let location = headers[header::LOCATION]
        .to_str()
        .expect("location")
        .to_owned();
    form_urlencoded::parse(location.split_once('?').expect("query").1.as_bytes())
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
}

fn redeem(code: &str, dpop: Option<String>) -> Request<Body> {
    form(
        "/auth/token",
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT),
            ("client_id", CLIENT),
            ("code_verifier", VERIFIER),
        ],
        dpop,
    )
}

async fn access(app: &Router, token: &str, key: &SigningKey, now: i64) -> StatusCode {
    let request = Request::get("/fhir/Patient")
        .header(header::AUTHORIZATION, format!("DPoP {token}"))
        .header("dpop", proof(key, "GET", RESOURCE, now, Some(token)))
        .body(Body::empty())
        .expect("request");
    send(app, request).await.0
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_flow_survives_a_restart_mid_session() {
    let node = node().await;
    let now = || node.clock.now().as_second();
    let key = SigningKey::random(&mut rand_core::OsRng);
    let app = serve(&node);
    let code = login(&app, "correct horse battery").await.expect("login");
    let (status, _, body) = send(
        &app,
        redeem(
            &code,
            Some(proof(&key, "POST", &format!("{ISSUER}/token"), now(), None)),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tokens: Value = serde_json::from_str(&body).expect("json");
    let access_token = tokens["access_token"].as_str().expect("access").to_owned();
    assert_eq!(
        access(&app, &access_token, &key, now()).await,
        StatusCode::OK
    );

    // The process restarts: a new server over the same database (and signing key).
    drop(app);
    let app = serve(&node);
    assert_eq!(
        access(&app, &access_token, &key, now()).await,
        StatusCode::OK,
        "the session outlived the restart"
    );
    let refresh = tokens["refresh_token"]
        .as_str()
        .expect("refresh")
        .to_owned();
    let refresh_request = |token: &str| {
        form(
            "/auth/token",
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", token),
                ("client_id", CLIENT),
            ],
            Some(proof(&key, "POST", &format!("{ISSUER}/token"), now(), None)),
        )
    };
    let (status, _, body) = send(&app, refresh_request(&refresh)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let rotated: Value = serde_json::from_str(&body).expect("json");
    let next_refresh = rotated["refresh_token"]
        .as_str()
        .expect("refresh")
        .to_owned();
    // A replayed (rotated-out) token ends the session, even across restarts.
    assert_eq!(
        send(&app, refresh_request(&refresh)).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(&app, refresh_request(&next_refresh)).await.0,
        StatusCode::BAD_REQUEST
    );

    // Logout through /revoke, on a fresh session.
    let code = login(&app, "correct horse battery").await.expect("login");
    let (_, _, body) = send(
        &app,
        redeem(
            &code,
            Some(proof(&key, "POST", &format!("{ISSUER}/token"), now(), None)),
        ),
    )
    .await;
    let tokens: Value = serde_json::from_str(&body).expect("json");
    let access_token = tokens["access_token"].as_str().expect("access").to_owned();
    send(
        &app,
        form(
            "/auth/revoke",
            &[("token", tokens["refresh_token"].as_str().expect("refresh"))],
            None,
        ),
    )
    .await;
    assert_eq!(
        access(&serve(&node), &access_token, &key, now()).await,
        StatusCode::UNAUTHORIZED
    );

    // Lockout counters persist across restarts too.
    for _ in 0..10 {
        assert!(login(&serve(&node), "wrong").await.is_none());
    }
    assert!(
        login(&serve(&node), "correct horse battery")
            .await
            .is_none(),
        "locked"
    );
    node.clock.advance(SignedDuration::from_mins(15));
    assert!(
        login(&serve(&node), "correct horse battery")
            .await
            .is_some()
    );

    // Expired codes and jtis, and sessions past the cap, are purged.
    node.clock.advance(SignedDuration::from_hours(13));
    assert!(node.store.purge(node.clock.now()).await.expect("purge") > 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_redemption_of_one_code_issues_tokens_once() {
    let node = node().await;
    let app = serve(&node);
    for _ in 0..5 {
        let code = login(&app, "correct horse battery").await.expect("login");
        let attempts: Vec<_> = (0..4)
            .map(|_| {
                let app = app.clone();
                let code = code.clone();
                tokio::spawn(async move { send(&app, redeem(&code, None)).await.0 })
            })
            .collect();
        let mut statuses = Vec::new();
        for attempt in attempts {
            statuses.push(attempt.await.expect("join"));
        }
        let issued = statuses.iter().filter(|s| **s == StatusCode::OK).count();
        assert_eq!(issued, 1, "{statuses:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_refresh_rotates_once() {
    let node = node().await;
    let app = serve(&node);
    let code = login(&app, "correct horse battery").await.expect("login");
    let (_, _, body) = send(&app, redeem(&code, None)).await;
    let tokens: Value = serde_json::from_str(&body).expect("json");
    let refresh = tokens["refresh_token"]
        .as_str()
        .expect("refresh")
        .to_owned();
    let attempts: Vec<_> = (0..4)
        .map(|_| {
            let app = app.clone();
            let refresh = refresh.clone();
            tokio::spawn(async move {
                send(
                    &app,
                    form(
                        "/auth/token",
                        &[
                            ("grant_type", "refresh_token"),
                            ("refresh_token", &refresh),
                            ("client_id", CLIENT),
                        ],
                        None,
                    ),
                )
                .await
                .0
            })
        })
        .collect();
    let mut statuses = Vec::new();
    for attempt in attempts {
        statuses.push(attempt.await.expect("join"));
    }
    assert!(
        statuses.iter().filter(|s| **s == StatusCode::OK).count() <= 1,
        "{statuses:?}"
    );
}
