//! Device enrollment through the built binary (lan-security.md § Device enrollment; ADR 0009;
//! #74): an unenrolled device cannot call the API, an approved one can, and a revoked one is
//! refused. The admin routes answer this machine, as the status page does.

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use common::{edge, terminate, wait_exit, wait_ready};
use health_auth::jose::{EcJwk, sign};
use p256::ecdsa::SigningKey;
use serde_json::{Value, json};

struct Node {
    port: u16,
}

impl Node {
    fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// `method path` with an optional DPoP proof and JSON body: the status and the JSON reply.
    fn call(
        &self,
        method: &str,
        path: &str,
        key: Option<&SigningKey>,
        body: Option<Value>,
    ) -> (u16, Value) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("timeout");
        let body = body.map(|b| b.to_string()).unwrap_or_default();
        let mut head = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
        if let Some(key) = key {
            head.push_str(&format!("DPoP: {}\r\n", self.proof(key, method, path)));
        }
        write!(stream, "{head}\r\n{body}").expect("write");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read");
        let status = response
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .expect("status");
        let json = response
            .split_once("\r\n\r\n")
            .and_then(|(_, b)| serde_json::from_str(b).ok())
            .unwrap_or(Value::Null);
        (status, json)
    }

    fn proof(&self, key: &SigningKey, method: &str, path: &str) -> String {
        let iat = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_secs();
        sign(
            key,
            &json!({ "typ": "dpop+jwt", "alg": "ES256", "jwk": EcJwk::of(key.verifying_key()) }),
            &json!({
                "jti": uuid::Uuid::now_v7().to_string(),
                "htm": method,
                "htu": format!("{}{path}", self.origin()),
                "iat": iat,
            }),
        )
    }
}

#[test]
fn only_enrolled_devices_reach_the_api() {
    let dir = tempfile::tempdir().expect("dir");
    let port = common::free_port();
    let node = Node { port };
    let mut child = edge(
        &dir.path().join("store"),
        port,
        &[],
        &[("HEALTH_ORIGINS", node.origin())],
    );
    wait_ready(port);
    let tablet = SigningKey::random(&mut rand_core::OsRng);

    let (status, _) = node.call("GET", "/api/device", None, None);
    assert_eq!(status, 401, "no proof");
    let (status, body) = node.call("GET", "/api/device", Some(&tablet), None);
    assert_eq!(
        (status, body["error"].as_str()),
        (403, Some("device-not-enrolled"))
    );

    let (status, request) = node.call(
        "POST",
        "/api/enrollment",
        Some(&tablet),
        Some(json!({ "label": "Chrome on Android" })),
    );
    assert_eq!(status, 201, "{request}");
    let (status, device) = node.call(
        "POST",
        "/api/admin/enrollments/approve",
        None,
        Some(json!({ "code": request["code"], "name": "Ward 3 tablet", "location": "Ward 3" })),
    );
    assert_eq!(status, 201, "{device}");

    let (status, body) = node.call("GET", "/api/device", Some(&tablet), None);
    assert_eq!(
        (status, body["name"].as_str()),
        (200, Some("Ward 3 tablet"))
    );

    let id = device["id"].as_str().expect("id");
    let (status, _) = node.call(
        "POST",
        &format!("/api/admin/devices/{id}/revoke"),
        None,
        None,
    );
    assert_eq!(status, 200);
    let (status, body) = node.call("GET", "/api/device", Some(&tablet), None);
    assert_eq!(
        (status, body["error"].as_str()),
        (403, Some("device-revoked"))
    );

    terminate(&child);
    assert_eq!(wait_exit(&mut child).code(), Some(0));
}
