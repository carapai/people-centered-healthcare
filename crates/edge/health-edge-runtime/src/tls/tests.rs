use std::net::IpAddr;
use std::path::Path;

use health_pki::{Facility, IssuingCa, Trust, request};

use super::*;

const NODE: &str = "ug-kla-0001";

/// A root, an issuing CA and this node's intermediate, installed as `provision install` does;
/// the intermediate's key, as the keystore would give it.
fn provisioned(pki_dir: &Path) -> (String, String) {
    let now = Timestamp::now();
    let root = Authority::root("Test Root", now).expect("root");
    let issuing_key = new_key().expect("key");
    let issuing = root
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
        Authority::from_pem(&issuing.pem(), &issuing_key.serialize_pem()).expect("issuing");
    let key = new_key().expect("key");
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
    std::fs::create_dir_all(pki_dir).expect("pki");
    std::fs::write(pki_dir.join(ROOT), root.certificate_pem()).expect("root");
    std::fs::write(
        pki_dir.join(CHAIN),
        format!("{}{}", facility.pem(), issuing.certificate_pem()),
    )
    .expect("chain");
    (root.certificate_pem(), key.serialize_pem())
}

fn identity(dir: &Path, bind: &str, key: Option<String>) -> Identity {
    Identity {
        pki_dir: dir.join("pki"),
        tls_dir: dir.join("tls"),
        names: vec![format!("{NODE}.local")],
        bind: bind.parse().expect("ip"),
        key: Some(Arc::new(move || Ok(key.clone()))),
    }
}

fn state() -> CertificateState {
    Arc::new(Mutex::new(Certificate::unprovisioned()))
}

fn served(identity: &Identity) -> String {
    let leaf = std::fs::read_to_string(identity.tls_dir.join(LEAF)).expect("leaf");
    format!("{leaf}{}", identity.chain().expect("chain"))
}

#[test]
fn before_provisioning_nothing_is_served_but_loopback() {
    let dir = tempfile::tempdir().expect("dir");
    let state = state();
    let identity = identity(dir.path(), "192.168.1.10", None);
    assert!(matches!(prepare(&identity, &state), Listen::Loopback));
    assert_eq!(lock(&state).state, "not-provisioned");
}

#[test]
fn the_leaf_is_issued_kept_and_renewed_at_two_thirds_of_its_life() {
    let dir = tempfile::tempdir().expect("dir");
    let (root, key) = provisioned(&dir.path().join("pki"));
    let state = state();
    let identity = identity(dir.path(), "192.168.1.10", Some(key));

    let now = Timestamp::now();
    let (_, issued) = identity.refresh(&state, now).expect("issued");
    assert!(issued);
    let trust = Trust::new(&root).expect("trust");
    trust
        .verify_server(&served(&identity), &format!("{NODE}.local"), now)
        .expect("the leaf serves the node's name");
    trust
        .verify_server(&served(&identity), "192.168.1.10", now)
        .expect("and its address");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(identity.tls_dir.join(LEAF_KEY))
            .expect("key")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "the leaf key is the service's alone");
    }
    {
        let state = lock(&state);
        assert_eq!(state.state, "serving");
        assert_eq!(state.names, vec![format!("{NODE}.local")]);
        assert_eq!(
            state.addresses,
            vec!["192.168.1.10".parse::<IpAddr>().expect("ip")]
        );
        assert!(state.renews < state.expires);
    }

    // A restart reuses it.
    let first = std::fs::read_to_string(identity.tls_dir.join(LEAF)).expect("leaf");
    let (_, issued) = identity.refresh(&state, now).expect("kept");
    assert!(!issued);

    // Two-thirds of its life on, it is renewed.
    let renews = lock(&state).renews.expect("renews");
    let (_, issued) = identity
        .refresh(
            &state,
            renews
                .checked_add(SignedDuration::from_secs(1))
                .expect("time"),
        )
        .expect("renewed");
    assert!(issued);
    assert_ne!(
        std::fs::read_to_string(identity.tls_dir.join(LEAF)).expect("leaf"),
        first
    );
}

#[test]
fn a_new_address_reissues_the_leaf_and_says_so() {
    let dir = tempfile::tempdir().expect("dir");
    let (root, key) = provisioned(&dir.path().join("pki"));
    let state = state();
    identity(dir.path(), "192.168.1.10", Some(key.clone()))
        .refresh(&state, Timestamp::now())
        .expect("issued");
    assert_eq!(lock(&state).address_changed, None);

    let moved = identity(dir.path(), "192.168.1.11", Some(key));
    let (_, issued) = moved.refresh(&state, Timestamp::now()).expect("reissued");
    assert!(issued);
    assert!(lock(&state).address_changed.is_some());
    Trust::new(&root)
        .expect("trust")
        .verify_server(&served(&moved), "192.168.1.11", Timestamp::now())
        .expect("the new address");
}

#[test]
fn without_the_key_the_current_leaf_is_kept_while_valid() {
    let dir = tempfile::tempdir().expect("dir");
    let (_, key) = provisioned(&dir.path().join("pki"));
    let state = state();
    identity(dir.path(), "192.168.1.10", Some(key))
        .refresh(&state, Timestamp::now())
        .expect("issued");

    // The keystore is locked: renewal fails, the leaf is still served, and the state says why.
    let locked = identity(dir.path(), "192.168.1.10", None);
    let renews = lock(&state).renews.expect("renews");
    let (_, issued) = locked
        .refresh(
            &state,
            renews
                .checked_add(SignedDuration::from_secs(1))
                .expect("time"),
        )
        .expect("kept");
    assert!(!issued);
    assert!(lock(&state).problem.is_some());
    assert_eq!(lock(&state).state, "serving");

    // With no leaf at all, nothing can be served.
    let empty = tempfile::tempdir().expect("dir");
    provisioned(&empty.path().join("pki"));
    let state = self::state();
    let locked = identity(empty.path(), "192.168.1.10", None);
    assert!(matches!(prepare(&locked, &state), Listen::Loopback));
    assert_eq!(lock(&state).state, "invalid");
}
