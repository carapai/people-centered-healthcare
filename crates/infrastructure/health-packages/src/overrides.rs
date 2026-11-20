//! Override points on disk (ADR 0026, ADR 0067; `health_configuration::overrides`): a base
//! package's declarations read from its [`OVERRIDE_POINTS`] file, an override package's settings
//! from its [`OVERRIDES`] file, and the settings checked against a base's declarations.
//!
//! Both files are checked against the release's schemas ([`OVERRIDE_POINTS_SCHEMA`],
//! [`OVERRIDES_SCHEMA`]) like any configuration; the publication rules add what a schema cannot
//! say: a default the point does not allow, a point named for hiding, a point set twice.

use health_configuration::overrides::{
    self, OVERRIDE_POINTS, OVERRIDES, Point, Setting, SettingProblem,
};
use health_configuration::package::{PackageContents, PackageKind, Problem};
use serde::Deserialize;

use crate::Error;
use crate::archive::Files;

/// The `$schema` of a base package's override point declarations.
pub const OVERRIDE_POINTS_SCHEMA: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/schema/override-points/1";

/// The `$schema` of an override package's settings.
pub const OVERRIDES_SCHEMA: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/schema/overrides/1";

#[derive(Deserialize)]
struct Declarations {
    points: Vec<Point>,
}

#[derive(Deserialize)]
struct Settings {
    settings: Vec<Setting>,
}

fn read<T: for<'de> Deserialize<'de>>(files: &Files, path: &str) -> Result<Option<T>, Error> {
    let Some(bytes) = files.get(path) else {
        return Ok(None);
    };
    serde_json::from_slice(bytes)
        .map(Some)
        .map_err(|e| Error::Json {
            path: path.to_owned(),
            detail: e.to_string(),
        })
}

/// The override points the package in `files` declares; none if it has no [`OVERRIDE_POINTS`].
pub fn points(files: &Files) -> Result<Vec<Point>, Error> {
    Ok(read::<Declarations>(files, OVERRIDE_POINTS)?.map_or_else(Vec::new, |read| read.points))
}

/// The settings the override package in `files` makes; none if it has no [`OVERRIDES`].
pub fn settings(files: &Files) -> Result<Vec<Setting>, Error> {
    Ok(read::<Settings>(files, OVERRIDES)?.map_or_else(Vec::new, |read| read.settings))
}

/// The problems with the declarations or settings in `files`, by the package's kind. An
/// unreadable file is an error.
pub(crate) fn problems(contents: &PackageContents, files: &Files) -> Result<Vec<Problem>, Error> {
    if contents.package.kind == PackageKind::Override {
        return Ok(overrides::setting_problems(&settings(files)?)
            .into_iter()
            .map(Problem::Setting)
            .collect());
    }
    Ok(overrides::point_problems(&points(files)?)
        .into_iter()
        .map(Problem::Point)
        .collect())
}

/// The settings of the override package `override_files` for the base package `base` checked
/// against the points `base_files` declares: undeclared (or orphaned) keys and values the point
/// does not allow.
pub fn check(
    base: &str,
    base_files: &Files,
    override_files: &Files,
) -> Result<Vec<SettingProblem>, Error> {
    Ok(overrides::check(
        base,
        &points(base_files)?,
        &settings(override_files)?,
    ))
}

#[cfg(test)]
mod tests;
