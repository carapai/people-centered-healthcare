//! Provisioning online through the built binary (#268; ADRs 0048, 0058): the edge redeems a
//! one-time code at central over HTTPS, installs the chain central issues for the node ID it
//! mints, and runs under that identity.
//!
//! Central here is `health-provisioning`'s router over SurrealDB in memory, served over TLS with
//! a self-signed certificate the edge is told to trust (`--central-ca`). As in `provision.rs`,
//! the keys stand already sealed in `$CREDENTIALS_DIRECTORY`; sealing needs root and is run under
//! systemd in edge-build.yml. Test machines fail the encryption gate, so the edge reports itself
//! unencrypted and only an exempt code provisions it.

#![cfg(unix)]

mod common;

use std::net::SocketAddr;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use common::BINARY;
use health_pki::{Authority, IssuingCa, new_key, request};
use health_provisioning::{Issuer, NodeRegistry};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use rustls_pki_types::pem::PemObject as _;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::server::TlsStream;

const KEY: &str = "health.facility-intermediate.key";
const FACILITY: &str = "ug-kla-mulago";

/// TLS connections for axum.
struct Tls {
    tcp: TcpListener,
    acceptor: TlsAcceptor,
}

impl axum::serve::Listener for Tls {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            if let Ok((stream, peer)) = self.tcp.accept().await
                && let Ok(tls) = self.acceptor.accept(stream).await
            {
                return (tls, peer);
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.tcp.local_addr()
    }
}

/// Central, serving `/api/provisioning/nodes` over HTTPS; returns its registry, its address, its
/// TLS certificate and the national root, PEM.
async fn central() -> (NodeRegistry, SocketAddr, String, String) {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "central".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let registry = NodeRegistry::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        Arc::new(SystemClock),
        "central",
    );
    let now = Timestamp::now();
    let root = Authority::root("Test Root", now).expect("root");
    let key = new_key().expect("key");
    let issued = root
        .issue_issuing_ca(
            &request(&key, "issuing").expect("csr"),
            &IssuingCa {
                common_name: "Test Issuing CA".into(),
                dns_suffixes: Vec::new(),
            },
            now,
        )
        .expect("issuing");
    let issuer: Arc<dyn Issuer> =
        Arc::new(Authority::from_pem(&issued.pem(), &key.serialize_pem()).expect("issuing"));
    let app = axum::Router::new().nest(
        "/api",
        health_provisioning::router(registry.clone(), issuer),
    );

    let certified =
        rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).expect("certificate");
    let cert_pem = certified.cert.pem();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("versions")
    .with_no_client_auth()
    .with_single_cert(
        vec![CertificateDer::from_pem_slice(cert_pem.as_bytes()).expect("cert")],
        PrivateKeyDer::from_pem_slice(certified.signing_key.serialize_pem().as_bytes())
            .expect("key"),
    )
    .expect("config");
    let tcp = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = tcp.local_addr().expect("address");
    let listener = Tls {
        tcp,
        acceptor: TlsAcceptor::from(Arc::new(config)),
    };
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (registry, address, cert_pem, root.certificate_pem())
}

fn provision(dir: &Path, args: &[String]) -> (Option<i32>, String) {
    let output = Command::new(BINARY)
        .arg("provision")
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HEALTH_DATA_DIR", dir.join("store"))
        .env("CREDENTIALS_DIRECTORY", dir.join("credentials"))
        .output()
        .expect("run");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned()
            + &String::from_utf8_lossy(&output.stderr),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn an_edge_provisions_itself_online_with_a_one_time_code() {
    let (registry, address, central_ca, root) = central().await;
    let dir = tempfile::tempdir().expect("dir");
    let credentials = dir.path().join("credentials");
    std::fs::create_dir_all(&credentials).expect("dir");
    // Sealed by an earlier `provision` that did not finish: the key is reused.
    std::fs::write(
        credentials.join(KEY),
        new_key().expect("key").serialize_pem(),
    )
    .expect("key");
    for name in ["health.pepper.key", "health.token-signing.key"] {
        std::fs::write(credentials.join(name), [7u8; 32]).expect("secret");
    }
    let write = |name: &str, contents: &str| {
        let path = dir.path().join(name);
        std::fs::write(&path, contents).expect("write");
        path.display().to_string()
    };
    let (root, central_ca) = (
        write("root.pem", &root),
        write("central-ca.pem", &central_ca),
    );
    let url = format!("https://localhost:{}", address.port());
    let run = |args: Vec<String>| {
        let dir = dir.path().to_owned();
        tokio::task::spawn_blocking(move || provision(&dir, &args))
    };
    let online = |code: &str, ca: Option<&str>| {
        let mut args = vec![
            "--code".to_owned(),
            code.to_owned(),
            "--central".to_owned(),
            url.clone(),
            "--root".to_owned(),
            root.clone(),
        ];
        if let Some(ca) = ca {
            args.extend(["--central-ca".to_owned(), ca.to_owned()]);
        }
        args
    };

    // Central's certificate is verified: the system store does not trust this one.
    let (plain, _) = registry
        .issue_code(FACILITY, Vec::new(), None, "ops:auma")
        .await
        .expect("code");
    let (code, output) = run(online(&plain, None)).await.expect("run");
    assert_eq!(code, Some(11), "{output}");
    assert!(output.contains("cannot reach"), "{output}");

    // Unencrypted, without an exemption: refused as the gate refuses.
    let (code, output) = run(online(&plain, Some(&central_ca))).await.expect("run");
    assert_eq!(code, Some(10), "{output}");
    assert!(output.contains("not encrypted"), "{output}");

    // A wrong code.
    let (code, output) = run(online("0000-0000", Some(&central_ca)))
        .await
        .expect("run");
    assert_eq!(code, Some(11), "{output}");
    assert!(output.contains("refused the code"), "{output}");

    // Exempt: provisioned, under the node ID central minted.
    let (exempt, _) = registry
        .issue_code(
            FACILITY,
            Vec::new(),
            Some("TPM-less PC pending replacement".into()),
            "ops:auma",
        )
        .await
        .expect("code");
    let (code, output) = run(online(&exempt, Some(&central_ca))).await.expect("run");
    assert_eq!(code, Some(0), "{output}");
    assert!(output.contains("provisioned as E1"), "{output}");
    let nodes = registry.nodes_at(FACILITY).await.expect("nodes");
    let node = nodes.first().expect("node");

    let (code, output) = run(vec!["status".to_owned()]).await.expect("run");
    assert_eq!(code, Some(0), "{output}");
    let status: serde_json::Value = serde_json::from_str(output.trim()).expect("status");
    assert_eq!(status["node_id"], node.id.to_string());
    assert_eq!(status["short_code"], "E1");
    assert!(status["intermediate_expires"].is_string(), "{status}");

    // Once provisioned, never again: a re-provisioned machine is a new node.
    let (again, _) = registry
        .issue_code(FACILITY, Vec::new(), Some("x".into()), "ops:auma")
        .await
        .expect("code");
    let (code, output) = run(online(&again, Some(&central_ca))).await.expect("run");
    assert_eq!(code, Some(11), "{output}");
    assert!(output.contains("already provisioned"), "{output}");
}
