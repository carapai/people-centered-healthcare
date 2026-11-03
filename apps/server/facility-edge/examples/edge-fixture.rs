//! Plays central for the client's edge e2e suite (#75): writes what `provision request` seals in
//! the keystore, central's snapshot keys, and a provisioning bundle for one member of staff.
//!
//! Usage: `edge-fixture <dir> <node-id> <facility> <username> <password>`. Writes:
//! - `credentials/`: the keys systemd would unseal into `$CREDENTIALS_DIRECTORY`;
//! - `snapshot-keys.json`: central's snapshot public keys (`HEALTH_SNAPSHOT_KEYS_FILE`);
//! - `bundle.jws`: the bundle, for `POST /api/admin/provision/accounts`.

#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use health_auth::{Bundle, BundleAccount, Credentials, SnapshotSigner};
use health_identity::snapshot::{PermissionSnapshot, RoleAssignment, RolePermissions};
use health_identity::{AccountId, AccountStatus};
use health_time::{SignedDuration, Timestamp};
use p256::pkcs8::DecodePrivateKey;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir, node, facility, username, password] = args.as_slice() else {
        return Err("usage: edge-fixture <dir> <node-id> <facility> <username> <password>".into());
    };
    let dir = PathBuf::from(dir);
    let credentials = dir.join("credentials");
    std::fs::create_dir_all(&credentials)?;
    let key = health_pki::new_key()?.serialize_pem();
    std::fs::write(
        credentials.join(health_edge_runtime::INTERMEDIATE_KEY),
        &key,
    )?;
    std::fs::write(
        credentials.join(health_edge_runtime::PEPPER_KEY),
        rand_bytes(),
    )?;
    std::fs::write(
        credentials.join(health_edge_runtime::TOKEN_KEY),
        p256::SecretKey::random(&mut rand_core::OsRng).to_bytes(),
    )?;

    let central = SnapshotSigner::generate();
    std::fs::write(
        dir.join("snapshot-keys.json"),
        serde_json::to_string(&[central.public()])?,
    )?;
    let account = AccountId(uuid::Uuid::now_v7());
    let now = Timestamp::now();
    let snapshot = PermissionSnapshot {
        account,
        facility: facility.clone(),
        roles: vec![RoleAssignment {
            role: "clinician".into(),
            locations: Vec::new(),
        }],
        programs: Vec::new(),
        issued_at: now,
        not_after: now.checked_add(SignedDuration::from_hours(24 * 14))?,
        snapshot_version: 1,
    };
    let roles = RolePermissions(BTreeMap::from([(
        "clinician".into(),
        BTreeSet::from(["record.read".into()]),
    )]));
    let bundle = Bundle {
        facility: facility.clone(),
        node_id: node.clone(),
        issued_at: now,
        accounts: vec![BundleAccount {
            id: account,
            username: username.clone(),
            status: AccountStatus::Active,
            phc: Credentials::hash(password).map_err(|e| e.to_string())?,
        }],
        snapshots: vec![central.sign(&snapshot, &roles)?],
    }
    .seal(
        &central,
        &p256::SecretKey::from_pkcs8_pem(&key)?.public_key(),
    )?;
    std::fs::write(dir.join("bundle.jws"), bundle)?;
    Ok(())
}

fn rand_bytes() -> [u8; 32] {
    p256::SecretKey::random(&mut rand_core::OsRng)
        .to_bytes()
        .into()
}
