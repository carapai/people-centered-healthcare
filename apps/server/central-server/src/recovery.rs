//! `central-server recovery …` (#268; ADR 0048 § Recovery keys): an edge's disk recovery key,
//! read out by a central operations person, by phone, to a facility admin whose identity they
//! verified against the device registry. There is no web endpoint that returns a key.
//!
//! - `recovery read <node-id> <system|data> --by <operator> --verified <how>` prints the key
//!   once. The read is audited as a warning with who read it and how the caller was verified,
//!   and the key is used from then on: it stays a `recovery-key-used` alert until the edge
//!   escrows its replacement.
//! - `recovery used` lists the keys read out and not yet rotated.
//!
//! Both need `HEALTH_KMS_KEY_FILE`, the master key that wraps them.

use std::io::Write as _;
use std::sync::Arc;

use health_identity::{NodeId, RecoveryVolume};
use health_provisioning::{LocalKeyWrap, NodeRegistry};
use health_time::SystemClock;

use crate::config::Config;

const USAGE: &str = "usage: central-server recovery (read <node-id> <system|data> --by <operator> --verified <how> | used)";

/// Runs `args` (after `recovery`).
pub(crate) async fn run(
    config: &Config,
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let key_file = config
        .auth
        .as_ref()
        .and_then(|auth| auth.kms_key_file.as_deref())
        .ok_or("recovery keys need HEALTH_AUTH_ISSUER and HEALTH_KMS_KEY_FILE")?;
    let kms = LocalKeyWrap::new(&crate::auth::read_kms_key(key_file)?);
    let registry = NodeRegistry::new(
        crate::provision::open_surreal(config).await?,
        Arc::new(SystemClock),
        "central",
    );
    let mut out = std::io::stdout();
    match args {
        [cmd] if cmd == "used" => {
            for key in registry.used_recovery_keys().await? {
                let last = key.retrievals.last();
                writeln!(
                    out,
                    "{} {} read {} by {}",
                    key.node,
                    key.volume.as_str(),
                    last.map(|r| r.at.to_string()).unwrap_or_default(),
                    last.map(|r| r.by.as_str()).unwrap_or_default(),
                )?;
            }
            Ok(())
        }
        [cmd, node, volume, rest @ ..] if cmd == "read" => {
            let node = NodeId(node.parse().map_err(|_| "the node id is not a UUID")?);
            let volume = match volume.as_str() {
                "system" => RecoveryVolume::System,
                "data" => RecoveryVolume::Data,
                _ => return Err(USAGE.into()),
            };
            let (mut by, mut verified) = (None, None);
            let mut flags = rest.iter();
            while let Some(flag) = flags.next() {
                let value = flags.next().ok_or(USAGE)?.clone();
                match flag.as_str() {
                    "--by" => by = Some(value),
                    "--verified" => verified = Some(value),
                    _ => return Err(USAGE.into()),
                }
            }
            let (Some(by), Some(verified)) = (by, verified) else {
                return Err(USAGE.into());
            };
            let key = registry
                .read_recovery(node, volume, &by, &verified, &kms)
                .await?;
            writeln!(
                out,
                "{key}\nread out and audited; the edge must rotate it (`facility-edge encryption rotate-recovery`)"
            )?;
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}
