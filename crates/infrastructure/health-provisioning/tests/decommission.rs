//! Decommissioning an edge (#268; ADR 0048 § Decommissioning), over SurrealDB in memory with a
//! real root and issuing CA: the crypto-erase is recorded with the node's revocation, its escrowed
//! recovery keys are destroyed, it can sign nothing more, and the CRL central publishes makes its
//! chain fail under the root.

use std::sync::Arc;

use health_identity::{DecommissionRefused, Erasure, NodeId, NodeStatus, RecoveryVolume};
use health_pki::{Authority, IssuingCa, KeyPair, Trust, new_key, request};
use health_provisioning::{Error, LocalKeyWrap, NodeRegistry, recovery_jws};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{Clock as _, FixedClock, SignedDuration, Timestamp};
use p256::ecdsa::SigningKey;
use p256::pkcs8::DecodePrivateKey as _;

struct Central {
    registry: NodeRegistry,
    clock: Arc<FixedClock>,
    issuer: Authority,
    trust: Trust,
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
    Central {
        registry: NodeRegistry::new(
            SurrealStore::new(connection, clock.clone()),
            clock.clone(),
            "central",
        ),
        clock,
        issuer: Authority::from_pem(&issued.pem(), &key.serialize_pem()).expect("issuing"),
        trust: Trust::new(&root.certificate_pem()).expect("trust"),
    }
}

/// A provisioned edge: its id, its intermediate as an authority, and its signing key.
async fn provisioned(central: &Central) -> (NodeId, Authority, SigningKey, String) {
    let (text, _) = central
        .registry
        .issue_code("ug-kla-mulago", Vec::new(), None, "ops:auma")
        .await
        .expect("code");
    let edge: KeyPair = new_key().expect("key");
    let provisioned = central
        .registry
        .redeem(
            &text,
            &request(&edge, "edge").expect("csr"),
            true,
            &central.issuer,
        )
        .await
        .expect("provisioned");
    (
        provisioned.node.id,
        Authority::from_pem(&provisioned.chain_pem, &edge.serialize_pem()).expect("intermediate"),
        SigningKey::from_pkcs8_pem(&edge.serialize_pem()).expect("key"),
        provisioned.chain_pem,
    )
}

#[tokio::test]
async fn a_decommissioned_node_is_revoked_and_its_recovery_keys_destroyed() {
    let central = central().await;
    let kms = LocalKeyWrap::new(&[42; 32]);
    let (node, intermediate, key, chain) = provisioned(&central).await;
    let keys = [(RecoveryVolume::System, "111111-222222".to_owned())];
    central
        .registry
        .escrow(&recovery_jws(node, &keys, &key, central.clock.now()), &kms)
        .await
        .expect("escrowed");
    central
        .registry
        .read_recovery(node, RecoveryVolume::System, "ops:okello", "callback", &kms)
        .await
        .expect("read");
    assert_eq!(
        central
            .registry
            .used_recovery_keys()
            .await
            .expect("used")
            .len(),
        1
    );

    // Its served chain verifies under the root, before.
    let name = format!("{node}.local");
    let leaf = intermediate
        .issue_server(
            &new_key().expect("key"),
            std::slice::from_ref(&name),
            &[],
            central.clock.now(),
        )
        .expect("leaf");
    let served = format!("{}{chain}", leaf.pem());
    let later = central.clock.now();
    central
        .trust
        .verify_server(&served, &name, later)
        .expect("verifies before");

    // Recorded with who, how and why.
    assert_eq!(
        central
            .registry
            .decommission(node, "ops:okello", Erasure::TpmCleared, " ")
            .await,
        Err(Error::Decommission(DecommissionRefused::Incomplete))
    );
    central.clock.advance(SignedDuration::from_mins(1));
    let decommissioned = central
        .registry
        .decommission(node, "ops:okello", Erasure::TpmCleared, "hardware replaced")
        .await
        .expect("decommissioned");
    assert_eq!(decommissioned.status, NodeStatus::Decommissioned);
    let record = decommissioned.decommissioned.expect("record");
    assert_eq!(
        (record.erasure, record.by.as_str()),
        (Erasure::TpmCleared, "ops:okello")
    );
    assert_eq!(
        central
            .registry
            .nodes_at("ug-kla-mulago")
            .await
            .expect("nodes")[0]
            .status,
        NodeStatus::Decommissioned
    );

    // Its recovery keys are gone, and the alert with them.
    assert!(matches!(
        central
            .registry
            .read_recovery(node, RecoveryVolume::System, "ops:okello", "callback", &kms)
            .await,
        Err(Error::NotFound(_))
    ));
    assert_eq!(
        central.registry.used_recovery_keys().await.expect("used"),
        []
    );
    // It can sign nothing more.
    central.clock.advance(SignedDuration::from_mins(1));
    assert!(matches!(
        central
            .registry
            .escrow(&recovery_jws(node, &keys, &key, central.clock.now()), &kms)
            .await,
        Err(Error::Unauthorized(_))
    ));
    // Once only, and only a node that exists.
    assert_eq!(
        central
            .registry
            .decommission(node, "ops:okello", Erasure::TpmCleared, "again")
            .await,
        Err(Error::Decommission(DecommissionRefused::Already))
    );
    assert!(matches!(
        central
            .registry
            .decommission(
                NodeId(uuid::Uuid::now_v7()),
                "ops:okello",
                Erasure::TpmCleared,
                "x"
            )
            .await,
        Err(Error::NotFound(_))
    ));

    // The CRL central publishes makes its chain fail under the root.
    let crl = central.registry.crl(&central.issuer).await.expect("crl");
    let mut trust = central.trust;
    trust.add_crl(&crl).expect("crl");
    assert!(trust.verify_server(&served, &name, later).is_err());
    // Each CRL takes the next number.
    let next = central.registry.crl(&central.issuer).await.expect("crl");
    assert_ne!(crl, next);
}
