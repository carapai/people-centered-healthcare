//! `facility-edge provision …`: the edge's half of provisioning (lan-security.md; ADRs 0008,
//! 0048, 0058; #268).
//!
//! - `provision --code CODE --central URL --root FILE [--central-ca FILE]`: provisioning online.
//!   1. The encryption gate runs. An edge that fails it says so to central, which accepts it only
//!      if its code carries an exemption.
//!   2. The intermediate's key is sealed, or reused from an attempt that did not finish.
//!   3. The request goes to central's `POST /api/provisioning/nodes` over HTTPS, with central
//!      verified against the system trust store, or `--central-ca` alone.
//!   4. Central's answer is checked and installed as `install` does.
//!   5. `node.json` records the node ID, short code and facility central gave, and where central
//!      is. The node then runs under them unless `HEALTH_NODE_ID` or `HEALTH_FACILITY` says
//!      otherwise.
//!   6. On an encrypted edge, the disk recovery keys are generated and escrowed at central
//!      ([`crate::recovery`]).
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
use health_pki::{Authority, KeyPair, Tier, Trust, new_key, request};
use health_time::Timestamp;

/// The facility intermediate's key in the keystore; the unit's `LoadCredentialEncrypted=`
/// names it too.
pub(crate) const KEY: &str = health_edge_runtime::INTERMEDIATE_KEY;
/// The installed national root.
const ROOT: &str = "root.pem";
/// The installed chain: the facility intermediate, then the issuing CA.
const CHAIN: &str = "facility-chain.pem";

const USAGE: &str = "usage: facility-edge provision (--code CODE --central URL --root FILE [--central-ca FILE] | request --out FILE | install --chain FILE --root FILE | status)";
/// The node's identity from central.
const IDENTITY: &str = "node.json";

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
        [first, ..] if first == "--code" => online(config, &keystore, args),
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
    let key = seal_keys(keystore, None)?;
    let csr = request(&key, &config.node_id).map_err(|e| e.to_string())?;
    std::fs::write(out, csr).map_err(|e| format!("cannot write {}: {e}", out.display()))?;
    eprintln!(
        "facility-edge: the facility intermediate's key is sealed; send {} to central",
        out.display()
    );
    Ok(())
}

/// Seals the intermediate's key, or returns `reuse` when it is already sealed, and seals the
/// authorization server's keys (offline-auth.md; ADR 0059) as root, where the platform keystore
/// can: the pepper key every stored hash is encrypted under, and the access-token signing key.
/// Those are kept if already sealed.
fn seal_keys(keystore: &Keystore, reuse: Option<KeyPair>) -> Result<KeyPair, Failure> {
    let key = match reuse {
        Some(key) => key,
        None => {
            let key = new_key().map_err(|e| e.to_string())?;
            keystore
                .seal(KEY, key.serialize_pem().as_bytes())
                .map_err(|e| e.to_string())?;
            key
        }
    };
    for name in [
        health_edge_runtime::PEPPER_KEY,
        health_edge_runtime::TOKEN_KEY,
    ] {
        if !keystore.contains(name) {
            let secret = p256::SecretKey::random(&mut rand_core::OsRng).to_bytes();
            keystore.seal(name, &secret).map_err(|e| e.to_string())?;
        }
    }
    Ok(key)
}

/// `provision --code …`: see the module documentation.
fn online(config: &EdgeConfig, keystore: &Keystore, args: &[String]) -> Result<(), Failure> {
    let (mut code, mut central, mut root, mut central_ca) = (None, None, None, None);
    let mut flags = args.iter();
    while let Some(flag) = flags.next() {
        let value = flags.next().ok_or_else(|| Failure::Failed(USAGE.into()))?;
        match flag.as_str() {
            "--code" => code = Some(value.clone()),
            "--central" => central = Some(value.trim_end_matches('/').to_owned()),
            "--root" => root = Some(PathBuf::from(value)),
            "--central-ca" => central_ca = Some(PathBuf::from(value)),
            _ => return Err(Failure::Failed(USAGE.into())),
        }
    }
    let (Some(code), Some(central), Some(root)) = (code, central, root) else {
        return Err(Failure::Failed(USAGE.into()));
    };
    let read = |p: &Path| {
        std::fs::read_to_string(p).map_err(|e| format!("cannot read {}: {e}", p.display()))
    };
    let root_pem = read(&root)?;
    let central_ca = central_ca.as_deref().map(read).transpose()?;

    // 1. The gate: central decides whether an unencrypted edge's code is exempt.
    let state = probe(&system_root(), &config.data_dir, Timestamp::now());
    let encrypted = match gate(&state, None) {
        Gate::Allowed => true,
        Gate::Exempt { .. } => false,
        Gate::Refused { findings } => {
            let codes: Vec<&str> = findings.iter().map(|f| f.code()).collect();
            eprintln!(
                "facility-edge: not encrypted as ADR 0048 requires ({}); central accepts this only \
                 under an exemption on the code",
                codes.join(", ")
            );
            false
        }
    };

    // 2. The key: a provisioned node is not provisioned again; an unfinished attempt is resumed.
    if config.pki_dir.join(CHAIN).is_file() {
        return Err(Failure::Failed(
            "this machine is already provisioned, and a re-provisioned machine is a new node \
             (identifiers.md)"
                .into(),
        ));
    }
    let sealed = keystore
        .unseal(KEY)
        .map_err(|e| e.to_string())?
        .map(|pem| {
            String::from_utf8(pem)
                .map_err(|_| format!("{KEY} is not PEM"))
                .and_then(|pem| KeyPair::from_pem(&pem).map_err(|e| e.to_string()))
        })
        .transpose()?;
    let key = seal_keys(keystore, sealed)?;
    let csr = request(&key, "facility-edge").map_err(|e| e.to_string())?;

    // 3. Central.
    let url = format!("{central}/api/provisioning/nodes");
    let (status, answer) = crate::central::post_json(
        &url,
        &serde_json::json!({ "code": code, "request": csr, "encrypted": encrypted }),
        central_ca.as_deref(),
    )?;
    let detail = || answer["detail"].as_str().unwrap_or("").to_owned();
    match status {
        201 => {}
        422 => return Err(Failure::Refused(format!("central refused: {}", detail()))),
        403 => {
            return Err(Failure::Failed(format!(
                "central refused the code: {}",
                detail()
            )));
        }
        status => {
            return Err(Failure::Failed(format!(
                "central answered {status}: {}",
                answer["error"].as_str().unwrap_or("no reason")
            )));
        }
    }
    let field = |name: &str| {
        answer[name]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("central's answer has no {name}"))
    };
    let (node_id, short_code, facility, chain) = (
        field("node_id")?,
        field("short_code")?,
        field("facility")?,
        field("chain")?,
    );

    // 4. The chain, checked as `install` checks it, for the node ID central gave.
    install_pem(config, keystore, &chain, &root_pem, &node_id)?;

    // 5. The identity, and where central is, for escrows to come.
    let central_ca_path = args
        .iter()
        .position(|a| a == "--central-ca")
        .and_then(|i| args.get(i + 1))
        .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p)));
    replace(
        &config.pki_dir.join(IDENTITY),
        &serde_json::json!({
            "node_id": node_id,
            "short_code": short_code,
            "facility": facility,
            "central": central,
            "central_ca": central_ca_path,
        })
        .to_string(),
    )?;
    eprintln!("facility-edge: provisioned as {short_code} ({node_id}) at {facility}");

    // 6. The disk recovery keys (ADR 0048), escrowed before anyone leaves the machine.
    if encrypted {
        let rotated = crate::recovery::rotate_here(config, keystore, None, None).map_err(|e| {
            Failure::Failed(format!(
                "provisioned, but the disk recovery keys were not escrowed: {e}; run \
                 `facility-edge encryption rotate-recovery`"
            ))
        })?;
        for key in &rotated.escrowed {
            eprintln!(
                "facility-edge: {} recovery key escrowed at central",
                key.volume.as_str()
            );
        }
    }
    Ok(())
}

/// The node ID, short code and facility central gave this node at provisioning, if it was
/// provisioned online.
pub(crate) fn identity(config: &EdgeConfig) -> Option<(String, String, String)> {
    let text = std::fs::read_to_string(config.pki_dir.join(IDENTITY)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let get = |name: &str| value[name].as_str().map(str::to_owned);
    Some((get("node_id")?, get("short_code")?, get("facility")?))
}

/// Runs the node under the identity central gave it, where the environment does not say
/// otherwise (`node_id_set`, `facility_set`).
pub(crate) fn adopt_identity(config: &mut EdgeConfig, node_id_set: bool, facility_set: bool) {
    if let Some((node_id, _, facility)) = identity(config) {
        if !node_id_set {
            config.node_id = node_id;
        }
        if !facility_set {
            config.facility = Some(facility);
        }
    }
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
    install_pem(
        config,
        keystore,
        &read(chain)?,
        &read(root)?,
        &config.node_id,
    )
}

/// Installs `chain_pem` under `root_pem` once a leaf it issues for `<node_id>.local` verifies.
fn install_pem(
    config: &EdgeConfig,
    keystore: &Keystore,
    chain_pem: &str,
    root_pem: &str,
    node_id: &str,
) -> Result<(), String> {
    let authority = authority(keystore, chain_pem)?;
    // The chain must serve: a leaf it issues for the node's name verifies against the root.
    let name = format!("{node_id}.local");
    let leaf = authority
        .issue_server(
            &new_key().map_err(|e| e.to_string())?,
            std::slice::from_ref(&name),
            &[],
            Timestamp::now(),
        )
        .map_err(|e| format!("the intermediate cannot certify {name}: {e}"))?;
    Trust::new(root_pem)
        .and_then(|trust| {
            trust.verify_server(
                &format!("{}{chain_pem}", leaf.pem()),
                &name,
                Timestamp::now(),
            )
        })
        .map_err(|e| format!("the chain does not verify against the root: {e}"))?;
    std::fs::create_dir_all(&config.pki_dir).map_err(|e| e.to_string())?;
    replace(&config.pki_dir.join(ROOT), root_pem)?;
    replace(&config.pki_dir.join(CHAIN), chain_pem)?;
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
    let central = identity(config);
    serde_json::json!({
        "node_id": config.node_id,
        "short_code": central.as_ref().map(|(_, short_code, _)| short_code),
        "key_sealed": keystore.contains(KEY),
        "root_installed": config.pki_dir.join(ROOT).is_file(),
        "intermediate_expires": installed.as_ref().and_then(|r| r.as_ref().ok()).map(Timestamp::to_string),
        "problem": installed.and_then(Result::err),
    })
}

/// The keystore's secrets by name, for the node: the facility intermediate's key to issue its TLS
/// leaf, and its authorization server's pepper and token-signing keys.
pub(crate) fn secrets() -> health_edge_runtime::Secrets {
    std::sync::Arc::new(|name: &str| Keystore::system().unseal(name).map_err(|e| e.to_string()))
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
