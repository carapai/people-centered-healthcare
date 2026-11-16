//! `central-server release …`: platform releases and their staged roll-out (#136; deployment.md
//! § Environments and rollout; ADRs 0041, 0045).
//!
//! - `release import <dir> <version>` reads release `<version>` from `<dir>`, where its GitHub
//!   release was downloaded (`gh release download v<version> --dir <dir>`, after
//!   `gh attestation verify`). Every artifact must match `SHA256SUMS` and verify against the
//!   keys nodes pin: the edge release-signing key (`HEALTH_RELEASE_KEY_FILE`, hex SEC1, as edges
//!   hold it) and the Tauri updater key (`HEALTH_UPDATER_KEY_FILE`, as Tauri prints it). Then the
//!   artifacts go into central's blob store (`HEALTH_FILES_DIR`), from which nodes fetch them.
//! - `release rollout <version> --scope <scope> --by <operator> [--effective <time>]
//!   [--reason <why>]` allows an imported release in `<scope>`: `national`, `district:<code>`
//!   or `facility:<code>`, from `<time>` (RFC 3339; now by default). Holding a wave back is a
//!   roll-out of the version it stays on; nodes never move to an older release.
//! - `release list` prints the imported releases, then every roll-out issued, oldest first.

use std::io::Write as _;
use std::path::Path;

use health_configuration::package::Version;
use health_configuration::rollout::{Rollout, RolloutRecord, check};
use health_files::{FsBlobStore, MAX_RELEASE_SIZE};
use health_packages::release::{ReleaseKeys, read_release};
use health_persistence::{Store as _, Transaction as _};
use health_time::Timestamp;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::package::{flag, flags, scope};

const USAGE: &str = "usage: central-server release (import <dir> <version> | rollout <version> --scope <scope> --by <operator> [--effective <time>] [--reason <why>] | list), where <scope> is national, district:<code> or facility:<code>";

type Failure = Box<dyn std::error::Error>;

/// An imported release, as central keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Imported {
    version: Version,
    artifacts: Vec<health_configuration::rollout::Artifact>,
}

fn env_file(name: &str) -> Result<String, Failure> {
    let path = std::env::var(name).map_err(|_| format!("release import needs {name}"))?;
    std::fs::read_to_string(&path).map_err(|e| format!("cannot read {name} ({path}): {e}").into())
}

fn blobs() -> Result<FsBlobStore, Failure> {
    let dir = std::env::var("HEALTH_FILES_DIR")
        .map_err(|_| "release commands need HEALTH_FILES_DIR, central's blob store")?;
    FsBlobStore::open(&dir).map_err(|e| format!("cannot open {dir}: {e}").into())
}

fn version(text: &str) -> Result<Version, Failure> {
    text.trim_start_matches('v')
        .parse()
        .map_err(|e| format!("{text}: {e}\n{USAGE}").into())
}

/// Runs `args` (after `release`).
pub(crate) async fn run(config: &Config, args: &[String]) -> Result<(), Failure> {
    let mut out = std::io::stdout();
    match args {
        [cmd, dir, v] if cmd == "import" => {
            let version = version(v)?;
            let keys = ReleaseKeys::from_text(
                &env_file("HEALTH_RELEASE_KEY_FILE")?,
                &env_file("HEALTH_UPDATER_KEY_FILE")?,
            )?;
            let artifacts = read_release(Path::new(dir), &version, &keys)?;
            let blobs = blobs()?;
            for (artifact, bytes) in &artifacts {
                blobs
                    .put_limited(bytes.as_slice(), MAX_RELEASE_SIZE)
                    .map_err(|e| format!("{}: {e}", artifact.file))?;
            }
            let imported = Imported {
                version: version.clone(),
                artifacts: artifacts.into_iter().map(|(a, _)| a).collect(),
            };
            let store = crate::provision::open_surreal(config).await?;
            let mut tx = store.begin().await?;
            let new = tx
                .put_platform_release(&version.to_string(), &serde_json::to_string(&imported)?)
                .await?;
            tx.commit().await?;
            if new {
                writeln!(out, "imported {version}: every artifact verifies")?;
                for a in &imported.artifacts {
                    writeln!(
                        out,
                        "  {}\t{}\t{} bytes\t{}",
                        a.target, a.file, a.size, a.sha256
                    )?;
                }
            } else {
                writeln!(out, "{version} was imported already; nothing changed")?;
            }
        }
        [cmd, v, rest @ ..] if cmd == "rollout" => {
            let found = flags(rest, &["--scope", "--by", "--effective", "--reason"])?;
            let version = version(v)?;
            let now = Timestamp::now();
            let effective = found
                .iter()
                .find(|(flag, _)| *flag == "--effective")
                .map(|(_, at)| at.parse().map_err(|e| format!("--effective {at}: {e}")))
                .transpose()?
                .unwrap_or(now);
            let store = crate::provision::open_surreal(config).await?;
            let mut tx = store.begin().await?;
            let imported: Imported = match tx.platform_release(&version.to_string()).await? {
                Some(json) => serde_json::from_str(&json)?,
                None => {
                    return Err(
                        format!("{version} is not imported; run release import first").into(),
                    );
                }
            };
            let record = RolloutRecord {
                rollout: Rollout {
                    id: uuid::Uuid::now_v7().to_string(),
                    version: version.clone(),
                    scope: scope(flag(&found, "--scope")?)?,
                    effective_at: effective,
                    issued_at: now,
                    by: flag(&found, "--by")?.to_owned(),
                    reason: found
                        .iter()
                        .find(|(flag, _)| *flag == "--reason")
                        .map(|(_, why)| (*why).to_owned()),
                },
                artifacts: imported.artifacts,
            };
            check(&record)?;
            let r = &record.rollout;
            tx.put_release_rollout(
                &r.id,
                &r.issued_at.to_string(),
                &serde_json::to_string(&record)?,
            )
            .await?;
            tx.commit().await?;
            writeln!(
                out,
                "{} allowed for {} from {} ({}). Nodes receive it in the packages lane.",
                r.version, r.scope, r.effective_at, r.id
            )?;
        }
        [cmd] if cmd == "list" => {
            let store = crate::provision::open_surreal(config).await?;
            let mut tx = store.begin().await?;
            for json in tx.platform_releases().await? {
                let imported: Imported = serde_json::from_str(&json)?;
                writeln!(
                    out,
                    "release {}\t{} artifacts",
                    imported.version,
                    imported.artifacts.len()
                )?;
            }
            for json in tx.release_rollouts().await? {
                let record: RolloutRecord = serde_json::from_str(&json)?;
                let r = &record.rollout;
                writeln!(
                    out,
                    "{}\t{}\t{}\tfrom {}\tby {}{}",
                    r.issued_at,
                    r.version,
                    r.scope,
                    r.effective_at,
                    r.by,
                    r.reason
                        .as_deref()
                        .map(|why| format!("\t{why}"))
                        .unwrap_or_default()
                )?;
            }
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}
