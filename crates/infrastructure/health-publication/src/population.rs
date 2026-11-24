//! Population packages at validation (ADR 0071; analytics.md § Age Band Sets and Population
//! Estimates; #553): a year's Population Estimates are configuration in a Population package,
//! `configuration/population.json`, written to the `population/1` schema. The schema checks
//! their shape when the package is checked; this checks the rest
//! ([`EstimatesDefinition::problems`]): the package is named for its year
//! (`ug.population.2026`) and holds its estimates at [`ESTIMATES_FILE`], each derived type names
//! a declared type and a percentage, types do not derive from each other in a circle, and each
//! figure is of a given type, once per place.
//!
//! Place codes are checked against the facility registry when the HMIS lead imports the CSV at
//! central, before the figures are written to the draft.

use health_analytics::{ESTIMATES_FILE, EstimatesDefinition};
use health_configuration::package::PackageKind;
use health_packages::{Files, POPULATION, Packed};
use serde_json::Value;

/// The year's estimates in a package's `files`, each with its file, as written or why they
/// cannot be read.
pub fn estimates_in(files: &Files) -> Vec<(String, Result<EstimatesDefinition, String>)> {
    files
        .iter()
        .filter(|(path, _)| path.starts_with("configuration/") && path.ends_with(".json"))
        .filter_map(|(path, bytes)| {
            let value = serde_json::from_slice::<Value>(bytes).ok()?;
            (value["$schema"] == POPULATION).then(|| {
                let read = serde_json::from_value::<EstimatesDefinition>(value)
                    .map_err(|e| format!("are not Population Estimates: {e}"));
                (path.clone(), read)
            })
        })
        .collect()
}

/// Every problem with the package's Population Estimates, each prefixed with its file.
pub(crate) fn problems(packed: &Packed) -> Vec<String> {
    let mut problems = Vec::new();
    let package = &packed.contents.package;
    let found = estimates_in(&packed.files);
    if package.kind == PackageKind::Population && found.is_empty() {
        problems.push(format!(
            "a Population package holds its estimates at {ESTIMATES_FILE}"
        ));
    }
    for (path, read) in found {
        if package.kind != PackageKind::Population {
            problems.push(format!(
                "{path}: Population Estimates belong in a Population package, not a {} package",
                serde_json::to_value(package.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default()
            ));
        }
        if path != ESTIMATES_FILE {
            problems.push(format!(
                "{path}: a Population package holds its estimates at {ESTIMATES_FILE}"
            ));
        }
        let estimates = match read {
            Ok(estimates) => estimates,
            Err(why) => {
                problems.push(format!("{path}: {why}"));
                continue;
            }
        };
        let year = estimates.year.to_string();
        if package.kind == PackageKind::Population
            && package.name.rsplit('.').next() != Some(year.as_str())
        {
            problems.push(format!(
                "{path}: the estimates are for {year}, so the package's name ends in .{year} (one Population package per year), not {}",
                package.name
            ));
        }
        problems.extend(
            estimates
                .problems()
                .into_iter()
                .map(|problem| format!("{path}: {problem}")),
        );
    }
    problems
}
