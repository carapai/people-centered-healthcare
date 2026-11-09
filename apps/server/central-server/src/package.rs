//! `central-server package …`: the publication lifecycle for Published Definition packages (#88;
//! configuration.md § Lifecycle; ADR 0025), until the Clinical Metadata Studio. Each step is
//! audited, with `--by` as who acted.
//!
//! - `package draft <dir> --by <author>` starts or revises the draft of the package whose files
//!   are under `<dir>` (`package.json` at its top).
//! - `package submit <name@version> --by <author>` submits it: validation and simulation run at
//!   once, and their problems are printed when it returns to Draft.
//! - `package review | approve <name@version> --by <person>`, and
//!   `package request-changes <name@version> --by <person> --reason <why>`.
//! - `package publish <name@version> --by <operator>` signs it with the publication-signing key
//!   (`HEALTH_PUBLICATION_KEY_FILE`, a PKCS#8 PEM, and `HEALTH_PUBLICATION_CHAIN_FILE`, its
//!   certificate and any chain below the national root).
//! - `package status <name@version>` prints its stage, people and records; `package list` the
//!   packages not yet published.
//! - `package export <name@version> --out <dir>` writes a published package's
//!   `<name>-<version>.tgz` and `.tgz.sig`.
//!
//! - `package activate <name@version> --scope <scope> --by <operator> [--effective <time>]
//!   [--reason <why>]` activates a published version in `<scope>`: `national`,
//!   `district:<code>`, `facility:<code>` or `program:<facility>:<program>`, from `<time>`
//!   (RFC 3339; now by default). Rollback is the activation of an earlier version (#89).
//! - `package activations` prints every activation issued, oldest first.
//!
//! Tarballs are kept in central's blob store, `HEALTH_FILES_DIR`, from which nodes fetch the
//! packages activated for them (`HEALTH_BLOB_DIR` is read when it is not set).

use std::io::Write as _;
use std::path::Path;
use std::sync::Arc;

use health_configuration::activation::Scope;
use health_configuration::lifecycle::{Lifecycle, Record, Step};
use health_configuration::package::Version;
use health_files::FsBlobStore;
use health_packages::Schemas;
use health_publication::{Files, LocalSigner, Publications, WorkflowSandbox};
use health_time::SystemClock;

use crate::config::Config;

const USAGE: &str = "usage: central-server package (draft <dir> --by <author> | submit <id> --by <author> | review <id> --by <reviewer> | approve <id> --by <approver> | request-changes <id> --by <person> --reason <why> | publish <id> --by <operator> | status <id> | list | export <id> --out <dir> | activate <id> --scope <scope> --by <operator> [--effective <time>] [--reason <why>] | activations), where <id> is name@version and <scope> is national, district:<code>, facility:<code> or program:<facility>:<program>";

type Failure = Box<dyn std::error::Error>;

/// A lifecycle refusal as its message, which is what the operator needs to read.
fn said(error: health_publication::Error) -> Failure {
    error.to_string().into()
}

/// `--flag value` pairs from `args`, refusing any flag not in `allowed`.
fn flags<'a>(args: &'a [String], allowed: &[&str]) -> Result<Vec<(&'a str, &'a str)>, Failure> {
    let mut found = Vec::new();
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        let value = rest.next().ok_or(USAGE)?;
        if !allowed.contains(&flag.as_str()) {
            return Err(USAGE.into());
        }
        found.push((flag.as_str(), value.as_str()));
    }
    Ok(found)
}

fn flag<'a>(found: &[(&str, &'a str)], name: &str) -> Result<&'a str, Failure> {
    found
        .iter()
        .find(|(flag, _)| *flag == name)
        .map(|(_, value)| *value)
        .ok_or_else(|| format!("{name} is required\n{USAGE}").into())
}

/// Every file under `dir`, by its path from `dir` with `/` separators.
fn read_dir(dir: &Path) -> Result<Files, Failure> {
    fn walk(root: &Path, dir: &Path, files: &mut Files) -> Result<(), Failure> {
        for entry in
            std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        {
            let path = entry?.path();
            if path.is_dir() {
                walk(root, &path, files)?;
            } else {
                let relative = path.strip_prefix(root)?;
                let name: Vec<String> = relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect();
                files.insert(name.join("/"), std::fs::read(&path)?);
            }
        }
        Ok(())
    }
    let mut files = Files::new();
    walk(dir, dir, &mut files)?;
    Ok(files)
}

fn publications(store: health_surrealdb::SurrealStore) -> Result<Publications, Failure> {
    // The store sync serves, so nodes can fetch what is activated for them.
    let dir = std::env::var("HEALTH_FILES_DIR")
        .or_else(|_| std::env::var("HEALTH_BLOB_DIR"))
        .map_err(|_| "package commands need HEALTH_FILES_DIR, central's blob store")?;
    let blobs = FsBlobStore::open(&dir).map_err(|e| format!("cannot open {dir}: {e}"))?;
    Ok(Publications::new(
        store,
        blobs,
        Arc::new(SystemClock),
        Schemas::platform(),
        Arc::new(WorkflowSandbox),
        "central",
    ))
}

fn signer() -> Result<LocalSigner, Failure> {
    let read = |name: &str| -> Result<String, Failure> {
        let path = std::env::var(name).map_err(|_| format!("publishing needs {name}"))?;
        std::fs::read_to_string(&path).map_err(|e| format!("cannot read {path}: {e}").into())
    };
    let key = read("HEALTH_PUBLICATION_KEY_FILE")?;
    let chain = read("HEALTH_PUBLICATION_CHAIN_FILE")?;
    Ok(LocalSigner::from_pem(&key, chain)?)
}

/// One line per lifecycle fact: stage, authors, reviewer and approvals.
fn summary(lifecycle: &Lifecycle) -> String {
    let join = |people: &std::collections::BTreeSet<String>| {
        people.iter().cloned().collect::<Vec<_>>().join(", ")
    };
    format!(
        "{}@{} ({:?}): {}\n  content {}\n  authors: {}\n  reviewer: {}\n  approvals: {} of {}{}",
        lifecycle.package,
        lifecycle.version,
        lifecycle.kind,
        lifecycle.stage,
        lifecycle.content,
        join(&lifecycle.authors),
        lifecycle.reviewer.as_deref().unwrap_or("-"),
        lifecycle.approvers.len(),
        lifecycle.approvals_required(),
        if lifecycle.approvers.is_empty() {
            String::new()
        } else {
            format!(" ({})", join(&lifecycle.approvers))
        },
    )
}

/// A record as a line, with any problems under it.
fn record_line(record: &Record) -> String {
    let (step, problems): (&str, &[String]) = match &record.step {
        Step::Created => ("created", &[]),
        Step::Revised => ("revised", &[]),
        Step::Submitted => ("submitted", &[]),
        Step::Validated => ("validated", &[]),
        Step::ValidationFailed { problems } => ("validation failed", problems),
        Step::Simulated => ("simulated", &[]),
        Step::SimulationFailed { problems } => ("simulation failed", problems),
        Step::Reviewed => ("reviewed", &[]),
        Step::ChangesRequested { reason } => {
            return format!("  {} {} changes requested: {reason}", record.at, record.by);
        }
        Step::Approved => ("approved", &[]),
        Step::Published => ("published", &[]),
    };
    let mut line = format!("  {} {} {step}", record.at, record.by);
    for problem in problems {
        line.push_str(&format!("\n    - {problem}"));
    }
    line
}

/// A scope as the operator writes it.
fn scope(text: &str) -> Result<Scope, Failure> {
    let parts: Vec<&str> = text.split(':').collect();
    let named = |part: &str| (!part.trim().is_empty()).then(|| part.trim().to_owned());
    let scope = match parts.as_slice() {
        ["national"] => Some(Scope::National),
        ["district", code] => named(code).map(|district| Scope::District { district }),
        ["facility", code] => named(code).map(|facility| Scope::Facility { facility }),
        ["program", facility, program] => named(facility)
            .zip(named(program))
            .map(|(facility, program)| Scope::Program { facility, program }),
        _ => None,
    };
    scope.ok_or_else(|| format!("{text:?} is not a scope\n{USAGE}").into())
}

/// `name@version`, split.
fn package_id(id: &str) -> Result<(&str, Version), Failure> {
    let (name, version) = id
        .rsplit_once('@')
        .ok_or_else(|| format!("{id:?} is not name@version"))?;
    Ok((name, version.parse().map_err(|e| format!("{id}: {e}"))?))
}

/// Runs `args` (after `package`).
pub(crate) async fn run(config: &Config, args: &[String]) -> Result<(), Failure> {
    let open = || async { publications(crate::provision::open_surreal(config).await?) };
    let mut out = std::io::stdout();
    match args {
        [cmd, dir, rest @ ..] if cmd == "draft" => {
            let found = flags(rest, &["--by"])?;
            let files = read_dir(Path::new(dir))?;
            let lifecycle = open()
                .await?
                .draft(&files, flag(&found, "--by")?)
                .await
                .map_err(said)?;
            writeln!(out, "{}", summary(&lifecycle))?;
        }
        [cmd, id, rest @ ..] if cmd == "submit" => {
            let found = flags(rest, &["--by"])?;
            let publications = open().await?;
            let lifecycle = publications
                .submit(id, flag(&found, "--by")?)
                .await
                .map_err(said)?;
            writeln!(out, "{}", summary(&lifecycle))?;
            let (_, records) = publications.lifecycle(id).await.map_err(said)?;
            if let Some(last) = records.last()
                && matches!(
                    last.step,
                    Step::ValidationFailed { .. } | Step::SimulationFailed { .. }
                )
            {
                writeln!(out, "{}", record_line(last))?;
            }
        }
        [cmd, id, rest @ ..] if cmd == "review" || cmd == "approve" => {
            let found = flags(rest, &["--by"])?;
            let by = flag(&found, "--by")?;
            let publications = open().await?;
            let lifecycle = if cmd == "review" {
                publications.review(id, by).await.map_err(said)?
            } else {
                publications.approve(id, by).await.map_err(said)?
            };
            writeln!(out, "{}", summary(&lifecycle))?;
        }
        [cmd, id, rest @ ..] if cmd == "request-changes" => {
            let found = flags(rest, &["--by", "--reason"])?;
            let lifecycle = open()
                .await?
                .request_changes(id, flag(&found, "--by")?, flag(&found, "--reason")?)
                .await
                .map_err(said)?;
            writeln!(out, "{}", summary(&lifecycle))?;
        }
        [cmd, id, rest @ ..] if cmd == "publish" => {
            let found = flags(rest, &["--by"])?;
            let signer = signer()?;
            let published = open()
                .await?
                .publish(id, flag(&found, "--by")?, &signer)
                .await
                .map_err(said)?;
            writeln!(
                out,
                "{} published, sha-256 {}. Export it with `central-server package export`.",
                published.id, published.digest
            )?;
        }
        [cmd, id] if cmd == "status" => {
            let (lifecycle, records) = open().await?.lifecycle(id).await.map_err(said)?;
            writeln!(out, "{}", summary(&lifecycle))?;
            for record in &records {
                writeln!(out, "{}", record_line(record))?;
            }
        }
        [cmd] if cmd == "list" => {
            for lifecycle in open().await?.open().await.map_err(said)? {
                writeln!(
                    out,
                    "{}@{}\t{}",
                    lifecycle.package, lifecycle.version, lifecycle.stage
                )?;
            }
        }
        [cmd, id, rest @ ..] if cmd == "export" => {
            let found = flags(rest, &["--out"])?;
            let dir = Path::new(flag(&found, "--out")?);
            let (published, tgz) = open().await?.package(id).await.map_err(said)?;
            std::fs::create_dir_all(dir)?;
            let name = format!("{}-{}.tgz", published.package, published.version);
            std::fs::write(dir.join(&name), tgz)?;
            std::fs::write(dir.join(format!("{name}.sig")), &published.signature)?;
            writeln!(out, "{name} and {name}.sig are in {}", dir.display())?;
        }
        [cmd, id, rest @ ..] if cmd == "activate" => {
            let found = flags(rest, &["--scope", "--by", "--effective", "--reason"])?;
            let (name, version) = package_id(id)?;
            let effective = found
                .iter()
                .find(|(flag, _)| *flag == "--effective")
                .map(|(_, at)| at.parse().map_err(|e| format!("--effective {at}: {e}")))
                .transpose()?;
            let reason = found
                .iter()
                .find(|(flag, _)| *flag == "--reason")
                .map(|(_, why)| *why);
            let record = open()
                .await?
                .activate(
                    name,
                    &version,
                    scope(flag(&found, "--scope")?)?,
                    effective,
                    flag(&found, "--by")?,
                    reason,
                )
                .await
                .map_err(said)?;
            let a = &record.activation;
            writeln!(
                out,
                "{}@{} activated for {} from {} ({}). Nodes receive it in the packages lane.",
                a.package, a.version, a.scope, a.effective_at, a.id
            )?;
        }
        [cmd] if cmd == "activations" => {
            for record in open().await?.activations().await.map_err(said)? {
                let a = &record.activation;
                writeln!(
                    out,
                    "{}\t{}@{}\t{}\tfrom {}\tby {}{}",
                    a.issued_at,
                    a.package,
                    a.version,
                    a.scope,
                    a.effective_at,
                    a.by,
                    a.reason
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_are_read_in_pairs_and_unknown_ones_refused() {
        let args: Vec<String> = ["--by", "ann", "--reason", "typo"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let found = flags(&args, &["--by", "--reason"]).expect("flags");
        assert_eq!(flag(&found, "--by").expect("by"), "ann");
        assert_eq!(flag(&found, "--reason").expect("reason"), "typo");
        assert!(flag(&found, "--out").is_err());
        assert!(flags(&args, &["--by"]).is_err());
        assert!(flags(&args[..1], &["--by"]).is_err());
    }

    #[test]
    fn scopes_and_ids_are_read_as_the_operator_writes_them() {
        assert_eq!(scope("national").expect("national"), Scope::National);
        assert_eq!(
            scope("district:kampala").expect("district"),
            Scope::District {
                district: "kampala".into()
            }
        );
        assert_eq!(
            scope("facility:KLA01").expect("facility"),
            Scope::Facility {
                facility: "KLA01".into()
            }
        );
        assert_eq!(
            scope("program:KLA01:art").expect("program"),
            Scope::Program {
                facility: "KLA01".into(),
                program: "art".into()
            }
        );
        for bad in ["", "facility", "facility:", "program:KLA01", "regional:x"] {
            assert!(scope(bad).is_err(), "{bad}");
        }
        let (name, version) = package_id("ug.moh.anc@1.2.0").expect("id");
        assert_eq!(
            (name, version.to_string().as_str()),
            ("ug.moh.anc", "1.2.0")
        );
        assert!(package_id("ug.moh.anc").is_err());
        assert!(package_id("ug.moh.anc@latest").is_err());
    }

    #[test]
    fn a_directory_is_read_with_forward_slashes() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("configuration/layouts")).expect("mkdir");
        std::fs::write(dir.path().join("package.json"), b"{}").expect("write");
        std::fs::write(dir.path().join("configuration/layouts/a.json"), b"[]").expect("write");
        let files = read_dir(dir.path()).expect("read");
        let paths: Vec<&String> = files.keys().collect();
        assert_eq!(paths, ["configuration/layouts/a.json", "package.json"]);
    }
}
