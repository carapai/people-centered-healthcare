//! LAN TLS through the built binary (lan-security.md § Certificate hierarchy; #74): once
//! provisioned, the node serves HTTPS with a leaf it issued under its facility intermediate,
//! which a client trusting only the national root accepts for `<node-id>.local`; it then no
//! longer speaks plain HTTP. Unix: the keystore stands in for systemd's `$CREDENTIALS_DIRECTORY`.

#![cfg(unix)]

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{BINARY, STARTUP, edge, terminate, wait_exit};
use health_pki::{Authority, Facility, IssuingCa, new_key, request};
use health_time::Timestamp;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, ServerName};

const NODE: &str = "ug-kla-0001";
const KEY: &str = "health.facility-intermediate.key";

/// Provisions `dir` as `facility-edge provision install` would after central issued the chain:
/// the national root's PEM.
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
    std::fs::write(credentials.join(KEY), key.serialize_pem()).expect("key");
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
    let chain = dir.join("chain.pem");
    let root_file = dir.join("root.pem");
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
    root.certificate_pem()
}

/// `GET path` over TLS to `port`, trusting only `root`, for the name `name`: the status line.
fn get_tls(port: u16, root: &str, name: &str, path: &str) -> Result<String, String> {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(CertificateDer::from_pem_slice(root.as_bytes()).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| e.to_string())?
    .with_root_certificates(roots)
    .with_no_client_auth();
    let server = ServerName::try_from(name.to_owned()).map_err(|e| e.to_string())?;
    let connection =
        rustls::ClientConnection::new(Arc::new(config), server).map_err(|e| e.to_string())?;
    let tcp = TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    tcp.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let mut tls = rustls::StreamOwned::new(connection, tcp);
    write!(
        tls,
        "GET {path} HTTP/1.1\r\nHost: {name}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|e| e.to_string())?;
    let mut response = String::new();
    // A peer that closes without close_notify is fine once the response is in.
    let _ = tls.read_to_string(&mut response);
    response
        .lines()
        .next()
        .map(str::to_owned)
        .filter(|l| !l.is_empty())
        .ok_or_else(|| "no response".into())
}

#[test]
fn a_provisioned_node_serves_https_under_its_intermediate() {
    let dir = tempfile::tempdir().expect("dir");
    let credentials = dir.path().join("credentials");
    std::fs::create_dir_all(&credentials).expect("dir");
    let root = provision(dir.path(), &credentials);
    let port = common::free_port();
    let mut node = edge(
        &dir.path().join("store"),
        port,
        &[],
        &[
            ("HEALTH_NODE_ID", NODE.to_owned()),
            ("CREDENTIALS_DIRECTORY", credentials.display().to_string()),
        ],
    );
    let name = format!("{NODE}.local");
    let started = Instant::now();
    loop {
        match get_tls(port, &root, &name, "/ready") {
            Ok(status) if status.contains(" 200 ") => break,
            other => {
                assert!(started.elapsed() < STARTUP, "not ready over TLS: {other:?}");
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    // Only the node's own names: a client checking another name refuses it.
    assert!(get_tls(port, &root, "ug-kla-0002.local", "/ready").is_err());
    // And no plain HTTP.
    assert_eq!(common::get(port, "/ready"), None);
    // The leaf and its key, kept for the next start.
    let tls = dir.path().join("tls");
    assert!(tls.join("leaf.pem").is_file() && tls.join("leaf.key").is_file());

    terminate(&node);
    assert_eq!(wait_exit(&mut node).code(), Some(0));
}
