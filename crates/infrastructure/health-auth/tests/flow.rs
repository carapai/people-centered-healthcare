//! End to end through the routes (offline-auth.md): login with PKCE, DPoP-bound access, refresh
//! with rotation, logout, lockout, and the 12-hour cap.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::routing::get;
use health_auth::jose::{EcJwk, sha256_b64, sign};
use health_auth::{
    AccessVerifier, AuthConfig, AuthServer, Client, Credentials, MemoryStore, TokenSigner,
};
use health_identity::{AccountId, AccountStatus, Lockout, LoginAccount};
use health_time::{FixedClock, SignedDuration, Timestamp};
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

struct Harness {
    app: Router,
    clock: Arc<FixedClock>,
    account: AccountId,
}

async fn protected(
    State(verifier): State<AccessVerifier<MemoryStore>>,
    headers: HeaderMap,
) -> Result<String, health_auth::AccessError> {
    verifier
        .verify(&headers, "GET", RESOURCE)
        .await
        .map(|p| p.account.to_string())
}

fn harness(require_dpop: bool) -> Harness {
    let clock = Arc::new(FixedClock::new(
        Timestamp::from_second(1_800_000_000).expect("time"),
    ));
    let credentials = Credentials::new(b"pepper".to_vec()).expect("credentials");
    let store = MemoryStore::default();
    let account = AccountId(Uuid::now_v7());
    store.put_account(LoginAccount {
        id: account,
        username: "nakato".into(),
        credential: credentials.hash("correct horse battery").expect("hash"),
        status: AccountStatus::Active,
        lockout: Lockout::default(),
    });
    let mut config = AuthConfig::new(ISSUER, "central", require_dpop);
    config.clients.push(Client {
        id: CLIENT.into(),
        redirect_uris: vec![REDIRECT.into()],
    });
    let server = AuthServer::new(
        config,
        TokenSigner::generate(),
        credentials,
        store,
        clock.clone(),
    );
    let app = Router::new().nest("/auth", server.router()).route(
        "/fhir/Patient",
        get(protected).with_state(server.verifier()),
    );
    Harness {
        app,
        clock,
        account,
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

fn authorize_fields<'a>(username: &'a str, password: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![
        ("response_type", "code"),
        ("client_id", CLIENT),
        ("redirect_uri", REDIRECT),
        ("scope", "openid fhirUser user/Patient.rs launch/patient"),
        ("state", "s1"),
        ("nonce", "n1"),
        (
            "code_challenge",
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        ),
        ("code_challenge_method", "S256"),
        ("username", username),
        ("password", password),
    ]
}

/// Logs in and returns the authorization code.
async fn login(h: &Harness) -> String {
    let (status, headers, _) = send(
        &h.app,
        form(
            "/auth/authorize",
            &authorize_fields("nakato", "correct horse battery"),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let location = headers[header::LOCATION].to_str().expect("location");
    let query: Vec<(String, String)> =
        form_urlencoded::parse(location.split_once('?').expect("query").1.as_bytes())
            .into_owned()
            .collect();
    assert!(location.starts_with(REDIRECT));
    assert!(query.contains(&("state".into(), "s1".into())));
    assert!(query.contains(&("iss".into(), ISSUER.into())));
    query
        .into_iter()
        .find(|(k, _)| k == "code")
        .expect("code")
        .1
}

fn now(h: &Harness) -> i64 {
    use health_time::Clock;
    h.clock.now().as_second()
}

async fn redeem(h: &Harness, code: &str, key: Option<&SigningKey>) -> (StatusCode, Value) {
    let dpop = key.map(|k| proof(k, "POST", &format!("{ISSUER}/token"), now(h), None));
    let fields = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", REDIRECT),
        ("client_id", CLIENT),
        ("code_verifier", VERIFIER),
    ];
    let (status, _, body) = send(&h.app, form("/auth/token", &fields, dpop)).await;
    (status, serde_json::from_str(&body).expect("json"))
}

async fn refresh(h: &Harness, token: &str, key: &SigningKey) -> (StatusCode, Value) {
    let dpop = Some(proof(key, "POST", &format!("{ISSUER}/token"), now(h), None));
    let fields = [
        ("grant_type", "refresh_token"),
        ("refresh_token", token),
        ("client_id", CLIENT),
    ];
    let (status, _, body) = send(&h.app, form("/auth/token", &fields, dpop)).await;
    (status, serde_json::from_str(&body).expect("json"))
}

async fn access(h: &Harness, scheme: &str, token: &str, dpop: Option<String>) -> StatusCode {
    let mut request =
        Request::get("/fhir/Patient").header(header::AUTHORIZATION, format!("{scheme} {token}"));
    if let Some(proof) = dpop {
        request = request.header("dpop", proof);
    }
    send(&h.app, request.body(Body::empty()).expect("request"))
        .await
        .0
}

#[tokio::test]
async fn login_dpop_access_refresh_and_logout_work_end_to_end() {
    let h = harness(false);
    let (status, _, page) = send(&h.app, Request::get(format!("/auth/authorize?response_type=code&client_id={CLIENT}&redirect_uri={REDIRECT}&scope=openid&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256&state=s1")).body(Body::empty()).expect("request")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("name=\"password\""));

    let key = SigningKey::random(&mut rand_core::OsRng);
    let code = login(&h).await;
    let (status, tokens) = redeem(&h, &code, Some(&key)).await;
    assert_eq!(status, StatusCode::OK, "{tokens}");
    assert_eq!(tokens["token_type"], "DPoP");
    assert_eq!(tokens["expires_in"], 900);
    // `launch/patient` is not granted.
    assert_eq!(tokens["scope"], "openid fhirUser user/Patient.rs");
    let access_token = tokens["access_token"]
        .as_str()
        .expect("access token")
        .to_owned();
    let claims: Value = serde_json::from_slice(&base64_decode(
        access_token.split('.').nth(1).expect("claims"),
    ))
    .expect("claims");
    assert_eq!(
        claims["cnf"]["jkt"],
        EcJwk::of(key.verifying_key()).thumbprint()
    );
    assert_eq!(claims["sub"], h.account.to_string());
    assert_eq!(claims["acr"], "central");
    let id_token: Value = serde_json::from_slice(&base64_decode(
        tokens["id_token"]
            .as_str()
            .expect("id token")
            .split('.')
            .nth(1)
            .expect("claims"),
    ))
    .expect("claims");
    assert_eq!(id_token["nonce"], "n1");
    assert_eq!(id_token["aud"], CLIENT);

    // DPoP-bound access: the proof must be fresh, for this request, by this key, for this token.
    assert_eq!(
        access(
            &h,
            "DPoP",
            &access_token,
            Some(proof(&key, "GET", RESOURCE, now(&h), Some(&access_token)))
        )
        .await,
        StatusCode::OK
    );
    let replayed = proof(&key, "GET", RESOURCE, now(&h), Some(&access_token));
    assert_eq!(
        access(&h, "DPoP", &access_token, Some(replayed.clone())).await,
        StatusCode::OK
    );
    assert_eq!(
        access(&h, "DPoP", &access_token, Some(replayed)).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        access(&h, "Bearer", &access_token, None).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        access(&h, "DPoP", &access_token, None).await,
        StatusCode::UNAUTHORIZED
    );
    let stranger = SigningKey::random(&mut rand_core::OsRng);
    assert_eq!(
        access(
            &h,
            "DPoP",
            &access_token,
            Some(proof(
                &stranger,
                "GET",
                RESOURCE,
                now(&h),
                Some(&access_token)
            ))
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        access(
            &h,
            "DPoP",
            &access_token,
            Some(proof(
                &key,
                "GET",
                RESOURCE,
                now(&h) - 600,
                Some(&access_token)
            ))
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        access(
            &h,
            "DPoP",
            &access_token,
            Some(proof(&key, "GET", RESOURCE, now(&h), None))
        )
        .await,
        StatusCode::UNAUTHORIZED
    );

    // Refresh rotates the refresh token and needs the same key.
    let first_refresh = tokens["refresh_token"]
        .as_str()
        .expect("refresh")
        .to_owned();
    assert_eq!(
        refresh(&h, &first_refresh, &stranger).await.0,
        StatusCode::BAD_REQUEST
    );
    let (status, rotated) = refresh(&h, &first_refresh, &key).await;
    assert_eq!(status, StatusCode::OK, "{rotated}");
    assert!(rotated.get("id_token").is_none());
    let second_refresh = rotated["refresh_token"]
        .as_str()
        .expect("refresh")
        .to_owned();
    assert_ne!(first_refresh, second_refresh);
    let new_access = rotated["access_token"].as_str().expect("access").to_owned();
    assert_eq!(
        access(
            &h,
            "DPoP",
            &new_access,
            Some(proof(&key, "GET", RESOURCE, now(&h), Some(&new_access)))
        )
        .await,
        StatusCode::OK
    );

    // Logout ends the session: its access tokens and refresh token stop working at once.
    let (status, _, _) = send(
        &h.app,
        form(
            "/auth/revoke",
            &[("token", &second_refresh), ("client_id", CLIENT)],
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        access(
            &h,
            "DPoP",
            &new_access,
            Some(proof(&key, "GET", RESOURCE, now(&h), Some(&new_access)))
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        refresh(&h, &second_refresh, &key).await.0,
        StatusCode::BAD_REQUEST
    );
}

fn base64_decode(part: &str) -> Vec<u8> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(part)
        .expect("base64")
}

#[tokio::test]
async fn a_replayed_refresh_token_ends_the_session() {
    let h = harness(false);
    let key = SigningKey::random(&mut rand_core::OsRng);
    let (_, tokens) = redeem(&h, &login(&h).await, Some(&key)).await;
    let first = tokens["refresh_token"]
        .as_str()
        .expect("refresh")
        .to_owned();
    let (_, rotated) = refresh(&h, &first, &key).await;
    // The old token comes back: whoever holds it is not the client, so the session ends.
    assert_eq!(refresh(&h, &first, &key).await.0, StatusCode::BAD_REQUEST);
    let current = rotated["refresh_token"].as_str().expect("refresh");
    assert_eq!(refresh(&h, current, &key).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn codes_are_single_use_and_pkce_bound() {
    let h = harness(false);
    let code = login(&h).await;
    let fields = [
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", REDIRECT),
        ("client_id", CLIENT),
        (
            "code_verifier",
            "wrong-verifier-wrong-verifier-wrong-verifier-x",
        ),
    ];
    let (status, _, body) = send(&h.app, form("/auth/token", &fields, None)).await;
    assert_eq!(
        (status, body.contains("invalid_grant")),
        (StatusCode::BAD_REQUEST, true)
    );
    // The failed attempt consumed the code.
    assert_eq!(redeem(&h, &code, None).await.0, StatusCode::BAD_REQUEST);

    // Without a proof, central issues a bearer token.
    let (status, tokens) = redeem(&h, &login(&h).await, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tokens["token_type"], "Bearer");
    let token = tokens["access_token"].as_str().expect("access");
    assert_eq!(access(&h, "Bearer", token, None).await, StatusCode::OK);

    // A code older than its minute is refused.
    let code = login(&h).await;
    h.clock.advance(SignedDuration::from_secs(61));
    assert_eq!(redeem(&h, &code, None).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn an_edge_requires_dpop() {
    let h = harness(true);
    let (status, body) = redeem(&h, &login(&h).await, None).await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::BAD_REQUEST, json!("invalid_dpop_proof"))
    );
}

#[tokio::test]
async fn lockout_rules_are_enforced() {
    let h = harness(false);
    for _ in 0..10 {
        let (status, _, page) = send(
            &h.app,
            form(
                "/auth/authorize",
                &authorize_fields("nakato", "wrong"),
                None,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(page.contains("role=\"alert\""));
    }
    // Locked: even the right password is refused, with the same message.
    let (status, _, page) = send(
        &h.app,
        form(
            "/auth/authorize",
            &authorize_fields("nakato", "correct horse battery"),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (_, _, unknown) = send(
        &h.app,
        form(
            "/auth/authorize",
            &authorize_fields("nobody", "wrong"),
            None,
        ),
    )
    .await;
    assert_eq!(
        page, unknown,
        "a locked account and an unknown one look the same"
    );
    h.clock.advance(SignedDuration::from_mins(15));
    login(&h).await;
}

#[tokio::test]
async fn sessions_stop_refreshing_after_twelve_hours() {
    let h = harness(false);
    let key = SigningKey::random(&mut rand_core::OsRng);
    let (_, tokens) = redeem(&h, &login(&h).await, Some(&key)).await;
    let mut token = tokens["refresh_token"]
        .as_str()
        .expect("refresh")
        .to_owned();
    h.clock
        .advance(SignedDuration::from_hours(11) + SignedDuration::from_mins(50));
    let (status, rotated) = refresh(&h, &token, &key).await;
    assert_eq!(status, StatusCode::OK);
    // The last access token stops at the cap, not 15 minutes later.
    assert_eq!(rotated["expires_in"], 600);
    token = rotated["refresh_token"]
        .as_str()
        .expect("refresh")
        .to_owned();
    h.clock.advance(SignedDuration::from_mins(10));
    assert_eq!(refresh(&h, &token, &key).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn bad_clients_and_redirects_are_never_redirected_to() {
    let h = harness(false);
    let (status, headers, _) = send(&h.app, Request::get("/auth/authorize?response_type=code&client_id=health-client&redirect_uri=http://evil.test/&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256").body(Body::empty()).expect("request")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(headers.get(header::LOCATION).is_none());
    // A known client without PKCE is sent back with an error.
    let (status, headers, _) = send(
        &h.app,
        Request::get(format!(
            "/auth/authorize?response_type=code&client_id={CLIENT}&redirect_uri={REDIRECT}&state=s"
        ))
        .body(Body::empty())
        .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert!(
        headers[header::LOCATION]
            .to_str()
            .expect("location")
            .contains("error=invalid_request")
    );
    let (_, _, metadata) = send(
        &h.app,
        Request::get("/auth/.well-known/smart-configuration")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    let metadata: Value = serde_json::from_str(&metadata).expect("json");
    assert_eq!(
        metadata["code_challenge_methods_supported"],
        json!(["S256"])
    );
    assert_eq!(metadata["token_endpoint"], format!("{ISSUER}/token"));
}
