//! The full-disk encryption check through the built binary (ADR 0048): the provisioning gate
//! and its exemption, and the probe's state reaching the status page.

#![cfg(unix)]

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use common::{BINARY, STARTUP, edge, terminate, wait_exit, wait_ready};

/// An edge whose system volume complies and whose data volume is unencrypted.
const UNENCRYPTED_DATA: &str = r#"{
  "checked": "2026-11-02T09:00:00Z", "tpm2": true, "secure_boot": true,
  "volumes": [
    {"role": "system", "path": "/", "protection": {"state": "encrypted", "tpm_protector": true, "suspended": false, "sealed_to_secure_boot": true}},
    {"role": "data", "path": "/var/lib/health/store", "protection": {"state": "unencrypted"}}
  ]
}"#;

fn status(port: u16) -> Option<serde_json::Value> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    write!(
        stream,
        "GET /api/admin/status HTTP/1.1\r\nHost: edge\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    serde_json::from_str(response.split_once("\r\n\r\n")?.1).ok()
}

fn alerts(report: &serde_json::Value) -> Vec<(String, Option<String>)> {
    report["alerts"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| {
            (
                a["code"].as_str().unwrap_or_default().to_owned(),
                a["subject"].as_str().map(str::to_owned),
            )
        })
        .collect()
}

/// The report once the node has run its first encryption check.
fn checked_report(dir: &Path, state_file: &Path, exemption: Option<&str>) -> serde_json::Value {
    let port = common::free_port();
    let mut env = vec![(
        "HEALTH_ENCRYPTION_STATE_FILE",
        state_file.display().to_string(),
    )];
    if let Some(grant) = exemption {
        env.push(("HEALTH_ENCRYPTION_EXEMPTION", grant.to_owned()));
    }
    let mut node = edge(&dir.join("store"), port, &[], &env);
    wait_ready(port);
    let started = Instant::now();
    let report = loop {
        if let Some(report) = status(port).filter(|r| !r["encryption"].is_null()) {
            break report;
        }
        assert!(started.elapsed() < STARTUP, "no encryption state reported");
        std::thread::sleep(Duration::from_millis(100));
    };
    terminate(&node);
    assert_eq!(wait_exit(&mut node).code(), Some(0));
    report
}

#[test]
fn the_probes_state_reaches_the_status_page_as_alerts() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("encryption.json");
    std::fs::write(&file, UNENCRYPTED_DATA).expect("state");

    let report = checked_report(dir.path(), &file, None);
    assert!(
        alerts(&report).contains(&("disk-unencrypted".into(), Some("data".into()))),
        "{report}"
    );
    assert_eq!(
        report["encryption"]["volumes"][1]["protection"],
        "unencrypted"
    );
    assert_eq!(report["encryption"]["volumes"][0]["protection"], "tpm");
    assert!(
        !report["pending"]
            .as_array()
            .expect("pending")
            .iter()
            .any(|p| p == "disk-encryption")
    );

    // An exemption is one standing alert in place of what it exempts.
    let dir = tempfile::tempdir().expect("dir");
    let report = checked_report(dir.path(), &file, Some("UG-FDE-2026-014"));
    let codes = alerts(&report);
    assert!(
        codes.contains(&("encryption-exempt".into(), None)),
        "{report}"
    );
    assert!(!codes.iter().any(|(code, _)| code == "disk-unencrypted"));
    assert_eq!(report["encryption"]["exemption"], "UG-FDE-2026-014");

    // A missing or corrupt file is unknown, never a pass.
    let dir = tempfile::tempdir().expect("dir");
    let corrupt = dir.path().join("encryption.json");
    std::fs::write(&corrupt, "{").expect("state");
    let codes = alerts(&checked_report(dir.path(), &corrupt, None));
    assert!(codes.contains(&("encryption-unknown".into(), Some("data".into()))));
    assert!(codes.contains(&("tpm-unknown".into(), None)));
}

fn encryption(dir: &Path, args: &[&str], exemption: Option<&str>) -> (Option<i32>, String) {
    let mut command = Command::new(BINARY);
    command
        .arg("encryption")
        .args(args)
        .env_clear()
        .env("HEALTH_DATA_DIR", dir.join("store"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default());
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    if let Some(grant) = exemption {
        command.env("HEALTH_ENCRYPTION_EXEMPTION", grant);
    }
    let output = command.output().expect("run");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned()
            + &String::from_utf8_lossy(&output.stderr),
    )
}

/// The gate on this machine. A developer machine or CI runner, unprivileged, cannot show TPM
/// sealing and encryption on both volumes, so provisioning is refused unless exempted.
#[test]
fn the_gate_refuses_this_machine_unless_exempted() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::create_dir_all(dir.path().join("store")).expect("store");
    let (code, output) = encryption(dir.path(), &["check"], None);
    assert_eq!(code, Some(10), "{output}");
    assert!(output.contains("provisioning refused"), "{output}");
    assert!(output.contains(r#""decision":"refused""#), "{output}");

    let (code, output) = encryption(dir.path(), &["check"], Some("UG-FDE-2026-014"));
    assert_eq!(code, Some(0), "{output}");
    assert!(output.contains(r#""decision":"exempt""#), "{output}");

    // The probe writes the file the Linux unit hands the service.
    let out = dir.path().join("encryption.json");
    let (code, output) = encryption(
        dir.path(),
        &["probe", "--out", out.to_str().expect("utf-8")],
        None,
    );
    assert_eq!(code, Some(0), "{output}");
    let state: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).expect("file")).expect("json");
    assert_eq!(state["volumes"].as_array().map(Vec::len), Some(2));

    let (code, _) = encryption(dir.path(), &["nonsense"], None);
    assert_eq!(code, Some(10));
}
