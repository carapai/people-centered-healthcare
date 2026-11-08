//! The edge's sync task against central's `/sync/v1` endpoints over HTTPS (#81; ADR 0062), in
//! process. The edge writes before it ever reaches central, and while central is down, and both
//! sides still end up holding the same records.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;

use health_edge_runtime::sync::{Sync, Upstream};
use health_identifiers::{NodeId, OpId, ResourceId};
use health_persistence::{Precondition, RawJson, Store as _, Transaction as _, WriteRequest};
use health_pki::{Authority, IssuingCa, KeyPair, new_key, request};
use health_provisioning::NodeRegistry;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_sync_protocol::{Compatibility, PROTOCOL, Verdict};
use health_sync_server::SyncServer;
use health_time::{SystemClock, Timestamp};
use p256::ecdsa::SigningKey;
use p256::pkcs8::DecodePrivateKey as _;
use rustls_pki_types::pem::PemObject as _;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::server::TlsStream;

const FACILITY: &str = "ug-kla-mulago";

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

async fn store(namespace: &str) -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: namespace.into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

struct Central {
    store: SurrealStore,
    origin: String,
    cert_pem: String,
    node: NodeId,
    key: SigningKey,
}

/// Central with one provisioned edge, serving `/sync/v1` over HTTPS on localhost.
async fn central() -> Central {
    let store = store("central").await;
    let registry = NodeRegistry::new(store.clone(), Arc::new(SystemClock), "central");
    let now = Timestamp::now();
    let root = Authority::root("Root", now).expect("root");
    let ca_key = new_key().expect("key");
    let issued = root
        .issue_issuing_ca(
            &request(&ca_key, "issuing").expect("csr"),
            &IssuingCa {
                common_name: "Issuing".into(),
                dns_suffixes: Vec::new(),
            },
            now,
        )
        .expect("issuing");
    let issuer = Authority::from_pem(&issued.pem(), &ca_key.serialize_pem()).expect("issuing");
    let (code, _) = registry
        .issue_code(FACILITY, Vec::new(), None, "ops:auma")
        .await
        .expect("code");
    let edge_key: KeyPair = new_key().expect("key");
    let node = registry
        .redeem(
            &code,
            &request(&edge_key, "edge").expect("csr"),
            true,
            &issuer,
        )
        .await
        .expect("provisioned")
        .node;

    let compatibility = Compatibility {
        protocol: PROTOCOL,
        software: env!("CARGO_PKG_VERSION").parse().expect("version"),
        approved: BTreeSet::new(),
    };
    let app = SyncServer::new(store.clone(), registry, compatibility).router();
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
    let port = tcp.local_addr().expect("address").port();
    let listener = Tls {
        tcp,
        acceptor: TlsAcceptor::from(Arc::new(config)),
    };
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Central {
        store,
        origin: format!("https://localhost:{port}"),
        cert_pem,
        node: node.id.to_string().parse().expect("node id"),
        key: SigningKey::from_pkcs8_pem(&edge_key.serialize_pem()).expect("p256 key"),
    }
}

impl Central {
    fn sync(&self, edge: &SurrealStore, origin: &str) -> Sync {
        Sync::new(
            edge.clone(),
            Upstream {
                origin: origin.to_owned(),
                ca_pem: Some(self.cert_pem.clone()),
                node: self.node,
            },
            self.key.clone(),
        )
        .expect("sync")
    }
}

async fn write(store: &SurrealStore, body: &Value) -> ResourceId {
    let id = ResourceId::new();
    let mut tx = store.begin().await.expect("begin");
    tx.write(WriteRequest {
        resource_type: body["resourceType"]
            .as_str()
            .expect("type")
            .parse()
            .expect("type"),
        id,
        json: RawJson::from_validated(body.to_string()),
        op_id: OpId::new(),
        precondition: Precondition::MustNotExist,
    })
    .await
    .expect("write");
    tx.commit().await.expect("commit");
    id
}

async fn version(store: &SurrealStore, resource_type: &str, id: ResourceId) -> Option<OpId> {
    let mut tx = store.begin().await.expect("begin");
    tx.read(&resource_type.parse().expect("type"), id)
        .await
        .expect("read")
        .map(|head| head.version.version_id)
}

fn observation() -> Value {
    json!({
        "resourceType": "Observation",
        "status": "final",
        "meta": {"tag": [{"system": "urn:health:facility", "code": FACILITY}]}
    })
}

#[tokio::test]
async fn the_edge_syncs_what_it_wrote_before_and_while_central_was_unreachable() {
    let central = central().await;
    let edge = store("edge").await;
    // Written before the edge ever reached central.
    let early = write(&edge, &observation()).await;
    let registered = write(&central.store, &json!({"resourceType": "Patient"})).await;

    // Central is unreachable: the round fails, and the edge keeps working.
    let unreachable = central.sync(&edge, "https://localhost:1");
    assert!(unreachable.round().await.is_err());
    let offline = write(&edge, &observation()).await;

    let sync = central.sync(&edge, &central.origin);
    let round = sync.round().await.expect("round");
    assert_eq!(round.verdict, Some(Verdict::Compatible));
    assert_eq!(round.pushed, 2, "both observations go up");
    assert_eq!(round.pulled, 1, "the patient comes down");
    for (resource_type, id) in [
        ("Observation", early),
        ("Observation", offline),
        ("Patient", registered),
    ] {
        let at_edge = version(&edge, resource_type, id).await;
        assert!(at_edge.is_some(), "{resource_type}/{id} at the edge");
        assert_eq!(at_edge, version(&central.store, resource_type, id).await);
    }

    // Caught up: the next round moves nothing.
    let again = sync.round().await.expect("round");
    assert_eq!((again.pushed, again.pulled), (0, 0));
}

#[tokio::test]
async fn an_edge_beyond_central_s_horizon_reseeds_and_keeps_its_outbox() {
    let central = central().await;
    let edge = store("edge").await;
    let sync = central.sync(&edge, &central.origin);
    let first = write(&central.store, &json!({"resourceType": "Patient"})).await;
    sync.round().await.expect("first round");

    // Offline past the horizon: the edge writes, central writes and prunes its whole log.
    let offline = write(&edge, &observation()).await;
    let meanwhile = write(&central.store, &json!({"resourceType": "Patient"})).await;
    central
        .store
        .prune_log(Timestamp::now() + health_time::SignedDuration::from_mins(1))
        .await
        .expect("prune");

    let round = sync.round().await.expect("round");
    assert_eq!(round.pushed, 1, "the outbox goes up first");
    assert!(round.seeded >= 1, "{round:?}");
    for (resource_type, id) in [
        ("Patient", first),
        ("Patient", meanwhile),
        ("Observation", offline),
    ] {
        let at_edge = version(&edge, resource_type, id).await;
        assert!(at_edge.is_some(), "{resource_type}/{id} at the edge");
        assert_eq!(at_edge, version(&central.store, resource_type, id).await);
    }
    let again = sync.round().await.expect("round");
    assert_eq!((again.pushed, again.pulled, again.seeded), (0, 0, 0));
}
