//! The client's sign-in page in front of `/authorize`, and `/session` as an authorized route.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use health_auth::{AuthConfig, AuthServer, Client, Credentials, MemoryStore, TokenSigner};
use health_identity::{AccountId, AccountStatus, Lockout, LoginAccount};
use health_time::{FixedClock, Timestamp};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

const QUERY: &str = "response_type=code&client_id=health-client&redirect_uri=http%3A%2F%2Fcentral.test%2Fcallback&scope=openid&state=s1&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256";

fn app() -> Router {
    let credentials = Credentials::new(Vec::new()).expect("credentials");
    let store = MemoryStore::default();
    store.put_account(LoginAccount {
        id: AccountId(Uuid::now_v7()),
        username: "nakato".into(),
        credential: credentials.hash("correct horse battery").expect("hash"),
        status: AccountStatus::Active,
        lockout: Lockout::default(),
    });
    let mut config = AuthConfig::new("http://central.test/auth", "central", false);
    config.clients.push(Client {
        id: "health-client".into(),
        redirect_uris: vec!["http://central.test/callback".into()],
    });
    config.login_page = Some("/sign-in".into());
    let clock = Arc::new(FixedClock::new(
        Timestamp::from_second(1_800_000_000).expect("time"),
    ));
    let server = AuthServer::new(config, TokenSigner::generate(), credentials, store, clock);
    Router::new()
        .nest("/auth", server.router())
        .nest("/api", server.session_route())
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Option<String>, String) {
    let response = app.clone().oneshot(request).await.expect("response");
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
        location,
        String::from_utf8_lossy(&body).into_owned(),
    )
}

fn post(path: &str, body: String) -> Request<Body> {
    Request::post(path)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("request")
}

#[tokio::test]
async fn the_sign_in_page_fronts_authorize_and_session_needs_a_token() {
    let app = app();
    // GET /authorize sends the browser to the client's page, with the request intact.
    let (status, location, _) = send(
        &app,
        Request::get(format!("/auth/authorize?{QUERY}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let location = location.expect("location");
    assert!(
        location.starts_with("/sign-in?")
            && location.contains("code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"),
        "{location}"
    );

    // A refused sign-in goes back to the page with error=refused, revealing nothing more.
    let (status, location, _) = send(
        &app,
        post(
            "/auth/authorize",
            format!("{QUERY}&username=nakato&password=wrong"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert!(location.expect("location").ends_with("&error=refused"));

    // The right password redirects to the client with a code.
    let (_, location, _) = send(
        &app,
        post(
            "/auth/authorize",
            format!("{QUERY}&username=nakato&password=correct+horse+battery"),
        ),
    )
    .await;
    let location = location.expect("location");
    assert!(
        location.starts_with("http://central.test/callback?code="),
        "{location}"
    );
    let code = form_urlencoded::parse(location.split_once('?').expect("query").1.as_bytes())
        .find(|(k, _)| k == "code")
        .expect("code")
        .1
        .into_owned();
    let token = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=http%3A%2F%2Fcentral.test%2Fcallback&client_id=health-client&code_verifier=dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
    );
    let (status, _, body) = send(&app, post("/auth/token", token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let access = serde_json::from_str::<Value>(&body).expect("json")["access_token"]
        .as_str()
        .expect("access")
        .to_owned();

    // /session is an authorized route: 401 without the token, the principal with it.
    let session = |token: Option<&str>| {
        let mut request = Request::get("/api/session");
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        request.body(Body::empty()).expect("request")
    };
    assert_eq!(send(&app, session(None)).await.0, StatusCode::UNAUTHORIZED);
    let (status, _, body) = send(&app, session(Some(&access))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let principal: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(principal["client_id"], "health-client");
    assert_eq!(principal["scope"], "openid");
}

#[test]
fn new_passwords_are_checked_against_the_breached_list() {
    use health_auth::{BreachedPasswords, SetPasswordError};
    use health_identity::{PasswordPolicy, PasswordRejected};
    let credentials = Credentials::new(Vec::new()).expect("credentials");
    let policy = PasswordPolicy::default();
    let list = BreachedPasswords::platform();
    // Long enough, but one of the most common passwords: refused, with no network needed.
    assert_eq!(
        credentials.set_password(&policy, list, "password123", true),
        Err(SetPasswordError::Rejected(PasswordRejected::Breached))
    );
    assert_eq!(
        credentials.set_password(&policy, list, "short", true),
        Err(SetPasswordError::Rejected(PasswordRejected::TooShort {
            minimum: 8
        }))
    );
    let hash = credentials
        .set_password(&policy, list, "tumbling otter quarry 7919", true)
        .expect("accepted");
    assert!(credentials.verify("tumbling otter quarry 7919", &hash));
}
