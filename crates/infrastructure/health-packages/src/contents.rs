//! A package's files as the domain sees them, and the FHIR NPM `.index.json` that lists its
//! artefacts.

use health_configuration::package::{
    Artefact, Canonical, INDEX, MANIFEST, PACKAGE_JSON, PackageContents, PackageJson,
};
use serde_json::{Value, json};

use crate::Error;
use crate::archive::Files;

fn json(path: &str, bytes: &[u8]) -> Result<Value, Error> {
    serde_json::from_slice(bytes).map_err(|e| Error::Json {
        path: path.to_owned(),
        detail: e.to_string(),
    })
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// The FHIR artefacts: top-level `*.json` files other than `package.json` and `.index.json`,
/// each with a `resourceType`.
fn artefacts(files: &Files) -> Result<Vec<(Artefact, Value)>, Error> {
    let mut artefacts = Vec::new();
    for (path, bytes) in files {
        if path.contains('/') || !path.ends_with(".json") || path == PACKAGE_JSON || path == INDEX {
            continue;
        }
        let value = json(path, bytes)?;
        let resource_type = text(&value, "resourceType").ok_or_else(|| Error::Json {
            path: path.clone(),
            detail: "a top-level JSON file must be a FHIR resource".into(),
        })?;
        artefacts.push((
            Artefact {
                path: path.clone(),
                resource_type,
                id: text(&value, "id"),
                url: text(&value, "url"),
                version: text(&value, "version"),
            },
            value,
        ));
    }
    Ok(artefacts)
}

/// The manifest Library's `depends-on` canonicals.
fn manifest(value: &Value) -> Result<Vec<Canonical>, Error> {
    if value.get("resourceType").and_then(Value::as_str) != Some("Library") {
        return Err(Error::Json {
            path: MANIFEST.into(),
            detail: "the manifest must be a Library".into(),
        });
    }
    Ok(value
        .get("relatedArtifact")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|related| related.get("type").and_then(Value::as_str) == Some("depends-on"))
        .filter_map(|related| related.get("resource").and_then(Value::as_str))
        .map(Canonical::parse)
        .collect())
}

/// Reads `files` into what the publication rules check.
pub(crate) fn read(files: &Files) -> Result<PackageContents, Error> {
    let package_json = files.get(PACKAGE_JSON).ok_or_else(|| Error::Json {
        path: PACKAGE_JSON.into(),
        detail: "missing".into(),
    })?;
    let package: PackageJson = serde_json::from_slice(package_json).map_err(|e| Error::Json {
        path: PACKAGE_JSON.into(),
        detail: e.to_string(),
    })?;
    let artefacts = artefacts(files)?;
    let manifest = artefacts
        .iter()
        .find(|(artefact, _)| artefact.path == MANIFEST)
        .map(|(_, value)| manifest(value))
        .transpose()?;
    Ok(PackageContents {
        package,
        manifest,
        artefacts: artefacts
            .into_iter()
            .map(|(artefact, _)| artefact)
            .collect(),
        paths: files.keys().cloned().collect(),
    })
}

/// The FHIR NPM index (version 2) of `files`' artefacts, in path order.
pub(crate) fn index(files: &Files) -> Result<Vec<u8>, Error> {
    let listed: Vec<Value> = artefacts(files)?
        .into_iter()
        .map(|(artefact, _)| {
            let mut entry = json!({
                "filename": artefact.path,
                "resourceType": artefact.resource_type,
            });
            for (key, value) in [
                ("id", artefact.id),
                ("url", artefact.url),
                ("version", artefact.version),
            ] {
                if let Some(value) = value {
                    entry[key] = Value::String(value);
                }
            }
            entry
        })
        .collect();
    serde_json::to_vec_pretty(&json!({ "index-version": 2, "files": listed })).map_err(|e| {
        Error::Json {
            path: INDEX.into(),
            detail: e.to_string(),
        }
    })
}
