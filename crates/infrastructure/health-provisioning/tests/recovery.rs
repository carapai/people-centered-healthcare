//! Escrowing edges' disk recovery keys (#268; ADR 0048), over SurrealDB in memory: a key is
//! escrowed only by its node's signature, wrapped and never stored in the clear; a replay cannot
//! bring back a key that was read out; and each read-out is recorded and leaves the key used
//! until the edge escrows its replacement.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use health_identity::{EscrowRefused, NodeId, RecoveryVolume};
use health_persistence::Store as _;
use health_pki::{Authority, IssuingCa, KeyPair, new_key, request};
use health_provisioning::{
    Error, KeyWrap, LocalKeyWrap, NodeRegistry, recovery_jws, recovery_router,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{Clock as _, FixedClock, SignedDuration, Timestamp};
use p256::ecdsa::SigningKey;
use p256::pkcs8::DecodePrivateKey as _;
use tower::ServiceExt as _;

const SYSTEM_KEY: &str = "111111-222222-333333-444444-555555-666666-777777-888888";
const DATA_KEY: &str = "fy3k-vmd2-x9qt-c4zd";

struct Central {
    registry: NodeRegistry,
    store: SurrealStore,
    clock: Arc<FixedClock>,
    kms: Arc<LocalKeyWrap>,
}

async fn central() -> Central {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "central".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let clock = Arc::new(FixedClock::new(
        "2026-11-04T09:00:00Z".parse().expect("timestamp"),
    ));
    let store = SurrealStore::new(connection, clock.clone());
    Central {
        registry: NodeRegistry::new(store.clone(), clock.clone(), "central"),
        store,
        clock,
        kms: Arc::new(LocalKeyWrap::new(&[42; 32])),
    }
}

/// A provisioned edge: its node id and its facility intermediate's key.
async fn provisioned(central: &Central) -> (NodeId, SigningKey) {
    let at: Timestamp = "2026-01-01T00:00:00Z".parse().expect("timestamp");
    let root = Authority::root("Root", at).expect("root");
    let key = new_key().expect("key");
    let issued = root
        .issue_issuing_ca(
            &request(&key, "issuing").expect("csr"),
            &IssuingCa {
                common_name: "Issuing".into(),
                dns_suffixes: Vec::new(),
            },
            at,
        )
        .expect("issuing");
    let issuer = Authority::from_pem(&issued.pem(), &key.serialize_pem()).expect("issuing");
    let (text, _) = central
        .registry
        .issue_code("ug-kla-mulago", Vec::new(), None, "ops:auma")
        .await
        .expect("code");
    let edge: KeyPair = new_key().expect("key");
    let node = central
        .registry
        .redeem(&text, &request(&edge, "edge").expect("csr"), true, &issuer)
        .await
        .expect("provisioned")
        .node;
    let signing = SigningKey::from_pkcs8_pem(&edge.serialize_pem()).expect("p256 key");
    (node.id, signing)
}

fn keys() -> Vec<(RecoveryVolume, String)> {
    vec![
        (RecoveryVolume::System, SYSTEM_KEY.to_owned()),
        (RecoveryVolume::Data, DATA_KEY.to_owned()),
    ]
}

#[tokio::test]
async fn a_node_escrows_its_keys_and_an_operator_reads_one_out() {
    let central = central().await;
    let (node, key) = provisioned(&central).await;
    let now = central.clock.now();
    let jws = recovery_jws(node, &keys(), &key, now);
    assert_eq!(
        central.registry.escrow(&jws, central.kms.as_ref()).await,
        Ok((node, vec![RecoveryVolume::System, RecoveryVolume::Data]))
    );

    // Wrapped at rest: neither the key nor anything that decodes to it is stored.
    let mut tx = central.store.begin().await.expect("tx");
    let (json, wrapped) = tx
        .recovery_key(&node.to_string(), "system")
        .await
        .expect("read")
        .expect("escrowed");
    assert!(!json.contains(SYSTEM_KEY) && !wrapped.contains(SYSTEM_KEY));
    let bytes = URL_SAFE_NO_PAD.decode(&wrapped).expect("base64url");
    assert!(
        !bytes
            .windows(SYSTEM_KEY.len())
            .any(|w| w == SYSTEM_KEY.as_bytes())
    );
    drop(tx);

    // A read-out needs who and how the caller was verified.
    assert!(matches!(
        central
            .registry
            .read_recovery(
                node,
                RecoveryVolume::System,
                "ops:okello",
                " ",
                central.kms.as_ref()
            )
            .await,
        Err(Error::Malformed(_))
    ));
    assert_eq!(
        central.registry.used_recovery_keys().await.expect("used"),
        []
    );
    let read = central
        .registry
        .read_recovery(
            node,
            RecoveryVolume::System,
            "ops:okello",
            "called the in-charge back on the registry's number",
            central.kms.as_ref(),
        )
        .await
        .expect("read out");
    assert_eq!(read, SYSTEM_KEY);
    let used = central.registry.used_recovery_keys().await.expect("used");
    assert_eq!(used.len(), 1);
    assert_eq!(
        (used[0].node, used[0].volume),
        (node, RecoveryVolume::System)
    );
    assert_eq!(used[0].retrievals[0].by, "ops:okello");

    // The replayed escrow cannot bring the read-out key back.
    assert_eq!(
        central.registry.escrow(&jws, central.kms.as_ref()).await,
        Err(Error::Escrow(EscrowRefused::Stale))
    );
    // The edge rotates it: a newer escrow replaces it, and the alert clears.
    central.clock.advance(SignedDuration::from_mins(10));
    let rotated = recovery_jws(
        node,
        &[(RecoveryVolume::System, "999999-000000".to_owned())],
        &key,
        central.clock.now(),
    );
    central
        .registry
        .escrow(&rotated, central.kms.as_ref())
        .await
        .expect("rotated");
    assert_eq!(
        central.registry.used_recovery_keys().await.expect("used"),
        []
    );
    assert_eq!(
        central
            .registry
            .read_recovery(
                node,
                RecoveryVolume::System,
                "ops:okello",
                "callback",
                central.kms.as_ref()
            )
            .await
            .expect("read"),
        "999999-000000"
    );
    // The data volume's key was never read: it is untouched by the rotation.
    assert_eq!(
        central
            .registry
            .read_recovery(
                node,
                RecoveryVolume::Data,
                "ops:okello",
                "callback",
                central.kms.as_ref()
            )
            .await
            .expect("read"),
        DATA_KEY
    );
}

#[tokio::test]
async fn only_an_active_node_escrows_with_its_own_key_and_on_time() {
    let central = central().await;
    let (node, key) = provisioned(&central).await;
    let now = central.clock.now();
    let unauthorized = |result| matches!(result, Err(Error::Unauthorized(_)));

    let impostor = SigningKey::random(&mut rand_core::OsRng);
    assert!(unauthorized(
        central
            .registry
            .escrow(
                &recovery_jws(node, &keys(), &impostor, now),
                central.kms.as_ref()
            )
            .await
    ));
    let stranger = NodeId(uuid::Uuid::now_v7());
    assert!(unauthorized(
        central
            .registry
            .escrow(
                &recovery_jws(stranger, &keys(), &key, now),
                central.kms.as_ref()
            )
            .await
    ));
    let early = now.checked_sub(SignedDuration::from_mins(6)).expect("time");
    assert_eq!(
        central
            .registry
            .escrow(
                &recovery_jws(node, &keys(), &key, early),
                central.kms.as_ref()
            )
            .await,
        Err(Error::Escrow(EscrowRefused::Clock))
    );
    for malformed in ["", "a.b", "not.a.jws"] {
        assert!(
            matches!(
                central
                    .registry
                    .escrow(malformed, central.kms.as_ref())
                    .await,
                Err(Error::Malformed(_))
            ),
            "{malformed:?}"
        );
    }
    assert!(matches!(
        central
            .registry
            .escrow(&recovery_jws(node, &[], &key, now), central.kms.as_ref())
            .await,
        Err(Error::Malformed(_))
    ));
    assert!(matches!(
        central
            .registry
            .read_recovery(
                node,
                RecoveryVolume::System,
                "ops:okello",
                "callback",
                central.kms.as_ref()
            )
            .await,
        Err(Error::NotFound(_))
    ));
    // Another master key cannot unwrap what this one wrapped.
    central
        .registry
        .escrow(
            &recovery_jws(node, &keys(), &key, now),
            central.kms.as_ref(),
        )
        .await
        .expect("escrowed");
    assert!(matches!(
        central
            .registry
            .read_recovery(
                node,
                RecoveryVolume::System,
                "ops:okello",
                "callback",
                &LocalKeyWrap::new(&[1; 32])
            )
            .await,
        Err(Error::Store(_))
    ));
}

#[tokio::test]
async fn the_route_answers_an_edge() {
    let central = central().await;
    let (node, key) = provisioned(&central).await;
    let kms: Arc<dyn KeyWrap> = central.kms.clone();
    let app = recovery_router(central.registry.clone(), kms);
    let post = |body: String| {
        let app = app.clone();
        async move {
            app.oneshot(
                Request::post("/provisioning/recovery")
                    .header("content-type", "application/jose")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response")
            .status()
        }
    };
    let jws = recovery_jws(node, &keys(), &key, central.clock.now());
    assert_eq!(post(jws.clone()).await, StatusCode::NO_CONTENT);
    assert_eq!(post(jws).await, StatusCode::CONFLICT);
    let impostor = SigningKey::random(&mut rand_core::OsRng);
    assert_eq!(
        post(recovery_jws(node, &keys(), &impostor, central.clock.now())).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(post("garbage".into()).await, StatusCode::BAD_REQUEST);
}
