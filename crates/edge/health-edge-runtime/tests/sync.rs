//! The edge's sync task against central's `/sync/v1` endpoints over HTTPS (#81; ADR 0062), in
//! process. The edge writes before it ever reaches central, and while central is down, and both
//! sides still end up holding the same records.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;

use health_auth::{
    AuthStore as _, Credentials, Provisioning, SnapshotSigner, SnapshotVerifier, SurrealAuthStore,
};
use health_edge_runtime::CentralAccounts;
use health_edge_runtime::sync::{Sync, Upstream};
use health_identifiers::{NodeId, OpId, ResourceId};
use health_identity::snapshot::{
    PermissionSnapshot, RoleAssignment, RolePermissions, SnapshotPolicy,
};
use health_identity::{AccountId, AccountStatus, Lockout, LoginAccount, SessionPolicy};
use health_persistence::{Precondition, RawJson, Store as _, Transaction as _, WriteRequest};
use health_pki::{Authority, IssuingCa, KeyPair, new_key, request};
use health_provisioning::NodeRegistry;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_sync_protocol::{Compatibility, PROTOCOL, Verdict};
use health_sync_server::{AccountExport, SyncServer};
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
    /// Central's Login Accounts and snapshots, sent to the edge each round (#889).
    accounts: SurrealAuthStore,
    credentials: Credentials,
    signer: SnapshotSigner,
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
    let accounts = SurrealAuthStore::new(store.clone(), SessionPolicy::default().hard_cap);
    let credentials = Credentials::new([5; 32].to_vec()).expect("credentials");
    let signer = SnapshotSigner::generate();
    let export = AccountExport::new(
        accounts.clone(),
        credentials.clone(),
        signer.clone(),
        health_auth::LocalConfig {
            roles: roles(),
            ..health_auth::LocalConfig::default()
        },
        registry.clone(),
    );
    let app = SyncServer::new(store.clone(), registry, compatibility)
        .with_blobs(files.clone())
        .with_accounts(export)
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
        accounts,
        credentials,
        signer,
    }
}

fn roles() -> RolePermissions {
    RolePermissions(std::collections::BTreeMap::from([
        ("clinician".into(), BTreeSet::from(["record.read".into()])),
        ("nurse".into(), BTreeSet::from(["record.read".into()])),
    ]))
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

impl Central {
    /// A Login Account at central for `username`, with `password`.
    async fn account(&self, username: &str, password: &str) -> AccountId {
        let id = AccountId(uuid::Uuid::now_v7());
        let phc = Credentials::hash(password).expect("hash");
        self.accounts
            .put_account(&LoginAccount {
                id,
                username: username.into(),
                credential: self.credentials.seal(id, &phc).expect("seal"),
                status: AccountStatus::Active,
                lockout: Lockout::default(),
            })
            .await
            .expect("account");
        id
    }

    /// Assigns `roles` to `account` here at the facility, as Roles at places does: a newly
    /// signed snapshot. Its JWS.
    async fn assign(&self, account: AccountId, assigned: &[&str]) -> String {
        let issued = Timestamp::now();
        let snapshot = PermissionSnapshot {
            account,
            facility: FACILITY.into(),
            roles: assigned
                .iter()
                .map(|role| RoleAssignment {
                    role: (*role).into(),
                    locations: Vec::new(),
                })
                .collect(),
            programs: Vec::new(),
            issued_at: issued,
            not_after: SnapshotPolicy::default().not_after(issued),
            snapshot_version: u64::try_from(issued.as_second()).expect("version"),
        };
        let jws = self.signer.sign(&snapshot, &roles()).expect("sign");
        self.accounts
            .put_snapshot(account, FACILITY, &jws)
            .await
            .expect("snapshot");
        jws
    }

    /// The edge's import of what central sends it, into `edge` under its own pepper key.
    fn edge_accounts(&self, edge: &SurrealAuthStore, credentials: &Credentials) -> CentralAccounts {
        // The facility intermediate's key, which the edge's requests are signed with too.
        let key = p256::SecretKey::from_bytes(&self.key.to_bytes()).expect("key");
        CentralAccounts(Provisioning::new(
            edge.clone(),
            credentials.clone(),
            SnapshotVerifier::new(&[self.signer.public()]).expect("verifier"),
            self.node.to_string(),
            FACILITY,
            Arc::new(move || Ok(key.clone())),
        ))
    }
}

/// Whether `account` may sign in at the edge at `now` with `password`, by what the edge holds:
/// its password under the edge's pepper key, and a snapshot central signed for this facility,
/// unexpired. What the edge's authorization server checks (`SnapshotGate`), with no central.
async fn signs_in_at_edge(
    edge: &SurrealAuthStore,
    credentials: &Credentials,
    verifier: &SnapshotVerifier,
    account: AccountId,
    password: &str,
) -> bool {
    let Some(stored) = edge.account(account).await.expect("account") else {
        return false;
    };
    let Some(jws) = edge.snapshot(account, FACILITY).await.expect("snapshot") else {
        return false;
    };
    credentials.verify(account, password, &stored.credential)
        && verifier.verify(&jws).is_ok_and(|snapshot| {
            snapshot.account == account
                && snapshot.facility == FACILITY
                && SnapshotPolicy::default()
                    .check(&snapshot, Timestamp::now())
                    .is_ok()
        })
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
async fn a_signed_consent_scan_comes_down_ahead_of_need_and_other_attachments_wait() {
    use health_sync_protocol::{ContentHash, blob_url};
    let central = central().await;
    let edge = store("edge").await;
    let dir = tempfile::tempdir().expect("edge files");
    let files = health_files::FsBlobStore::open(dir.path()).expect("edge blob store");
    let sync = central
        .sync(&edge, &central.origin)
        .with_blobs(files.clone());
    let patient = patient_seen_here(&central.store).await;
    let attachment = |hash: &health_files::BlobHash, size: usize, kind: &str| {
        let hash: ContentHash = hash.to_string().parse().expect("hash");
        json!({"url": blob_url(&hash), "size": size, "contentType": kind})
    };

    // A signed consent whose scan central holds, and one whose scan has not reached it yet.
    let signed = b"a signed consent".to_vec();
    let held = central.files.put(signed.as_slice()).expect("put").blob;
    let late = b"a consent scan still uploading".to_vec();
    let late_hash = health_files::BlobHash::of(&late);
    for (hash, size) in [(&held.hash, signed.len()), (&late_hash, late.len())] {
        write(
            &central.store,
            &json!({
                "resourceType": "Consent",
                "status": "active",
                "subject": {"reference": format!("Patient/{patient}")},
                "sourceAttachment": [attachment(hash, size, "application/pdf")]
            }),
        )
        .await;
    }
    // A wound photo: fetched only when first opened.
    let photo = b"a wound photo".to_vec();
    let photo_hash = central.files.put(photo.as_slice()).expect("put").blob.hash;
    write(
        &central.store,
        &json!({
            "resourceType": "DocumentReference",
            "status": "current",
            "meta": {"tag": [{"system": "urn:health:facility", "code": FACILITY}]},
            "content": [{"attachment": attachment(&photo_hash, photo.len(), "image/jpeg")}]
        }),
    )
    .await;

    let round = sync.round().await.expect("round");
    assert_eq!(round.fetched, 1, "{round:?}");
    assert_eq!(files.read(&held.hash).expect("pre-fetched"), signed);
    assert!(!files.contains(&photo_hash), "on demand only");
    assert!(!files.contains(&late_hash));

    // The late scan reaches central; the next round brings it down, and nothing is fetched twice.
    central.files.put(late.as_slice()).expect("put");
    assert_eq!(sync.round().await.expect("round").fetched, 1);
    assert_eq!(files.read(&late_hash).expect("pre-fetched"), late);
    assert_eq!(sync.round().await.expect("round").fetched, 0);
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

/// Roles at places (#889): a role assigned at central works on the edge after its next sync, and
/// offline after that; one withdrawn at central is refused after the next; and the snapshot the
/// edge holds is exactly the one central signed.
#[tokio::test]
async fn roles_assigned_and_withdrawn_at_central_reach_the_edge_with_its_next_sync() {
    const PASSWORD: &str = "correct horse battery";
    let central = central().await;
    let edge = store("edge").await;
    let edge_auth = SurrealAuthStore::new(edge.clone(), SessionPolicy::default().hard_cap);
    let edge_credentials = Credentials::new([9; 32].to_vec()).expect("credentials");
    let verifier = SnapshotVerifier::new(&[central.signer.public()]).expect("verifier");
    let sync = central.sync(&edge, &central.origin).with_accounts(Arc::new(
        central.edge_accounts(&edge_auth, &edge_credentials),
    ));
    let nakato = central.account("nakato", PASSWORD).await;
    let okello = central.account("okello", PASSWORD).await;
    central.assign(okello, &["nurse"]).await;
    assert_eq!(sync.round().await.expect("round").accounts, Some((1, 0)));
    assert!(
        !signs_in_at_edge(&edge_auth, &edge_credentials, &verifier, nakato, PASSWORD).await,
        "no role at the facility yet"
    );

    // Assigned at central: it works on the edge after its next sync.
    let signed = central.assign(nakato, &["clinician"]).await;
    assert_eq!(sync.round().await.expect("round").accounts, Some((2, 0)));
    assert!(signs_in_at_edge(&edge_auth, &edge_credentials, &verifier, nakato, PASSWORD).await);
    // The snapshot the edge holds is the one central signed, byte for byte.
    assert_eq!(
        edge_auth.snapshot(nakato, FACILITY).await.expect("read"),
        Some(signed)
    );
    let stored = edge_auth
        .account(nakato)
        .await
        .expect("read")
        .expect("account");
    assert!(
        !stored.credential.contains("argon2"),
        "the hash is kept under the edge's pepper key"
    );

    // Offline thereafter: central cannot be reached, and the edge still signs the user in.
    let offline = central
        .sync(&edge, "https://localhost:1")
        .with_accounts(Arc::new(
            central.edge_accounts(&edge_auth, &edge_credentials),
        ));
    assert!(offline.round().await.is_err());
    assert!(signs_in_at_edge(&edge_auth, &edge_credentials, &verifier, nakato, PASSWORD).await);

    // Changed at central: the edge holds the new snapshot after its next sync.
    let changed = central.assign(nakato, &["clinician", "nurse"]).await;
    sync.round().await.expect("round");
    assert_eq!(
        edge_auth.snapshot(nakato, FACILITY).await.expect("read"),
        Some(changed)
    );

    // Withdrawn at central: refused on the edge after its next sync; the others keep theirs.
    central
        .accounts
        .remove_snapshot(nakato, FACILITY)
        .await
        .expect("withdraw");
    assert!(
        signs_in_at_edge(&edge_auth, &edge_credentials, &verifier, nakato, PASSWORD).await,
        "until the edge syncs"
    );
    assert_eq!(sync.round().await.expect("round").accounts, Some((1, 1)));
    assert!(!signs_in_at_edge(&edge_auth, &edge_credentials, &verifier, nakato, PASSWORD).await);
    assert_eq!(
        edge_auth.snapshot(nakato, FACILITY).await.expect("read"),
        None
    );
    assert!(signs_in_at_edge(&edge_auth, &edge_credentials, &verifier, okello, PASSWORD).await);
}

/// An edge whose central sends no accounts (no snapshot key or access policy there) syncs as
/// before, keeping the staff its provisioning bundle gave it.
#[tokio::test]
async fn an_edge_whose_central_sends_no_accounts_keeps_its_staff() {
    let central = central().await;
    let edge = store("edge").await;
    let edge_auth = SurrealAuthStore::new(edge.clone(), SessionPolicy::default().hard_cap);
    let edge_credentials = Credentials::new([9; 32].to_vec()).expect("credentials");
    // A central without the export, on the same store and registry.
    let registry = NodeRegistry::new(central.store.clone(), Arc::new(SystemClock), "central");
    let plain = SyncServer::new(
        central.store.clone(),
        registry,
        Compatibility {
            protocol: PROTOCOL,
            software: env!("CARGO_PKG_VERSION").parse().expect("version"),
            approved: BTreeSet::new(),
        },
    )
    .router();
    let (listener, origin, cert_pem) = tls().await;
    tokio::spawn(async move {
        let _ = axum::serve(listener, plain).await;
    });
    let account = AccountId(uuid::Uuid::now_v7());
    edge_auth
        .put_snapshot(account, FACILITY, "kept")
        .await
        .expect("snapshot");
    let sync = Sync::new(
        edge.clone(),
        Upstream {
            origin,
            ca_pem: Some(cert_pem),
            node: central.node,
        },
        central.key.clone(),
    )
    .expect("sync")
    .with_accounts(Arc::new(
        central.edge_accounts(&edge_auth, &edge_credentials),
    ));
    assert_eq!(sync.round().await.expect("round").accounts, None);
    assert_eq!(
        edge_auth.snapshot(account, FACILITY).await.expect("read"),
        Some("kept".into())
    );
}

/// A summary `MeasureReport` of ANC 1st visits at this facility, as the edge computes it.
fn anc_summary(count: u64) -> health_analytics::StoredSummary {
    let march = health_analytics::Period::parse(health_analytics::PeriodType::Monthly, "2026-03")
        .expect("march");
    health_analytics::StoredSummary {
        measure: "https://x/Measure/anc1|1.0.0".into(),
        period: march,
        facility: FACILITY.into(),
        report: json!({
            "resourceType": "MeasureReport",
            "meta": {"tag": [{"system": "urn:health:facility", "code": FACILITY}]},
            "status": "complete",
            "type": "summary",
            "measure": "https://x/Measure/anc1|1.0.0",
            "date": "2026-04-01T06:00:00Z",
            "reporter": {"identifier": {"system": "urn:health:facility", "value": FACILITY}},
            "period": {"start": "2026-03-01", "end": "2026-03-31"},
            "group": [{"population": [{"code": {"coding": [{"code": "initial-population"}]}, "count": count}]}]
        })
        .to_string(),
    }
}

#[tokio::test]
async fn indicator_summaries_reach_central_and_nothing_patient_level_does() {
    use health_analytics::{FacilitySummaries as _, IndicatorStore as _, Position};

    let central = central().await;
    let edge = store("edge").await;
    let march = anc_summary(0).period;
    let mut tx = edge.begin().await.expect("begin");
    tx.set_indicator_position(&Position {
        epoch: "e".into(),
        seq: 0,
    })
    .await
    .expect("position");
    tx.put_summary(&anc_summary(3)).await.expect("summary");
    tx.commit().await.expect("commit");

    let sync = central.sync(&edge, &central.origin);
    assert_eq!(sync.round().await.expect("round").summaries, 1);
    assert_eq!(sync.round().await.expect("round").summaries, 0, "sent once");
    let mut tx = central.store.begin().await.expect("begin");
    let kept = tx
        .facility_summaries("https://x/Measure/anc1|1.0.0", &march)
        .await
        .expect("kept");
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].facility, FACILITY);
    let report: Value = serde_json::from_str(&kept[0].report).expect("json");
    assert_eq!(report["group"][0]["population"][0]["count"], 3);
    assert!(
        tx.summaries_synced()
            .await
            .expect("synced")
            .contains_key(FACILITY),
        "the facility's last sync is kept"
    );
    drop(tx);

    // A summary that names a patient is refused, and the round goes on.
    let mut named = anc_summary(4);
    let mut report: Value = serde_json::from_str(&named.report).expect("json");
    report["subject"] = json!({"reference": "Patient/p-1"});
    named.report = report.to_string();
    let mut tx = edge.begin().await.expect("begin");
    tx.put_summary(&named).await.expect("summary");
    tx.commit().await.expect("commit");
    assert_eq!(sync.round().await.expect("round").summaries, 0);
    let mut tx = central.store.begin().await.expect("begin");
    let kept = tx
        .facility_summaries("https://x/Measure/anc1|1.0.0", &march)
        .await
        .expect("kept");
    let report: Value = serde_json::from_str(&kept[0].report).expect("json");
    assert_eq!(report["group"][0]["population"][0]["count"], 3, "unchanged");
    assert!(!kept[0].report.contains("Patient/"));
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
async fn an_allowed_release_is_fetched_in_ranges_verified_and_staged_and_never_older() {
    use health_configuration::activation::Chain;
    use health_configuration::rollout::Target;
    use health_edge_runtime::sync::releases::{ReleaseStep, Releases};
    use health_edge_runtime::update::{ChannelStager, Updates, pack};
    use p256::ecdsa::signature::Signer as _;

    let central = central().await;
    let edge = store("edge").await;
    let dir = tempfile::tempdir().expect("edge files");
    let files = health_files::FsBlobStore::open(dir.path().join("files")).expect("edge blob store");
    let updates = Updates::new(dir.path().join("updates"));
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();

    // Release 9.9.0: the edge's package, signed with the release key, and a desktop installer
    // larger than one range.
    let release_key = SigningKey::from_slice(&[5; 32]).expect("release key");
    let package = pack(b"the new edge", "facility-edge", "9.9.0").expect("pack");
    let signature: p256::ecdsa::Signature = release_key.sign(&package);
    let installer: Vec<u8> = (0..5 * 1024 * 1024 + 17).map(|i| (i % 251) as u8).collect();
    for bytes in [&package, &installer] {
        central
            .files
            .put_limited(bytes.as_slice(), health_files::MAX_RELEASE_SIZE)
            .expect("central blob");
    }
    let sha = |bytes: &[u8]| health_files::BlobHash::of(bytes).to_string();
    let rollout = |id: &str, version: &str, scope: Value, issued: &str| {
        json!({
            "rollout": {
                "id": id, "version": version, "scope": scope,
                "effectiveAt": issued, "issuedAt": issued, "by": "release-manager"
            },
            "artifacts": [
                {"target": "edge-linux-x64", "file": "facility-edge-9.9.0-linux-x64.tar.gz",
                 "sha256": sha(&package), "size": package.len(), "signature": hex(&signature.to_bytes())},
                {"target": "desktop-windows-x64", "file": "health-platform-9.9.0-x64-setup.exe",
                 "sha256": sha(&installer), "size": installer.len(), "signature": "updater"}
            ]
        })
    };
    let issue = |body: Value| {
        let store = central.store.clone();
        async move {
            let mut tx = store.begin().await.expect("begin");
            tx.put_release_rollout(
                body["rollout"]["id"].as_str().expect("id"),
                body["rollout"]["issuedAt"].as_str().expect("issued"),
                &body.to_string(),
            )
            .await
            .expect("roll-out");
            tx.commit().await.expect("commit");
        }
    };
    issue(rollout(
        "r-1",
        "9.9.0",
        json!({"level": "national"}),
        "2026-01-01T00:00:00Z",
    ))
    .await;

    let releases = Releases {
        chain: Chain {
            facility: Some(FACILITY.into()),
            ..Chain::default()
        },
        own: Target::EdgeLinux,
        running: "0.1.0".parse().expect("version"),
        downstream: vec![Target::DesktopWindows, Target::DesktopLinux],
        stager: Some(Arc::new(ChannelStager::new(
            updates.clone(),
            *release_key.verifying_key(),
        ))),
    };
    let sync = central
        .sync(&edge, &central.origin)
        .with_blobs(files.clone())
        .with_releases(releases.clone());
    let round = sync.round().await.expect("round");
    assert_eq!(
        round.release,
        Some(ReleaseStep::Staged("9.9.0".parse().expect("version"))),
        "{round:?}"
    );
    assert_eq!(
        updates
            .staged()
            .expect("staged")
            .map(|m| m.version)
            .as_deref(),
        Some("9.9.0")
    );
    // The desktops' installer is kept here for the LAN, whole and verified.
    assert_eq!(
        files
            .read(&health_files::BlobHash::of(&installer))
            .expect("installer"),
        installer
    );
    // Staged already: the next round leaves it.
    assert_eq!(sync.round().await.expect("round").release, None);

    // This facility is held on an older release: the edge stays, and says so.
    issue(rollout(
        "r-2",
        "0.0.9",
        json!({"level": "facility", "facility": FACILITY}),
        "2026-01-02T00:00:00Z",
    ))
    .await;
    assert_eq!(
        sync.round().await.expect("round").release,
        Some(ReleaseStep::Held("0.0.9".parse().expect("version")))
    );

    // A package signed by another key is never staged.
    let fresh = tempfile::tempdir().expect("fresh");
    let other = SigningKey::from_slice(&[6; 32]).expect("other key");
    let edge2 = store("edge2").await;
    let sync = central
        .sync(&edge2, &central.origin)
        .with_blobs(health_files::FsBlobStore::open(fresh.path().join("files")).expect("files"))
        .with_releases(Releases {
            chain: Chain::default(),
            stager: Some(Arc::new(ChannelStager::new(
                Updates::new(fresh.path().join("updates")),
                *other.verifying_key(),
            ))),
            ..releases
        });
    assert_eq!(
        sync.round().await.expect("round").release,
        None,
        "refused, and the round goes on"
    );
    assert_eq!(
        Updates::new(fresh.path().join("updates"))
            .staged()
            .expect("staged"),
        None
    );
}

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
