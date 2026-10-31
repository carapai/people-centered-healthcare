//! Writes the generated resources as NDJSON, one file per load step, with a manifest of counts
//! and SHA-256 digests: the same seed and size give byte-identical files.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::population::Population;
use crate::world::World;

/// The manifest file in an output directory.
pub const MANIFEST: &str = "manifest.json";

/// What a directory holds, in load order: a file's references point only at earlier files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The generator and its version.
    pub generator: String,
    /// The seed.
    pub seed: u64,
    /// The number of patients.
    pub patients: usize,
    /// The files, in load order.
    pub files: Vec<FileEntry>,
}

/// One NDJSON file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// The file name, relative to the manifest.
    pub path: String,
    /// The resource type of every line.
    #[serde(rename = "resourceType")]
    pub resource_type: String,
    /// The number of resources.
    pub count: usize,
    /// The SHA-256 of the file, in hex.
    pub sha256: String,
}

/// Generates the world and `patients` patients for `seed` into `dir`.
pub fn generate(
    seed: u64,
    patients: usize,
    health_worker_phones: usize,
    dir: &Path,
) -> Result<Manifest> {
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let world = World::new(seed);
    let fixed: [(&str, &str, &[Value]); 8] = [
        ("Organization.ndjson", "Organization", &world.organizations),
        (
            "Location.facility.ndjson",
            "Location",
            &world.facility_locations,
        ),
        ("Location.store.ndjson", "Location", &world.store_locations),
        ("Medication.ndjson", "Medication", &world.medications),
        ("InventoryItem.ndjson", "InventoryItem", &world.inventory),
        (
            "ChargeItemDefinition.ndjson",
            "ChargeItemDefinition",
            &world.prices,
        ),
        ("InsurancePlan.ndjson", "InsurancePlan", &world.plans),
        ("Contract.ndjson", "Contract", &world.contracts),
    ];
    let mut files = Vec::new();
    for (path, resource_type, resources) in fixed {
        files.push(write(dir, path, resource_type, resources.iter().cloned())?);
    }
    let population = Population::new(seed, &world, health_worker_phones).take(patients);
    files.push(write(dir, "Patient.ndjson", "Patient", population)?);
    let manifest = Manifest {
        generator: concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION")).to_owned(),
        seed,
        patients,
        files,
    };
    let text = serde_json::to_string_pretty(&manifest)? + "\n";
    std::fs::write(dir.join(MANIFEST), text).context("write the manifest")?;
    Ok(manifest)
}

fn write(
    dir: &Path,
    path: &str,
    resource_type: &str,
    resources: impl Iterator<Item = Value>,
) -> Result<FileEntry> {
    let file = File::create(dir.join(path)).with_context(|| format!("create {path}"))?;
    let mut out = BufWriter::new(file);
    let mut digest = Sha256::new();
    let mut count = 0;
    for resource in resources {
        let mut line = serde_json::to_vec(&resource)?;
        line.push(b'\n');
        digest.update(&line);
        out.write_all(&line)?;
        count += 1;
    }
    out.flush()?;
    Ok(FileEntry {
        path: path.to_owned(),
        resource_type: resource_type.to_owned(),
        count,
        sha256: digest
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    })
}

/// Reads the manifest of `dir`.
pub fn read_manifest(dir: &Path) -> Result<Manifest> {
    let text = std::fs::read_to_string(dir.join(MANIFEST))
        .with_context(|| format!("read {}/{MANIFEST}", dir.display()))?;
    Ok(serde_json::from_str(&text)?)
}
