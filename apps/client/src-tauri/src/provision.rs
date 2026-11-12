//! `health-desktop provision --code CODE --central URL --root FILE [--central-ca FILE]` (#134;
//! desktop-client.md § Provisioning, mode B): the standalone desktop becomes its facility's
//! Authority Node, as an edge does online (ADR 0062).
//!
//! 1. The keystore must work: there is no install without one.
//! 2. The volumes are probed, and central is told whether they are encrypted; an unencrypted
//!    machine is accepted only under an exemption on the code.
//! 3. A provisioned machine is not provisioned again: a re-provisioned machine is a new node.
//! 4. The node's key is made in the OS keystore, or the one an unfinished attempt left is used.
//! 5. The code is redeemed at central with a request for that key.
//! 6. The chain is kept only once it verifies against the national root. Then the root, the chain
//!    and the node's identity are written under the data directory's `pki/`.

use std::path::{Path, PathBuf};

use health_desktop_runtime::keystore::{self, Keystore};
use health_pki::KeyPair;
use health_sync_client::provisioning::{RedeemError, Redeemed, verify_chain};

/// The node key's name in the OS keystore.
pub(crate) const KEY: &str = "health.node.key";

const USAGE: &str =
    "usage: health-desktop provision --code CODE --central URL --root FILE [--central-ca FILE]";

/// The arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Args {
    code: String,
    central: String,
    root: PathBuf,
    central_ca: Option<PathBuf>,
}

impl Args {
    /// From the flags after `provision`.
    pub(crate) fn parse(args: &[String]) -> Result<Self, String> {
        let (mut code, mut central, mut root, mut central_ca) = (None, None, None, None);
        let mut flags = args.iter();
        while let Some(flag) = flags.next() {
            let value = flags.next().ok_or(USAGE)?;
            match flag.as_str() {
                "--code" => code = Some(value.clone()),
                "--central" => central = Some(value.trim_end_matches('/').to_owned()),
                "--root" => root = Some(PathBuf::from(value)),
                "--central-ca" => central_ca = Some(PathBuf::from(value)),
                _ => return Err(USAGE.into()),
            }
        }
        match (code, central, root) {
            (Some(code), Some(central), Some(root)) => Ok(Self {
                code,
                central,
                root,
                central_ca,
            }),
            _ => Err(USAGE.into()),
        }
    }
}

/// The node's directory of certificates under `data_dir`.
pub(crate) fn pki_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("pki")
}

/// Provisions this machine; `redeem` is central (`health_sync_client::provisioning::redeem`, or a
/// stand-in in tests). Returns what central gave.
pub(crate) fn provision(
    args: &Args,
    data_dir: &Path,
    keys: &dyn Keystore,
    encrypted: bool,
    redeem: impl FnOnce(&str, &str, &str, bool, Option<&str>) -> Result<Redeemed, RedeemError>,
) -> Result<Redeemed, String> {
    let read = |p: &Path| {
        std::fs::read_to_string(p).map_err(|e| format!("cannot read {}: {e}", p.display()))
    };
    // 1. No keystore, no install.
    keystore::check(keys).map_err(|e| e.to_string())?;
    let root_pem = read(&args.root)?;
    let central_ca = args.central_ca.as_deref().map(read).transpose()?;
    // 3. Once only.
    let pki = pki_dir(data_dir);
    if pki.join("chain.pem").is_file() {
        return Err(
            "this machine is already provisioned, and a re-provisioned machine is a new node \
             (identifiers.md)"
                .into(),
        );
    }
    // 4. The key: the one an unfinished attempt left, or a new one, held only in the keystore.
    let key = match keys.get(KEY).map_err(|e| e.to_string())? {
        Some(pem) => String::from_utf8(pem)
            .map_err(|_| format!("{KEY} is not PEM"))
            .and_then(|pem| KeyPair::from_pem(&pem).map_err(|e| e.to_string()))?,
        None => {
            let key = health_pki::new_key().map_err(|e| e.to_string())?;
            keys.set(KEY, key.serialize_pem().as_bytes())
                .map_err(|e| e.to_string())?;
            key
        }
    };
    let csr = health_pki::request(&key, "health-desktop").map_err(|e| e.to_string())?;
    // 5. Central.
    let redeemed = redeem(
        &args.central,
        &args.code,
        &csr,
        encrypted,
        central_ca.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    // 6. Kept only once it verifies.
    let authority = verify_chain(
        &redeemed.chain,
        &key.serialize_pem(),
        &root_pem,
        &redeemed.node_id,
    )?;
    std::fs::create_dir_all(&pki).map_err(|e| e.to_string())?;
    let write = |name: &str, contents: &str| {
        std::fs::write(pki.join(name), contents).map_err(|e| format!("{name}: {e}"))
    };
    write("root.pem", &root_pem)?;
    write(
        "node.json",
        &serde_json::json!({
            "node_id": redeemed.node_id,
            "short_code": redeemed.short_code,
            "facility": redeemed.facility,
            "central": args.central,
            "central_ca": args.central_ca,
            "expires": authority.expires().to_string(),
        })
        .to_string(),
    )?;
    // Last: its presence is what "provisioned" means.
    write("chain.pem", &redeemed.chain)?;
    Ok(redeemed)
}

#[cfg(test)]
mod tests {
    use health_desktop_runtime::keystore::MemoryKeystore;
    use health_pki::{Authority, Facility, IssuingCa, new_key, request};
    use health_time::Timestamp;

    use super::*;

    /// The national root, an issuing CA, and a central that issues for whoever redeems.
    struct Central {
        root: Authority,
        issuing: Authority,
    }

    impl Central {
        fn new() -> Self {
            let at = Timestamp::now();
            let root = Authority::root("Root", at).expect("root");
            let key = new_key().expect("key");
            let issued = root
                .issue_issuing_ca(
                    &request(&key, "issuing").expect("csr"),
                    &IssuingCa {
                        common_name: "Issuing".into(),
                        dns_suffixes: Vec::new(),
                    },
                    at,
                )
                .expect("issuing");
            let issuing =
                Authority::from_pem(&issued.pem(), &key.serialize_pem()).expect("issuing");
            Self { root, issuing }
        }

        fn redeem(&self, csr: &str, encrypted: bool) -> Result<Redeemed, RedeemError> {
            assert!(encrypted, "the probe's answer reaches central");
            let issued = self
                .issuing
                .issue_facility(
                    csr,
                    &Facility {
                        node_id: "ug-kla-0002".into(),
                        dns_names: Vec::new(),
                    },
                    Timestamp::now(),
                )
                .map_err(|e| RedeemError::Failed(e.to_string()))?;
            Ok(Redeemed {
                node_id: "ug-kla-0002".into(),
                short_code: "D1".into(),
                facility: "KLA001".into(),
                chain: format!("{}{}", issued.pem(), self.issuing.certificate_pem()),
            })
        }
    }

    fn args(dir: &Path, root: &Authority) -> Args {
        let path = dir.join("root.pem");
        std::fs::write(&path, root.certificate_pem()).expect("root");
        Args::parse(&[
            "--code".into(),
            "ABCD-EFGH".into(),
            "--central".into(),
            "https://central.example/".into(),
            "--root".into(),
            path.to_string_lossy().into_owned(),
        ])
        .expect("args")
    }

    #[test]
    fn a_desktop_is_provisioned_once_with_its_key_in_the_keystore() {
        let central = Central::new();
        let dir = tempfile::tempdir().expect("dir");
        let args = args(dir.path(), &central.root);
        assert_eq!(args.central, "https://central.example");
        let keys = MemoryKeystore::default();
        let data = dir.path().join("data");

        let redeemed = provision(&args, &data, &keys, true, |_, code, csr, encrypted, _| {
            assert_eq!(code, "ABCD-EFGH");
            central.redeem(csr, encrypted)
        })
        .expect("provisioned");
        assert_eq!(redeemed.short_code, "D1");
        let pki = pki_dir(&data);
        assert!(pki.join("chain.pem").is_file() && pki.join("root.pem").is_file());
        let identity: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(pki.join("node.json")).expect("node"))
                .expect("json");
        assert_eq!(identity["facility"], "KLA001");
        // The key is in the keystore only, never on disk.
        assert!(keys.get(KEY).expect("get").is_some());
        let on_disk: String = std::fs::read_dir(&pki)
            .expect("dir")
            .map(|e| std::fs::read_to_string(e.expect("entry").path()).unwrap_or_default())
            .collect();
        assert!(!on_disk.contains("PRIVATE KEY"));

        // Once only.
        let again = provision(&args, &data, &keys, true, |_, _, csr, encrypted, _| {
            central.redeem(csr, encrypted)
        });
        assert!(again.is_err_and(|e| e.contains("already provisioned")));
    }

    #[test]
    fn a_chain_that_does_not_verify_is_not_kept_and_the_key_is_reused_on_retry() {
        let central = Central::new();
        let other = Central::new();
        let dir = tempfile::tempdir().expect("dir");
        let args = args(dir.path(), &central.root);
        let keys = MemoryKeystore::default();
        let data = dir.path().join("data");

        // A chain under another root is refused, and nothing is written.
        let refused = provision(&args, &data, &keys, true, |_, _, csr, encrypted, _| {
            other.redeem(csr, encrypted)
        });
        assert!(refused.is_err_and(|e| e.contains("does not verify")));
        assert!(!pki_dir(&data).join("chain.pem").exists());
        let first = keys.get(KEY).expect("get").expect("the key stays");

        // Central refuses the code: an error, nothing written.
        let code = provision(&args, &data, &keys, true, |_, _, _, _, _| {
            Err(RedeemError::Code("used".into()))
        });
        assert!(code.is_err_and(|e| e.contains("refused the code")));

        // The retry uses the same key.
        provision(&args, &data, &keys, true, |_, _, csr, encrypted, _| {
            central.redeem(csr, encrypted)
        })
        .expect("provisioned");
        assert_eq!(keys.get(KEY).expect("get"), Some(first));
    }

    #[test]
    fn the_flags_are_required() {
        assert!(Args::parse(&["--code".into(), "X".into()]).is_err());
        assert!(Args::parse(&["--code".into()]).is_err());
        assert!(Args::parse(&["--what".into(), "x".into()]).is_err());
    }
}
