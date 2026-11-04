//! `central-server node …` (#268; ADR 0048 § Decommissioning; ADR 0058 § Revocation):
//!
//! - `node decommission <node-id> --by <operator> --erased <tpm-cleared|key-slots-destroyed>
//!   --reason <why>` records a crypto-erased edge's decommissioning, after the facility has
//!   cleared its TPM or destroyed its key slots. In one audited transaction, its facility
//!   intermediate is revoked and its escrowed recovery keys are destroyed. It is never active
//!   again.
//! - `node crl --out <file>` writes the issuing CA's next CRL, listing every decommissioned
//!   node's intermediate, for distribution to the edges. It needs `HEALTH_ISSUING_CA_CERT_FILE`
//!   and `HEALTH_ISSUING_CA_KEY_FILE`.

use std::io::Write as _;
use std::sync::Arc;

use health_identity::{Erasure, NodeId};
use health_provisioning::NodeRegistry;
use health_time::SystemClock;

use crate::config::Config;

const USAGE: &str = "usage: central-server node (decommission <node-id> --by <operator> --erased <tpm-cleared|key-slots-destroyed> --reason <why> | crl --out <file>)";

/// Runs `args` (after `node`).
pub(crate) async fn run(
    config: &Config,
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let registry = || async {
        Ok::<_, Box<dyn std::error::Error>>(NodeRegistry::new(
            crate::provision::open_surreal(config).await?,
            Arc::new(SystemClock),
            "central",
        ))
    };
    let mut out = std::io::stdout();
    match args {
        [cmd, node, rest @ ..] if cmd == "decommission" => {
            let node = NodeId(node.parse().map_err(|_| "the node id is not a UUID")?);
            let (mut by, mut erased, mut reason) = (None, None, None);
            let mut flags = rest.iter();
            while let Some(flag) = flags.next() {
                let value = flags.next().ok_or(USAGE)?.clone();
                match flag.as_str() {
                    "--by" => by = Some(value),
                    "--reason" => reason = Some(value),
                    "--erased" => {
                        erased = Some(match value.as_str() {
                            "tpm-cleared" => Erasure::TpmCleared,
                            "key-slots-destroyed" => Erasure::KeySlotsDestroyed,
                            _ => return Err(USAGE.into()),
                        });
                    }
                    _ => return Err(USAGE.into()),
                }
            }
            let (Some(by), Some(erased), Some(reason)) = (by, erased, reason) else {
                return Err(USAGE.into());
            };
            let node = registry()
                .await?
                .decommission(node, &by, erased, &reason)
                .await?;
            writeln!(
                out,
                "{} ({}) at {} decommissioned; intermediate {} revoked, recovery keys destroyed. \
                 Publish the next CRL with `central-server node crl`.",
                node.short_code, node.id, node.facility, node.intermediate_serial
            )?;
            Ok(())
        }
        [cmd, flag, file] if cmd == "crl" && flag == "--out" => {
            let (cert, key) = config
                .auth
                .as_ref()
                .and_then(|auth| auth.issuing_ca.as_ref())
                .ok_or("a CRL needs HEALTH_ISSUING_CA_CERT_FILE and HEALTH_ISSUING_CA_KEY_FILE")?;
            let issuer = crate::read_issuing_ca(cert, key)?;
            let crl = registry().await?.crl(&issuer).await?;
            std::fs::write(file, crl).map_err(|e| format!("cannot write {file}: {e}"))?;
            writeln!(out, "the issuing CA's CRL is in {file}")?;
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}
