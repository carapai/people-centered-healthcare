//! The provisioning bundle (ADR 0059): central signs it, its hashes are readable only with the
//! receiving edge's facility intermediate key, and an edge refuses one for another node, key or
//! signer.

use base64::Engine as _;
use health_auth::{
    Bundle, BundleAccount, BundleError, Credentials, SnapshotSigner, SnapshotVerifier,
};
use health_identity::{AccountId, AccountStatus};
use health_time::Timestamp;
use p256::SecretKey;
use p256::pkcs8::DecodePrivateKey;
use uuid::Uuid;

const NODE: &str = "ug-kla-0001";

fn bundle() -> Bundle {
    Bundle {
        facility: "ug-kla-mulago".into(),
        node_id: NODE.into(),
        issued_at: Timestamp::from_second(1_800_000_000).expect("time"),
        accounts: vec![BundleAccount {
            id: AccountId(Uuid::now_v7()),
            username: "nakato".into(),
            status: AccountStatus::Active,
            phc: Credentials::hash("correct horse battery").expect("hash"),
        }],
        snapshots: vec!["header.claims.signature".into()],
        local: health_auth::LocalConfig::default(),
    }
}

/// The edge's intermediate key as its keystore holds it: an rcgen key, PKCS#8 PEM.
fn edge_key() -> SecretKey {
    let pem = health_pki::new_key().expect("key").serialize_pem();
    SecretKey::from_pkcs8_pem(&pem).expect("the intermediate key is a P-256 key")
}

#[test]
fn central_signs_and_only_the_receiving_edge_reads_the_hashes() {
    let central = SnapshotSigner::generate();
    let trusted = SnapshotVerifier::new(&[central.public()]).expect("verifier");
    let edge = edge_key();
    let sent = bundle();
    let jws = sent.seal(&central, &edge.public_key()).expect("seal");

    // In transit, the hashes are ciphertext.
    let payload = jws.split('.').nth(1).expect("payload");
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .expect("b64");
    let text = String::from_utf8_lossy(&decoded);
    assert!(!text.contains("argon2"), "{text}");
    assert!(text.contains("nakato"));

    let opened = Bundle::open(&jws, &trusted, NODE, &edge).expect("open");
    assert_eq!(opened, sent);
    // And the hash verifies a password, as on any node.
    let account = &opened.accounts[0];
    let credentials = Credentials::new([7; 32].to_vec()).expect("credentials");
    let stored = credentials.seal(account.id, &account.phc).expect("seal");
    assert!(credentials.verify(account.id, "correct horse battery", &stored));
}

#[test]
fn an_edge_refuses_a_bundle_for_another_node_key_or_signer() {
    let central = SnapshotSigner::generate();
    let trusted = SnapshotVerifier::new(&[central.public()]).expect("verifier");
    let edge = edge_key();
    let jws = bundle().seal(&central, &edge.public_key()).expect("seal");

    assert_eq!(
        Bundle::open(&jws, &trusted, "ug-kla-0002", &edge),
        Err(BundleError::WrongNode(NODE.into()))
    );
    assert_eq!(
        Bundle::open(&jws, &trusted, NODE, &edge_key()),
        Err(BundleError::WrongKey)
    );
    let impostor = SnapshotSigner::generate().public();
    let untrusted = SnapshotVerifier::new(&[impostor]).expect("verifier");
    assert_eq!(
        Bundle::open(&jws, &untrusted, NODE, &edge),
        Err(BundleError::Signature)
    );
    // A snapshot JWS is not a bundle, even from central's own key.
    assert_eq!(
        Bundle::open("a.b.c", &trusted, NODE, &edge),
        Err(BundleError::Signature)
    );
}
