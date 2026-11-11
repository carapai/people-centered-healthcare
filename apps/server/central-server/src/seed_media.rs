//! `central-server seed export <facility> <file>` (#83; synchronization.md § Seeding): writes the
//! facility's snapshot to `<file>` for removable media, a page at a time, and its detached
//! signature to `<file>.jws`, by the publication signer (`HEALTH_PUBLICATION_KEY_FILE` and
//! `HEALTH_PUBLICATION_CHAIN_FILE`) as a seed. The edge applies it with
//! `facility-edge seed import <file>`, for its own facility only.

use std::io::Write as _;
use std::path::Path;

use health_packages::{Digest, SEED_TYP, sign_detached};
use health_time::Timestamp;

use crate::config::Config;

const USAGE: &str = "usage: central-server seed export <facility> <file>";

/// Runs `args` (after `seed`).
pub(crate) async fn run(
    config: &Config,
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let [cmd, facility, file] = args else {
        return Err(USAGE.into());
    };
    if cmd != "export" {
        return Err(USAGE.into());
    }
    // The signer first: a seed nobody can verify is not written.
    let signer = crate::package::signer()?;
    let store = crate::provision::open_surreal(config).await?;
    let path = Path::new(file);
    let now = Timestamp::now();
    let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
    let (digest, heads) = health_sync_server::write_seed(&store, facility, now, &mut out).await?;
    out.into_inner().map_err(|e| e.into_error())?.sync_all()?;
    let jws = sign_detached(&Digest(digest), &signer, now, SEED_TYP)?;
    let signature = path.with_file_name(format!(
        "{}.jws",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("seed")
    ));
    std::fs::write(&signature, format!("{jws}\n"))?;
    writeln!(
        std::io::stdout(),
        "{heads} heads for {facility} in {}, signed in {}",
        path.display(),
        signature.display()
    )?;
    Ok(())
}
