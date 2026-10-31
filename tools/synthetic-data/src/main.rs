//! `synthetic-data generate` writes NDJSON with a manifest; `synthetic-data load` sends it
//! through a FHIR API. See README.md.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use synthetic_data::load::{Target, load};
use synthetic_data::output::generate;

const USAGE: &str = "usage:
  synthetic-data generate --out DIR [--seed N] [--patients N] [--health-worker-phones N]
  synthetic-data load --dir DIR --base URL [--bearer TOKEN] [--batch N] [--concurrency N]";

/// Health-worker phones in the spike: 3,000 at 45M people.
fn default_phones(patients: usize) -> usize {
    (patients.saturating_mul(3_000) / 45_000_000).max(1)
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let mut options = std::collections::HashMap::new();
    while let Some(flag) = args.next() {
        let Some(name) = flag.strip_prefix("--") else {
            bail!("unexpected {flag}\n{USAGE}")
        };
        let value = args
            .next()
            .with_context(|| format!("--{name} needs a value\n{USAGE}"))?;
        options.insert(name.to_owned(), value);
    }
    let number = |name: &str, default: usize| -> Result<usize> {
        options.get(name).map_or(Ok(default), |v| {
            v.parse().with_context(|| format!("--{name}"))
        })
    };
    let path = |name: &str| -> Result<PathBuf> {
        options
            .get(name)
            .map(PathBuf::from)
            .with_context(|| format!("--{name} is required\n{USAGE}"))
    };
    match command.as_str() {
        "generate" => {
            let patients = number("patients", 1_000)?;
            let seed = options
                .get("seed")
                .map_or(Ok(1), |v| v.parse())
                .context("--seed")?;
            let started = Instant::now();
            let manifest = generate(
                seed,
                patients,
                number("health-worker-phones", default_phones(patients))?,
                &path("out")?,
            )?;
            for file in &manifest.files {
                eprintln!("{}: {} ({})", file.path, file.count, &file.sha256[..16]);
            }
            eprintln!("generated in {:.1} s", started.elapsed().as_secs_f64());
            Ok(())
        }
        "load" => {
            let target = Target {
                base: options
                    .get("base")
                    .cloned()
                    .with_context(|| format!("--base is required\n{USAGE}"))?,
                bearer: options.get("bearer").cloned(),
                batch: number("batch", 250)?,
                concurrency: number("concurrency", 8)?,
            };
            let started = Instant::now();
            let runtime = tokio::runtime::Runtime::new()?;
            let loaded = runtime.block_on(load(&path("dir")?, &target))?;
            let total: usize = loaded.iter().map(|l| l.count).sum();
            eprintln!(
                "loaded {total} resources in {:.1} s",
                started.elapsed().as_secs_f64()
            );
            Ok(())
        }
        _ => bail!("{USAGE}"),
    }
}
