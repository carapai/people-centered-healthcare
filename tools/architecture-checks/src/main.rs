//! `architecture-checks [--manifest-path Cargo.toml] [--allowlist allowlist.toml]`
//!
//! Exits 0 when the workspace follows the layer rules, 1 when it breaks any, 2 on error.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use architecture_checks::{Allowlist, check};

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(error) => {
            eprintln!("architecture-checks: {error:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<bool> {
    let mut manifest = PathBuf::from("Cargo.toml");
    let mut allowlist_path = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--manifest-path" => {
                manifest = args.next().context("--manifest-path needs a value")?.into()
            }
            "--allowlist" => {
                allowlist_path = Some(PathBuf::from(
                    args.next().context("--allowlist needs a value")?,
                ))
            }
            other => bail!("unknown argument `{other}`"),
        }
    }
    let allowlist_path = allowlist_path.unwrap_or_else(|| {
        manifest
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("tools/architecture-checks/allowlist.toml")
    });
    let allowlist = Allowlist::load(&allowlist_path)?;
    let violations = check(&manifest, &allowlist)?;

    let mut out = std::io::stdout().lock();
    for violation in &violations {
        writeln!(out, "{violation}")?;
    }
    if violations.is_empty() {
        writeln!(out, "architecture-checks: ok")?;
    } else {
        writeln!(
            out,
            "architecture-checks: {} violation(s)",
            violations.len()
        )?;
    }
    Ok(violations.is_empty())
}
