//! The standalone desktop as a node (#134; desktop-client.md, mode B): once provisioned, it syncs
//! with central as an edge does (`health-sync-client`). It pushes what it wrote, pulls its
//! facility's records, re-seeds from central's snapshot when told to, and activates the packages
//! its activation records name. `health-desktop seed import <file>` seeds it from removable media
//! instead, for a first install the link cannot carry.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use health_configuration::activation::Chain;
use health_desktop_runtime::keystore::Keystore;
use health_surrealdb::{Connection, SurrealStore};
use health_sync_client::{Sync, Upstream};
use p256::pkcs8::DecodePrivateKey as _;

use crate::provision::{KEY, pki_dir};

/// How often the node syncs when central sends no hint: `HEALTH_SYNC_INTERVAL_SECS`, default 300.
pub(crate) fn interval() -> Duration {
    std::env::var("HEALTH_SYNC_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|s| *s > 0)
        .map_or(Duration::from_secs(300), Duration::from_secs)
}

/// This node's facility, as central gave it at provisioning.
pub(crate) fn facility(data_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(pki_dir(data_dir).join("node.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value["facility"].as_str().map(str::to_owned)
}

/// The store sync writes to: indexed as the clinical API searches, so what central sends down is
/// found here.
pub(crate) fn store(connection: Connection) -> SurrealStore {
    SurrealStore::new(connection, Arc::new(health_time::SystemClock))
        .with_registry(health_fhir_server::search_registry())
}

/// The sync task, once this machine is provisioned; `None` before, with why.
pub(crate) fn sync(
    connection: Connection,
    data_dir: &Path,
    keys: &dyn Keystore,
) -> Result<Option<Sync>, String> {
    let pki = pki_dir(data_dir);
    let Some(upstream) = Upstream::from_identity(&pki) else {
        return Ok(None);
    };
    let pem = keys
        .get(KEY)
        .map_err(|e| e.to_string())?
        .ok_or("provisioned, but the node key is not in the keystore")?;
    let key = std::str::from_utf8(&pem)
        .ok()
        .and_then(|pem| p256::ecdsa::SigningKey::from_pkcs8_pem(pem).ok())
        .ok_or("the node key in the keystore is not a P-256 PKCS#8 key")?;
    let mut sync = Sync::new(store(connection), upstream, key)?;
    // Attachments beside the store, where backups take them from.
    match health_files::FsBlobStore::open(data_dir.join("files")) {
        Ok(blobs) => sync = sync.with_blobs(blobs),
        Err(error) => {
            eprintln!("health-desktop: no attachment store, attachments do not sync: {error}")
        }
    }
    // Package activation, against the national root installed at provisioning.
    let chain = Chain {
        facility: facility(data_dir),
        ..Chain::default()
    };
    match health_activation::Activator::from_root_file(&pki.join("root.pem"), chain) {
        Ok(activator) => sync = sync.with_activator(activator),
        Err(error) => eprintln!("health-desktop: packages are not activated here: {error}"),
    }
    Ok(Some(sync))
}

/// The hourly backups (#134; desktop-client.md § Storage and backup; ADR 0040), as an edge takes
/// them: encrypted, to `HEALTH_BACKUP_DIR` (a second volume or USB) with the key in
/// `HEALTH_BACKUP_KEY_FILE`, every `HEALTH_BACKUP_INTERVAL_SECS` (default 3600), and the newest
/// test-restored every `HEALTH_BACKUP_TEST_SECS` (default 604800). `None` with why, before the
/// desktop is provisioned or when no backup directory is set.
pub(crate) fn backups(
    data_dir: &Path,
    get: impl Fn(&str) -> Option<String>,
) -> Result<health_backup::node::Schedule, String> {
    let node = std::fs::read_to_string(pki_dir(data_dir).join("node.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| value["node_id"].as_str().map(str::to_owned))
        .ok_or("not provisioned")?;
    let dir =
        get("HEALTH_BACKUP_DIR").ok_or("no HEALTH_BACKUP_DIR: this desktop takes no backups")?;
    let key_file = get("HEALTH_BACKUP_KEY_FILE")
        .ok_or("HEALTH_BACKUP_DIR is set, but HEALTH_BACKUP_KEY_FILE is not")?;
    let seconds = |name: &str, default: u64| -> Result<Duration, String> {
        match get(name) {
            None => Ok(Duration::from_secs(default)),
            Some(text) => text
                .trim()
                .parse::<u64>()
                .ok()
                .filter(|s| *s > 0)
                .map(Duration::from_secs)
                .ok_or_else(|| format!("{name} is not a positive number of seconds")),
        }
    };
    Ok(health_backup::node::Schedule {
        backups: health_backup::node::Backups::new(dir, node),
        key_file: key_file.into(),
        every: seconds("HEALTH_BACKUP_INTERVAL_SECS", 3600)?,
        test_every: seconds("HEALTH_BACKUP_TEST_SECS", 604_800)?,
        data_dir: data_dir.to_path_buf(),
    })
}

/// `health-desktop seed import <file>`, with `<file>.jws` beside it: the seed must be signed by
/// the publication signer under the national root installed at provisioning, and be for this
/// desktop's facility. Returns the heads applied.
pub(crate) async fn seed_import(
    connection: Connection,
    data_dir: &Path,
    file: &Path,
) -> Result<usize, String> {
    let facility =
        facility(data_dir).ok_or("this desktop is not provisioned: provision it first")?;
    let root = pki_dir(data_dir).join("root.pem");
    let trust = std::fs::read_to_string(&root)
        .map_err(|e| format!("{}: {e}", root.display()))
        .and_then(|pem| health_pki::Trust::new(&pem).map_err(|e| e.to_string()))?;
    let signature = file.with_file_name(format!(
        "{}.jws",
        file.file_name().and_then(|n| n.to_str()).unwrap_or("seed")
    ));
    let jws =
        std::fs::read_to_string(&signature).map_err(|e| format!("{}: {e}", signature.display()))?;
    let imported = health_sync_client::seed::import(
        &store(connection),
        file,
        jws.trim(),
        &trust,
        &facility,
        health_time::Timestamp::now(),
    )
    .await?;
    Ok(imported.applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unprovisioned_desktop_has_no_facility_and_no_sync() {
        let dir = tempfile::tempdir().expect("dir");
        assert_eq!(facility(dir.path()), None);
        assert!(Upstream::from_identity(&pki_dir(dir.path())).is_none());
        std::fs::create_dir_all(pki_dir(dir.path())).expect("pki");
        std::fs::write(
            pki_dir(dir.path()).join("node.json"),
            r#"{"node_id":"018f0000-0000-7000-8000-000000000000","short_code":"D1","facility":"KLA001","central":"https://central.example/"}"#,
        )
        .expect("node");
        assert_eq!(facility(dir.path()).as_deref(), Some("KLA001"));
        let upstream = Upstream::from_identity(&pki_dir(dir.path())).expect("upstream");
        assert_eq!(upstream.origin, "https://central.example");
    }

    #[test]
    fn backups_follow_the_edge_s_settings_once_provisioned() {
        let dir = tempfile::tempdir().expect("dir");
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| (*v).to_owned())
            }
        };
        let set = env(&[
            ("HEALTH_BACKUP_DIR", "/mnt/backup"),
            ("HEALTH_BACKUP_KEY_FILE", "/etc/k"),
        ]);
        assert_eq!(
            backups(dir.path(), set).err().as_deref(),
            Some("not provisioned")
        );
        std::fs::create_dir_all(pki_dir(dir.path())).expect("pki");
        std::fs::write(
            pki_dir(dir.path()).join("node.json"),
            r#"{"node_id":"ug-kla-0002","facility":"KLA001","central":"https://central.example"}"#,
        )
        .expect("node");
        let schedule = backups(dir.path(), set).expect("schedule");
        assert_eq!(schedule.every, Duration::from_secs(3600));
        assert_eq!(schedule.test_every, Duration::from_secs(604_800));
        assert!(backups(dir.path(), env(&[])).is_err_and(|e| e.contains("no HEALTH_BACKUP_DIR")));
        assert!(
            backups(dir.path(), env(&[("HEALTH_BACKUP_DIR", "/mnt/backup")]))
                .is_err_and(|e| e.contains("HEALTH_BACKUP_KEY_FILE"))
        );
        let hourly = env(&[
            ("HEALTH_BACKUP_DIR", "/mnt/backup"),
            ("HEALTH_BACKUP_KEY_FILE", "/etc/k"),
            ("HEALTH_BACKUP_INTERVAL_SECS", "0"),
        ]);
        assert!(backups(dir.path(), hourly).is_err());
    }

    #[test]
    fn the_interval_defaults_to_five_minutes() {
        assert!(interval() >= Duration::from_secs(1));
    }
}
