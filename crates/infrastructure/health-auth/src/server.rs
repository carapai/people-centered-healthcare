//! The authorization server's HTTP endpoints and the resource side's access check.

use std::sync::Arc;

use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use health_identity::{
    AccountId, AccountStatus, LockoutPolicy, OtpPurpose, OtpRefused, PhoneAccount, PhoneNumber,
    Scope, Scopes, SessionPolicy, SessionState,
};
use health_time::{Clock, SignedDuration, Timestamp};
use rand_core::RngCore;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::Credentials;
use crate::jose::{TokenSigner, b64, sha256_b64, verify_dpop};
use crate::otp::{PhoneOtp, PhoneSignIn, code_matches, hash_code, new_code};
use crate::snapshot::{SnapshotGate, token_claims};
use crate::store::{AuthStore, AuthorizationCode, Session};
use health_identity::snapshot::PermissionSnapshot;

/// A registered public client (authorization code with PKCE; no client secret).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    /// The `client_id`.
    pub id: String,
    /// Redirect URIs, compared exactly.
    pub redirect_uris: Vec<String>,
    /// A patient client (the portal): its users sign in with a one-time code to their phone
    /// ([`AuthConfig::phone_sign_in`]), never a password, and need no permission snapshot.
    pub patient: bool,
}

/// How this node runs its authorization server.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// The issuer URL, e.g. `https://central.example/auth`; endpoints sit under it.
    pub issuer: String,
    /// `central` at central, `offline-local` at an edge (offline-auth.md § Tokens and sessions).
    pub acr: String,
    /// Whether tokens must be DPoP-bound (always on an edge; ADR 0009).
    pub require_dpop: bool,
    /// The registered clients.
    pub clients: Vec<Client>,
    /// Lockout after failed logins.
    pub lockout: LockoutPolicy,
    /// Token and session lifetimes.
    pub session: SessionPolicy,
    /// How long an authorization code may wait to be redeemed.
    pub code_lifetime: SignedDuration,
    /// How old a DPoP proof's `iat` may be, and how far ahead of our clock.
    pub dpop_window: SignedDuration,
    /// On an edge: logins and refreshes need a valid permission snapshot (ADR 0011).
    pub snapshots: Option<SnapshotGate>,
    /// The client application's sign-in page (e.g. `/sign-in`). When set, `GET /authorize`
    /// sends the user there with the request's parameters, and a refused sign-in returns there
    /// with `error=refused`; the page posts the credentials back to `/authorize`. Without it,
    /// this server's minimal form is shown.
    pub login_page: Option<String>,
    /// Sign-in by one-time code for patient clients (#111). Without it they cannot sign in.
    pub phone_sign_in: Option<PhoneSignIn>,
}

impl AuthConfig {
    /// The defaults for `issuer`, with no clients.
    pub fn new(issuer: impl Into<String>, acr: impl Into<String>, require_dpop: bool) -> Self {
        Self {
            issuer: issuer.into().trim_end_matches('/').to_owned(),
            acr: acr.into(),
            require_dpop,
            clients: Vec::new(),
            lockout: LockoutPolicy::default(),
            session: SessionPolicy::default(),
            code_lifetime: SignedDuration::from_secs(60),
            dpop_window: SignedDuration::from_secs(300),
            snapshots: None,
            login_page: None,
            phone_sign_in: None,
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.issuer)
    }

    /// Whether `client_id` is a patient client.
    fn patient(&self, client_id: &str) -> bool {
        self.clients.iter().any(|c| c.id == client_id && c.patient)
    }
}

struct Inner<S> {
    config: AuthConfig,
    signer: TokenSigner,
    credentials: Credentials,
    store: S,
    clock: Arc<dyn Clock>,
}

/// The authorization server: build its routes with [`router`](Self::router) and check access
/// tokens on resource routes with [`verifier`](Self::verifier).
pub struct AuthServer<S> {
    inner: Arc<Inner<S>>,
}

impl<S> Clone for AuthServer<S> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<S: AuthStore + 'static> AuthServer<S> {
    /// A server over `store`, signing with `signer`.
    pub fn new(
        config: AuthConfig,
        signer: TokenSigner,
        credentials: Credentials,
        store: S,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                signer,
                credentials,
                store,
                clock,
            }),
        }
    }

    /// The store, for provisioning and administration.
    pub fn store(&self) -> &S {
        &self.inner.store
    }

    /// The routes, relative to the issuer URL's path; nest them there.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/.well-known/openid-configuration", get(metadata::<S>))
            .route(
                "/.well-known/smart-configuration",
                get(smart_configuration::<S>),
            )
            .route("/jwks", get(jwks::<S>))
            .route(
                "/authorize",
                get(authorize_form::<S>).post(authorize_login::<S>),
            )
            .route("/otp", post(send_otp::<S>))
            .route("/token", post(token::<S>))
            .route("/revoke", post(revoke::<S>))
            .with_state(Arc::clone(&self.inner))
    }

    /// `GET /session`: who the request's access token belongs to (account, client, scopes),
    /// or 401. Mount it with the API; the web client reads it after signing in.
    pub fn session_route(&self) -> Router {
        Router::new()
            .route("/session", get(session_info::<S>))
            .with_state(self.verifier())
    }

    /// The resource side's check of access tokens issued here.
    pub fn verifier(&self) -> AccessVerifier<S> {
        AccessVerifier {
            inner: Arc::clone(&self.inner),
        }
    }
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand_core::OsRng.fill_bytes(&mut bytes);
    b64(&bytes)
}

fn seconds(at: Timestamp) -> i64 {
    at.as_second()
}

// ---------------------------------------------------------------------------------------------
// Metadata

async fn metadata<S: AuthStore>(State(inner): State<Arc<Inner<S>>>) -> Json<Value> {
    let c = &inner.config;
    Json(json!({
        "issuer": c.issuer,
        "authorization_endpoint": c.endpoint("/authorize"),
        "token_endpoint": c.endpoint("/token"),
        "revocation_endpoint": c.endpoint("/revoke"),
        "jwks_uri": c.endpoint("/jwks"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["ES256"],
        "dpop_signing_alg_values_supported": ["ES256"],
        "authorization_response_iss_parameter_supported": true,
        "scopes_supported": ["openid", "fhirUser", "offline_access", "online_access"],
    }))
}

async fn smart_configuration<S: AuthStore>(state: State<Arc<Inner<S>>>) -> Json<Value> {
    let Json(mut value) = metadata(state).await;
    value["capabilities"] = json!([
        "launch-standalone",
        "client-public",
        "sso-openid-connect",
        "permission-v2",
        "permission-user",
        "authorize-post"
    ]);
    Json(value)
}

async fn jwks<S: AuthStore>(State(inner): State<Arc<Inner<S>>>) -> Json<Value> {
    Json(inner.signer.jwks())
}

// ---------------------------------------------------------------------------------------------
// Authorization endpoint

/// The parameters of an authorization request (RFC 6749 § 4.1.1, RFC 7636, OIDC).
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct AuthorizeParams {
    response_type: Option<String>,
    client_id: Option<String>,
    redirect_uri: Option<String>,
    scope: Option<String>,
    state: Option<String>,
    nonce: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
}

/// The login form's submission: the request's parameters plus the credentials.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct LoginForm {
    #[serde(flatten)]
    request: AuthorizeParams,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    /// The facility to act for, where the user chooses it (central, ADR 0056).
    #[serde(default)]
    facility: String,
    /// A patient's phone number (patient clients).
    #[serde(default)]
    phone: String,
    /// The one-time code sent to it.
    #[serde(default)]
    code: String,
}

/// A request for a one-time code: the authorization request's parameters plus the phone.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct OtpForm {
    #[serde(flatten)]
    request: AuthorizeParams,
    #[serde(default)]
    phone: String,
}

/// A validated authorization request.
struct Request {
    client_id: String,
    redirect_uri: String,
    scopes: Scopes,
    state: Option<String>,
    nonce: Option<String>,
    code_challenge: String,
}

/// Validates a request. A bad client or redirect URI is shown, never redirected to (RFC 6749
/// § 4.1.2.1); anything else is redirected back as an error.
fn validate<S>(inner: &Inner<S>, p: &AuthorizeParams) -> Result<Request, Box<Response>> {
    let client = p
        .client_id
        .as_deref()
        .and_then(|id| inner.config.clients.iter().find(|c| c.id == id));
    let (Some(client), Some(redirect_uri)) = (client, p.redirect_uri.as_deref()) else {
        return Err(Box::new(error_page(
            StatusCode::BAD_REQUEST,
            "Unknown client or redirect URI.",
        )));
    };
    if !client.redirect_uris.iter().any(|u| u == redirect_uri) {
        return Err(Box::new(error_page(
            StatusCode::BAD_REQUEST,
            "Unknown client or redirect URI.",
        )));
    }
    let back = |error: &str| {
        redirect_with(
            &inner.config,
            redirect_uri,
            &[("error", error)],
            p.state.as_deref(),
        )
    };
    if p.response_type.as_deref() != Some("code") {
        return Err(Box::new(back("unsupported_response_type")));
    }
    // PKCE with S256 is mandatory; `plain` is refused.
    let Some(code_challenge) = p.code_challenge.clone().filter(|c| c.len() == 43) else {
        return Err(Box::new(back("invalid_request")));
    };
    if p.code_challenge_method.as_deref() != Some("S256") {
        return Err(Box::new(back("invalid_request")));
    }
    let Ok(scopes) = Scopes::parse(p.scope.as_deref().unwrap_or_default()) else {
        return Err(Box::new(back("invalid_scope")));
    };
    Ok(Request {
        client_id: client.id.clone(),
        redirect_uri: redirect_uri.to_owned(),
        scopes: scopes.granted(),
        state: p.state.clone(),
        nonce: p.nonce.clone(),
        code_challenge,
    })
}

fn redirect_with(
    config: &AuthConfig,
    uri: &str,
    params: &[(&str, &str)],
    state: Option<&str>,
) -> Response {
    let mut query = form_urlencoded::Serializer::new(String::new());
    for (name, value) in params {
        query.append_pair(name, value);
    }
    if let Some(state) = state {
        query.append_pair("state", state);
    }
    // RFC 9207: the issuer identifies who answered.
    query.append_pair("iss", &config.issuer);
    let separator = if uri.contains('?') { '&' } else { '?' };
    Redirect::to(&format!("{uri}{separator}{}", query.finish())).into_response()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn error_page(status: StatusCode, message: &str) -> Response {
    (status, no_store(), Html(format!("<!doctype html><html lang=\"en\"><title>Sign-in error</title><main><h1>Sign-in error</h1><p>{}</p></main></html>", escape(message))))
        .into_response()
}

fn no_store() -> [(header::HeaderName, HeaderValue); 2] {
    [
        (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
        (header::PRAGMA, HeaderValue::from_static("no-cache")),
    ]
}

/// The login form. The client application's sign-in screen replaces it in the UI; this one keeps
/// the protocol complete and testable.
fn login_page(p: &AuthorizeParams, error: Option<&str>) -> String {
    let hidden = hidden_fields(p);
    let alert = error
        .map(|e| format!("<p role=\"alert\">{}</p>", escape(e)))
        .unwrap_or_default();
    format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Sign in</title><main><h1>Sign in</h1>{alert}\
         <form method=\"post\" action=\"authorize\">{hidden}\
         <label for=\"username\">Username</label><input id=\"username\" name=\"username\" autocomplete=\"username\" required>\
         <label for=\"password\">Password</label><input id=\"password\" name=\"password\" type=\"password\" autocomplete=\"current-password\" required>\
         <label for=\"facility\">Facility (if you work at more than one)</label><input id=\"facility\" name=\"facility\">\
         <button type=\"submit\">Sign in</button></form></main></html>"
    )
}

/// A patient's first step: their phone number, posted to `/otp`.
fn phone_page(p: &AuthorizeParams, error: Option<&str>) -> String {
    let hidden = hidden_fields(p);
    let alert = error
        .map(|e| format!("<p role=\"alert\">{}</p>", escape(e)))
        .unwrap_or_default();
    format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Sign in</title><main><h1>Sign in</h1>{alert}\
         <form method=\"post\" action=\"otp\">{hidden}\
         <label for=\"phone\">Phone number</label><input id=\"phone\" name=\"phone\" type=\"tel\" autocomplete=\"tel\" required>\
         <button type=\"submit\">Send me a code</button></form></main></html>"
    )
}

/// A patient's second step: the code sent to their phone, posted to `/authorize`.
fn code_page(p: &AuthorizeParams, phone: &str, error: Option<&str>) -> String {
    let hidden = hidden_fields(p);
    let alert = error
        .map(|e| format!("<p role=\"alert\">{}</p>", escape(e)))
        .unwrap_or_default();
    let phone = escape(phone);
    format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Enter your code</title><main><h1>Enter your code</h1>{alert}\
         <p>We sent a code to {phone}.</p>\
         <form method=\"post\" action=\"authorize\">{hidden}<input type=\"hidden\" name=\"phone\" value=\"{phone}\">\
         <label for=\"code\">Code</label><input id=\"code\" name=\"code\" inputmode=\"numeric\" autocomplete=\"one-time-code\" required>\
         <button type=\"submit\">Sign in</button></form></main></html>"
    )
}

/// The authorization request's parameters as hidden inputs.
fn hidden_fields(p: &AuthorizeParams) -> String {
    [
        ("response_type", &p.response_type),
        ("client_id", &p.client_id),
        ("redirect_uri", &p.redirect_uri),
        ("scope", &p.scope),
        ("state", &p.state),
        ("nonce", &p.nonce),
        ("code_challenge", &p.code_challenge),
        ("code_challenge_method", &p.code_challenge_method),
    ]
    .iter()
    .filter_map(|(name, value)| {
        value.as_deref().map(|v| {
            format!(
                "<input type=\"hidden\" name=\"{name}\" value=\"{}\">",
                escape(v)
            )
        })
    })
    .collect()
}

async fn authorize_form<S: AuthStore>(
    State(inner): State<Arc<Inner<S>>>,
    Query(params): Query<AuthorizeParams>,
) -> Response {
    match validate(&inner, &params) {
        Ok(request) => match &inner.config.login_page {
            Some(page) => Redirect::to(&with_query(page, &params, None)).into_response(),
            None if inner.config.patient(&request.client_id) => {
                (no_store(), Html(phone_page(&params, None))).into_response()
            }
            None => (no_store(), Html(login_page(&params, None))).into_response(),
        },
        Err(response) => *response,
    }
}

/// `page` with the authorization request's parameters (and an error code) as its query.
fn with_query(page: &str, p: &AuthorizeParams, error: Option<&str>) -> String {
    with_pairs(page, p, &error.map(|e| [("error", e)]).unwrap_or_default())
}

/// `page` with the authorization request's parameters and `extra` as its query.
fn with_pairs(page: &str, p: &AuthorizeParams, extra: &[(&str, &str)]) -> String {
    let mut query = form_urlencoded::Serializer::new(String::new());
    for (name, value) in [
        ("response_type", &p.response_type),
        ("client_id", &p.client_id),
        ("redirect_uri", &p.redirect_uri),
        ("scope", &p.scope),
        ("state", &p.state),
        ("nonce", &p.nonce),
        ("code_challenge", &p.code_challenge),
        ("code_challenge_method", &p.code_challenge_method),
    ] {
        if let Some(value) = value {
            query.append_pair(name, value);
        }
    }
    for (name, value) in extra {
        query.append_pair(name, value);
    }
    let separator = if page.contains('?') { '&' } else { '?' };
    format!("{page}{separator}{}", query.finish())
}

/// One message for every refusal, so a response does not reveal whether an account exists or
/// is locked.
const LOGIN_REFUSED: &str = "The username or password is incorrect, or the account is locked.";

/// Shown when the user works at several facilities and named none of them.
const CHOOSE_FACILITY: &str =
    "You work at more than one facility: enter the one you are signing in for.";

/// Where a sign-in's facility comes from.
enum FacilityChoice {
    /// This one (or none, where the node keeps no snapshots).
    Chosen(Option<String>),
    /// The user must name one of several.
    Ask,
    /// None the user may act for.
    Refused,
}

async fn authorize_login<S: AuthStore>(
    State(inner): State<Arc<Inner<S>>>,
    Form(form): Form<LoginForm>,
) -> Response {
    let request = match validate(&inner, &form.request) {
        Ok(request) => request,
        Err(response) => return *response,
    };
    if inner.config.patient(&request.client_id) {
        return phone_login(&inner, &form, request).await;
    }
    let refused = || match &inner.config.login_page {
        Some(page) => {
            Redirect::to(&with_query(page, &form.request, Some("refused"))).into_response()
        }
        None => (
            StatusCode::UNAUTHORIZED,
            no_store(),
            Html(login_page(&form.request, Some(LOGIN_REFUSED))),
        )
            .into_response(),
    };
    let now = inner.clock.now();
    let account = match inner.store.account_by_username(&form.username).await {
        Ok(account) => account,
        Err(_) => return error_page(StatusCode::SERVICE_UNAVAILABLE, "Sign-in is unavailable."),
    };
    let Some(account) = account else {
        inner.credentials.verify_decoy(&form.password);
        return refused();
    };
    if account.status != AccountStatus::Active
        || account.lockout.check(&inner.config.lockout, now).is_err()
    {
        inner.credentials.verify_decoy(&form.password);
        return refused();
    }
    if !inner
        .credentials
        .verify(&form.password, &account.credential)
    {
        let lockout = account.lockout.failed(&inner.config.lockout, now);
        return match inner.store.set_lockout(account.id, lockout).await {
            Ok(()) => refused(),
            Err(_) => error_page(StatusCode::SERVICE_UNAVAILABLE, "Sign-in is unavailable."),
        };
    }
    let facility = match inner.sign_in_facility(account.id, &form.facility).await {
        FacilityChoice::Chosen(facility) => facility,
        FacilityChoice::Refused => return refused(),
        FacilityChoice::Ask => {
            return match &inner.config.login_page {
                Some(page) => {
                    Redirect::to(&with_query(page, &form.request, Some("facility"))).into_response()
                }
                None => (
                    StatusCode::BAD_REQUEST,
                    no_store(),
                    Html(login_page(&form.request, Some(CHOOSE_FACILITY))),
                )
                    .into_response(),
            };
        }
    };
    // The password alone is not enough: the user needs a live snapshot for the facility. The
    // refusal looks like any other.
    if inner
        .current_snapshot(account.id, facility.as_deref(), now)
        .await
        .is_err()
    {
        return refused();
    }
    if inner
        .store
        .set_lockout(account.id, account.lockout.reset())
        .await
        .is_err()
    {
        return error_page(StatusCode::SERVICE_UNAVAILABLE, "Sign-in is unavailable.");
    }
    issue_code(&inner, request, account.id, facility, now).await
}

/// Redirects back with a new authorization code for `account`.
async fn issue_code<S: AuthStore>(
    inner: &Inner<S>,
    request: Request,
    account: AccountId,
    facility: Option<String>,
    now: Timestamp,
) -> Response {
    let code = random_token();
    let stored = AuthorizationCode {
        client_id: request.client_id,
        redirect_uri: request.redirect_uri.clone(),
        code_challenge: request.code_challenge,
        scopes: request.scopes,
        nonce: request.nonce,
        account,
        facility,
        auth_time: now,
        expires_at: now.checked_add(inner.config.code_lifetime).unwrap_or(now),
    };
    match inner.store.put_code(sha256_b64(&code), stored).await {
        Ok(()) => redirect_with(
            &inner.config,
            &request.redirect_uri,
            &[("code", &code)],
            request.state.as_deref(),
        ),
        Err(_) => error_page(StatusCode::SERVICE_UNAVAILABLE, "Sign-in is unavailable."),
    }
}

/// Shown when a patient's phone number cannot be read.
const PHONE_INVALID: &str = "Enter your phone number, for example 0772 123456.";
/// Shown when codes are asked for too often.
const OTP_WAIT: &str = "A code was sent recently. Wait a minute, then ask for a new one.";
/// One message for a wrong code, whatever the reason, apart from asking for a new one.
const OTP_REFUSED: &str = "That code is not right. Check it, or ask for a new one.";
/// Shown when the code can no longer be used.
const OTP_EXPIRED: &str = "That code can no longer be used. Ask for a new one.";

/// `POST /otp`: sends a patient a one-time code. Signing up and signing in are the same step,
/// so the answer never reveals whether the phone has an account.
async fn send_otp<S: AuthStore>(
    State(inner): State<Arc<Inner<S>>>,
    Form(form): Form<OtpForm>,
) -> Response {
    let request = match validate(&inner, &form.request) {
        Ok(request) => request,
        Err(response) => return *response,
    };
    let (true, Some(phone_sign_in)) = (
        inner.config.patient(&request.client_id),
        &inner.config.phone_sign_in,
    ) else {
        return error_page(
            StatusCode::BAD_REQUEST,
            "This client does not sign in by phone.",
        );
    };
    let back = |status: StatusCode, error: &str, message: &str| match &inner.config.login_page {
        Some(page) => {
            Redirect::to(&with_pairs(page, &form.request, &[("error", error)])).into_response()
        }
        None => (
            status,
            no_store(),
            Html(phone_page(&form.request, Some(message))),
        )
            .into_response(),
    };
    let Ok(phone) = PhoneNumber::parse(&form.phone, &phone_sign_in.country_code) else {
        return back(StatusCode::BAD_REQUEST, "phone", PHONE_INVALID);
    };
    let now = inner.clock.now();
    let Ok(current) = inner.store.phone_otp(&phone).await else {
        return error_page(StatusCode::SERVICE_UNAVAILABLE, "Sign-in is unavailable.");
    };
    let sent = current.map(|otp| otp.sent).unwrap_or_default();
    if phone_sign_in.policy.may_send(&sent, now).is_err() {
        return back(StatusCode::TOO_MANY_REQUESTS, "wait", OTP_WAIT);
    }
    let purpose = match inner.store.phone_account(&phone).await {
        Ok(Some(_)) => OtpPurpose::SignIn,
        Ok(None) => OtpPurpose::SignUp,
        Err(_) => return error_page(StatusCode::SERVICE_UNAVAILABLE, "Sign-in is unavailable."),
    };
    let code = new_code(phone_sign_in.policy.digits);
    let hour = SignedDuration::from_hours(1);
    let mut sent: Vec<Timestamp> = sent.into_iter().filter(|at| now < *at + hour).collect();
    sent.push(now);
    let otp = PhoneOtp {
        challenge: phone_sign_in
            .policy
            .issue(phone.clone(), purpose, hash_code(&code), now),
        sent,
    };
    if inner.store.put_phone_otp(&otp).await.is_err()
        || phone_sign_in.sender.send(&phone, &code).await.is_err()
    {
        return error_page(StatusCode::SERVICE_UNAVAILABLE, "Sign-in is unavailable.");
    }
    match &inner.config.login_page {
        Some(page) => Redirect::to(&with_pairs(
            page,
            &form.request,
            &[("otp", "sent"), ("phone", phone.as_str())],
        ))
        .into_response(),
        None => (
            no_store(),
            Html(code_page(&form.request, phone.as_str(), None)),
        )
            .into_response(),
    }
}

/// `POST /authorize` for a patient client: the phone and the code sent to it. The first
/// accepted code creates the phone's Login Account (`level1`), which reaches no patient record
/// until the client registry links its Person.
async fn phone_login<S: AuthStore>(
    inner: &Inner<S>,
    form: &LoginForm,
    request: Request,
) -> Response {
    let Some(phone_sign_in) = &inner.config.phone_sign_in else {
        return error_page(
            StatusCode::BAD_REQUEST,
            "This client does not sign in by phone.",
        );
    };
    let refused = |error: &str, message: &str| match &inner.config.login_page {
        Some(page) => Redirect::to(&with_pairs(
            page,
            &form.request,
            &[
                ("error", error),
                ("otp", "sent"),
                ("phone", form.phone.as_str()),
            ],
        ))
        .into_response(),
        None => (
            StatusCode::UNAUTHORIZED,
            no_store(),
            Html(code_page(&form.request, &form.phone, Some(message))),
        )
            .into_response(),
    };
    let Ok(phone) = PhoneNumber::parse(&form.phone, &phone_sign_in.country_code) else {
        return refused("refused", OTP_REFUSED);
    };
    let now = inner.clock.now();
    let otp = match inner.store.phone_otp(&phone).await {
        Ok(Some(otp)) => otp,
        Ok(None) => return refused("refused", OTP_REFUSED),
        Err(_) => return error_page(StatusCode::SERVICE_UNAVAILABLE, "Sign-in is unavailable."),
    };
    let before = otp.challenge.attempts;
    let matches = code_matches(form.code.trim(), &otp.challenge.code_hash);
    let mut tried = otp.clone();
    let outcome = tried.challenge.attempt(&phone_sign_in.policy, matches, now);
    match outcome {
        Err(OtpRefused::Expired | OtpRefused::Used | OtpRefused::Spent) => {
            return refused("expired", OTP_EXPIRED);
        }
        Err(OtpRefused::TooSoon { .. } | OtpRefused::TooMany { .. }) => {
            return refused("refused", OTP_REFUSED);
        }
        Ok(()) | Err(OtpRefused::Wrong { .. }) => {}
    }
    // Every try counts once: a concurrent try of the same challenge loses and is refused.
    match inner.store.attempt_phone_otp(before, &tried).await {
        Ok(true) => {}
        Ok(false) => return refused("refused", OTP_REFUSED),
        Err(_) => return error_page(StatusCode::SERVICE_UNAVAILABLE, "Sign-in is unavailable."),
    }
    if outcome.is_err() {
        return refused("refused", OTP_REFUSED);
    }
    let account = match inner.store.phone_account(&phone).await {
        Ok(Some(account)) => Ok(account),
        Ok(None) => {
            inner
                .store
                .create_phone_account(PhoneAccount {
                    id: AccountId(Uuid::now_v7()),
                    phone,
                    status: AccountStatus::Active,
                    verified_at: now,
                })
                .await
        }
        Err(error) => Err(error),
    };
    match account {
        Ok(account) if account.status == AccountStatus::Active => {
            issue_code(inner, request, account.id, None, now).await
        }
        Ok(_) => refused("refused", OTP_REFUSED),
        Err(_) => error_page(StatusCode::SERVICE_UNAVAILABLE, "Sign-in is unavailable."),
    }
}

// ---------------------------------------------------------------------------------------------
// Token endpoint

/// A token request (RFC 6749 § 4.1.3, § 6).
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct TokenParams {
    grant_type: Option<String>,
    code: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    code_verifier: Option<String>,
    refresh_token: Option<String>,
}

fn oauth_error(status: StatusCode, error: &str) -> Response {
    (status, no_store(), Json(json!({ "error": error }))).into_response()
}

impl<S: AuthStore> Inner<S> {
    /// Checks a DPoP proof for `method` on `uri`: signature, binding, freshness and replay.
    async fn check_proof(
        &self,
        proof: &str,
        method: &str,
        uri: &str,
        now: Timestamp,
    ) -> Result<crate::jose::DpopProof, ()> {
        let verified = verify_dpop(proof, method, uri).map_err(|_| ())?;
        let window = self.config.dpop_window.as_secs();
        let age = seconds(now) - verified.iat;
        if age > window || age < -window {
            return Err(());
        }
        let until = now.checked_add(self.config.dpop_window).unwrap_or(now);
        match self.store.remember_jti(&verified.jti, until, now).await {
            Ok(true) => Ok(verified),
            _ => Err(()),
        }
    }

    /// The token response for a live session.
    fn tokens(
        &self,
        session: &Session,
        auth_time: Timestamp,
        refresh: &str,
        nonce: Option<&str>,
        snapshot: Option<&PermissionSnapshot>,
        now: Timestamp,
    ) -> Value {
        // No token outlives the session's cap or the user's permission snapshot.
        let mut expires = self.config.session.access_expiry(auth_time, now);
        if let Some(snapshot) = snapshot {
            expires = expires.min(snapshot.not_after);
        }
        let mut claims = json!({
            "iss": self.config.issuer,
            "sub": session.account.to_string(),
            "client_id": session.client_id,
            "scope": session.scopes.to_string(),
            "iat": seconds(now),
            "exp": seconds(expires),
            "jti": Uuid::now_v7().to_string(),
            "sid": session.id.to_string(),
            "auth_time": seconds(auth_time),
            "amr": [if self.config.patient(&session.client_id) { "otp" } else { "pwd" }],
            "acr": self.config.acr,
        });
        if let Some(jkt) = &session.jkt {
            claims["cnf"] = json!({ "jkt": jkt });
        }
        if let (Some(snapshot), Some(object)) = (snapshot, claims.as_object_mut())
            && let Value::Object(extra) = token_claims(snapshot)
        {
            object.extend(extra);
        }
        let mut response = json!({
            "access_token": self.signer.sign(&claims),
            "token_type": if session.jkt.is_some() { "DPoP" } else { "Bearer" },
            "expires_in": seconds(expires) - seconds(now),
            "scope": session.scopes.to_string(),
            "refresh_token": refresh,
        });
        if let Some(snapshot) = snapshot {
            // The client warns from three days before this (offline-auth.md).
            response["snapshot_not_after"] = json!(seconds(snapshot.not_after));
        }
        if session.scopes.contains(&Scope::OpenId) {
            let mut id = json!({
                "iss": self.config.issuer,
                "sub": session.account.to_string(),
                "aud": session.client_id,
                "iat": seconds(now),
                "exp": seconds(expires),
                "auth_time": seconds(auth_time),
                "sid": session.id.to_string(),
                "acr": self.config.acr,
            });
            if let Some(nonce) = nonce {
                id["nonce"] = json!(nonce);
            }
            response["id_token"] = json!(self.signer.sign_id_token(&id));
        }
        response
    }

    /// The facility a sign-in acts for: the edge's own; at central the one the user named, or
    /// their only one (ADR 0056); none where the node keeps no snapshots.
    async fn sign_in_facility(&self, account: AccountId, named: &str) -> FacilityChoice {
        let Some(gate) = &self.config.snapshots else {
            return FacilityChoice::Chosen(None);
        };
        if let Some(facility) = &gate.facility {
            return FacilityChoice::Chosen(Some(facility.clone()));
        }
        let Ok(facilities) = self.store.facilities(account).await else {
            return FacilityChoice::Refused;
        };
        let named = named.trim();
        match facilities.as_slice() {
            _ if !named.is_empty() => {
                if facilities.iter().any(|f| f == named) {
                    FacilityChoice::Chosen(Some(named.to_owned()))
                } else {
                    FacilityChoice::Refused
                }
            }
            [] => FacilityChoice::Refused,
            [only] => FacilityChoice::Chosen(Some(only.clone())),
            _ => FacilityChoice::Ask,
        }
    }

    /// The account's snapshot for `facility`, when the node requires one: present, signed by
    /// central, for this account and facility, and unexpired at a clock that has not gone back
    /// past the floor. Each check advances the floor to the snapshot's issue time. `Ok(None)`
    /// when the node needs no snapshot.
    async fn current_snapshot(
        &self,
        account: AccountId,
        facility: Option<&str>,
        now: Timestamp,
    ) -> Result<Option<PermissionSnapshot>, ()> {
        let Some(gate) = &self.config.snapshots else {
            return Ok(None);
        };
        // An edge's sessions are for its own facility only.
        let facility = facility.ok_or(())?;
        if gate.facility.as_deref().is_some_and(|f| f != facility) {
            return Err(());
        }
        let floor = self.store.clock_floor().await.map_err(|_| ())?;
        let at = floor.check(now).map_err(|_| ())?;
        let jws = self
            .store
            .snapshot(account, facility)
            .await
            .map_err(|_| ())?
            .ok_or(())?;
        let snapshot = gate.verifier.verify(&jws).map_err(|_| ())?;
        if snapshot.account != account || snapshot.facility != facility {
            return Err(());
        }
        gate.policy.check(&snapshot, at).map_err(|_| ())?;
        self.store
            .set_clock_floor(floor.observe(snapshot.issued_at))
            .await
            .map_err(|_| ())?;
        Ok(Some(snapshot))
    }

    /// Whether the account is active: a phone account for a patient client, a staff account
    /// otherwise, so neither kind's id works with the other's clients.
    async fn account_active(&self, id: AccountId, client_id: &str) -> bool {
        if self.config.patient(client_id) {
            matches!(self.store.phone_account_by_id(id).await, Ok(Some(a)) if a.status == AccountStatus::Active)
        } else {
            matches!(self.store.account(id).await, Ok(Some(a)) if a.status == AccountStatus::Active)
        }
    }

    /// The snapshot a session's tokens are issued under: none for a patient client, whose
    /// patients have no roles; otherwise [`current_snapshot`](Self::current_snapshot).
    async fn session_snapshot(
        &self,
        client_id: &str,
        account: AccountId,
        facility: Option<&str>,
        now: Timestamp,
    ) -> Result<Option<PermissionSnapshot>, ()> {
        if self.config.patient(client_id) {
            return Ok(None);
        }
        self.current_snapshot(account, facility, now).await
    }
}

async fn token<S: AuthStore>(
    State(inner): State<Arc<Inner<S>>>,
    headers: HeaderMap,
    Form(params): Form<TokenParams>,
) -> Response {
    let now = inner.clock.now();
    let proof = match headers.get("dpop").map(|v| v.to_str()) {
        None => None,
        Some(Ok(proof)) => match inner
            .check_proof(proof, "POST", &inner.config.endpoint("/token"), now)
            .await
        {
            Ok(verified) => Some(verified),
            Err(()) => return oauth_error(StatusCode::BAD_REQUEST, "invalid_dpop_proof"),
        },
        Some(Err(_)) => return oauth_error(StatusCode::BAD_REQUEST, "invalid_dpop_proof"),
    };
    if inner.config.require_dpop && proof.is_none() {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_dpop_proof");
    }
    match params.grant_type.as_deref() {
        Some("authorization_code") => {
            authorization_code(&inner, &params, proof.map(|p| p.jkt), now).await
        }
        Some("refresh_token") => refresh(&inner, &params, proof.map(|p| p.jkt), now).await,
        _ => oauth_error(StatusCode::BAD_REQUEST, "unsupported_grant_type"),
    }
}

async fn authorization_code<S: AuthStore>(
    inner: &Inner<S>,
    params: &TokenParams,
    jkt: Option<String>,
    now: Timestamp,
) -> Response {
    let (Some(code), Some(verifier)) = (params.code.as_deref(), params.code_verifier.as_deref())
    else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let Ok(Some(stored)) = inner.store.take_code(&sha256_b64(code)).await else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    let verifier_ok = (43..=128).contains(&verifier.len())
        && verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
        && sha256_b64(verifier) == stored.code_challenge;
    if now >= stored.expires_at
        || params.client_id.as_deref() != Some(stored.client_id.as_str())
        || params.redirect_uri.as_deref() != Some(stored.redirect_uri.as_str())
        || !verifier_ok
        || !inner
            .account_active(stored.account, &stored.client_id)
            .await
    {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    }
    let Ok(snapshot) = inner
        .session_snapshot(
            &stored.client_id,
            stored.account,
            stored.facility.as_deref(),
            now,
        )
        .await
    else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    let refresh = random_token();
    let session = Session {
        id: Uuid::now_v7(),
        account: stored.account,
        client_id: stored.client_id,
        scopes: stored.scopes,
        state: SessionState::Live {
            auth_time: stored.auth_time,
        },
        facility: stored.facility,
        jkt,
        refresh_hash: sha256_b64(&refresh),
    };
    if inner.store.put_session(session.clone()).await.is_err() {
        return oauth_error(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable");
    }
    let body = inner.tokens(
        &session,
        stored.auth_time,
        &refresh,
        stored.nonce.as_deref(),
        snapshot.as_ref(),
        now,
    );
    (no_store(), Json(body)).into_response()
}

async fn refresh<S: AuthStore>(
    inner: &Inner<S>,
    params: &TokenParams,
    jkt: Option<String>,
    now: Timestamp,
) -> Response {
    let Some(token) = params.refresh_token.as_deref() else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let hash = sha256_b64(token);
    let Ok(Some((session, current))) = inner.store.session_by_refresh(&hash).await else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    if !current {
        // A rotated-out token came back: someone else holds it, so the session ends.
        let _ = inner.store.end_session(session.id).await;
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    }
    let SessionState::Live { auth_time } = session.state else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    if params.client_id.as_deref() != Some(session.client_id.as_str())
        || !inner.config.session.may_refresh(session.state, now)
        || session.jkt != jkt && session.jkt.is_some()
        || !inner
            .account_active(session.account, &session.client_id)
            .await
    {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    }
    // Permissions are never cached past their snapshot: an expired one ends the session.
    let Ok(snapshot) = inner
        .session_snapshot(
            &session.client_id,
            session.account,
            session.facility.as_deref(),
            now,
        )
        .await
    else {
        let _ = inner.store.end_session(session.id).await;
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    let next = random_token();
    match inner
        .store
        .rotate_refresh(session.id, &hash, sha256_b64(&next))
        .await
    {
        Ok(true) => {}
        _ => return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant"),
    }
    let mut body = inner.tokens(&session, auth_time, &next, None, snapshot.as_ref(), now);
    // A refresh does not re-authenticate, so it issues no new ID token.
    if let Some(object) = body.as_object_mut() {
        object.remove("id_token");
    }
    (no_store(), Json(body)).into_response()
}

/// A revocation request (RFC 7009).
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct RevokeParams {
    token: Option<String>,
}

/// Logout: revoking either token ends the session, and with it every access token issued in it.
/// The answer is always 200, so it reveals nothing about the token.
async fn revoke<S: AuthStore>(
    State(inner): State<Arc<Inner<S>>>,
    Form(params): Form<RevokeParams>,
) -> Response {
    if let Some(token) = params.token.as_deref() {
        let session = match inner.store.session_by_refresh(&sha256_b64(token)).await {
            Ok(Some((session, _))) => Some(session.id),
            _ => inner
                .signer
                .verify(token)
                .ok()
                .and_then(|claims| claims["sid"].as_str().and_then(|s| s.parse().ok())),
        };
        if let Some(id) = session {
            let _ = inner.store.end_session(id).await;
        }
    }
    (StatusCode::OK, no_store()).into_response()
}

// ---------------------------------------------------------------------------------------------
// Resource side

/// Who an access token authorizes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    /// The account.
    pub account: AccountId,
    /// The client.
    pub client_id: String,
    /// The scopes granted.
    pub scopes: Scopes,
    /// The session.
    pub session: Uuid,
    /// The facility the session acts for, when the node keeps snapshots (ADR 0056).
    pub facility: Option<String>,
    /// The version of the permission snapshot the token was issued under.
    pub snapshot_version: Option<u64>,
    /// Whether it is a patient's (a patient client's phone account), not staff.
    pub patient: bool,
}

/// Why access was refused; maps to 401 with a `WWW-Authenticate` challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AccessError {
    /// No credentials.
    #[error("no access token")]
    Missing,
    /// Bad, expired or foreign token, or its session has ended.
    #[error("invalid access token")]
    InvalidToken,
    /// A DPoP-bound token without a valid proof of possession.
    #[error("invalid DPoP proof")]
    InvalidProof,
}

impl IntoResponse for AccessError {
    fn into_response(self) -> Response {
        let challenge = match self {
            Self::Missing => r#"DPoP algs="ES256", Bearer"#,
            Self::InvalidToken => r#"DPoP error="invalid_token", Bearer error="invalid_token""#,
            Self::InvalidProof => r#"DPoP error="invalid_dpop_proof""#,
        };
        (
            StatusCode::UNAUTHORIZED,
            [(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_static(challenge),
            )],
        )
            .into_response()
    }
}

/// Checks access tokens on resource requests (RFC 6750, RFC 9449 § 7).
pub struct AccessVerifier<S> {
    inner: Arc<Inner<S>>,
}

impl<S> Clone for AccessVerifier<S> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<S: AuthStore> AccessVerifier<S> {
    /// Verifies the `Authorization` (and `DPoP`) headers of a `method` request to the absolute
    /// `uri`. The session must still be live, so logout takes effect at once.
    pub async fn verify(
        &self,
        headers: &HeaderMap,
        method: &str,
        uri: &str,
    ) -> Result<Principal, AccessError> {
        let inner = &self.inner;
        let now = inner.clock.now();
        let authorization = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AccessError::Missing)?;
        let (scheme, token) = authorization
            .split_once(' ')
            .ok_or(AccessError::InvalidToken)?;
        let claims = inner
            .signer
            .verify(token)
            .map_err(|_| AccessError::InvalidToken)?;
        let exp = claims["exp"].as_i64().unwrap_or(0);
        if claims["iss"] != inner.config.issuer.as_str() || seconds(now) >= exp {
            return Err(AccessError::InvalidToken);
        }
        match claims["cnf"]["jkt"].as_str() {
            Some(jkt) => {
                if !scheme.eq_ignore_ascii_case("DPoP") {
                    return Err(AccessError::InvalidToken);
                }
                let proof = headers
                    .get("dpop")
                    .and_then(|v| v.to_str().ok())
                    .ok_or(AccessError::InvalidProof)?;
                let verified = inner
                    .check_proof(proof, method, uri, now)
                    .await
                    .map_err(|()| AccessError::InvalidProof)?;
                if verified.jkt != jkt
                    || verified.ath.as_deref() != Some(sha256_b64(token).as_str())
                {
                    return Err(AccessError::InvalidProof);
                }
            }
            None if scheme.eq_ignore_ascii_case("Bearer") && !inner.config.require_dpop => {}
            None => return Err(AccessError::InvalidToken),
        }
        let session: Uuid = claims["sid"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .ok_or(AccessError::InvalidToken)?;
        let live = matches!(
            inner.store.session(session).await,
            Ok(Some(s)) if inner.config.session.may_refresh(s.state, now)
        );
        let account = claims["sub"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .map(AccountId);
        let scopes = Scopes::parse(claims["scope"].as_str().unwrap_or_default())
            .map_err(|_| AccessError::InvalidToken)?;
        match (live, account) {
            (true, Some(account)) => Ok(Principal {
                account,
                patient: inner
                    .config
                    .patient(claims["client_id"].as_str().unwrap_or_default()),
                client_id: claims["client_id"].as_str().unwrap_or_default().to_owned(),
                scopes,
                session,
                facility: claims["facility"].as_str().map(str::to_owned),
                snapshot_version: claims["snapshot_version"].as_u64(),
            }),
            _ => Err(AccessError::InvalidToken),
        }
    }
}

impl<S: AuthStore> AccessVerifier<S> {
    /// The verified, current permission snapshot of `account` at `facility`, when the node
    /// keeps snapshots: what each request's permissions come from (ADR 0056).
    pub(crate) async fn current_snapshot(
        &self,
        account: AccountId,
        facility: &str,
    ) -> Result<PermissionSnapshot, AccessError> {
        let now = self.inner.clock.now();
        match self
            .inner
            .current_snapshot(account, Some(facility), now)
            .await
        {
            Ok(Some(snapshot)) => Ok(snapshot),
            _ => Err(AccessError::InvalidToken),
        }
    }
}

impl<S> AccessVerifier<S> {
    /// The server's store.
    pub(crate) fn store(&self) -> &S {
        &self.inner.store
    }

    /// The server's clock.
    pub(crate) fn now(&self) -> Timestamp {
        self.inner.clock.now()
    }

    /// The issuer's origin (`scheme://host[:port]`), where resource routes are served.
    pub(crate) fn origin(&self) -> String {
        self.inner
            .config
            .issuer
            .split('/')
            .take(3)
            .collect::<Vec<_>>()
            .join("/")
    }
}

async fn session_info<S: AuthStore>(
    State(verifier): State<AccessVerifier<S>>,
    headers: HeaderMap,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Result<Json<Value>, AccessError> {
    // The proof's `htu` is the absolute URL; this route sits on the issuer's origin.
    let origin = verifier.origin();
    let principal = verifier
        .verify(&headers, "GET", &format!("{origin}{}", uri.path()))
        .await?;
    Ok(Json(json!({
        "account": principal.account.to_string(),
        "client_id": principal.client_id,
        "scope": principal.scopes.to_string(),
        "session": principal.session.to_string(),
        "facility": principal.facility,
    })))
}
