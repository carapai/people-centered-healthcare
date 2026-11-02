//! The tooling end to end: a root ceremony, the issuing CA and a facility intermediate from
//! requests, a chain that verifies, and a revocation that makes it fail.

use std::path::Path;
use std::process::Command;

use health_pki::{Authority, new_key};
use health_time::Timestamp;

/// Runs `pki`, returning its stderr, and panics with it if `pki` fails.
fn pki(args: &[&str]) -> String {
    let (ok, stderr) = try_pki(args);
    assert!(ok, "pki {args:?}: {stderr}");
    stderr
}

fn try_pki(args: &[&str]) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_pki"))
        .args(args)
        .output()
        .expect("run pki");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn serial(stderr: &str) -> String {
    stderr
        .trim()
        .strip_prefix("issued serial ")
        .expect("a serial")
        .to_owned()
}

fn s(path: &Path) -> &str {
    path.to_str().expect("utf-8 path")
}

#[test]
fn a_ceremony_issues_a_chain_that_verifies_until_revoked() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join("root");
    let issuing = dir.path().join("issuing");
    let edge = dir.path().join("edge");

    pki(&["root", "--name", "Test Health Root CA", "--out", s(&root)]);
    pki(&["request", "--name", "issuing", "--out", s(&issuing)]);
    pki(&[
        "issuing-ca",
        "--ca",
        s(&root),
        "--request",
        s(&issuing.join("request.pem")),
        "--name",
        "Test Facility Issuing CA",
        "--suffixes",
        "facilities.example.ug",
        "--out",
        s(&issuing.join("certificate.pem")),
    ]);
    std::fs::write(
        issuing.join("revocations.json"),
        r#"{"number":0,"revoked":[]}"#,
    )
    .expect("list");
    pki(&["request", "--name", "edge", "--out", s(&edge)]);
    let facility = serial(&pki(&[
        "facility",
        "--ca",
        s(&issuing),
        "--request",
        s(&edge.join("request.pem")),
        "--node-id",
        "ug-test-0001",
        "--dns",
        "clinic.facilities.example.ug",
        "--out",
        s(&edge.join("certificate.pem")),
    ]));

    // A name outside the issuing CA's suffixes is refused.
    let (ok, stderr) = try_pki(&[
        "facility",
        "--ca",
        s(&issuing),
        "--request",
        s(&edge.join("request.pem")),
        "--node-id",
        "ug-test-0002",
        "--dns",
        "clinic.example.com",
        "--out",
        s(&dir.path().join("refused.pem")),
    ]);
    assert!(
        !ok && stderr.contains("outside the issuer's names"),
        "{stderr}"
    );

    // Keys are owner-only, and nothing is overwritten.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(root.join("key.pem"))
            .expect("key")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let (ok, _) = try_pki(&["root", "--name", "again", "--out", s(&root)]);
    assert!(!ok, "a second root over the first is refused");

    // The edge's leaf, as the edge issues it (#74), served with its chain.
    let read = |p: &Path| std::fs::read_to_string(p).expect("read");
    let intermediate = Authority::from_pem(
        &read(&edge.join("certificate.pem")),
        &read(&edge.join("key.pem")),
    )
    .expect("facility");
    let leaf = intermediate
        .issue_server(
            &new_key().expect("key"),
            &["ug-test-0001.local".into()],
            &[],
            Timestamp::now(),
        )
        .expect("leaf");
    let chain = dir.path().join("chain.pem");
    std::fs::write(
        &chain,
        format!(
            "{}{}{}",
            leaf.pem(),
            read(&edge.join("certificate.pem")),
            read(&issuing.join("certificate.pem"))
        ),
    )
    .expect("chain");
    let root_certificate = root.join("certificate.pem");
    let verify = |crls: &str| {
        let mut args = vec![
            "verify",
            "--root",
            s(&root_certificate),
            "--chain",
            s(&chain),
            "--name",
            "ug-test-0001.local",
        ];
        if !crls.is_empty() {
            args.extend(["--crls", crls]);
        }
        try_pki(&args)
    };
    assert!(verify("").0);

    let crl = dir.path().join("issuing.crl.pem");
    let stderr = pki(&[
        "revoke",
        "--ca",
        s(&issuing),
        "--serial",
        &facility,
        "--reason",
        "key-compromise",
        "--crl",
        s(&crl),
    ]);
    assert!(stderr.contains("CRL 1 lists 1 revoked"), "{stderr}");
    let (ok, stderr) = verify(s(&crl));
    assert!(!ok && stderr.contains("CertRevoked"), "{stderr}");

    // The root's yearly CRL, with nothing revoked, leaves the chain trusted.
    let root_crl = dir.path().join("root.crl.pem");
    pki(&["crl", "--ca", s(&root), "--crl", s(&root_crl)]);
    assert!(verify(s(&root_crl)).0);
}
