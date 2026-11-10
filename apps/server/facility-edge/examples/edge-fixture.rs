//! Plays central for the client's edge e2e suite (#75): writes what `provision request` seals in
//! the keystore, central's snapshot keys, and a provisioning bundle for one member of staff.
//!
//! Usage: `edge-fixture <dir> <node-id> <facility> <username> <password> [<package-dir>]`.
//! Writes:
//! - `credentials/`: the keys systemd would unseal into `$CREDENTIALS_DIRECTORY`;
//! - `snapshot-keys.json`: central's snapshot public keys (`HEALTH_SNAPSHOT_KEYS_FILE`);
//! - `bundle.jws`: the bundle, for `POST /api/admin/provision/accounts`;
//! - with a package directory, its definitions into the edge's store (`store/`), each under the
//!   id activation gives it, as central's sync would bring them down (#106).
//!
//! The member of staff is a `clinician` and a `facility-admin`, who administers access offline;
//! `clinician` is the one locally grantable role.

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
    let (dir, node, facility, username, password, package) = match args.as_slice() {
        [dir, node, facility, username, password] => {
            (dir, node, facility, username, password, None)
        }
        [dir, node, facility, username, password, package] => {
            (dir, node, facility, username, password, Some(package))
        }
        _ => {
            return Err(
                "usage: edge-fixture <dir> <node-id> <facility> <username> <password> [<package-dir>]"
                    .into(),
            );
        }
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
        roles: ["clinician", "facility-admin"]
            .map(|role| RoleAssignment {
                role: role.into(),
                locations: Vec::new(),
            })
            .to_vec(),
        programs: Vec::new(),
        issued_at: now,
        not_after: now.checked_add(SignedDuration::from_hours(24 * 14))?,
        snapshot_version: 1,
    };
    let roles = RolePermissions(BTreeMap::from([
        // The OPD work the edge serves with the WAN down (#106).
        (
            "clinician".into(),
            [
                "record.read",
                "Patient.crs",
                "Encounter.rs",
                "Observation.crus",
                "Questionnaire.rs",
                "QuestionnaireResponse.crus",
                "queues.view",
                "queues.work",
                "encounters.checkin",
                "encounters.manage",
                "consultation.document",
            ]
            .into_iter()
            .map(Into::into)
            .collect(),
        ),
        (
            "facility-admin".into(),
            BTreeSet::from([health_identity::snapshot::ADMINISTER_LOCAL.into()]),
        ),
    ]));
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
        local: health_auth::LocalConfig {
            roles: roles.clone(),
            grantable: vec!["clinician".into()],
            labels: vec![health_auth::SensitiveLabel {
                system: "http://terminology.hl7.org/CodeSystem/v3-ActCode".into(),
                code: "HIV".into(),
                program: "hiv".into(),
            }],
        },
    }
    .seal(
        &central,
        &p256::SecretKey::from_pkcs8_pem(&key)?.public_key(),
    )?;
    std::fs::write(dir.join("bundle.jws"), bundle)?;
    if let Some(package) = package {
        tokio::runtime::Runtime::new()?
            .block_on(definitions(&dir.join("store"), package.as_ref()))?;
    }
    Ok(())
}

/// Writes the definitions of the package in `package` into the edge's store at `store`.
async fn definitions(
    store: &std::path::Path,
    package: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use health_persistence::{Precondition, RawJson, Store as _, Transaction as _, WriteRequest};
    use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
    let connection = Connection::open(&Settings {
        target: Target::SurrealKv {
            path: store.to_path_buf(),
        },
        namespace: "ug".into(),
        database: "main".into(),
    })
    .await?;
    migrate(&connection, MIGRATIONS).await?;
    // Indexed as the edge's sync indexes what central sends down.
    let store = SurrealStore::new(connection, std::sync::Arc::new(health_time::SystemClock))
        .with_registry(health_fhir_server::search_registry());
    let mut files = health_packages::Files::new();
    for entry in std::fs::read_dir(package)? {
        let path = entry?.path();
        if path.is_file() {
            files.insert(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                std::fs::read(&path)?,
            );
        }
    }
    for (resource_type, id, resource) in health_activation::definitions::definitions(&files) {
        let mut tx = store.begin().await?;
        tx.write(WriteRequest {
            resource_type,
            id,
            json: RawJson::from_validated(resource.to_string()),
            op_id: health_identifiers::OpId::new(),
            precondition: Precondition::MustNotExist,
        })
        .await?;
        tx.commit().await?;
    }
    Ok(())
}

fn rand_bytes() -> [u8; 32] {
    p256::SecretKey::random(&mut rand_core::OsRng)
        .to_bytes()
        .into()
}
