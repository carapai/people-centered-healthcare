//! Online provisioning (#268; #134): a node redeems its one-time code at central with a
//! certificate request for its key, and gets its node ID, short code, facility and the facility
//! intermediate's chain. The chain is kept only once it verifies: its first certificate is a
//! facility intermediate for the node's key, and a leaf it issues for `<node-id>.local` verifies
//! against the national root. Shared by the edge and the standalone desktop, which keep the key
//! in their own keystores.

use health_pki::{Authority, Tier, Trust, new_key};
use health_time::Timestamp;

/// What central gave a node at provisioning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redeemed {
    /// The node ID central minted.
    pub node_id: String,
    /// Its short code, in the identifiers it mints (`E1`, …).
    pub short_code: String,
    /// The facility it serves.
    pub facility: String,
    /// The facility intermediate's certificate, then the issuing CA's, as PEM.
    pub chain: String,
}

/// Why a code was not redeemed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RedeemError {
    /// Central refused the machine (422): unencrypted without an exemption on the code.
    #[error("central refused: {0}")]
    Refused(String),
    /// Central refused the code (403): unknown, used or expired.
    #[error("central refused the code: {0}")]
    Code(String),
    /// Central could not be reached, or answered otherwise.
    #[error("{0}")]
    Failed(String),
}

/// Redeems `code` at `central` (its origin) with `csr` (PEM) for a machine that is `encrypted`,
/// verifying central against `central_ca` when given, else the OS trust store.
pub fn redeem(
    central: &str,
    code: &str,
    csr: &str,
    encrypted: bool,
    central_ca: Option<&str>,
) -> Result<Redeemed, RedeemError> {
    let url = format!("{}/api/provisioning/nodes", central.trim_end_matches('/'));
    let (status, answer) = crate::central::post_json(
        &url,
        &serde_json::json!({ "code": code, "request": csr, "encrypted": encrypted }),
        central_ca,
    )
    .map_err(RedeemError::Failed)?;
    let detail = || answer["detail"].as_str().unwrap_or("").to_owned();
    match status {
        201 => {}
        422 => return Err(RedeemError::Refused(detail())),
        403 => return Err(RedeemError::Code(detail())),
        status => {
            return Err(RedeemError::Failed(format!(
                "central answered {status}: {}",
                answer["error"].as_str().unwrap_or("no reason")
            )));
        }
    }
    let field = |name: &str| {
        answer[name]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| RedeemError::Failed(format!("central's answer has no {name}")))
    };
    Ok(Redeemed {
        node_id: field("node_id")?,
        short_code: field("short_code")?,
        facility: field("facility")?,
        chain: field("chain")?,
    })
}

/// The facility intermediate from `chain_pem` and its key `key_pem`, once a leaf it issues for
/// `<node_id>.local` verifies against `root_pem`.
pub fn verify_chain(
    chain_pem: &str,
    key_pem: &str,
    root_pem: &str,
    node_id: &str,
) -> Result<Authority, String> {
    let authority = Authority::from_pem(chain_pem, key_pem).map_err(|e| e.to_string())?;
    if authority.tier() != Tier::Facility {
        return Err("the chain's first certificate is not a facility intermediate".into());
    }
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
    Ok(authority)
}
