//! The edge's disk recovery keys (ADR 0048 § Recovery keys; #268), escrowed at central and never
//! kept on site. `facility-edge encryption rotate-recovery` runs this, and so does `provision
//! --code` once the node has its identity.
//!
//! [`rotate`], for each encrypted volume (the system volume, and the data directory's when it is
//! another one):
//! 1. adds a new recovery password the edge generated;
//! 2. escrows them all at central in one JWS signed with the node's facility intermediate key.
//!    If central does not take them, the new keys are removed again, so no key exists on the disk
//!    that central does not hold;
//! 3. records the new keys' ids in `recovery.json` (ids only, never a key);
//! 4. removes the keys it escrowed before.
//!
//! Recovery keys the edge did not create, such as the install passphrase on Linux, are left
//! alone.

use std::path::{Path, PathBuf};

use health_identity::{NodeId, RecoveryVolume};
use health_node_identity::encryption::recovery::{RecoveryProtectors, new_recovery_password};
use health_time::Timestamp;
use p256::ecdsa::SigningKey;
use serde::{Deserialize, Serialize};

/// The ids of the recovery keys this edge escrowed.
const FILE: &str = "recovery.json";

/// A recovery key this edge added and escrowed: where, and its id there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Escrowed {
    pub(crate) volume: RecoveryVolume,
    pub(crate) device: String,
    pub(crate) id: String,
}

fn read(pki_dir: &Path) -> Vec<Escrowed> {
    std::fs::read_to_string(pki_dir.join(FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write(pki_dir: &Path, escrowed: &[Escrowed]) -> Result<(), String> {
    let path = pki_dir.join(FILE);
    let temporary = path.with_extension("json.new");
    std::fs::write(
        &temporary,
        serde_json::to_string_pretty(escrowed).map_err(|e| e.to_string())?,
    )
    .and_then(|()| std::fs::rename(&temporary, &path))
    .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// What a rotation did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rotated {
    /// The keys now escrowed.
    pub(crate) escrowed: Vec<Escrowed>,
    /// Earlier keys that could not be removed, and why: still on the disk, still at central.
    pub(crate) left: Vec<String>,
}

/// Rotates the recovery keys of the volumes holding `system` and `data`; see the module
/// documentation. `escrow` sends the signed JWS to central.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rotate(
    protectors: &dyn RecoveryProtectors,
    system: &Path,
    data: &Path,
    pki_dir: &Path,
    node: NodeId,
    key: &SigningKey,
    now: Timestamp,
    escrow: impl FnOnce(String) -> Result<(), String>,
) -> Result<Rotated, String> {
    let mut volumes: Vec<(RecoveryVolume, String)> = Vec::new();
    for (volume, path) in [
        (RecoveryVolume::System, system),
        (RecoveryVolume::Data, data),
    ] {
        if let Some(device) = protectors.volume_of(path)?
            && !volumes.iter().any(|(_, d)| d == &device)
        {
            volumes.push((volume, device));
        }
    }
    if volumes.is_empty() {
        return Err("no volume is encrypted, so there is no recovery key to escrow".into());
    }

    // 1. New keys, each removed again if anything after it fails.
    let mut added: Vec<(Escrowed, String)> = Vec::new();
    let undo = |added: &[(Escrowed, String)]| {
        for (key, _) in added {
            let _ = protectors.remove(&key.device, &key.id);
        }
    };
    for (volume, device) in &volumes {
        let password = new_recovery_password();
        match protectors.add(device, &password) {
            Ok(id) => added.push((
                Escrowed {
                    volume: *volume,
                    device: device.clone(),
                    id,
                },
                password,
            )),
            Err(error) => {
                undo(&added);
                return Err(format!("cannot add a recovery key to {device}: {error}"));
            }
        }
    }

    // 2. Escrowed, or undone.
    let keys: Vec<(RecoveryVolume, String)> = added
        .iter()
        .map(|(key, password)| (key.volume, password.clone()))
        .collect();
    let jws = health_provisioning::recovery_jws(node, &keys, key, now);
    if let Err(error) = escrow(jws) {
        undo(&added);
        return Err(format!(
            "central did not take the recovery keys, so they were removed again: {error}"
        ));
    }
    let escrowed: Vec<Escrowed> = added.into_iter().map(|(key, _)| key).collect();

    // 3 and 4. Recorded, then the earlier ones removed.
    let earlier = read(pki_dir);
    write(pki_dir, &escrowed)?;
    let left = earlier
        .iter()
        .filter(|old| !escrowed.contains(old))
        .filter_map(|old| {
            protectors.remove(&old.device, &old.id).err().map(|error| {
                format!(
                    "{} key {} on {}: {error}",
                    old.volume.as_str(),
                    old.id,
                    old.device
                )
            })
        })
        .collect();
    Ok(Rotated { escrowed, left })
}

/// Rotates this node's recovery keys and escrows them at central: `central` and `central_ca`
/// when given, else where `node.json` says central is.
pub(crate) fn rotate_here(
    config: &health_edge_runtime::EdgeConfig,
    keystore: &health_node_identity::keystore::Keystore,
    central_url: Option<String>,
    central_ca: Option<PathBuf>,
) -> Result<Rotated, String> {
    use p256::pkcs8::DecodePrivateKey as _;
    let (node, _, _) = crate::provision::identity(config)
        .ok_or("this node was not provisioned online: it has no node.json")?;
    let node = NodeId(
        node.parse()
            .map_err(|_| "node.json's node_id is not a UUID")?,
    );
    let recorded = central(&config.pki_dir);
    let url = central_url
        .or_else(|| recorded.as_ref().map(|(url, _)| url.clone()))
        .ok_or("where is central? give --central")?;
    let ca = central_ca.or_else(|| recorded.and_then(|(_, ca)| ca));
    let ca_pem = ca
        .map(|path| {
            std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))
        })
        .transpose()?;
    let pem = keystore
        .unseal(crate::provision::KEY)
        .map_err(|e| e.to_string())?
        .ok_or("the facility intermediate's key is not sealed")?;
    let key = SigningKey::from_pkcs8_pem(
        std::str::from_utf8(&pem).map_err(|_| "the sealed key is not PEM")?,
    )
    .map_err(|e| format!("the sealed key: {e}"))?;
    rotate(
        &health_node_identity::encryption::recovery::SystemProtectors,
        &health_node_identity::encryption::system_root(),
        &config.data_dir,
        &config.pki_dir,
        node,
        &key,
        Timestamp::now(),
        |jws| {
            let endpoint = format!("{}/api/provisioning/recovery", url.trim_end_matches('/'));
            match crate::central::post(
                &endpoint,
                "application/jose",
                jws.into_bytes(),
                ca_pem.as_deref(),
            )? {
                (204, _) => Ok(()),
                (status, answer) => Err(format!(
                    "central answered {status}: {}",
                    answer["detail"]
                        .as_str()
                        .or(answer["error"].as_str())
                        .unwrap_or("no reason")
                )),
            }
        },
    )
}

/// Where central is, from `node.json`: its origin and, if given at provisioning, its CA.
pub(crate) fn central(pki_dir: &Path) -> Option<(String, Option<PathBuf>)> {
    let text = std::fs::read_to_string(pki_dir.join("node.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some((
        value["central"].as_str()?.to_owned(),
        value["central_ca"].as_str().map(PathBuf::from),
    ))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use health_node_identity::encryption::recovery::valid_recovery_password;

    use super::*;

    /// Volumes by path; keys by device, with the next slot to hand out.
    struct Fake {
        devices: BTreeMap<PathBuf, String>,
        keys: RefCell<BTreeMap<String, Vec<(String, String)>>>,
        next: RefCell<u32>,
        fail_add_on: Option<String>,
    }

    impl Fake {
        fn new(devices: &[(&str, &str)]) -> Self {
            Self {
                devices: devices
                    .iter()
                    .map(|(path, device)| (PathBuf::from(path), (*device).to_owned()))
                    .collect(),
                keys: RefCell::default(),
                next: RefCell::new(1),
                fail_add_on: None,
            }
        }

        fn ids(&self, device: &str) -> Vec<String> {
            self.keys
                .borrow()
                .get(device)
                .map(|keys| keys.iter().map(|(id, _)| id.clone()).collect())
                .unwrap_or_default()
        }
    }

    impl RecoveryProtectors for Fake {
        fn volume_of(&self, path: &Path) -> Result<Option<String>, String> {
            Ok(self.devices.get(path).cloned())
        }

        fn add(&self, volume: &str, password: &str) -> Result<String, String> {
            assert!(valid_recovery_password(password), "{password}");
            if self.fail_add_on.as_deref() == Some(volume) {
                return Err("the volume is busy".into());
            }
            let mut next = self.next.borrow_mut();
            let id = next.to_string();
            *next += 1;
            self.keys
                .borrow_mut()
                .entry(volume.to_owned())
                .or_default()
                .push((id.clone(), password.to_owned()));
            Ok(id)
        }

        fn remove(&self, volume: &str, id: &str) -> Result<(), String> {
            let mut keys = self.keys.borrow_mut();
            let keys = keys.get_mut(volume).ok_or("no such volume")?;
            let before = keys.len();
            keys.retain(|(slot, _)| slot != id);
            if keys.len() == before {
                return Err(format!("no key {id}"));
            }
            Ok(())
        }
    }

    fn signer() -> SigningKey {
        SigningKey::random(&mut rand_core::OsRng)
    }

    fn at() -> Timestamp {
        "2026-11-04T09:00:00Z".parse().expect("timestamp")
    }

    #[test]
    fn a_rotation_escrows_new_keys_before_removing_the_old_ones() {
        let dir = tempfile::tempdir().expect("dir");
        let fake = Fake::new(&[("/", "/dev/sda3"), ("/var/lib/health", "/dev/sdb1")]);
        let node = NodeId(uuid::Uuid::now_v7());
        let key = signer();
        let mut sent = Vec::new();
        let first = rotate(
            &fake,
            Path::new("/"),
            Path::new("/var/lib/health"),
            dir.path(),
            node,
            &key,
            at(),
            |jws| {
                sent.push(jws);
                Ok(())
            },
        )
        .expect("rotated");
        assert_eq!(
            first
                .escrowed
                .iter()
                .map(|k| (k.volume, k.device.as_str()))
                .collect::<Vec<_>>(),
            [
                (RecoveryVolume::System, "/dev/sda3"),
                (RecoveryVolume::Data, "/dev/sdb1")
            ]
        );
        assert_eq!(read(dir.path()), first.escrowed);
        // The escrow carries both keys, and the record on disk carries none.
        let payload = sent[0].split('.').nth(1).expect("payload");
        let claims: serde_json::Value = serde_json::from_slice(
            &base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, payload)
                .expect("base64"),
        )
        .expect("json");
        assert_eq!(claims["keys"].as_array().map(Vec::len), Some(2));
        let on_disk = std::fs::read_to_string(dir.path().join(FILE)).expect("file");
        for entry in claims["keys"].as_array().expect("keys") {
            assert!(!on_disk.contains(entry["key"].as_str().expect("key")));
        }

        // Rotating replaces each, removing only the keys this edge escrowed.
        fake.keys
            .borrow_mut()
            .entry("/dev/sda3".into())
            .or_default()
            .insert(0, ("0".into(), "install passphrase".into()));
        let second = rotate(
            &fake,
            Path::new("/"),
            Path::new("/var/lib/health"),
            dir.path(),
            node,
            &key,
            at(),
            |_| Ok(()),
        )
        .expect("rotated");
        assert_eq!(second.left, Vec::<String>::new());
        assert_eq!(fake.ids("/dev/sda3"), ["0", "3"]);
        assert_eq!(fake.ids("/dev/sdb1"), ["4"]);
        assert_eq!(read(dir.path()), second.escrowed);
    }

    #[test]
    fn nothing_is_left_on_the_disk_that_central_does_not_hold() {
        let dir = tempfile::tempdir().expect("dir");
        let node = NodeId(uuid::Uuid::now_v7());
        let key = signer();

        // Central refuses: the new keys are removed again, and nothing is recorded.
        let fake = Fake::new(&[("/", "/dev/sda3"), ("/data", "/dev/sdb1")]);
        let refused = rotate(
            &fake,
            Path::new("/"),
            Path::new("/data"),
            dir.path(),
            node,
            &key,
            at(),
            |_| Err("central answered 503".into()),
        );
        assert!(refused.is_err_and(|e| e.contains("removed again")));
        assert_eq!(
            (fake.ids("/dev/sda3"), fake.ids("/dev/sdb1")),
            (Vec::new(), Vec::new())
        );
        assert_eq!(read(dir.path()), []);

        // A volume that cannot take a key: the ones already added are removed.
        let mut busy = Fake::new(&[("/", "/dev/sda3"), ("/data", "/dev/sdb1")]);
        busy.fail_add_on = Some("/dev/sdb1".into());
        assert!(
            rotate(
                &busy,
                Path::new("/"),
                Path::new("/data"),
                dir.path(),
                node,
                &key,
                at(),
                |_| Ok(())
            )
            .is_err()
        );
        assert_eq!(busy.ids("/dev/sda3"), Vec::<String>::new());

        // One device for both: one key. None encrypted: nothing to escrow.
        let shared = Fake::new(&[("/", "/dev/sda3"), ("/data", "/dev/sda3")]);
        let rotated = rotate(
            &shared,
            Path::new("/"),
            Path::new("/data"),
            dir.path(),
            node,
            &key,
            at(),
            |_| Ok(()),
        )
        .expect("rotated");
        assert_eq!(rotated.escrowed.len(), 1);
        let none = Fake::new(&[]);
        assert!(
            rotate(
                &none,
                Path::new("/"),
                Path::new("/data"),
                dir.path(),
                node,
                &key,
                at(),
                |_| Ok(())
            )
            .is_err()
        );
    }
}
