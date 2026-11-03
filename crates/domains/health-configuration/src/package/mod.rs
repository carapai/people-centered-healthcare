//! A package of Published Definitions (ADR 0024; configuration.md § Package format): a FHIR NPM
//! package for R5 with a CRMI-style manifest, its `configuration/` and its `tests/`.
//!
//! ```text
//! package/
//! ├── package.json            name, version, fhirVersions ["5.0.0"], dependencies, kind, platform
//! ├── Library-manifest.json   every canonical depended on, as url|version
//! ├── *.json                  FHIR artefacts
//! ├── configuration/          non-FHIR configuration
//! ├── tests/                  fixtures
//! └── .index.json
//! ```
//!
//! [`PackageContents::problems_for_publication`] is the publication gate's package check: a
//! package with any problem is not published. Authors write dependency ranges; publication pins
//! them, so a range here is refused.

mod version;

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

pub use version::{NotExact, Version};

/// The FHIR version every package is for.
pub const FHIR_VERSION: &str = "5.0.0";

/// Where a package's files sit inside its archive.
pub const ROOT: &str = "package/";

/// The package manifest's path, under [`ROOT`].
pub const PACKAGE_JSON: &str = "package.json";

/// The CRMI-style manifest Library's path, under [`ROOT`].
pub const MANIFEST: &str = "Library-manifest.json";

/// The FHIR NPM index's path, under [`ROOT`].
pub const INDEX: &str = ".index.json";

/// The five kinds of package (configuration.md), each with its own approvers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageKind {
    /// Profiles, base terminology, SearchParameters.
    NationalCore,
    /// Large code systems and expansions.
    Terminology,
    /// Forms, mappings, workflow and program definitions.
    Program,
    /// Roles and permissions, deployment settings.
    CountryConfiguration,
    /// District or facility configuration at declared override points.
    Override,
}

/// What software a package needs (configuration.md § Distribution and activation on nodes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Platform {
    /// The oldest software release that may activate it.
    pub min_software: String,
    /// The package format version it is written in.
    pub format_version: u32,
}

/// `package/package.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageJson {
    /// The package name, e.g. `ug.moh.anc`.
    pub name: String,
    /// Its version.
    pub version: String,
    /// The FHIR versions it is for: [`FHIR_VERSION`] only.
    pub fhir_versions: Vec<String>,
    /// The packages it depends on, by name: exact versions once published.
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    /// Which kind of package.
    pub kind: PackageKind,
    /// What software it needs.
    pub platform: Platform,
}

/// A canonical reference, `url|version`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Canonical {
    /// The canonical URL.
    pub url: String,
    /// The version after `|`, if any.
    pub version: Option<String>,
}

impl Canonical {
    /// Reads `url|version`, or a bare `url`.
    pub fn parse(text: &str) -> Self {
        match text.split_once('|') {
            Some((url, version)) => Self {
                url: url.to_owned(),
                version: Some(version.to_owned()).filter(|v| !v.is_empty()),
            },
            None => Self {
                url: text.to_owned(),
                version: None,
            },
        }
    }
}

impl fmt::Display for Canonical {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.version {
            Some(version) => write!(f, "{}|{version}", self.url),
            None => f.write_str(&self.url),
        }
    }
}

/// A FHIR artefact in the package: one of its top-level `*.json` files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artefact {
    /// Its path under [`ROOT`].
    pub path: String,
    /// Its `resourceType`.
    pub resource_type: String,
    /// Its `id`, if any.
    pub id: Option<String>,
    /// Its canonical `url`, if it is a canonical resource.
    pub url: Option<String>,
    /// Its `version`.
    pub version: Option<String>,
}

/// A package's contents as read from its archive: what the rules check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageContents {
    /// `package.json`.
    pub package: PackageJson,
    /// The manifest's `depends-on` canonicals, if the package has a manifest.
    pub manifest: Option<Vec<Canonical>>,
    /// The FHIR artefacts, the manifest Library among them.
    pub artefacts: Vec<Artefact>,
    /// Every file's path under [`ROOT`].
    pub paths: Vec<String>,
}

/// Why a package may not be published.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Problem {
    /// Not a valid package name: lower-case letters, digits, `.`, `-` and `_`.
    #[error("{0:?} is not a package name")]
    Name(String),
    /// The package's own version is not exact.
    #[error("the package version: {0}")]
    Version(NotExact),
    /// Not for FHIR R5 alone.
    #[error("fhirVersions must be [\"{FHIR_VERSION}\"], not {0:?}")]
    FhirVersions(Vec<String>),
    /// `platform.minSoftware` is not an exact version.
    #[error("platform.minSoftware: {0}")]
    MinSoftware(NotExact),
    /// A dependency left as a range, or not a package name.
    #[error("dependency {name} is not pinned to an exact version: {range:?}")]
    Unpinned {
        /// The dependency.
        name: String,
        /// What was written.
        range: String,
    },
    /// A package depending on itself.
    #[error("the package depends on itself")]
    SelfDependency,
    /// No manifest Library.
    #[error("there is no {MANIFEST}")]
    NoManifest,
    /// A canonical in the manifest without a version.
    #[error("the manifest does not pin {0}")]
    UnpinnedCanonical(String),
    /// A canonical artefact in the package without a version.
    #[error("{0} has a url but no version")]
    UnversionedArtefact(String),
    /// Two artefacts with one `url|version`.
    #[error("{0} is in the package twice")]
    Duplicate(String),
    /// A file outside the package layout.
    #[error("{0} is outside the package layout")]
    Misplaced(String),
}

/// Lower-case letters, digits, `.`, `-` and `_`, starting with a letter or digit.
pub fn valid_name(name: &str) -> bool {
    name.len() <= 214
        && name
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

/// Whether `path`, under [`ROOT`], is somewhere the package layout puts files.
pub fn in_layout(path: &str) -> bool {
    let top_level_json = !path.contains('/') && path.ends_with(".json");
    let folder = |prefix: &str| {
        path.strip_prefix(prefix)
            .is_some_and(|rest| !rest.is_empty() && !rest.ends_with('/'))
    };
    top_level_json || folder("configuration/") || folder("tests/")
}

impl PackageContents {
    /// `name@version`.
    pub fn id(&self) -> String {
        format!("{}@{}", self.package.name, self.package.version)
    }

    /// Every reason the package may not be published, in a stable order; none if it may.
    pub fn problems_for_publication(&self) -> Vec<Problem> {
        let mut problems = Vec::new();
        let package = &self.package;
        if !valid_name(&package.name) {
            problems.push(Problem::Name(package.name.clone()));
        }
        if let Err(not_exact) = package.version.parse::<Version>() {
            problems.push(Problem::Version(not_exact));
        }
        if package.fhir_versions != [FHIR_VERSION] {
            problems.push(Problem::FhirVersions(package.fhir_versions.clone()));
        }
        if let Err(not_exact) = package.platform.min_software.parse::<Version>() {
            problems.push(Problem::MinSoftware(not_exact));
        }
        for (name, range) in &package.dependencies {
            if name == &package.name {
                problems.push(Problem::SelfDependency);
            } else if !valid_name(name) || range.parse::<Version>().is_err() {
                problems.push(Problem::Unpinned {
                    name: name.clone(),
                    range: range.clone(),
                });
            }
        }
        match &self.manifest {
            None => problems.push(Problem::NoManifest),
            Some(canonicals) => {
                for canonical in canonicals.iter().filter(|c| c.version.is_none()) {
                    problems.push(Problem::UnpinnedCanonical(canonical.url.clone()));
                }
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for artefact in &self.artefacts {
            let Some(url) = &artefact.url else { continue };
            match &artefact.version {
                None => problems.push(Problem::UnversionedArtefact(artefact.path.clone())),
                Some(version) => {
                    let canonical = format!("{url}|{version}");
                    if !seen.insert(canonical.clone()) {
                        problems.push(Problem::Duplicate(canonical));
                    }
                }
            }
        }
        for path in &self.paths {
            if !in_layout(path) {
                problems.push(Problem::Misplaced(path.clone()));
            }
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contents() -> PackageContents {
        PackageContents {
            package: PackageJson {
                name: "ug.moh.anc".into(),
                version: "1.4.0".into(),
                fhir_versions: vec![FHIR_VERSION.into()],
                dependencies: BTreeMap::from([("ug.moh.core".into(), "2.1.0".into())]),
                kind: PackageKind::Program,
                platform: Platform {
                    min_software: "0.3.0".into(),
                    format_version: 1,
                },
            },
            manifest: Some(vec![Canonical::parse(
                "http://health.go.ug/fhir/ValueSet/anc-visit|2.1.0",
            )]),
            artefacts: vec![Artefact {
                path: "Questionnaire-anc-visit.json".into(),
                resource_type: "Questionnaire".into(),
                id: Some("anc-visit".into()),
                url: Some("http://health.go.ug/fhir/Questionnaire/anc-visit".into()),
                version: Some("1.4.0".into()),
            }],
            paths: vec![
                PACKAGE_JSON.into(),
                MANIFEST.into(),
                INDEX.into(),
                "Questionnaire-anc-visit.json".into(),
                "configuration/layouts/anc-visit.json".into(),
                "tests/anc-visit/extraction.json".into(),
            ],
        }
    }

    #[test]
    fn a_pinned_r5_package_in_its_layout_may_be_published() {
        assert_eq!(contents().problems_for_publication(), []);
        assert_eq!(contents().id(), "ug.moh.anc@1.4.0");
    }

    #[test]
    fn ranges_are_refused_at_publication() {
        for range in ["^2.1.0", "~2.1.0", "2.x", "*", ">=2.0.0", "2.1"] {
            let mut package = contents();
            package
                .package
                .dependencies
                .insert("ug.moh.core".into(), range.into());
            assert_eq!(
                package.problems_for_publication(),
                [Problem::Unpinned {
                    name: "ug.moh.core".into(),
                    range: range.into()
                }],
                "{range}"
            );
        }
    }

    #[test]
    fn every_other_rule_is_reported() {
        let mut package = contents();
        package.package.name = "UG.MoH.ANC".into();
        package.package.version = "1.4".into();
        package.package.fhir_versions = vec!["4.0.1".into(), "5.0.0".into()];
        package.package.platform.min_software = "latest".into();
        package
            .package
            .dependencies
            .insert("UG.MoH.ANC".into(), "1.0.0".into());
        package.manifest = Some(vec![Canonical::parse(
            "http://health.go.ug/fhir/ValueSet/anc-visit",
        )]);
        let mut twice = package.artefacts[0].clone();
        twice.path = "Questionnaire-anc-visit-copy.json".into();
        package.artefacts.push(twice);
        package.artefacts.push(Artefact {
            path: "ValueSet-x.json".into(),
            resource_type: "ValueSet".into(),
            id: None,
            url: Some("http://health.go.ug/fhir/ValueSet/x".into()),
            version: None,
        });
        package.paths.extend([
            "scripts/install.sh".into(),
            "other/notes.txt".into(),
            "configuration/".into(),
        ]);
        let problems = package.problems_for_publication();
        assert_eq!(
            problems,
            [
                Problem::Name("UG.MoH.ANC".into()),
                Problem::Version(NotExact("1.4".into())),
                Problem::FhirVersions(vec!["4.0.1".into(), "5.0.0".into()]),
                Problem::MinSoftware(NotExact("latest".into())),
                Problem::SelfDependency,
                Problem::UnpinnedCanonical("http://health.go.ug/fhir/ValueSet/anc-visit".into()),
                Problem::Duplicate("http://health.go.ug/fhir/Questionnaire/anc-visit|1.4.0".into()),
                Problem::UnversionedArtefact("ValueSet-x.json".into()),
                Problem::Misplaced("scripts/install.sh".into()),
                Problem::Misplaced("other/notes.txt".into()),
                Problem::Misplaced("configuration/".into()),
            ]
        );
        package.manifest = None;
        assert!(
            package
                .problems_for_publication()
                .contains(&Problem::NoManifest)
        );
    }

    #[test]
    fn canonicals_read_and_print_url_and_version() {
        let pinned = Canonical::parse("http://x/ValueSet/a|1.0.0");
        assert_eq!(pinned.version.as_deref(), Some("1.0.0"));
        assert_eq!(pinned.to_string(), "http://x/ValueSet/a|1.0.0");
        assert_eq!(Canonical::parse("http://x/ValueSet/a|").version, None);
        assert_eq!(Canonical::parse("http://x/ValueSet/a").version, None);
    }

    #[test]
    fn package_json_reads_as_written() {
        let json = r#"{
            "name": "ug.moh.anc", "version": "1.4.0", "fhirVersions": ["5.0.0"],
            "dependencies": { "ug.moh.core": "2.1.0" },
            "kind": "program", "platform": { "minSoftware": "0.3.0", "formatVersion": 1 }
        }"#;
        let read: PackageJson = serde_json::from_str(json).expect("package.json");
        assert_eq!(read, contents().package);
        assert_eq!(
            serde_json::to_value(PackageKind::CountryConfiguration).ok(),
            Some(serde_json::json!("country-configuration"))
        );
    }
}
