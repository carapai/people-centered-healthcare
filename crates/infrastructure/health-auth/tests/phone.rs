//! Patients sign in with a one-time code to their phone, every time (#111; ADR 0029): the first
//! accepted code creates the phone's Login Account, later ones sign in to it, and a code is
//! limited in time, tries and how often it is sent.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::routing::get;
use health_auth::{
    AccessVerifier, AuthConfig, AuthServer, AuthStore, Client, Credentials, DevOutbox, MemoryStore,
    PhoneSignIn, TokenSigner,
};
use health_identity::PhoneNumber;
use health_time::{FixedClock, SignedDuration, Timestamp};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

const ISSUER: &str = "http://central.test/auth";
const PORTAL: &str = "portal";
const STAFF: &str = "health-client";
const REDIRECT: &str = "http://central.test/callback";
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const PHONE: &str = "0772 123456";

struct Harness {
    app: Router,
    clock: Arc<FixedClock>,
    outbox: DevOutbox,
    server: AuthServer<MemoryStore>,
}

async fn whoami(
    State(verifier): State<AccessVerifier<MemoryStore>>,
    headers: HeaderMap,
) -> Result<String, health_auth::AccessError> {
    verifier
        .verify(&headers, "GET", "http://central.test/whoami")
        .await
        .map(|p| format!("{} {}", p.account, p.patient))
}

async fn portal_subject(
    subject: Option<axum::Extension<health_access_policy::PatientSubject>>,
) -> String {
    subject.map_or_else(|| "none".to_owned(), |s| s.0.phone.clone())
}

fn harness() -> Harness {
    let clock = Arc::new(FixedClock::new(
        Timestamp::from_second(1_800_000_000).expect("time"),
    ));
    let outbox = DevOutbox::default();
    let mut config = AuthConfig::new(ISSUER, "central", false);
    for (id, patient) in [(PORTAL, true), (STAFF, false)] {
        config.clients.push(Client {
            id: id.into(),
            redirect_uris: vec![REDIRECT.into()],
            patient,
        });
    }
    config.phone_sign_in = Some(PhoneSignIn::new("256", Arc::new(outbox.clone())));
    let server = AuthServer::new(
        config,
        TokenSigner::generate(),
        Credentials::new(b"pepper".to_vec()).expect("credentials"),
        MemoryStore::default(),
        clock.clone(),
    );
    let portal = Router::new().route("/portal", get(portal_subject)).layer(
        axum::middleware::from_fn_with_state(
            server.verifier(),
            health_auth::patient_layer::<MemoryStore>,
        ),
    );
    let app = Router::new()
        .nest("/auth", server.router())
        .route("/whoami", get(whoami).with_state(server.verifier()))
        .merge(portal);
    Harness {
        app,
        clock,
        outbox,
        server,
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

fn form(path: &str, fields: &[(&str, &str)]) -> Request<Body> {
    let body = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields)
        .finish();
    Request::post(path)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("request")
}

fn request_fields<'a>(client: &'a str, extra: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
    let mut fields = vec![
        ("response_type", "code"),
        ("client_id", client),
        ("redirect_uri", REDIRECT),
        ("scope", "openid"),
        ("state", "s1"),
        (
            "code_challenge",
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        ),
        ("code_challenge_method", "S256"),
    ];
    fields.extend_from_slice(extra);
    fields
}

fn phone() -> PhoneNumber {
    PhoneNumber::parse(PHONE, "256").expect("phone")
}

async fn ask_code(h: &Harness) -> (StatusCode, String) {
    let (status, _, body) = send(
        &h.app,
        form("/auth/otp", &request_fields(PORTAL, &[("phone", PHONE)])),
    )
    .await;
    (status, body)
}

async fn try_code(h: &Harness, code: &str) -> (StatusCode, HeaderMap, String) {
    send(
        &h.app,
        form(
            "/auth/authorize",
            &request_fields(PORTAL, &[("phone", PHONE), ("code", code)]),
        ),
    )
    .await
}

/// Signs in with the code in the outbox and returns `whoami` for the tokens.
async fn sign_in(h: &Harness) -> (String, Value) {
    let (status, _) = ask_code(h).await;
    assert_eq!(status, StatusCode::OK);
    let code = h.outbox.latest(&phone()).expect("code sent");
    assert_eq!(code.len(), 6);
    assert!(code.bytes().all(|b| b.is_ascii_digit()));
    let (status, headers, _) = try_code(h, &code).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let location = headers[header::LOCATION].to_str().expect("location");
    assert!(location.starts_with(REDIRECT));
    let auth_code = form_urlencoded::parse(location.split_once('?').expect("query").1.as_bytes())
        .find(|(k, _)| k == "code")
        .expect("code")
        .1
        .into_owned();
    let (status, _, body) = send(
        &h.app,
        form(
            "/auth/token",
            &[
                ("grant_type", "authorization_code"),
                ("code", &auth_code),
                ("redirect_uri", REDIRECT),
                ("client_id", PORTAL),
                ("code_verifier", VERIFIER),
            ],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tokens: Value = serde_json::from_str(&body).expect("json");
    let token = tokens["access_token"].as_str().expect("token");
    let (status, _, who) = send(
        &h.app,
        Request::get("/whoami")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    (who, tokens)
}

#[tokio::test]
async fn the_first_code_creates_the_account_and_later_codes_sign_in_to_it() {
    let h = harness();
    assert!(
        h.server
            .store()
            .phone_account(&phone())
            .await
            .expect("store")
            .is_none()
    );
    let (first, tokens) = sign_in(&h).await;
    let account = h
        .server
        .store()
        .phone_account(&phone())
        .await
        .expect("store")
        .expect("created");
    assert_eq!(first, format!("{} true", account.id));
    assert!(tokens["refresh_token"].is_string());
    assert!(tokens.get("snapshot_not_after").is_none());

    h.clock.advance(SignedDuration::from_mins(2));
    let (second, _) = sign_in(&h).await;
    assert_eq!(second, first, "one account per phone");
}

#[tokio::test]
async fn a_code_has_five_tries_and_then_a_new_one_is_needed() {
    let h = harness();
    ask_code(&h).await;
    let code = h.outbox.latest(&phone()).expect("code");
    let wrong = if code == "000000" { "000001" } else { "000000" };
    for _ in 0..5 {
        let (status, _, body) = try_code(&h, wrong).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(body.contains("not right"), "{body}");
    }
    let (status, _, body) = try_code(&h, &code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("no longer be used"), "{body}");
    assert!(
        h.server
            .store()
            .phone_account(&phone())
            .await
            .expect("store")
            .is_none()
    );
}

#[tokio::test]
async fn a_code_expires_after_five_minutes_and_is_used_once() {
    let h = harness();
    ask_code(&h).await;
    let code = h.outbox.latest(&phone()).expect("code");
    h.clock.advance(SignedDuration::from_mins(5));
    let (status, _, body) = try_code(&h, &code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("no longer be used"));

    h.clock.advance(SignedDuration::from_mins(1));
    sign_in(&h).await;
    let code = h.outbox.latest(&phone()).expect("code");
    let (status, _, _) = try_code(&h, &code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a used code is refused");
}

#[tokio::test]
async fn codes_to_one_phone_are_rate_limited() {
    let h = harness();
    assert_eq!(ask_code(&h).await.0, StatusCode::OK);
    let (status, body) = ask_code(&h).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert!(body.contains("Wait a minute"));
    for _ in 0..4 {
        h.clock.advance(SignedDuration::from_mins(1));
        assert_eq!(ask_code(&h).await.0, StatusCode::OK);
    }
    h.clock.advance(SignedDuration::from_mins(1));
    assert_eq!(
        ask_code(&h).await.0,
        StatusCode::TOO_MANY_REQUESTS,
        "5 an hour"
    );
    h.clock.advance(SignedDuration::from_mins(55));
    assert_eq!(ask_code(&h).await.0, StatusCode::OK);
}

#[tokio::test]
async fn concurrent_tries_of_one_challenge_count_once() {
    let h = harness();
    ask_code(&h).await;
    let store = h.server.store();
    let otp = store
        .phone_otp(&phone())
        .await
        .expect("store")
        .expect("otp");
    let mut tried = otp.clone();
    tried.challenge.attempts = 1;
    // Two tries that both read 0 tries: only the first is stored, the second is refused.
    assert!(store.attempt_phone_otp(0, &tried).await.expect("store"));
    assert!(!store.attempt_phone_otp(0, &tried).await.expect("store"));
}

#[tokio::test]
async fn phone_sign_in_is_only_for_patient_clients_and_passwords_only_for_staff() {
    let h = harness();
    let (status, _, _) = send(
        &h.app,
        form("/auth/otp", &request_fields(STAFF, &[("phone", PHONE)])),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(h.outbox.latest(&phone()).is_none());

    // A portal sign-in with a username and password is a refused code.
    let (status, _, _) = send(
        &h.app,
        form(
            "/auth/authorize",
            &request_fields(PORTAL, &[("username", "nakato"), ("password", "secret")]),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // An unreadable phone number.
    let (status, _, body) = send(
        &h.app,
        form("/auth/otp", &request_fields(PORTAL, &[("phone", "07x2")])),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("phone number"));

    // The portal's authorize page asks for a phone, not a password.
    let query = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(request_fields(PORTAL, &[]))
        .finish();
    let (status, _, body) = send(
        &h.app,
        Request::get(format!("/auth/authorize?{query}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("name=\"phone\"") && !body.contains("password"));
}

#[tokio::test]
async fn the_patient_layer_gives_portal_routes_the_signed_in_patient() {
    let h = harness();
    let (_, tokens) = sign_in(&h).await;
    let token = tokens["access_token"].as_str().expect("token");
    let get = |auth: Option<String>| {
        let mut request = Request::get("/portal");
        if let Some(auth) = auth {
            request = request.header(header::AUTHORIZATION, auth);
        }
        request.body(Body::empty()).expect("request")
    };
    let (status, _, body) = send(&h.app, get(Some(format!("Bearer {token}")))).await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "+256772123456"));
    let (status, _, body) = send(&h.app, get(None)).await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "none"));
    let (status, _, _) = send(&h.app, get(Some("Bearer not-a-token".into()))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
