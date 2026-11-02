//! `facility-edge provision …`: the edge's half of provisioning (lan-security.md; ADRs 0008,
//! 0048, 0058). Central's half, the one-time code, the node ID and recovery-key escrow, is #268.
//!
//! - `provision request --out FILE`: refuse a machine that fails the full-disk encryption gate
//!   (exit 10) unless it is exempted, then generate the facility intermediate's key, seal it in
//!   the keystore, and write its certificate request for central's issuing CA.
//! - `provision install --chain FILE --root FILE`: check the issued chain (the facility
//!   intermediate, then the issuing CA) against the national root and the sealed key, then
//!   install both under `HEALTH_PKI_DIR`.
//! - `provision status`: what is sealed and installed, and when the intermediate expires.
//!
//! The key is generated on the edge and only ever sealed: it never leaves the keystore in the
//! clear on disk, and never leaves the machine.

use std::path::{Path, PathBuf};

use health_edge_runtime::EdgeConfig;
use health_node_identity::encryption::{Exemption, Gate, gate, probe, system_root};
use health_node_identity::keystore::Keystore;
use health_pki::{Authority, Tier, Trust, new_key, request};
use health_time::Timestamp;

/// The facility intermediate's key in the keystore; the unit's `LoadCredentialEncrypted=`
/// names it too.
pub(crate) const KEY: &str = "health.facility-intermediate.key";
/// The installed national root.
const ROOT: &str = "root.pem";
/// The installed chain: the facility intermediate, then the issuing CA.
const CHAIN: &str = "facility-chain.pem";

const USAGE: &str = "usage: facility-edge provision (request --out FILE | install --chain FILE --root FILE | status)";

/// A refusal by the encryption gate (exit 10), or any other failure (exit 11).
pub(crate) enum Failure {
    Refused(String),
    Failed(String),
}

impl From<String> for Failure {
    fn from(error: String) -> Self {
        Self::Failed(error)
    }
}

/// Runs `args` (after `provision`).
pub(crate) fn run(config: &EdgeConfig, args: &[String]) -> Result<(), Failure> {
    let keystore = Keystore::system();
    match args {
        [cmd, flag, out] if cmd == "request" && flag == "--out" => {
            request_identity(config, &keystore, Path::new(out))
        }
        [cmd, a, chain, b, root] if cmd == "install" && a == "--chain" && b == "--root" => {
            install(config, &keystore, Path::new(chain), Path::new(root)).map_err(Failure::from)
        }
        [cmd] if cmd == "status" => {
            use std::io::Write as _;
            writeln!(std::io::stdout(), "{}", status(config, &keystore))
                .map_err(|e| Failure::Failed(e.to_string()))
        }
        _ => Err(Failure::Failed(USAGE.into())),
    }
}

fn request_identity(config: &EdgeConfig, keystore: &Keystore, out: &Path) -> Result<(), Failure> {
    let state = probe(&system_root(), &config.data_dir, Timestamp::now());
    let exemption = config
        .encryption_exemption
        .clone()
        .map(|grant| Exemption { grant });
    match gate(&state, exemption.as_ref()) {
        Gate::Allowed => {}
        Gate::Exempt { grant, .. } => {
            eprintln!("facility-edge: exempt from full-disk encryption by {grant}");
        }
        Gate::Refused { findings } => {
            let codes: Vec<&str> = findings.iter().map(|f| f.code()).collect();
            return Err(Failure::Refused(format!(
                "provisioning refused: {}",
                codes.join(", ")
            )));
        }
    }
    if keystore.contains(KEY) {
        return Err(Failure::Failed(format!(
            "{KEY} is already sealed: this machine already has an identity, and a re-provisioned \
             machine is a new node (identifiers.md)"
        )));
    }
    let key = new_key().map_err(|e| e.to_string())?;
    let csr = request(&key, &config.node_id).map_err(|e| e.to_string())?;
    keystore
        .seal(KEY, key.serialize_pem().as_bytes())
        .map_err(|e| e.to_string())?;
    std::fs::write(out, csr).map_err(|e| format!("cannot write {}: {e}", out.display()))?;
    eprintln!(
        "facility-edge: the facility intermediate's key is sealed; send {} to central",
        out.display()
    );
    Ok(())
}

fn install(
    config: &EdgeConfig,
    keystore: &Keystore,
    chain: &Path,
    root: &Path,
) -> Result<(), String> {
    let read = |p: &Path| {
        std::fs::read_to_string(p).map_err(|e| format!("cannot read {}: {e}", p.display()))
    };
    let (chain_pem, root_pem) = (read(chain)?, read(root)?);
    let authority = authority(keystore, &chain_pem)?;
    // The chain must serve: a leaf it issues for the node's name verifies against the root.
    let name = format!("{}.local", config.node_id);
    let leaf = authority
        .issue_server(
            &new_key().map_err(|e| e.to_string())?,
            std::slice::from_ref(&name),
            &[],
            Timestamp::now(),
        )
        .map_err(|e| format!("the intermediate cannot certify {name}: {e}"))?;
    Trust::new(&root_pem)
        .and_then(|trust| {
            trust.verify_server(
                &format!("{}{chain_pem}", leaf.pem()),
                &name,
                Timestamp::now(),
            )
        })
        .map_err(|e| format!("the chain does not verify against the root: {e}"))?;
    std::fs::create_dir_all(&config.pki_dir).map_err(|e| e.to_string())?;
    replace(&config.pki_dir.join(ROOT), &root_pem)?;
    replace(&config.pki_dir.join(CHAIN), &chain_pem)?;
    eprintln!(
        "facility-edge: facility intermediate installed, valid until {}",
        authority.expires()
    );
    Ok(())
}

/// The facility intermediate, from the chain's first certificate and the sealed key.
fn authority(keystore: &Keystore, chain_pem: &str) -> Result<Authority, String> {
    let key = keystore
        .unseal(KEY)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("{KEY} is not sealed: run `provision request` first"))?;
    let key = String::from_utf8(key).map_err(|_| format!("{KEY} is not PEM"))?;
    let authority = Authority::from_pem(chain_pem, &key).map_err(|e| e.to_string())?;
    if authority.tier() != Tier::Facility {
        return Err("the chain's first certificate is not a facility intermediate".into());
    }
    Ok(authority)
}

/// The node's identity, for `provision status` and the start-up log.
pub(crate) fn status(config: &EdgeConfig, keystore: &Keystore) -> serde_json::Value {
    let chain = std::fs::read_to_string(config.pki_dir.join(CHAIN)).ok();
    let installed = chain
        .as_deref()
        .map(|chain| authority(keystore, chain).map(|a| a.expires()));
    serde_json::json!({
        "node_id": config.node_id,
        "key_sealed": keystore.contains(KEY),
        "root_installed": config.pki_dir.join(ROOT).is_file(),
        "intermediate_expires": installed.as_ref().and_then(|r| r.as_ref().ok()).map(Timestamp::to_string),
        "problem": installed.and_then(Result::err),
    })
}

/// The facility intermediate's key from the keystore, for the node to issue its TLS leaf.
pub(crate) fn intermediate_key() -> health_edge_runtime::IntermediateKey {
    std::sync::Arc::new(|| {
        Keystore::system()
            .unseal(KEY)
            .map_err(|e| e.to_string())?
            .map(|key| String::from_utf8(key).map_err(|_| format!("{KEY} is not PEM")))
            .transpose()
    })
}

/// Logs whether this start found its identity: `identity-loaded`, or a `not-provisioned` or
/// `identity-invalid` alert. The node serves either way until LAN TLS needs it (#74).
pub(crate) fn log_identity(config: &EdgeConfig) {
    let status = status(config, &Keystore::system());
    if status["intermediate_expires"].is_string() {
        tracing::info!(
            outcome = "identity-loaded",
            "the facility intermediate is installed"
        );
    } else if status["problem"].is_string() {
        tracing::warn!(
            alert = "identity-invalid",
            "the installed facility intermediate does not load"
        );
    } else {
        tracing::warn!(
            alert = "not-provisioned",
            "this edge has no facility intermediate yet"
        );
    }
}

fn replace(path: &PathBuf, contents: &str) -> Result<(), String> {
    let temporary = path.with_extension("pem.new");
    std::fs::write(&temporary, contents)
        .and_then(|()| std::fs::rename(&temporary, path))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}
