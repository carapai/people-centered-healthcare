//! Device enrollment and the device gate (lan-security.md § Device enrollment; ADR 0009; #74),
//! over the memory store and over SurrealDB: an unenrolled device cannot call the API, approval
//! needs the code the device shows and expires after 10 minutes, and a revoked device is refused.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use health_auth::jose::{EcJwk, sign};
use health_auth::{
    AuthStore, DeviceConfig, DeviceGate, DeviceStore, MemoryStore, SurrealAuthStore,
};
use health_identity::SessionPolicy;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{Clock, FixedClock, SignedDuration, SystemClock};
use http_body_util::BodyExt;
use p256::ecdsa::SigningKey;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

const ORIGIN: &str = "https://ug-kla-0001.local";

struct Edge {
    app: Router,
    clock: Arc<FixedClock>,
}

fn edge<S: AuthStore + DeviceStore + 'static>(store: S) -> Edge {
    let clock = Arc::new(FixedClock::new(
        "2026-11-02T09:00:00Z".parse().expect("now"),
    ));
    let gate = DeviceGate::new(
        store,
        DeviceConfig::new(vec![format!("{ORIGIN}/")], "ug-kla-0001"),
        Arc::clone(&clock) as Arc<dyn Clock>,
    );
    let app = gate
        .enrollment_router()
        .merge(gate.admin_router())
        .merge(gate.device_router());
    Edge { app, clock }
}

async fn surreal() -> SurrealAuthStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "auth".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealAuthStore::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        SessionPolicy::default().hard_cap,
    )
}

fn proof_at(key: &SigningKey, method: &str, uri: &str, iat: i64) -> String {
    let claims =
        json!({ "jti": Uuid::now_v7().to_string(), "htm": method, "htu": uri, "iat": iat });
    sign(
        key,
        &json!({ "typ": "dpop+jwt", "alg": "ES256", "jwk": EcJwk::of(key.verifying_key()) }),
        &claims,
    )
}

impl Edge {
    fn now(&self) -> i64 {
        self.clock.now().as_second()
    }

    fn proof(&self, key: &SigningKey, method: &str, path: &str) -> String {
        proof_at(key, method, &format!("{ORIGIN}{path}"), self.now())
    }

    async fn send(&self, request: Request<Body>) -> (StatusCode, Value) {
        let response = self.app.clone().oneshot(request).await.expect("response");
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
        )
    }

    async fn call(
        &self,
        method: &str,
        path: &str,
        proof: Option<String>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(proof) = proof {
            request = request.header("dpop", proof);
        }
        let body = match body {
            Some(body) => {
                request = request.header("content-type", "application/json");
                Body::from(body.to_string())
            }
            None => Body::empty(),
        };
        self.send(request.body(body).expect("request")).await
    }

    /// The device calls `path` with a fresh proof.
    async fn device(&self, key: &SigningKey, method: &str, path: &str) -> (StatusCode, Value) {
        let proof = self.proof(key, method, path);
        self.call(method, path, Some(proof), None).await
    }

    async fn enroll(&self, key: &SigningKey) -> (StatusCode, Value) {
        let proof = self.proof(key, "POST", "/api/enrollment");
        self.call(
            "POST",
            "/api/enrollment",
            Some(proof),
            Some(json!({ "label": "Chrome on Windows" })),
        )
        .await
    }

    async fn approve(&self, code: &str, name: &str) -> (StatusCode, Value) {
        self.call(
            "POST",
            "/api/admin/enrollments/approve",
            None,
            Some(json!({ "code": code, "name": name, "location": "Ward 3" })),
        )
        .await
    }
}

fn key() -> SigningKey {
    SigningKey::random(&mut rand_core::OsRng)
}

async fn enrollment_and_revocation(edge: Edge) {
    let tablet = key();

    // Unenrolled: no proof is 401, a proof by an unknown key is 403.
    let (status, _) = edge.call("GET", "/api/device", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, body) = edge.device(&tablet, "GET", "/api/device").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "device-not-enrolled");
    let (status, _) = edge.device(&tablet, "GET", "/api/enrollment").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A request needs a label and a proof.
    let proof = edge.proof(&tablet, "POST", "/api/enrollment");
    let (status, _) = edge
        .call("POST", "/api/enrollment", Some(proof), Some(json!({})))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = edge
        .call(
            "POST",
            "/api/enrollment",
            None,
            Some(json!({ "label": "x" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, request) = edge.enroll(&tablet).await;
    assert_eq!(status, StatusCode::CREATED, "{request}");
    let code = request["code"].as_str().expect("code").to_owned();
    assert_eq!(code.len(), 6);
    let (status, body) = edge.device(&tablet, "GET", "/api/enrollment").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "pending");
    assert_eq!(body["code"], code.as_str());

    // The admin sees the request without its code, and must type the one the device shows.
    let (_, pending) = edge.call("GET", "/api/admin/enrollments", None, None).await;
    assert_eq!(pending.as_array().map(Vec::len), Some(1));
    assert!(pending[0].get("code").is_none());
    assert_eq!(pending[0]["label"], "Chrome on Windows");
    let wrong = if code == "000000" { "000001" } else { "000000" };
    let (status, _) = edge.approve(wrong, "Ward 3 tablet").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = edge.approve(&code, " ").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    edge.clock.advance(SignedDuration::from_mins(9));
    let (status, device) = edge.approve(&code, "Ward 3 tablet").await;
    assert_eq!(status, StatusCode::CREATED, "{device}");
    assert_eq!(device["enrolled_by"], "edge-console");
    let (status, _) = edge.approve(&code, "Ward 3 tablet").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a code is used once");

    // Enrolled: the gate lets it through, once per proof, on our origin only.
    let (status, body) = edge.device(&tablet, "GET", "/api/device").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Ward 3 tablet");
    assert_eq!(body["location"], "Ward 3");
    let proof = edge.proof(&tablet, "GET", "/api/device");
    let (status, _) = edge
        .call("GET", "/api/device", Some(proof.clone()), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = edge.call("GET", "/api/device", Some(proof), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a replayed proof");
    let foreign = proof_at(
        &tablet,
        "GET",
        "https://elsewhere.local/api/device",
        edge.now(),
    );
    let (status, _) = edge.call("GET", "/api/device", Some(foreign), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "another origin");
    let stale = proof_at(
        &tablet,
        "GET",
        &format!("{ORIGIN}/api/device"),
        edge.now() - 301,
    );
    let (status, _) = edge.call("GET", "/api/device", Some(stale), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a stale proof");
    let (status, body) = edge.enroll(&tablet).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // Revoked: its next request is refused, and it cannot ask again with the same key.
    let id = device["id"].as_str().expect("id");
    let (status, revoked) = edge
        .call(
            "POST",
            &format!("/api/admin/devices/{id}/revoke"),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(revoked["status"], "revoked");
    assert_eq!(revoked["revoked_by"], "edge-console");
    let (status, body) = edge.device(&tablet, "GET", "/api/device").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "device-revoked");
    let (status, body) = edge.enroll(&tablet).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "device-revoked");
    let (_, body) = edge.device(&tablet, "GET", "/api/enrollment").await;
    assert_eq!(body["status"], "revoked");
    let (status, _) = edge
        .call(
            "POST",
            &format!("/api/admin/devices/{id}/revoke"),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, devices) = edge.call("GET", "/api/admin/devices", None, None).await;
    assert_eq!(devices.as_array().map(Vec::len), Some(1));
    assert_eq!(devices[0]["status"], "revoked");

    // Pending requests expire after 10 minutes.
    let laptop = key();
    let (_, request) = edge.enroll(&laptop).await;
    let code = request["code"].as_str().expect("code").to_owned();
    edge.clock.advance(SignedDuration::from_mins(10));
    let (status, _) = edge.approve(&code, "Ward 3 laptop").await;
    assert_eq!(status, StatusCode::GONE);
    let (_, pending) = edge.call("GET", "/api/admin/enrollments", None, None).await;
    assert_eq!(pending.as_array().map(Vec::len), Some(0));
    let (status, _) = edge.device(&laptop, "GET", "/api/enrollment").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Asking again gives a fresh request.
    let (status, _) = edge.enroll(&laptop).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn enrollment_and_revocation_in_memory() {
    enrollment_and_revocation(edge(MemoryStore::default())).await;
}

#[tokio::test]
async fn enrollment_and_revocation_in_surrealdb() {
    enrollment_and_revocation(edge(surreal().await)).await;
}

#[tokio::test]
async fn enrollment_is_rate_limited_per_address() {
    let edge = edge(MemoryStore::default());
    let from = |address: &str| {
        let key = key();
        let proof = edge.proof(&key, "POST", "/api/enrollment");
        let mut request = Request::post("/api/enrollment")
            .header("dpop", proof)
            .header("content-type", "application/json")
            .body(Body::from(json!({ "label": "x" }).to_string()))
            .expect("request");
        let address: SocketAddr = address.parse().expect("address");
        request.extensions_mut().insert(ConnectInfo(address));
        request
    };
    for _ in 0..10 {
        let (status, _) = edge.send(from("192.168.1.20:5000")).await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let (status, _) = edge.send(from("192.168.1.20:5001")).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let (status, _) = edge.send(from("192.168.1.21:5000")).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "another address is not limited"
    );
    edge.clock.advance(SignedDuration::from_mins(10));
    let (status, _) = edge.send(from("192.168.1.20:5000")).await;
    assert_eq!(status, StatusCode::CREATED, "the window moves on");
}
