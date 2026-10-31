//! The authorization server's HTTP endpoints and the resource side's access check.

use std::sync::Arc;

use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use health_identity::{
    AccountId, AccountStatus, LockoutPolicy, Scope, Scopes, SessionPolicy, SessionState,
};
use health_time::{Clock, SignedDuration, Timestamp};
use rand_core::RngCore;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::Credentials;
use crate::jose::{TokenSigner, b64, sha256_b64, verify_dpop};
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
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.issuer)
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
            .route("/token", post(token::<S>))
            .route("/revoke", post(revoke::<S>))
            .with_state(Arc::clone(&self.inner))
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
    let hidden: String = [
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
    .collect();
    let alert = error
        .map(|e| format!("<p role=\"alert\">{}</p>", escape(e)))
        .unwrap_or_default();
    format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Sign in</title><main><h1>Sign in</h1>{alert}\
         <form method=\"post\" action=\"authorize\">{hidden}\
         <label for=\"username\">Username</label><input id=\"username\" name=\"username\" autocomplete=\"username\" required>\
         <label for=\"password\">Password</label><input id=\"password\" name=\"password\" type=\"password\" autocomplete=\"current-password\" required>\
         <button type=\"submit\">Sign in</button></form></main></html>"
    )
}

async fn authorize_form<S: AuthStore>(
    State(inner): State<Arc<Inner<S>>>,
    Query(params): Query<AuthorizeParams>,
) -> Response {
    match validate(&inner, &params) {
        Ok(_) => (no_store(), Html(login_page(&params, None))).into_response(),
        Err(response) => *response,
    }
}

/// One message for every refusal, so a response does not reveal whether an account exists or
/// is locked.
const LOGIN_REFUSED: &str = "The username or password is incorrect, or the account is locked.";

async fn authorize_login<S: AuthStore>(
    State(inner): State<Arc<Inner<S>>>,
    Form(form): Form<LoginForm>,
) -> Response {
    let request = match validate(&inner, &form.request) {
        Ok(request) => request,
        Err(response) => return *response,
    };
    let refused = || {
        (
            StatusCode::UNAUTHORIZED,
            no_store(),
            Html(login_page(&form.request, Some(LOGIN_REFUSED))),
        )
            .into_response()
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
    // On an edge the password alone is not enough: the user needs a live snapshot here. The
    // refusal looks like any other.
    if inner.current_snapshot(account.id, now).await.is_err() {
        return refused();
    }
    let code = random_token();
    let stored = AuthorizationCode {
        client_id: request.client_id,
        redirect_uri: request.redirect_uri.clone(),
        code_challenge: request.code_challenge,
        scopes: request.scopes,
        nonce: request.nonce,
        account: account.id,
        auth_time: now,
        expires_at: now.checked_add(inner.config.code_lifetime).unwrap_or(now),
    };
    let saved = async {
        inner
            .store
            .set_lockout(account.id, account.lockout.reset())
            .await?;
        inner.store.put_code(sha256_b64(&code), stored).await
    };
    match saved.await {
        Ok(()) => redirect_with(
            &inner.config,
            &request.redirect_uri,
            &[("code", &code)],
            request.state.as_deref(),
        ),
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
            "amr": ["pwd"],
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

    /// The account's snapshot for this node's facility, when the node requires one: present,
    /// signed by central, for this account and facility, and unexpired at a clock that has not
    /// gone back past the floor. Each check advances the floor to the snapshot's issue time.
    /// `Ok(None)` when the node needs no snapshot (central).
    async fn current_snapshot(
        &self,
        account: AccountId,
        now: Timestamp,
    ) -> Result<Option<PermissionSnapshot>, ()> {
        let Some(gate) = &self.config.snapshots else {
            return Ok(None);
        };
        let floor = self.store.clock_floor().await.map_err(|_| ())?;
        let at = floor.check(now).map_err(|_| ())?;
        let jws = self
            .store
            .snapshot(account, &gate.facility)
            .await
            .map_err(|_| ())?
            .ok_or(())?;
        let snapshot = gate.verifier.verify(&jws).map_err(|_| ())?;
        if snapshot.account != account || snapshot.facility != gate.facility {
            return Err(());
        }
        gate.policy.check(&snapshot, at).map_err(|_| ())?;
        self.store
            .set_clock_floor(floor.observe(snapshot.issued_at))
            .await
            .map_err(|_| ())?;
        Ok(Some(snapshot))
    }

    async fn account_active(&self, id: AccountId) -> bool {
        matches!(self.store.account(id).await, Ok(Some(a)) if a.status == AccountStatus::Active)
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
        || !inner.account_active(stored.account).await
    {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    }
    let Ok(snapshot) = inner.current_snapshot(stored.account, now).await else {
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
        || !inner.account_active(session.account).await
    {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    }
    // Permissions are never cached past their snapshot: an expired one ends the session.
    let Ok(snapshot) = inner.current_snapshot(session.account, now).await else {
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
                client_id: claims["client_id"].as_str().unwrap_or_default().to_owned(),
                scopes,
                session,
            }),
            _ => Err(AccessError::InvalidToken),
        }
    }
}
