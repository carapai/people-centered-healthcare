//! Plays central for the client's edge e2e suite (#75): writes what `provision request` seals in
//! the keystore, central's snapshot keys, and a provisioning bundle for one member of staff.
//!
//! Usage: `edge-fixture <dir> <node-id> <facility> <username> <password> [<package-dir>]
//! [--activate <package-dir>]... [--first-visits <n>]`.
//! Writes:
//! - `credentials/`: the keys systemd would unseal into `$CREDENTIALS_DIRECTORY`;
//! - `snapshot-keys.json`: central's snapshot public keys (`HEALTH_SNAPSHOT_KEYS_FILE`);
//! - `bundle.jws`: the bundle, for `POST /api/admin/provision/accounts`;
//! - with a package directory, its definitions into the edge's store (`store/`), each under the
//!   id activation gives it, as central's sync would bring them down (#106);
//! - with `--activate`, each package staged (its tarball in `files/`) and the set of them
//!   active, as central's activation would leave the node (#879);
//! - with `--first-visits`, that many women each with a first ANC visit today, recorded here,
//!   which the active sample indicators count (#879), with the active packages' indicators
//!   computed here once, as the edge computes them within seconds of starting.
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
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut activate = Vec::new();
    let mut first_visits = 0;
    while let Some(at) = args.iter().position(|a| a.starts_with("--")) {
        if at + 1 >= args.len() {
            return Err(format!("{} needs a value", args[at]).into());
        }
        let value = args.remove(at + 1);
        match args.remove(at).as_str() {
            "--activate" => activate.push(PathBuf::from(value)),
            "--first-visits" => first_visits = value.parse()?,
            other => return Err(format!("unknown option {other}").into()),
        }
    }
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
                // The facility's own indicators and standard Dashboards, offline (#599, #879).
                "reporting.view",
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
    if package.is_some() || !activate.is_empty() || first_visits > 0 {
        tokio::runtime::Runtime::new()?.block_on(async {
            let store = open(&dir.join("store")).await?;
            if let Some(package) = package {
                definitions(&store, package.as_ref()).await?;
            }
            let packages = activated(&store, &dir.join("files"), &activate).await?;
            visits(&store, first_visits).await?;
            computed(&store, &packages, facility).await
        })?;
    }
    Ok(())
}

/// The edge's store at `store`, migrated.
async fn open(
    store: &std::path::Path,
) -> Result<health_surrealdb::SurrealStore, Box<dyn std::error::Error>> {
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
    Ok(
        SurrealStore::new(connection, std::sync::Arc::new(health_time::SystemClock))
            .with_registry(health_fhir_server::search_registry()),
    )
}

/// Every file under `dir` but its test patients, by its path in the package.
fn tree(dir: &std::path::Path) -> Result<health_packages::Files, Box<dyn std::error::Error>> {
    fn walk(
        root: &std::path::Path,
        dir: &std::path::Path,
        files: &mut health_packages::Files,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "tests") {
                    walk(root, &path, files)?;
                }
            } else if let Ok(relative) = path.strip_prefix(root) {
                files.insert(
                    relative.to_string_lossy().replace('\\', "/"),
                    std::fs::read(&path)?,
                );
            }
        }
        Ok(())
    }
    let mut files = health_packages::Files::new();
    walk(dir, dir, &mut files)?;
    Ok(files)
}

/// Stages each package in `packages` (its tarball in the blob store at `blobs`) and makes the
/// set of them the node's active set, if there are any: each package's id and files.
async fn activated(
    store: &health_surrealdb::SurrealStore,
    blobs: &std::path::Path,
    packages: &[PathBuf],
) -> Result<Vec<(String, health_packages::Files)>, Box<dyn std::error::Error>> {
    use health_persistence::{Store as _, Transaction as _};
    if packages.is_empty() {
        return Ok(Vec::new());
    }
    let mut staged = Vec::new();
    let blobs = health_files::FsBlobStore::open(blobs)?;
    let mut active = serde_json::Map::new();
    for dir in packages {
        let packed = health_packages::pack(&tree(dir)?)?;
        blobs.put(packed.tgz.as_slice())?;
        let mut tx = store.begin().await?;
        tx.put_staged_package(&packed.id, &packed.digest.to_string(), "{}")
            .await?;
        tx.commit().await?;
        let (name, version) = packed
            .id
            .rsplit_once('@')
            .ok_or("a package id is name@version")?;
        active.insert(name.into(), version.into());
        staged.push((packed.id, packed.files));
    }
    let mut tx = store.begin().await?;
    tx.set_active_packages(&serde_json::Value::Object(active).to_string())
        .await?;
    tx.commit().await?;
    Ok(staged)
}

/// Computes the indicators of `packages` for `facility` over `store`, once, as the edge does.
async fn computed(
    store: &health_surrealdb::SurrealStore,
    packages: &[(String, health_packages::Files)],
    facility: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use health_node_api::indicators::{catch_up, load};
    if packages.is_empty() {
        return Ok(());
    }
    let (indicators, problems) = load(
        packages.iter().map(|(id, files)| (id.as_str(), files)),
        facility,
    );
    if !problems.is_empty() {
        return Err(problems.join("; ").into());
    }
    let now = Timestamp::now().to_zoned(jiff::tz::TimeZone::system());
    catch_up(&indicators, store, &now, true).await?;
    Ok(())
}

/// Records `count` women, each with a first ANC visit today.
async fn visits(
    store: &health_surrealdb::SurrealStore,
    count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use health_persistence::{Precondition, RawJson, Store as _, Transaction as _, WriteRequest};
    let now = Timestamp::now().to_string();
    for _ in 0..count {
        let patient = uuid::Uuid::now_v7().to_string();
        let woman = serde_json::json!({
            "resourceType": "Patient",
            "id": patient,
            "gender": "female",
            "birthDate": "2000-04-04",
        });
        let visit = serde_json::json!({
            "resourceType": "Encounter",
            "id": uuid::Uuid::now_v7().to_string(),
            "status": "completed",
            "class": [{ "coding": [{
                "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode",
                "code": "AMB",
            }] }],
            "type": [{ "coding": [{
                "system": "https://health.go.ug/fhir/CodeSystem/anc-visit-type",
                "code": "first",
            }] }],
            "subject": { "reference": format!("Patient/{patient}") },
            "actualPeriod": { "start": now },
        });
        for resource in [woman, visit] {
            let mut tx = store.begin().await?;
            tx.write(WriteRequest {
                resource_type: resource["resourceType"]
                    .as_str()
                    .unwrap_or_default()
                    .parse()?,
                id: resource["id"].as_str().unwrap_or_default().parse()?,
                json: RawJson::from_validated(resource.to_string()),
                op_id: health_identifiers::OpId::new(),
                precondition: Precondition::MustNotExist,
            })
            .await?;
            tx.commit().await?;
        }
    }
    Ok(())
}

/// Writes the definitions of the package in `package` into the edge's `store`.
async fn definitions(
    store: &health_surrealdb::SurrealStore,
    package: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use health_persistence::{Precondition, RawJson, Store as _, Transaction as _, WriteRequest};
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
