//! `facility-edge update …`: staging, applying and rolling back signed updates
//! (`health_edge_runtime::update`).
//!
//! - `update stage <package> <signature>`: verify against `HEALTH_UPDATE_KEY_FILE` and unpack.
//!   The node may keep running.
//! - `update apply`: with the service stopped, swap the staged binary in for this one.
//! - `update rollback <installed-binary>`: run from `previous/` by the service manager when the
//!   new binary keeps failing; restores the store, then this binary over the installed one.
//! - `update status`: print the journal.
//! - `update pack <out>`: package this binary at its own version, for the release job to sign.
//! - `update verify <package> <signature>`: check a signed package against
//!   `HEALTH_UPDATE_KEY_FILE` without staging it: the release job's check of what it signed.

use std::io::Write;
use std::path::Path;

use health_edge_runtime::EdgeConfig;
use health_edge_runtime::update::{self, Updates};

/// Runs `args` (after `update`); the error is printed and becomes exit code 7.
pub(crate) fn run(config: &EdgeConfig, args: &[String]) -> Result<(), String> {
    let updates = Updates::new(&config.update_dir);
    let fail = |e: update::UpdateError| e.to_string();
    let key = || -> Result<_, String> {
        let key_file = config
            .update_key_file
            .as_ref()
            .ok_or("HEALTH_UPDATE_KEY_FILE is required")?;
        std::fs::read_to_string(key_file)
            .ok()
            .and_then(|text| update::release_key(&text))
            .ok_or_else(|| "HEALTH_UPDATE_KEY_FILE is not a P-256 public key".into())
    };
    let signed = |package: &str, signature: &str| -> Result<(Vec<u8>, String), String> {
        let package = std::fs::read(package).map_err(|_| "cannot read the package")?;
        let signature =
            std::fs::read_to_string(signature).map_err(|_| "cannot read the signature")?;
        Ok((package, signature))
    };
    match args {
        [cmd, package, signature] if cmd == "stage" => {
            let key = key()?;
            let (package, signature) = signed(package, signature)?;
            let manifest = update::stage(&updates, &package, &signature, &key).map_err(fail)?;
            say(&format!("staged {}", manifest.version))
        }
        [cmd, package, signature] if cmd == "verify" => {
            let key = key()?;
            let (package, signature) = signed(package, signature)?;
            let manifest = update::verify(&package, &signature, &key).map_err(fail)?;
            say(&format!("verified {} {}", manifest.binary, manifest.version))
        }
        [cmd, out] if cmd == "pack" => {
            let binary = std::env::current_exe().map_err(|_| "cannot find this binary")?;
            let name = binary
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or("the binary has no file name")?
                .to_owned();
            let bytes = std::fs::read(&binary).map_err(|_| "cannot read this binary")?;
            let package =
                update::pack(&bytes, &name, env!("CARGO_PKG_VERSION")).map_err(fail)?;
            std::fs::write(out, package).map_err(|_| "cannot write the package")?;
            say(&format!("packed {name} {}", env!("CARGO_PKG_VERSION")))
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
        _ => Err("usage: facility-edge update (stage <package> <signature> | apply | rollback <installed-binary> | status | pack <out> | verify <package> <signature>)".into()),
    }
}

fn say(line: &str) -> Result<(), String> {
    writeln!(std::io::stdout(), "{line}").map_err(|_| "cannot write to stdout".into())
}
