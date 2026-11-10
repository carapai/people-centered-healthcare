//! Offline user and role administration on an edge (offline-auth.md § Offline user and role
//! administration; ADR 0011): a facility admin grants a locally grantable role for at most 7 days
//! and revokes local access; a role carrying a never-offline permission is refused even when the
//! country lists it; someone without `access.administer-local` can do neither.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use health_auth::{
    AuthConfig, AuthServer, AuthStore, Client, Credentials, LocalAdmin, LocalConfig, MemoryStore,
    SnapshotGate, SnapshotSigner, SnapshotVerifier, TokenSigner,
};
use health_identity::snapshot::{
    ADMINISTER_LOCAL, PermissionSnapshot, RoleAssignment, RolePermissions, SnapshotPolicy,
};
use health_identity::{AccountId, AccountStatus, Lockout, LoginAccount};
use health_time::{FixedClock, Timestamp};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

const ISSUER: &str = "http://edge.test/auth";
const CLIENT: &str = "health-client";
const REDIRECT: &str = "http://edge.test/callback";
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const FACILITY: &str = "KAM013";

fn mappings() -> RolePermissions {
    let set = |p: &[&str]| p.iter().map(|s| (*s).to_owned()).collect::<BTreeSet<_>>();
    RolePermissions(BTreeMap::from([
        ("admin".to_owned(), set(&[ADMINISTER_LOCAL])),
        ("clerk".to_owned(), set(&["Patient.cru"])),
        ("nurse".to_owned(), set(&["Observation.cru"])),
        (
            "in-charge".to_owned(),
            set(&["Patient.cru", "break-glass.alerts"]),
        ),
    ]))
}

struct Edge {
    app: Router,
}

impl Edge {
    async fn send(&self, request: Request<Body>) -> (StatusCode, Value, Option<String>) {
        let response = self.app.clone().oneshot(request).await.expect("response");
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            location,
        )
    }

    async fn post(
        &self,
        path: &str,
        fields: &[(&str, &str)],
        token: Option<&str>,
    ) -> (StatusCode, Value, Option<String>) {
        let body = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(fields)
            .finish();
        let mut request =
            Request::post(path).header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        self.send(request.body(Body::from(body)).expect("request"))
            .await
    }

    async fn get(&self, path: &str, token: &str) -> (StatusCode, Value) {
        let request = Request::get(path)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .expect("request");
        let (status, body, _) = self.send(request).await;
        (status, body)
    }

    /// Signs in as `username`: the access token, or `None` when refused.
    async fn sign_in(&self, username: &str) -> Option<String> {
        self.sign_in_with(username, "correct horse battery").await
    }

    /// Signs in as `username` with `password`.
    async fn sign_in_with(&self, username: &str, password: &str) -> Option<String> {
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
                    ("username", username),
                    ("password", password),
                ],
                None,
            )
            .await;
        if status != StatusCode::SEE_OTHER {
            return None;
        }
        let location = location.expect("location");
        let code = form_urlencoded::parse(location.split_once('?').expect("query").1.as_bytes())
            .find(|(k, _)| k == "code")
            .map(|(_, v)| v.into_owned())?;
        let (status, body, _) = self
            .post(
                "/auth/token",
                &[
                    ("grant_type", "authorization_code"),
                    ("code", &code),
                    ("redirect_uri", REDIRECT),
                    ("client_id", CLIENT),
                    ("code_verifier", VERIFIER),
                ],
                None,
            )
            .await;
        (status == StatusCode::OK).then(|| body["access_token"].as_str().expect("token").to_owned())
    }
}

async fn edge() -> Edge {
    let now = Timestamp::now();
    let clock = Arc::new(FixedClock::new(now));
    let credentials = Credentials::new(Vec::new()).expect("credentials");
    let store = MemoryStore::default();
    let central = SnapshotSigner::generate();
    for (username, role) in [("okello", "admin"), ("nakato", "clerk"), ("auma", "clerk")] {
        let account = AccountId(Uuid::now_v7());
        store.put_account(LoginAccount {
            id: account,
            username: username.into(),
            credential: credentials
                .credential(account, "correct horse battery")
                .expect("hash"),
            status: AccountStatus::Active,
            lockout: Lockout::default(),
        });
        let snapshot = PermissionSnapshot {
            account,
            facility: FACILITY.into(),
            roles: vec![RoleAssignment {
                role: role.into(),
                locations: Vec::new(),
            }],
            programs: Vec::new(),
            issued_at: now,
            not_after: SnapshotPolicy::default().not_after(now),
            snapshot_version: 1,
        };
        store.put_snapshot(
            account,
            FACILITY,
            central.sign(&snapshot, &mappings()).expect("sign"),
        );
    }
    // The country lists "in-charge" too, but it carries a break-glass permission.
    store
        .put_local_config(&LocalConfig {
            roles: mappings(),
            grantable: vec!["nurse".into(), "in-charge".into()],
            labels: Vec::new(),
        })
        .await
        .expect("config");
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
    let server = AuthServer::new(config, TokenSigner::generate(), credentials, store, clock);
    let signer = Arc::new(|claims: &Value| Ok(format!("signed:{claims}")));
    let admin = LocalAdmin::new(server.verifier(), signer, "edge");
    Edge {
        app: Router::new()
            .nest("/auth", server.router())
            .nest("/api", admin.router()),
    }
}

#[tokio::test]
async fn a_facility_admin_grants_locally_grantable_roles_and_revokes_access() {
    let edge = edge().await;
    let admin = edge.sign_in("okello").await.expect("admin signs in");
    let clerk = edge.sign_in("nakato").await.expect("clerk signs in");

    // Not an admin: nothing.
    let (status, _, _) = edge
        .post(
            "/api/access/grants",
            &[("username", "auma"), ("role", "nurse"), ("days", "3")],
            Some(&clerk),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        edge.get("/api/access/roles", &clerk).await.0,
        StatusCode::FORBIDDEN
    );

    // What may be granted: on the list, and carrying no never-offline permission.
    let (status, roles) = edge.get("/api/access/roles", &admin).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(roles, serde_json::json!(["nurse"]));

    // A grantable role, for at most 7 days.
    let (status, grant, _) = edge
        .post(
            "/api/access/grants",
            &[("username", "nakato"), ("role", "nurse"), ("days", "3")],
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{grant}");
    assert_eq!(grant["role"], "nurse");
    for (role, days, why) in [
        ("nurse", "8", "at most 7 days"),
        ("in-charge", "1", "never be granted offline"),
        ("admin", "1", "never be granted offline"),
        ("clerk", "1", "not locally grantable"),
    ] {
        let (status, body, _) = edge
            .post(
                "/api/access/grants",
                &[("username", "nakato"), ("role", role), ("days", days)],
                Some(&admin),
            )
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{role}");
        assert!(
            body["detail"].as_str().unwrap_or_default().contains(why),
            "{role}: {body}"
        );
    }
    let (status, grants) = edge.get("/api/access/grants", &admin).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(grants.as_array().map(Vec::len), Some(1));
    assert_eq!(grants[0]["username"], "nakato");

    // Revoked: the clerk's next request is refused, and they cannot sign in here.
    let (status, _, _) = edge
        .post(
            "/api/access/revocations",
            &[("username", "nakato")],
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = edge.get("/api/access/grants", &clerk).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(edge.sign_in("nakato").await, None);
    // The admin cannot lock themselves out.
    let (status, _, _) = edge
        .post(
            "/api/access/revocations",
            &[("username", "okello")],
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, revoked) = edge.get("/api/access/revocations", &admin).await;
    assert_eq!(revoked.as_array().map(Vec::len), Some(1));
    assert_eq!(revoked[0]["username"], "nakato");
}

/// New staff offline (offline-auth.md § Offline user and role administration): a provisional
/// account whose role is a local grant, whose user must set their own password first.
#[tokio::test]
async fn a_provisional_account_signs_in_once_its_user_sets_their_password() {
    let edge = edge().await;
    let admin = edge.sign_in("okello").await.expect("admin signs in");
    let create = |username: &'static str, password: &'static str, role: &'static str| {
        let admin = admin.clone();
        let edge = &edge;
        async move {
            edge.post(
                "/api/access/accounts",
                &[
                    ("username", username),
                    ("password", password),
                    ("role", role),
                    ("days", "5"),
                ],
                Some(&admin),
            )
            .await
        }
    };
    let (status, created, _) = create("mukasa", "tumbling otter quarry 7919", "nurse").await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["role"], "nurse");
    assert_eq!(
        create("mukasa", "tumbling otter quarry 7919", "nurse")
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        create("waswa", "tumbling otter quarry 7919", "in-charge")
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        create("waswa", "password123", "nurse").await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // The admin's password works once only to set their own.
    let (status, body, _) = edge
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
                ("username", "mukasa"),
                ("password", "tumbling otter quarry 7919"),
            ],
            None,
        )
        .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("change_password"))
    );
    let change = |old: &'static str, new: &'static str| {
        let edge = &edge;
        async move {
            edge.post(
                "/auth/password",
                &[
                    ("username", "mukasa"),
                    ("password", old),
                    ("new_password", new),
                ],
                None,
            )
            .await
            .0
        }
    };
    assert_eq!(
        change("wrong old password", "bright kettle river 5521").await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        change("tumbling otter quarry 7919", "password123").await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        change("tumbling otter quarry 7919", "bright kettle river 5521").await,
        StatusCode::NO_CONTENT
    );

    // Now signed in, with nothing but the granted role: not an admin.
    let token = edge
        .sign_in_with("mukasa", "bright kettle river 5521")
        .await
        .expect("signs in with their own password");
    assert_eq!(
        edge.get("/api/access/grants", &token).await.0,
        StatusCode::FORBIDDEN
    );
    let (_, grants) = edge.get("/api/access/grants", &admin).await;
    assert!(
        grants
            .as_array()
            .expect("grants")
            .iter()
            .any(|g| g["role"] == "nurse")
    );
}
