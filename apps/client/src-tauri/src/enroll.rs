//! `health-desktop enroll --edge URL --ca FILE [--label TEXT]` (#133; desktop-client.md, mode A;
//! lan-security.md § Device enrollment): a desktop under an edge enrolls there as a LAN device,
//! as a browser does, and its device key's DPoP proofs are its credential from then on, for sync
//! too (ADR 0062: desktops under an edge use device enrollment, not node provisioning).
//!
//! 1. The keystore must work: there is no install without one.
//! 2. A desktop already provisioned as a standalone node, or already enrolled, is not enrolled.
//! 3. The device key is made in the OS keystore, or the one an unfinished attempt left is used.
//! 4. The edge is asked, and the pairing code it gives is shown for a facility admin to type on
//!    the edge. The command waits, polling, until the request is approved, lapses or is refused.
//! 5. Once approved, the edge and the device id are written to `pki/edge.json`, with the edge's
//!    CA beside it.

use std::path::{Path, PathBuf};

use health_desktop_runtime::keystore::{self, Keystore};
use health_sync_client::enrollment::Enrollment;
use health_sync_client::{Sync, Upstream};
use p256::ecdsa::SigningKey;
use p256::pkcs8::DecodePrivateKey as _;

use crate::provision::pki_dir;

/// The device key's name in the OS keystore.
pub(crate) const DEVICE_KEY: &str = "health.device.key";

const USAGE: &str = "usage: health-desktop enroll --edge URL --ca FILE [--label TEXT]";

/// The arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Args {
    /// The edge's origin, as its `HEALTH_ORIGINS` names it.
    pub(crate) edge: String,
    /// The CA the edge's certificate is verified against: the national root, or the facility's
    /// chain. Required: the edge's certificates are not in the OS trust store, and the staff the
    /// edge hands over for offline sign-in are trusted because the edge is (ADR 0063).
    pub(crate) ca: PathBuf,
    /// What the admin sees the request as.
    pub(crate) label: String,
}

impl Args {
    /// From the flags after `enroll`.
    pub(crate) fn parse(args: &[String]) -> Result<Self, String> {
        let (mut edge, mut ca, mut label) = (None, None, None);
        let mut flags = args.iter();
        while let Some(flag) = flags.next() {
            let value = flags.next().ok_or(USAGE)?;
            match flag.as_str() {
                "--edge" => edge = Some(value.trim_end_matches('/').to_owned()),
                "--ca" => ca = Some(PathBuf::from(value)),
                "--label" => label = Some(value.clone()),
                _ => return Err(USAGE.into()),
            }
        }
        Ok(Self {
            edge: edge.ok_or(USAGE)?,
            ca: ca.ok_or(USAGE)?,
            label: label.unwrap_or_else(|| format!("health-desktop on {}", std::env::consts::OS)),
        })
    }
}

/// How the edge is reached: asked to enroll the key, and asked where the key stands.
pub(crate) trait Edge {
    /// Asks to enroll `key`.
    fn request(&mut self, key: &SigningKey) -> Result<Enrollment, String>;
    /// Where `key` stands.
    fn status(&mut self, key: &SigningKey) -> Result<Enrollment, String>;
    /// Waits before asking again.
    fn wait(&mut self);
}

/// The edge over HTTPS, polled every 5 seconds.
pub(crate) struct Https {
    edge: String,
    ca_pem: Option<String>,
    label: String,
}

impl Https {
    pub(crate) fn new(args: &Args) -> Result<Self, String> {
        Ok(Self {
            edge: args.edge.clone(),
            ca_pem: Some(read(&args.ca)?),
            label: args.label.clone(),
        })
    }
}

impl Edge for Https {
    fn request(&mut self, key: &SigningKey) -> Result<Enrollment, String> {
        health_sync_client::enrollment::request(
            &self.edge,
            &self.label,
            key,
            self.ca_pem.as_deref(),
        )
    }

    fn status(&mut self, key: &SigningKey) -> Result<Enrollment, String> {
        health_sync_client::enrollment::status(&self.edge, key, self.ca_pem.as_deref())
    }

    fn wait(&mut self) {
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

/// The device key in `keys`, if one was made.
pub(crate) fn device_key(keys: &dyn Keystore) -> Result<Option<SigningKey>, String> {
    let Some(pem) = keys.get(DEVICE_KEY).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    std::str::from_utf8(&pem)
        .ok()
        .and_then(|pem| SigningKey::from_pkcs8_pem(pem).ok())
        .map(Some)
        .ok_or_else(|| format!("{DEVICE_KEY} in the keystore is not a P-256 PKCS#8 key"))
}

/// Enrolls this machine with `edge`, showing the pairing code with `show`. Returns the device id.
pub(crate) fn enroll(
    args: &Args,
    data_dir: &Path,
    keys: &dyn Keystore,
    edge: &mut dyn Edge,
    mut show: impl FnMut(&str),
) -> Result<String, String> {
    // 1. No keystore, no install.
    keystore::check(keys).map_err(|e| e.to_string())?;
    let ca_pem = Some(read(&args.ca)?);
    // 2. One role per machine.
    let pki = pki_dir(data_dir);
    if pki.join("chain.pem").is_file() {
        return Err("this machine is provisioned as a standalone node (mode B)".into());
    }
    if pki.join("edge.json").is_file() {
        return Err("this machine is already enrolled with an edge".into());
    }
    // 3. The key, held only in the keystore.
    let key = match device_key(keys)? {
        Some(key) => key,
        None => {
            let pem = health_pki::new_key()
                .map_err(|e| e.to_string())?
                .serialize_pem();
            keys.set(DEVICE_KEY, pem.as_bytes())
                .map_err(|e| e.to_string())?;
            SigningKey::from_pkcs8_pem(&pem).map_err(|e| e.to_string())?
        }
    };
    // 4. Ask, and wait for the admin.
    let mut shown = None;
    let mut state = edge.request(&key)?;
    let device = loop {
        match state {
            Enrollment::Enrolled { device } => break device,
            Enrollment::Pending { code } => {
                if shown.as_ref() != Some(&code) {
                    show(&code);
                    shown = Some(code);
                }
            }
            Enrollment::Revoked => {
                return Err("the edge revoked this device; it cannot enroll again".into());
            }
            Enrollment::None => {
                return Err("the request lapsed unapproved: run `enroll` again".into());
            }
        }
        edge.wait();
        state = edge.status(&key)?;
    };
    // 5. Kept.
    std::fs::create_dir_all(&pki).map_err(|e| e.to_string())?;
    let ca_file = match &ca_pem {
        Some(pem) => {
            let path = pki.join("edge-ca.pem");
            std::fs::write(&path, pem).map_err(|e| format!("edge-ca.pem: {e}"))?;
            Some(path)
        }
        None => None,
    };
    std::fs::write(
        pki.join("edge.json"),
        serde_json::json!({
            "edge": args.edge,
            "edge_ca": ca_file,
            "device_id": device,
        })
        .to_string(),
    )
    .map_err(|e| format!("edge.json: {e}"))?;
    Ok(device)
}

/// The edge this desktop is enrolled with, as a sync upstream whose node is its device id.
pub(crate) fn upstream(data_dir: &Path) -> Option<Upstream> {
    let text = std::fs::read_to_string(pki_dir(data_dir).join("edge.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let ca_pem = match value["edge_ca"].as_str() {
        Some(path) => Some(std::fs::read_to_string(path).ok()?),
        None => None,
    };
    Some(Upstream {
        origin: value["edge"].as_str()?.trim_end_matches('/').to_owned(),
        ca_pem,
        node: value["device_id"].as_str()?.parse().ok()?,
    })
}

/// Sync with the edge, once enrolled; `None` before.
pub(crate) fn sync(
    store: health_surrealdb::SurrealStore,
    data_dir: &Path,
    keys: &dyn Keystore,
) -> Result<Option<Sync>, String> {
    let Some(upstream) = upstream(data_dir) else {
        return Ok(None);
    };
    let key = device_key(keys)?.ok_or("enrolled, but the device key is not in the keystore")?;
    Sync::device(store, upstream, key).map(Some)
}

#[cfg(test)]
mod tests {
    use health_desktop_runtime::keystore::MemoryKeystore;

    use super::*;

    /// An edge that answers from a script.
    struct Scripted {
        answers: Vec<Enrollment>,
        keys: Vec<String>,
    }

    impl Scripted {
        fn next(&mut self, key: &SigningKey) -> Result<Enrollment, String> {
            self.keys
                .push(format!("{:?}", key.verifying_key().to_encoded_point(true)));
            Ok(self.answers.remove(0))
        }
    }

    impl Edge for Scripted {
        fn request(&mut self, key: &SigningKey) -> Result<Enrollment, String> {
            self.next(key)
        }
        fn status(&mut self, key: &SigningKey) -> Result<Enrollment, String> {
            self.next(key)
        }
        fn wait(&mut self) {}
    }

    fn args() -> Args {
        let ca = std::env::temp_dir().join("health-desktop-enroll-test-ca.pem");
        std::fs::write(&ca, "-----BEGIN CERTIFICATE-----\n").expect("ca");
        Args::parse(&[
            "--edge".into(),
            "https://edge.local/".into(),
            "--ca".into(),
            ca.to_string_lossy().into_owned(),
        ])
        .expect("args")
    }

    fn device() -> String {
        "0193a000-0000-7000-8000-000000000001".into()
    }

    #[test]
    fn a_desktop_enrolls_once_the_admin_approves_and_then_syncs_as_its_device() {
        let dir = tempfile::tempdir().expect("dir");
        let keys = MemoryKeystore::default();
        let pending = || Enrollment::Pending {
            code: "123456".into(),
        };
        let mut edge = Scripted {
            answers: vec![
                pending(),
                pending(),
                Enrollment::Enrolled { device: device() },
            ],
            keys: Vec::new(),
        };
        let mut shown = Vec::new();
        let id = enroll(&args(), dir.path(), &keys, &mut edge, |code| {
            shown.push(code.to_owned())
        })
        .expect("enrolled");
        assert_eq!(id, device());
        assert_eq!(shown, ["123456"], "the code is shown once");
        assert!(
            edge.keys.windows(2).all(|w| w[0] == w[1]),
            "one key throughout"
        );
        assert!(keys.get(DEVICE_KEY).expect("get").is_some());

        let upstream = upstream(dir.path()).expect("upstream");
        assert_eq!(upstream.origin, "https://edge.local");
        assert_eq!(upstream.node.to_string(), device());
        let again = enroll(&args(), dir.path(), &keys, &mut edge, |_| {});
        assert!(again.is_err_and(|e| e.contains("already enrolled")));
    }

    #[test]
    fn a_lapsed_or_revoked_request_leaves_nothing_and_the_key_is_reused() {
        let dir = tempfile::tempdir().expect("dir");
        let keys = MemoryKeystore::default();
        for (answer, why) in [
            (Enrollment::None, "lapsed"),
            (Enrollment::Revoked, "revoked"),
        ] {
            let mut edge = Scripted {
                answers: vec![answer],
                keys: Vec::new(),
            };
            let refused = enroll(&args(), dir.path(), &keys, &mut edge, |_| {});
            assert!(refused.is_err_and(|e| e.contains(why)));
            assert!(upstream(dir.path()).is_none());
        }
        let first = keys.get(DEVICE_KEY).expect("get");
        let mut edge = Scripted {
            answers: vec![Enrollment::Enrolled { device: device() }],
            keys: Vec::new(),
        };
        enroll(&args(), dir.path(), &keys, &mut edge, |_| {}).expect("enrolled");
        assert_eq!(keys.get(DEVICE_KEY).expect("get"), first);
    }

    #[test]
    fn a_standalone_node_does_not_enroll_and_the_edge_is_required() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::create_dir_all(pki_dir(dir.path())).expect("pki");
        std::fs::write(pki_dir(dir.path()).join("chain.pem"), "x").expect("chain");
        let mut edge = Scripted {
            answers: Vec::new(),
            keys: Vec::new(),
        };
        let refused = enroll(
            &args(),
            dir.path(),
            &MemoryKeystore::default(),
            &mut edge,
            |_| {},
        );
        assert!(refused.is_err_and(|e| e.contains("standalone")));
        assert!(Args::parse(&["--ca".into(), "x".into()]).is_err());
        assert!(
            Args::parse(&["--edge".into(), "https://edge.local".into()]).is_err(),
            "the CA is required"
        );
    }
}
