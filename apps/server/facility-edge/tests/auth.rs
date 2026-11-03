//! The edge's local authorization server through the built binary (offline-auth.md; ADRs 0009,
//! 0010, 0011, 0059; #75), with nothing but the edge itself: a member of staff whose snapshot
//! central provisioned in a bundle signs in on an enrolled device and uses the API; one whose
//! snapshot has expired is refused. Unix: the keystore stands in for systemd's
//! `$CREDENTIALS_DIRECTORY`.

#![cfg(unix)]

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use common::{BINARY, STARTUP, edge, terminate, wait_exit};
use health_auth::jose::{EcJwk, sha256_b64, sign};
use health_auth::{Bundle, BundleAccount, Credentials, SnapshotSigner};
use health_identity::snapshot::{PermissionSnapshot, RoleAssignment, RolePermissions};
use health_identity::{AccountId, AccountStatus};
use health_pki::{Authority, Facility, IssuingCa, new_key, request};
use health_time::{SignedDuration, Timestamp};
use p256::ecdsa::SigningKey;
use p256::pkcs8::DecodePrivateKey;
use serde_json::{Value, json};
use uuid::Uuid;

const NODE: &str = "ug-kla-0001";
const FACILITY: &str = "ug-kla-mulago";
const LAN: &str = "https://ug-kla-0001.local";
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

/// Installs a chain for this node and seals its keys, as `provision request` and `install` do;
/// the intermediate key's PEM.
fn provision(dir: &Path, credentials: &Path) -> String {
    let now = Timestamp::now();
    let root = Authority::root("Test Root", now).expect("root");
    let issuing_key = new_key().expect("key");
    let issued = root
        .issue_issuing_ca(
            &request(&issuing_key, "issuing").expect("csr"),
            &IssuingCa {
                common_name: "Test Issuing CA".into(),
                dns_suffixes: Vec::new(),
            },
            now,
        )
        .expect("issuing");
    let issuing =
        Authority::from_pem(&issued.pem(), &issuing_key.serialize_pem()).expect("issuing");
    let key = new_key().expect("key");
    let key_pem = key.serialize_pem();
    std::fs::write(
        credentials.join(health_edge_runtime::INTERMEDIATE_KEY),
        &key_pem,
    )
    .expect("key");
    std::fs::write(credentials.join(health_edge_runtime::PEPPER_KEY), [5u8; 32]).expect("pepper");
    let token = p256::SecretKey::random(&mut rand_core::OsRng).to_bytes();
    std::fs::write(credentials.join(health_edge_runtime::TOKEN_KEY), token).expect("token");
    let facility = issuing
        .issue_facility(
            &request(&key, NODE).expect("csr"),
            &Facility {
                node_id: NODE.into(),
                dns_names: Vec::new(),
            },
            now,
        )
        .expect("facility");
    let (chain, root_file) = (dir.join("chain.pem"), dir.join("root.pem"));
    std::fs::write(
        &chain,
        format!("{}{}", facility.pem(), issuing.certificate_pem()),
    )
    .expect("chain");
    std::fs::write(&root_file, root.certificate_pem()).expect("root");
    let installed = Command::new(BINARY)
        .args(["provision", "install", "--chain"])
        .arg(&chain)
        .arg("--root")
        .arg(&root_file)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HEALTH_DATA_DIR", dir.join("store"))
        .env("HEALTH_NODE_ID", NODE)
        .env("CREDENTIALS_DIRECTORY", credentials)
        .output()
        .expect("install");
    assert!(installed.status.success(), "{installed:?}");
    key_pem
}

struct Node {
    port: u16,
}

struct Reply {
    status: u16,
    location: Option<String>,
    body: Value,
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(b).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn proof(key: &SigningKey, method: &str, url: &str, token: Option<&str>) -> String {
    let iat = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let mut claims =
        json!({ "jti": Uuid::now_v7().to_string(), "htm": method, "htu": url, "iat": iat });
    if let Some(token) = token {
        claims["ath"] = json!(sha256_b64(token));
    }
    sign(
        key,
        &json!({ "typ": "dpop+jwt", "alg": "ES256", "jwk": EcJwk::of(key.verifying_key()) }),
        &claims,
    )
}

impl Node {
    fn console(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn send(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, String)],
        body: (&str, String),
    ) -> Reply {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .expect("timeout");
        let mut head = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: {}\r\nContent-Length: {}\r\n",
            body.0,
            body.1.len()
        );
        for (name, value) in headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        write!(stream, "{head}\r\n{}", body.1).expect("write");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read");
        let (head, body) = response.split_once("\r\n\r\n").unwrap_or((&response, ""));
        let status = head
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .expect("status");
        let location = head
            .lines()
            .find_map(|l| {
                l.strip_prefix("location: ")
                    .or_else(|| l.strip_prefix("Location: "))
            })
            .map(str::to_owned);
        Reply {
            status,
            location,
            body: serde_json::from_str(body).unwrap_or(Value::Null),
        }
    }

    fn json(&self, method: &str, path: &str, key: Option<&SigningKey>, body: Value) -> Reply {
        let headers: Vec<(&str, String)> = key
            .map(|k| ("DPoP", proof(k, method, &format!("{LAN}{path}"), None)))
            .into_iter()
            .collect();
        self.send(
            method,
            path,
            &headers,
            ("application/json", body.to_string()),
        )
    }

    fn form(&self, path: &str, fields: &[(&str, &str)], headers: &[(&str, String)]) -> Reply {
        let body = fields
            .iter()
            .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        self.send(
            "POST",
            path,
            headers,
            ("application/x-www-form-urlencoded", body),
        )
    }

    /// Signs in as `username`: the authorization code, or where the refusal sent the browser.
    fn login(&self, username: &str) -> Result<String, String> {
        let redirect = format!("{}/callback", self.console());
        let reply = self.form(
            "/auth/authorize",
            &[
                ("response_type", "code"),
                ("client_id", health_edge_runtime::CLIENT_ID),
                ("redirect_uri", &redirect),
                ("scope", "openid user/Patient.rs"),
                ("state", "s1"),
                ("code_challenge", CHALLENGE),
                ("code_challenge_method", "S256"),
                ("username", username),
                ("password", "correct horse battery"),
            ],
            &[],
        );
        let location = reply.location.unwrap_or_default();
        location
            .strip_prefix(&format!("{redirect}?"))
            .and_then(|query| query.split('&').find_map(|p| p.strip_prefix("code=")))
            .map(str::to_owned)
            .ok_or(location)
    }
}

fn snapshot(account: AccountId, issued: Timestamp, not_after: Timestamp) -> PermissionSnapshot {
    PermissionSnapshot {
        account,
        facility: FACILITY.into(),
        roles: vec![RoleAssignment {
            role: "clinician".into(),
            locations: Vec::new(),
        }],
        programs: Vec::new(),
        issued_at: issued,
        not_after,
        snapshot_version: 1,
    }
}

#[test]
fn staff_sign_in_on_an_enrolled_device_with_no_internet() {
    let dir = tempfile::tempdir().expect("dir");
    let credentials = dir.path().join("credentials");
    std::fs::create_dir_all(&credentials).expect("dir");
    let intermediate = provision(dir.path(), &credentials);
    let central = SnapshotSigner::generate();
    let keys_file = dir.path().join("snapshot-keys.json");
    std::fs::write(
        &keys_file,
        serde_json::to_string(&[central.public()]).expect("json"),
    )
    .expect("keys");
    let port = common::free_port();
    let node = Node { port };
    let mut child = edge(
        &dir.path().join("store"),
        port,
        &[],
        &[
            ("HEALTH_NODE_ID", NODE.to_owned()),
            ("HEALTH_FACILITY", FACILITY.to_owned()),
            ("HEALTH_SNAPSHOT_KEYS_FILE", keys_file.display().to_string()),
            ("HEALTH_ORIGINS", format!("{LAN},{}", node.console())),
            ("CREDENTIALS_DIRECTORY", credentials.display().to_string()),
            ("HEALTH_MDNS", "off".into()),
        ],
    );
    let started = Instant::now();
    while common::get(port, "/ready") != Some(200) {
        assert!(started.elapsed() < STARTUP, "not ready in time");
        std::thread::sleep(Duration::from_millis(100));
    }
    let runtime = node.send("GET", "/runtime.json", &[], ("text/plain", String::new()));
    assert_eq!(runtime.body["auth"]["issuer"], format!("{LAN}/auth"));

    // The device enrolls.
    let device = SigningKey::random(&mut rand_core::OsRng);
    let asked = node.json(
        "POST",
        "/api/enrollment",
        Some(&device),
        json!({ "label": "Chrome on Windows" }),
    );
    assert_eq!(asked.status, 201, "{}", asked.body);
    let approved = node.json(
        "POST",
        "/api/admin/enrollments/approve",
        None,
        json!({ "code": asked.body["code"], "name": "Ward 3 PC", "location": "Ward 3" }),
    );
    assert_eq!(approved.status, 201, "{}", approved.body);

    // Central's bundle: one member of staff current, one whose snapshot has expired.
    let now = Timestamp::now();
    let (nakato, okello) = (AccountId(Uuid::now_v7()), AccountId(Uuid::now_v7()));
    let roles = RolePermissions(std::collections::BTreeMap::from([(
        "clinician".into(),
        std::collections::BTreeSet::from(["record.read".into()]),
    )]));
    let fortnight = SignedDuration::from_hours(24 * 14);
    let current = snapshot(nakato, now, now.checked_add(fortnight).expect("time"));
    let lapsed_issue = now
        .checked_sub(fortnight)
        .expect("time")
        .checked_sub(fortnight)
        .expect("time");
    let lapsed = snapshot(
        okello,
        lapsed_issue,
        now.checked_sub(fortnight).expect("time"),
    );
    let account = |id, username: &str| BundleAccount {
        id,
        username: username.into(),
        status: AccountStatus::Active,
        phc: Credentials::hash("correct horse battery").expect("hash"),
    };
    let recipient = p256::SecretKey::from_pkcs8_pem(&intermediate)
        .expect("key")
        .public_key();
    let bundle = Bundle {
        facility: FACILITY.into(),
        node_id: NODE.into(),
        issued_at: now,
        accounts: vec![account(nakato, "nakato"), account(okello, "okello")],
        snapshots: vec![
            central.sign(&current, &roles).expect("sign"),
            central.sign(&lapsed, &roles).expect("sign"),
        ],
        local: health_auth::LocalConfig::default(),
    }
    .seal(&central, &recipient)
    .expect("seal");
    let imported = node.send(
        "POST",
        "/api/admin/provision/accounts",
        &[],
        ("text/plain", bundle),
    );
    assert_eq!(imported.status, 200, "{}", imported.body);
    assert_eq!(imported.body["accounts"], 2);

    // Signed in, the token is bound to the enrolled device and opens the API.
    let code = node.login("nakato").expect("signed in");
    let redirect = format!("{}/callback", node.console());
    let token = node.form(
        "/auth/token",
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect),
            ("client_id", health_edge_runtime::CLIENT_ID),
            ("code_verifier", VERIFIER),
        ],
        &[(
            "DPoP",
            proof(&device, "POST", &format!("{LAN}/auth/token"), None),
        )],
    );
    assert_eq!(token.status, 200, "{}", token.body);
    assert_eq!(token.body["token_type"], "DPoP");
    let access = token.body["access_token"]
        .as_str()
        .expect("token")
        .to_owned();
    let claims: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(access.split('.').nth(1).expect("claims"))
            .expect("b64"),
    )
    .expect("json");
    assert_eq!(claims["acr"], "offline-local");
    assert_eq!(claims["facility"], FACILITY);
    let session = node.send(
        "GET",
        "/api/session",
        &[
            ("Authorization", format!("DPoP {access}")),
            (
                "DPoP",
                proof(&device, "GET", &format!("{LAN}/api/session"), Some(&access)),
            ),
        ],
        ("text/plain", String::new()),
    );
    assert_eq!(session.status, 200, "{}", session.body);
    assert_eq!(session.body["account"], nakato.to_string());

    // An expired snapshot: the right password still signs nobody in.
    let refused = node.login("okello").expect_err("refused");
    assert!(refused.contains("error=refused"), "{refused}");

    terminate(&child);
    assert_eq!(wait_exit(&mut child).code(), Some(0));
}
