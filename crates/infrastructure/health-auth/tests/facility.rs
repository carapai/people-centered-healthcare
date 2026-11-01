//! Choosing the facility at sign-in at central (ADR 0056): the session acts for one facility,
//! named by the user when they hold snapshots for several, and the token carries it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use health_auth::{
    AuthConfig, AuthServer, Client, Credentials, MemoryStore, SnapshotGate, SnapshotSigner,
    SnapshotVerifier, TokenSigner,
};
use health_identity::snapshot::{
    PermissionSnapshot, RoleAssignment, RolePermissions, SnapshotPolicy,
};
use health_identity::{AccountId, AccountStatus, Lockout, LoginAccount};
use health_time::{FixedClock, SignedDuration, Timestamp};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

const ISSUER: &str = "http://central.test/auth";
const CLIENT: &str = "health-client";
const REDIRECT: &str = "http://central.test/callback";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const PASSWORD: &str = "correct horse battery";

struct Central {
    app: Router,
}

fn roles() -> RolePermissions {
    RolePermissions(BTreeMap::from([(
        "clinician".into(),
        BTreeSet::from(["*.rs".into()]),
    )]))
}

/// Central with `nakato` holding snapshots at `facilities` and `okello` at KAM013 only.
fn central(facilities: &[&str]) -> Central {
    let issued = Timestamp::from_second(1_800_000_000).expect("time");
    let clock = Arc::new(FixedClock::new(
        issued
            .checked_add(SignedDuration::from_hours(1))
            .expect("time"),
    ));
    let credentials = Credentials::new(Vec::new()).expect("credentials");
    let store = MemoryStore::default();
    let signer = SnapshotSigner::generate();
    let provision = |username: &str, facilities: &[&str]| {
        let account = AccountId(Uuid::now_v7());
        store.put_account(LoginAccount {
            id: account,
            username: username.into(),
            credential: credentials.hash(PASSWORD).expect("hash"),
            status: AccountStatus::Active,
            lockout: Lockout::default(),
        });
        for (version, facility) in facilities.iter().enumerate() {
            let snapshot = PermissionSnapshot {
                account,
                facility: (*facility).into(),
                roles: vec![RoleAssignment {
                    role: "clinician".into(),
                    locations: vec![],
                }],
                programs: vec![],
                issued_at: issued,
                not_after: SnapshotPolicy::default().not_after(issued),
                snapshot_version: version as u64 + 1,
            };
            store.put_snapshot(
                account,
                facility,
                signer.sign(&snapshot, &roles()).expect("sign"),
            );
        }
    };
    provision("nakato", facilities);
    provision("okello", &["KAM013"]);
    let mut config = AuthConfig::new(ISSUER, "central", false);
    config.clients.push(Client {
        id: CLIENT.into(),
        redirect_uris: vec![REDIRECT.into()],
        patient: false,
    });
    config.login_page = Some("/sign-in".into());
    config.snapshots = Some(SnapshotGate {
        facility: None,
        verifier: SnapshotVerifier::new(&[signer.public()]).expect("verifier"),
        policy: SnapshotPolicy::default(),
    });
    let server = AuthServer::new(config, TokenSigner::generate(), credentials, store, clock);
    Central {
        app: Router::new()
            .nest("/auth", server.router())
            .nest("/api", server.session_route()),
    }
}

impl Central {
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

    /// Signs in as `username` naming `facility` ("" for none): the redirect's location.
    async fn sign_in(&self, username: &str, facility: &str) -> String {
        let (_, _, location) = self
            .post(
                "/auth/authorize",
                &[
                    ("response_type", "code"),
                    ("client_id", CLIENT),
                    ("redirect_uri", REDIRECT),
                    ("scope", "openid"),
                    ("state", "s"),
                    ("code_challenge", CHALLENGE),
                    ("code_challenge_method", "S256"),
                    ("username", username),
                    ("password", PASSWORD),
                    ("facility", facility),
                ],
            )
            .await;
        location.expect("redirect")
    }

    /// The facility `/api/session` reports after redeeming the code in `location`.
    async fn facility_of(&self, location: &str) -> Value {
        let code = form_urlencoded::parse(location.split_once('?').expect("query").1.as_bytes())
            .find(|(k, _)| k == "code")
            .expect("code")
            .1
            .into_owned();
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
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let tokens: Value = serde_json::from_str(&body).expect("json");
        let (status, body, _) = self
            .send(
                Request::get("/api/session")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", tokens["access_token"].as_str().expect("token")),
                    )
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        serde_json::from_str::<Value>(&body).expect("json")["facility"].clone()
    }
}

#[tokio::test]
async fn a_user_at_several_facilities_names_one_at_sign_in() {
    let central = central(&["KAM013", "WAK002"]);
    // Naming none sends the browser back to the sign-in page to choose.
    let location = central.sign_in("nakato", "").await;
    assert!(
        location.starts_with("/sign-in?") && location.ends_with("&error=facility"),
        "{location}"
    );
    // A facility the user holds no snapshot for is refused like a wrong password.
    let location = central.sign_in("nakato", "GUL001").await;
    assert!(location.ends_with("&error=refused"), "{location}");
    // The facility named is the session's, and the token carries it.
    let location = central.sign_in("nakato", "WAK002").await;
    assert!(location.starts_with(REDIRECT), "{location}");
    assert_eq!(central.facility_of(&location).await, "WAK002");
}

#[tokio::test]
async fn a_user_at_one_facility_need_not_name_it() {
    let central = central(&[]);
    let location = central.sign_in("okello", "").await;
    assert_eq!(central.facility_of(&location).await, "KAM013");
    // Without any snapshot there is nothing to act for.
    let location = central.sign_in("nakato", "").await;
    assert!(location.ends_with("&error=refused"), "{location}");
}
