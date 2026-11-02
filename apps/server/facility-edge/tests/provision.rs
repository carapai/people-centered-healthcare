//! The edge's half of provisioning through the built binary (lan-security.md; ADRs 0048, 0058):
//! the encryption gate before any key exists, and installing only a chain that serves.
//!
//! The service reads its unsealed key from `$CREDENTIALS_DIRECTORY`, as systemd provides it,
//! so these tests stand in for the keystore there. Sealing with systemd-creds needs root, and is
//! run under systemd in edge-build.yml.

#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::Command;

use common::BINARY;
use health_pki::{Authority, Facility, IssuingCa, KeyPair, new_key, request};
use health_time::Timestamp;

const NODE: &str = "ug-kla-0001";
const KEY: &str = "health.facility-intermediate.key";

struct Pki {
    root: Authority,
    issuing: Authority,
}

fn pki() -> Pki {
    let now = Timestamp::now();
    let root = Authority::root("Test Root", now).expect("root");
    let key = new_key().expect("key");
    let issued = root
        .issue_issuing_ca(
            &request(&key, "x").expect("csr"),
            &IssuingCa {
                common_name: "Test Issuing CA".into(),
                dns_suffixes: Vec::new(),
            },
            now,
        )
        .expect("issuing");
    let issuing = Authority::from_pem(&issued.pem(), &key.serialize_pem()).expect("issuing");
    Pki { root, issuing }
}

impl Pki {
    /// The chain central returns for `key`'s request, issued to `node`.
    fn chain_for(&self, key: &KeyPair, node: &str) -> String {
        let facility = self
            .issuing
            .issue_facility(
                &request(key, node).expect("csr"),
                &Facility {
                    node_id: node.into(),
                    dns_names: Vec::new(),
                },
                Timestamp::now(),
            )
            .expect("facility");
        format!("{}{}", facility.pem(), self.issuing.certificate_pem())
    }
}

fn provision(dir: &Path, args: &[&str], credentials: &Path) -> (Option<i32>, String) {
    let output = Command::new(BINARY)
        .arg("provision")
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HEALTH_DATA_DIR", dir.join("store"))
        .env("HEALTH_NODE_ID", NODE)
        .env("CREDENTIALS_DIRECTORY", credentials)
        .output()
        .expect("run");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned()
            + &String::from_utf8_lossy(&output.stderr),
    )
}

#[test]
fn only_a_chain_that_serves_this_node_is_installed() {
    let dir = tempfile::tempdir().expect("dir");
    let credentials = dir.path().join("credentials");
    std::fs::create_dir_all(&credentials).expect("dir");
    let key = new_key().expect("key");
    std::fs::write(credentials.join(KEY), key.serialize_pem()).expect("key");
    let pki = pki();
    let write = |name: &str, contents: &str| {
        let path = dir.path().join(name);
        std::fs::write(&path, contents).expect("write");
        path.display().to_string()
    };
    let root = write("root.pem", &pki.root.certificate_pem());

    // Issued to another node: it cannot certify this node's name.
    let other = write("other.pem", &pki.chain_for(&key, "ug-kla-0002"));
    let (code, output) = provision(
        dir.path(),
        &["install", "--chain", &other, "--root", &root],
        &credentials,
    );
    assert_eq!(code, Some(11), "{output}");
    assert!(output.contains("cannot certify"), "{output}");

    // Issued for someone else's key.
    let stranger = write(
        "stranger.pem",
        &pki.chain_for(&new_key().expect("key"), NODE),
    );
    let (code, output) = provision(
        dir.path(),
        &["install", "--chain", &stranger, "--root", &root],
        &credentials,
    );
    assert_eq!(code, Some(11), "{output}");
    assert!(output.contains("does not belong"), "{output}");

    // Under another root.
    let chain = write("chain.pem", &pki.chain_for(&key, NODE));
    let foreign = write("foreign.pem", &self::pki().root.certificate_pem());
    let (code, output) = provision(
        dir.path(),
        &["install", "--chain", &chain, "--root", &foreign],
        &credentials,
    );
    assert_eq!(code, Some(11), "{output}");
    assert!(output.contains("does not verify"), "{output}");

    let (code, output) = provision(dir.path(), &["status"], &credentials);
    assert_eq!(code, Some(0), "{output}");
    assert!(
        output.contains(r#""intermediate_expires":null"#),
        "nothing installed yet: {output}"
    );

    // The real one.
    let (code, output) = provision(
        dir.path(),
        &["install", "--chain", &chain, "--root", &root],
        &credentials,
    );
    assert_eq!(code, Some(0), "{output}");
    let installed = dir.path().join("pki");
    assert_eq!(
        std::fs::read_to_string(installed.join("root.pem")).expect("root"),
        pki.root.certificate_pem()
    );
    assert!(installed.join("facility-chain.pem").is_file());
    let (code, output) = provision(dir.path(), &["status"], &credentials);
    assert_eq!(code, Some(0), "{output}");
    let status: serde_json::Value = serde_json::from_str(output.trim()).expect("json");
    assert_eq!(status["key_sealed"], true);
    assert_eq!(status["root_installed"], true);
    assert!(status["intermediate_expires"].is_string(), "{status}");
    assert!(
        !installed.join(KEY).exists(),
        "the key is never written beside the chain"
    );
}

#[test]
fn the_encryption_gate_comes_before_any_key() {
    let dir = tempfile::tempdir().expect("dir");
    let credentials = dir.path().join("credentials");
    std::fs::create_dir_all(&credentials).expect("dir");
    let out = dir.path().join("request.pem");
    let (code, output) = provision(
        dir.path(),
        &["request", "--out", out.to_str().expect("utf-8")],
        &credentials,
    );
    assert_eq!(code, Some(10), "{output}");
    assert!(output.contains("provisioning refused"), "{output}");
    assert!(!out.exists(), "no request without passing the gate");

    let (code, _) = provision(dir.path(), &["nonsense"], &credentials);
    assert_eq!(code, Some(11));
}
