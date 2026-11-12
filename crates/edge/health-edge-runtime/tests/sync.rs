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

/// A TLS listener on localhost with a self-signed certificate: the listener, its origin and the
/// certificate's PEM.
async fn tls() -> (Tls, String, String) {
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
    (listener, format!("https://localhost:{port}"), cert_pem)
}

struct Central {
    store: SurrealStore,
    files: health_files::FsBlobStore,
    _dir: tempfile::TempDir,
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
    let dir = tempfile::tempdir().expect("files");
    let files = health_files::FsBlobStore::open(dir.path()).expect("blob store");
    let app = SyncServer::new(store.clone(), registry, compatibility)
        .with_blobs(files.clone())
        .router();
    let (listener, origin, cert_pem) = tls().await;
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Central {
        store,
        files,
        _dir: dir,
        origin,
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

/// A patient registered at central with a visit at this facility, so their golden record comes
/// down (golden-record narrowing).
async fn patient_seen_here(central: &SurrealStore) -> ResourceId {
    let patient = write(central, &json!({"resourceType": "Patient"})).await;
    write(
        central,
        &json!({
            "resourceType": "Encounter", "status": "planned",
            "meta": {"tag": [{"system": "urn:health:facility", "code": FACILITY}]},
            "subject": {"reference": format!("Patient/{patient}")}
        }),
    )
    .await;
    patient
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
    let registered = patient_seen_here(&central.store).await;

    // Central is unreachable: the round fails, and the edge keeps working.
    let unreachable = central.sync(&edge, "https://localhost:1");
    assert!(unreachable.round().await.is_err());
    let offline = write(&edge, &observation()).await;

    let sync = central.sync(&edge, &central.origin);
    let round = sync.round().await.expect("round");
    assert_eq!(round.verdict, Some(Verdict::Compatible));
    assert_eq!(round.pushed, 2, "both observations go up");
    assert_eq!(round.pulled, 2, "the patient and their visit come down");
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
    let first = patient_seen_here(&central.store).await;
    sync.round().await.expect("first round");

    // Offline past the horizon: the edge writes, central writes and prunes its whole log.
    let offline = write(&edge, &observation()).await;
    let meanwhile = patient_seen_here(&central.store).await;
    // More than two of the edge's batches, so the snapshot is read and applied as a stream (#83).
    let mut bulk = Vec::new();
    let mut tx = central.store.begin().await.expect("begin");
    for _ in 0..450 {
        let id = ResourceId::new();
        tx.write(WriteRequest {
            resource_type: "Observation".parse().expect("type"),
            id,
            json: RawJson::from_validated(observation().to_string()),
            op_id: OpId::new(),
            precondition: Precondition::MustNotExist,
        })
        .await
        .expect("write");
        bulk.push(id);
    }
    tx.commit().await.expect("commit");
    central
        .store
        .prune_log(Timestamp::now() + health_time::SignedDuration::from_mins(1))
        .await
        .expect("prune");

    let round = sync.round().await.expect("round");
    assert_eq!(round.pushed, 1, "the outbox goes up first");
    assert!(round.seeded >= bulk.len(), "{round:?}");
    for id in &bulk {
        assert!(
            version(&edge, "Observation", *id).await.is_some(),
            "Observation/{id} at the edge"
        );
    }
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

#[tokio::test]
async fn an_attachment_follows_its_record_and_comes_down_on_demand() {
    use health_sync_protocol::{BlobRef, ContentHash, blob_url};
    let central = central().await;
    let edge = store("edge").await;
    let dir = tempfile::tempdir().expect("edge files");
    let files = health_files::FsBlobStore::open(dir.path()).expect("edge blob store");
    let sync = central
        .sync(&edge, &central.origin)
        .with_blobs(files.clone());

    // A scanned consent at the edge: the record references the blob by hash.
    let scan: Vec<u8> = (0..3_000_000u32).map(|i| (i % 253) as u8).collect();
    let stored = files.put(scan.as_slice()).expect("put");
    let hash: ContentHash = stored.blob.hash.to_string().parse().expect("hash");
    let document = json!({
        "resourceType": "DocumentReference",
        "status": "current",
        "meta": {"tag": [{"system": "urn:health:facility", "code": FACILITY}]},
        "content": [{"attachment": {
            "url": blob_url(&hash),
            "size": scan.len(),
            "contentType": "application/pdf"
        }}]
    });
    write(&edge, &document).await;

    let round = sync.round().await.expect("round");
    assert_eq!((round.pushed, round.blobs), (1, 1));
    assert_eq!(
        central.files.read(&stored.blob.hash).expect("at central"),
        scan
    );
    assert_eq!(sync.round().await.expect("round").blobs, 0, "sent once");

    // Central holds a blob the edge does not: it comes down when asked for.
    let photo: Vec<u8> = b"a wound photo".to_vec();
    let at_central = central.files.put(photo.as_slice()).expect("put");
    let reference = BlobRef {
        hash: at_central.blob.hash.to_string().parse().expect("hash"),
        size: at_central.blob.size,
        content_type: "image/jpeg".into(),
    };
    assert!(!files.contains(&at_central.blob.hash));
    // Only once a record in the edge's facility references it (#84).
    assert!(
        sync.fetch_blob(&reference).await.is_err(),
        "no record references it"
    );
    write(
        &central.store,
        &json!({
            "resourceType": "DocumentReference",
            "status": "current",
            "meta": {"tag": [{"system": "urn:health:facility", "code": FACILITY}]},
            "content": [{"attachment": {
                "url": blob_url(&reference.hash),
                "size": photo.len(),
                "contentType": "image/jpeg"
            }}]
        }),
    )
    .await;
    sync.fetch_blob(&reference).await.expect("fetch");
    assert_eq!(
        files.read(&at_central.blob.hash).expect("at the edge"),
        photo
    );
}

#[tokio::test]
async fn a_grant_made_offline_reaches_central_s_review_queue_due_from_arrival() {
    let central = central().await;
    let edge = store("edge").await;
    // Made while central was unreachable, a week ago.
    let made = Timestamp::now() - health_time::SignedDuration::from_hours(24 * 7);
    let mut tx = edge.begin().await.expect("begin");
    tx.put_break_glass(&health_surrealdb::BreakGlassRow {
        id: "bg-offline".into(),
        session: "s-1".into(),
        account: "a-nurse".into(),
        facility: FACILITY.into(),
        patient: "Patient/p-1".into(),
        reason: "BTG".into(),
        justification: "unconscious".into(),
        at: made,
        expires_at: made + health_time::SignedDuration::from_hours(1),
        review_due: made + health_time::SignedDuration::from_hours(72),
        acknowledged: false,
        review: None,
        escalated_at: None,
    })
    .await
    .expect("grant");
    tx.commit().await.expect("commit");

    // The edge has never reached central: it does now.
    let unreachable = central.sync(&edge, "https://localhost:1");
    assert!(unreachable.round().await.is_err());
    let sync = central.sync(&edge, &central.origin);
    let before = Timestamp::now();
    let round = sync.round().await.expect("round");
    assert_eq!(round.grants, 1);
    let mut tx = central.store.begin().await.expect("begin");
    let reviews = tx.break_glass_reviews(10).await.expect("reviews");
    drop(tx);
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0].at, made);
    assert!(
        reviews[0].review_due >= before + health_time::SignedDuration::from_hours(72),
        "due 72 h after it arrived, not after it was made"
    );
    assert_eq!(sync.round().await.expect("round").grants, 0, "sent once");
}

/// A package `ug.moh.anc@version` with one Questionnaire, depending on `dependencies`.
fn anc_package(version: &str, dependencies: Value) -> health_packages::Files {
    let file = |path: &str, json: Value| (path.to_owned(), json.to_string().into_bytes());
    let depends_on: Vec<Value> = dependencies
        .as_object()
        .map(|deps| {
            deps.keys()
                .map(|_| json!({"type": "depends-on", "resource": "http://health.go.ug/fhir/ValueSet/core-codes|2.1.0"}))
                .collect()
        })
        .unwrap_or_default();
    health_packages::Files::from([
        file(
            "package.json",
            json!({
                "name": "ug.moh.anc", "version": version, "fhirVersions": ["5.0.0"],
                "dependencies": dependencies,
                "kind": "program", "platform": { "minSoftware": "0.1.0", "formatVersion": 1 }
            }),
        ),
        file(
            "Library-manifest.json",
            json!({
                "resourceType": "Library", "id": "manifest",
                "url": "http://health.go.ug/fhir/Library/ug.moh.anc-manifest", "version": version,
                "status": "active", "type": { "text": "asset-collection" },
                "relatedArtifact": depends_on
            }),
        ),
        file(
            "Questionnaire-anc-visit.json",
            json!({
                "resourceType": "Questionnaire", "id": "anc-visit",
                "url": "http://health.go.ug/fhir/Questionnaire/anc-visit", "version": version,
                "status": "active",
                "item": [{ "linkId": "bp", "text": "Blood pressure", "type": "string" }]
            }),
        ),
    ])
}

#[tokio::test]
async fn an_activated_package_is_fetched_verified_staged_and_switched_to() {
    use health_configuration::activation::Chain;
    use health_edge_runtime::activation::{Activator, Outcome, Report};
    use health_packages::{LocalSigner, Schemas, publish};
    use health_pki::Trust;

    let central = central().await;
    let edge = store("edge").await;
    let dir = tempfile::tempdir().expect("edge files");
    let files = health_files::FsBlobStore::open(dir.path()).expect("edge blob store");

    // The national root and its publication signer.
    let now = Timestamp::now();
    let root = Authority::root("Uganda Health Root", now).expect("root");
    let key = new_key().expect("key");
    let issued = root
        .issue_publication(
            &request(&key, "MoH publication signing").expect("csr"),
            "MoH publication signing",
            now,
        )
        .expect("publication certificate");
    let signer = LocalSigner::from_pem(&key.serialize_pem(), issued.pem()).expect("signer");
    let schemas = Schemas::platform();

    // Central publishes two versions: 1.1.0 depends on a core package no one activated.
    let mut records = Vec::new();
    for (version, dependencies) in [
        ("1.0.0", json!({})),
        ("1.1.0", json!({ "ug.moh.core": "2.1.0" })),
    ] {
        let published =
            publish(&anc_package(version, dependencies), &schemas, &signer, now).expect("publish");
        central
            .files
            .put(published.tgz.as_slice())
            .expect("central blob");
        records.push((version, published.digest.to_string(), published.signature));
    }
    let activate = |id: &str, version: &str, at: &str| {
        let (_, digest, signature) = records
            .iter()
            .find(|(v, ..)| *v == version)
            .expect("published")
            .clone();
        json!({
            "activation": {
                "id": id, "package": "ug.moh.anc", "version": version,
                "scope": {"level": "facility", "facility": FACILITY},
                "effectiveAt": at, "issuedAt": at, "by": "ops"
            },
            "digest": digest,
            "signature": signature
        })
    };
    let issue = |body: Value| {
        let store = central.store.clone();
        async move {
            let mut tx = store.begin().await.expect("begin");
            tx.put_package_activation(
                body["activation"]["id"].as_str().expect("id"),
                "ug.moh.anc",
                body["activation"]["issuedAt"].as_str().expect("issued"),
                &body.to_string(),
            )
            .await
            .expect("activation");
            tx.commit().await.expect("commit");
        }
    };
    issue(activate("act-1", "1.0.0", "2026-01-01T00:00:00Z")).await;

    let activator = Activator::new(
        Trust::new(&root.certificate_pem()).expect("trust"),
        Chain {
            facility: Some(FACILITY.into()),
            ..Chain::default()
        },
    )
    .expect("activator");
    let sync = central
        .sync(&edge, &central.origin)
        .with_blobs(files.clone())
        .with_activator(activator);
    let round = sync.round().await.expect("round");
    assert_eq!(round.activation, Some(Outcome::Activated), "{round:?}");
    let mut tx = edge.begin().await.expect("begin");
    let active: Value =
        serde_json::from_str(&tx.active_packages().await.expect("read").expect("active"))
            .expect("json");
    assert_eq!(active, json!({ "ug.moh.anc": "1.0.0" }));
    assert_eq!(tx.staged_packages().await.expect("staged").len(), 1);
    drop(tx);
    assert!(
        files.contains(&records[0].1.parse().expect("hash")),
        "the tarball is retained"
    );
    assert_eq!(
        sync.round().await.expect("round").activation,
        None,
        "unchanged"
    );

    // 1.1.0 needs a core package no one activated: the edge keeps 1.0.0 and reports why.
    issue(activate("act-2", "1.1.0", "2026-01-02T00:00:00Z")).await;
    let round = sync.round().await.expect("round");
    assert_eq!(round.activation, Some(Outcome::Failed));
    let mut tx = edge.begin().await.expect("begin");
    let active: Value =
        serde_json::from_str(&tx.active_packages().await.expect("read").expect("active"))
            .expect("json");
    assert_eq!(
        active,
        json!({ "ug.moh.anc": "1.0.0" }),
        "the previous set stays"
    );
    let report: Report =
        serde_json::from_str(&tx.activation_reports(1).await.expect("reports")[0]).expect("report");
    assert_eq!(report.outcome, Outcome::Failed);
    assert_eq!(
        report.problems,
        vec!["ug.moh.anc needs ug.moh.core@2.1.0, which is not active".to_owned()]
    );
    assert!(
        tx.unsent_activation_reports(10)
            .await
            .expect("unsent")
            .is_empty(),
        "sent to central"
    );
    drop(tx);
    // Central holds both reports, the failure newest, under this node.
    let mut tx = central.store.begin().await.expect("begin");
    let at_central = tx.node_activation_reports(10).await.expect("reports");
    drop(tx);
    assert_eq!(at_central.len(), 2, "{at_central:?}");
    assert!(
        at_central
            .iter()
            .all(|(node, _)| *node == central.node.to_string())
    );
    let newest: Report = serde_json::from_str(&at_central[0].1).expect("report");
    assert_eq!(newest, report);

    // A record whose signature is not the package's never stages: a node seeing it for the
    // first time keeps nothing active and reports the signature.
    let mut forged = activate("act-3", "1.0.0", "2026-01-03T00:00:00Z");
    forged["signature"] = json!(records[1].2.clone());
    issue(forged).await;
    let fresh = store("fresh").await;
    let fresh_dir = tempfile::tempdir().expect("fresh files");
    let fresh_sync = central
        .sync(&fresh, &central.origin)
        .with_blobs(health_files::FsBlobStore::open(fresh_dir.path()).expect("blobs"))
        .with_activator(
            Activator::new(
                Trust::new(&root.certificate_pem()).expect("trust"),
                Chain {
                    facility: Some(FACILITY.into()),
                    ..Chain::default()
                },
            )
            .expect("activator"),
        );
    let round = fresh_sync.round().await.expect("round");
    assert_eq!(round.activation, Some(Outcome::Failed));
    let mut tx = fresh.begin().await.expect("begin");
    assert_eq!(tx.active_packages().await.expect("read"), None);
    assert!(tx.staged_packages().await.expect("staged").is_empty());
    let report: Report =
        serde_json::from_str(&tx.activation_reports(1).await.expect("reports")[0]).expect("report");
    assert!(
        report
            .problems
            .iter()
            .any(|p| p.starts_with("ug.moh.anc@1.0.0: ")),
        "{report:?}"
    );
}

/// The edge's peers in this test: the enrolled device its gate verified, in the facility's scope
/// (as `health_edge_runtime`'s own, #133).
struct Desktops;

impl health_sync_server::PeerAuth for Desktops {
    fn authenticate<'a>(
        &'a self,
        _method: &'a axum::http::Method,
        _uri: &'a axum::http::Uri,
        _headers: &'a axum::http::HeaderMap,
        extensions: &'a axum::http::Extensions,
        _body: &'a [u8],
    ) -> std::pin::Pin<
        Box<
            dyn Future<
                    Output = Result<
                        health_sync_server::AuthenticatedPeer,
                        health_sync_server::PeerRefused,
                    >,
                > + Send
                + 'a,
        >,
    > {
        let peer = extensions
            .get::<health_auth::EnrolledDevice>()
            .map(|enrolled| health_sync_server::AuthenticatedPeer {
                id: enrolled.device.id.to_string().parse().expect("node id"),
                scope: health_surrealdb::SyncScope::Facility(FACILITY.into()),
            })
            .ok_or_else(|| health_sync_server::PeerRefused::Unauthorized("no device".into()));
        Box::pin(std::future::ready(peer))
    }
}

/// A desktop under an edge (#133): it asks to enroll over HTTPS and learns once it is approved;
/// then its sync client proves each request with a DPoP proof by its enrolled device key, the
/// edge's gate admits it, and records move both ways over TLS.
#[tokio::test]
async fn a_desktop_syncs_with_its_edge_by_its_enrolled_device_key() {
    use health_auth::{DeviceConfig, DeviceGate, DeviceStore as _, SurrealAuthStore};
    use health_identity::{DeviceId, SessionPolicy};
    use health_sync_client::enrollment::{self, Enrollment};

    let edge = store("edge").await;
    let (listener, origin, cert_pem) = tls().await;
    let devices = SurrealAuthStore::new(edge.clone(), SessionPolicy::default().hard_cap);
    let key = SigningKey::from_bytes(&[11; 32].into()).expect("key");
    let gate = DeviceGate::new(
        devices.clone(),
        DeviceConfig::new(vec![origin.clone()], "edge"),
        Arc::new(SystemClock),
    );
    let compatibility = Compatibility {
        protocol: PROTOCOL,
        software: env!("CARGO_PKG_VERSION").parse().expect("version"),
        approved: BTreeSet::new(),
    };
    let app =
        gate.enrollment_router()
            .merge(gate.device_router())
            .merge(gate.layer(
                SyncServer::with_auth(edge.clone(), Arc::new(Desktops), compatibility).router(),
            ));
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    // The desktop asks over HTTPS and is shown a code; the admin approves it on the edge.
    let ask = |key: SigningKey, first: bool| {
        let (origin, ca) = (origin.clone(), cert_pem.clone());
        tokio::task::spawn_blocking(move || {
            if first {
                enrollment::request(&origin, "health-desktop on linux", &key, Some(&ca))
            } else {
                enrollment::status(&origin, &key, Some(&ca))
            }
        })
    };
    let code = match ask(key.clone(), true)
        .await
        .expect("join")
        .expect("request")
    {
        Enrollment::Pending { code } => code,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        ask(key.clone(), false)
            .await
            .expect("join")
            .expect("status"),
        Enrollment::Pending { code: code.clone() }
    );
    let request = devices
        .enrollment_by_code(&code)
        .await
        .expect("read")
        .expect("pending");
    let device = DeviceId(uuid::Uuid::now_v7());
    let approved = request
        .approve(device, "Reception", "OPD", "admin", Timestamp::now())
        .expect("approve");
    assert!(
        devices
            .enroll(
                request.id,
                &approved,
                &json!({"resourceType": "AuditEvent"})
            )
            .await
            .expect("enroll")
    );
    assert_eq!(
        ask(key.clone(), false)
            .await
            .expect("join")
            .expect("status"),
        Enrollment::Enrolled {
            device: device.to_string()
        }
    );

    // Its commands reach the edge's gated routes as the enrolled device (pass-through).
    let client = health_sync_client::edge::Edge::new(
        &origin,
        Some(&cert_pem),
        key.clone(),
        std::time::Duration::from_secs(10),
    )
    .expect("edge client");
    let answer = client
        .send(
            health_sync_client::edge::Request {
                method: "GET".into(),
                path: "/api/device".into(),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("answer");
    assert_eq!(answer.status, 200);
    let shown: Value = serde_json::from_slice(&answer.body).expect("json");
    assert_eq!(shown["id"], device.to_string());
    let gone = health_sync_client::edge::Edge::new(
        "https://localhost:1",
        Some(&cert_pem),
        key.clone(),
        std::time::Duration::from_secs(10),
    )
    .expect("edge client");
    assert!(
        gone.send(
            health_sync_client::edge::Request {
                method: "GET".into(),
                path: "/api/device".into(),
                ..Default::default()
            },
            None
        )
        .await
        .is_err(),
        "an edge that cannot be reached gives no answer"
    );

    let desktop = store("desktop").await;
    let registered = patient_seen_here(&edge).await;
    let recorded = write(&desktop, &observation()).await;
    let upstream = |key: SigningKey| {
        Sync::device(
            desktop.clone(),
            Upstream {
                origin: origin.clone(),
                ca_pem: Some(cert_pem.clone()),
                node: device.to_string().parse().expect("node id"),
            },
            key,
        )
        .expect("sync")
    };

    // Another key is not the enrolled device.
    let stranger = SigningKey::from_bytes(&[12; 32].into()).expect("key");
    assert!(upstream(stranger).round().await.is_err());

    let sync = upstream(key);
    let round = sync.round().await.expect("round");
    assert_eq!(round.verdict, Some(Verdict::Compatible));
    assert_eq!(round.pushed, 1, "the desktop's observation goes up");
    assert_eq!(round.pulled, 2, "the patient and their visit come down");
    for (resource_type, id) in [("Observation", recorded), ("Patient", registered)] {
        let at_desktop = version(&desktop, resource_type, id).await;
        assert!(at_desktop.is_some(), "{resource_type}/{id} at the desktop");
        assert_eq!(at_desktop, version(&edge, resource_type, id).await);
    }
    let again = sync.round().await.expect("round");
    assert_eq!((again.pushed, again.pulled), (0, 0));
}
