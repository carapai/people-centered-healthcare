//! Loads a generated directory through the FHIR API: batch Bundles of `PUT {type}/{id}`
//! (update-as-create). The ids are deterministic, so loading twice cannot duplicate: the server
//! refuses the second load's updates, which lack If-Match. Load into an empty database. Files load
//! in manifest order, one at a time, because references point only at earlier files.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tokio::task::JoinSet;

use crate::output::read_manifest;

/// How to reach the server.
#[derive(Debug, Clone)]
pub struct Target {
    /// The FHIR base URL, e.g. `http://127.0.0.1:8080/fhir`. Plain HTTP only: every TLS option of
    /// the HTTP client brings a root-certificate crate whose licence ADR 0045 does not allow.
    pub base: String,
    /// A bearer token, if the server needs one.
    pub bearer: Option<String>,
    /// Resources per batch Bundle.
    pub batch: usize,
    /// Batch Bundles in flight.
    pub concurrency: usize,
}

/// Loaded resources per file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    /// The file name.
    pub path: String,
    /// Resources the server accepted.
    pub count: usize,
}

/// Loads every file of `dir` into `target`, failing on the first entry the server refuses.
pub async fn load(dir: &Path, target: &Target) -> Result<Vec<Loaded>> {
    let manifest = read_manifest(dir)?;
    let client = Arc::new(reqwest::Client::new());
    let mut loaded = Vec::new();
    for file in &manifest.files {
        let started = Instant::now();
        let reader = BufReader::new(
            std::fs::File::open(dir.join(&file.path))
                .with_context(|| format!("open {}", file.path))?,
        );
        let mut lines = reader.lines();
        let mut in_flight = JoinSet::new();
        let mut count = 0;
        loop {
            let mut chunk = Vec::with_capacity(target.batch);
            for line in lines.by_ref().take(target.batch) {
                chunk.push(serde_json::from_str::<Value>(&line?)?);
            }
            if chunk.is_empty() {
                break;
            }
            if in_flight.len() >= target.concurrency {
                count += joined(in_flight.join_next().await)?;
            }
            in_flight.spawn(send(Arc::clone(&client), target.clone(), chunk));
        }
        while let Some(result) = in_flight.join_next().await {
            count += joined(Some(result))?;
        }
        if count != file.count {
            bail!("{}: loaded {count} of {}", file.path, file.count);
        }
        eprintln!(
            "{}: {count} in {:.1} s",
            file.path,
            started.elapsed().as_secs_f64()
        );
        loaded.push(Loaded {
            path: file.path.clone(),
            count,
        });
    }
    Ok(loaded)
}

fn joined(result: Option<Result<Result<usize>, tokio::task::JoinError>>) -> Result<usize> {
    match result {
        Some(result) => result?,
        None => Ok(0),
    }
}

/// Sends one batch; every entry must come back 200 or 201.
async fn send(
    client: Arc<reqwest::Client>,
    target: Target,
    resources: Vec<Value>,
) -> Result<usize> {
    let entries: Vec<Value> = resources
        .into_iter()
        .map(|resource| {
            let url = format!(
                "{}/{}",
                resource["resourceType"].as_str().unwrap_or_default(),
                resource["id"].as_str().unwrap_or_default()
            );
            json!({ "resource": resource, "request": { "method": "PUT", "url": url } })
        })
        .collect();
    let count = entries.len();
    let bundle = json!({ "resourceType": "Bundle", "type": "batch", "entry": entries });
    let mut request = client
        .post(target.base.trim_end_matches('/'))
        .header("content-type", "application/fhir+json")
        .header("accept", "application/fhir+json")
        .body(serde_json::to_vec(&bundle)?);
    if let Some(token) = &target.bearer {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.context("send a batch")?;
    let status = response.status();
    let body: Value = serde_json::from_slice(&response.bytes().await?).unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("batch refused with {status}: {}", diagnostics(&body));
    }
    let entries = body["entry"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    for entry in entries {
        let code = entry["response"]["status"].as_str().unwrap_or_default();
        if !(code.starts_with("200") || code.starts_with("201")) {
            bail!(
                "entry refused with {code}: {}",
                diagnostics(&entry["response"]["outcome"])
            );
        }
    }
    if entries.len() != count {
        bail!("{} responses for {count} entries", entries.len());
    }
    Ok(count)
}

/// The issues of an OperationOutcome (synthetic data, so diagnostics are safe to print).
fn diagnostics(outcome: &Value) -> String {
    outcome["issue"]
        .as_array()
        .map(|issues| {
            issues
                .iter()
                .map(|i| {
                    format!(
                        "{} {}",
                        i["code"].as_str().unwrap_or("?"),
                        i["diagnostics"].as_str().unwrap_or("")
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        })
        .unwrap_or_else(|| "no OperationOutcome".to_owned())
}
