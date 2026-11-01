//! `facility-edge update …`: staging, applying and rolling back signed updates
//! (`health_edge_runtime::update`).
//!
//! - `update stage <package> <signature>`: verify against `HEALTH_UPDATE_KEY_FILE` and unpack.
//!   The node may keep running.
//! - `update apply`: with the service stopped, swap the staged binary in for this one.
//! - `update rollback <installed-binary>`: run from `previous/` by the service manager when the
//!   new binary keeps failing; restores the store, then this binary over the installed one.
//! - `update status`: print the journal.

use std::io::Write;
use std::path::Path;

use health_edge_runtime::EdgeConfig;
use health_edge_runtime::update::{self, Updates};

/// Runs `args` (after `update`); the error is printed and becomes exit code 7.
pub(crate) fn run(config: &EdgeConfig, args: &[String]) -> Result<(), String> {
    let updates = Updates::new(&config.update_dir);
    let fail = |e: update::UpdateError| e.to_string();
    match args {
        [cmd, package, signature] if cmd == "stage" => {
            let key_file = config
                .update_key_file
                .as_ref()
                .ok_or("HEALTH_UPDATE_KEY_FILE is required to stage")?;
            let key = std::fs::read_to_string(key_file)
                .ok()
                .and_then(|text| update::release_key(&text))
                .ok_or("HEALTH_UPDATE_KEY_FILE is not a P-256 public key")?;
            let package = std::fs::read(package).map_err(|_| "cannot read the package")?;
            let signature =
                std::fs::read_to_string(signature).map_err(|_| "cannot read the signature")?;
            let manifest = update::stage(&updates, &package, &signature, &key).map_err(fail)?;
            say(&format!("staged {}", manifest.version))
        }
        [cmd] if cmd == "apply" => {
            let binary = std::env::current_exe().map_err(|_| "cannot find this binary")?;
            let manifest =
                update::apply(&updates, &binary, env!("CARGO_PKG_VERSION")).map_err(fail)?;
            say(&format!("applied {}; start the service", manifest.version))
        }
        [cmd, installed] if cmd == "rollback" => {
            update::roll_back(&updates, &config.data_dir, Path::new(installed)).map_err(fail)?;
            say("rolled back; start the service")
        }
        [cmd] if cmd == "status" => {
            let journal = updates.journal().map_err(fail)?;
            let staged = updates.staged().map_err(fail)?;
            let text = serde_json::json!({ "journal": journal, "staged": staged });
            say(&text.to_string())
        }
        _ => Err("usage: facility-edge update (stage <package> <signature> | apply | rollback <installed-binary> | status)".into()),
    }
}

fn say(line: &str) -> Result<(), String> {
    writeln!(std::io::stdout(), "{line}").map_err(|_| "cannot write to stdout".into())
}
