//! The semver diff check (metadata.md § Versioning): validation compares a package version with
//! the latest one published under its name, and refuses a version that understates its changes.
//!
//! Every artefact carries its own semver, so the package's bump follows from its artefacts':
//!
//! | Change | Bump at least |
//! |---|---|
//! | An artefact's own version, or a pinned dependency, takes a patch, minor or major bump | the same |
//! | An artefact or a dependency is added | minor |
//! | An artefact or a dependency is removed, or the package's kind changes | major |
//! | A `configuration/` file is added or changed | minor |
//! | A `configuration/` file is removed | major |
//! | A `tests/` file is added, changed or removed | patch |
//!
//! Each artefact's own bump is checked too, against the [`ContentChange`]s found in its content
//! (`health-packages`' `bump` module reads the JSON): a removed item is major, a new optional
//! one minor, a display text a patch.
//!
//! Also refused, whatever the bump:
//! - a version no newer than the latest published one;
//! - an artefact whose `url|version` is unchanged but whose content changed, since a published
//!   `url|version` never changes;
//! - an artefact whose version went down, or is not an exact semver version.
//!
//! **Major version zero** (semver § 4) promises nothing, so between two `0.y.z` versions a minor
//! bump is enough for a major change. **Pre-releases** promise nothing either: between versions
//! that differ only in their pre-release, such as `2.0.0-rc.1` and `2.0.0`, any change goes. The
//! core version already took the bump against the release before.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::package::{
    INDEX, MANIFEST, NotExact, PACKAGE_JSON, PackageContents, PackageKind, Version,
};

/// How far a version moved, least first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Bump {
    /// Not at all.
    None,
    /// The patch number.
    Patch,
    /// The minor number.
    Minor,
    /// The major number.
    Major,
}

impl fmt::Display for Bump {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::None => "none",
            Self::Patch => "patch",
            Self::Minor => "minor",
            Self::Major => "major",
        })
    }
}

impl Bump {
    /// The bump from `from` to `to`, by their core numbers.
    pub fn between(from: &Version, to: &Version) -> Self {
        if to.major != from.major {
            Self::Major
        } else if to.minor != from.minor {
            Self::Minor
        } else if to.patch != from.patch {
            Self::Patch
        } else {
            Self::None
        }
    }
}

/// A canonical artefact in a release: its own version and its content's fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinned {
    /// Its `version`, as written.
    pub version: String,
    /// Its content's fingerprint.
    pub fingerprint: String,
}

/// How something changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Nature {
    /// It is new.
    Added,
    /// It is gone.
    Removed,
    /// It means something else.
    Changed,
    /// It allows less.
    Tightened,
    /// It allows more.
    Relaxed,
    /// Its wording changed.
    Reworded,
}

/// One change in an artefact, with the bump it needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentChange {
    /// Where, e.g. `item[vitals/bp].type` or `element[Patient.name].min`.
    pub at: String,
    /// How it changed.
    pub nature: Nature,
    /// The bump it needs.
    pub bump: Bump,
}

/// What the check compares: one package version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The package version.
    pub version: Version,
    /// Its kind.
    pub kind: PackageKind,
    /// Its pinned dependencies, by name.
    pub dependencies: BTreeMap<String, String>,
    /// Its canonical artefacts, by canonical URL. The manifest Library is not among them: it
    /// follows the package's own version.
    pub artefacts: BTreeMap<String, Pinned>,
    /// Every other file's fingerprint, by path, without `package.json`, `.index.json` or the
    /// manifest, which are generated.
    pub files: BTreeMap<String, String>,
}

impl Release {
    /// The release of `contents`, given each file's fingerprint by path. A file missing from
    /// `fingerprints` gets an empty fingerprint.
    pub fn of(
        contents: &PackageContents,
        fingerprints: &BTreeMap<String, String>,
    ) -> Result<Self, NotExact> {
        let fingerprint = |path: &str| fingerprints.get(path).cloned().unwrap_or_default();
        let mut artefacts = BTreeMap::new();
        let mut canonical_paths = Vec::new();
        for artefact in &contents.artefacts {
            if artefact.path == MANIFEST {
                continue;
            }
            if let Some(url) = &artefact.url {
                canonical_paths.push(artefact.path.as_str());
                artefacts.insert(
                    url.clone(),
                    Pinned {
                        version: artefact.version.clone().unwrap_or_default(),
                        fingerprint: fingerprint(&artefact.path),
                    },
                );
            }
        }
        let files = contents
            .paths
            .iter()
            .filter(|path| {
                ![PACKAGE_JSON, INDEX, MANIFEST].contains(&path.as_str())
                    && !canonical_paths.contains(&path.as_str())
            })
            .map(|path| (path.clone(), fingerprint(path)))
            .collect();
        Ok(Self {
            version: contents.package.version.parse()?,
            kind: contents.package.kind,
            dependencies: contents.package.dependencies.clone(),
            artefacts,
            files,
        })
    }
}

/// One difference between two releases, with the bump it needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "change")]
pub enum Change {
    /// The package's kind changed.
    Kind {
        /// The kind before.
        from: PackageKind,
        /// The kind after.
        to: PackageKind,
    },
    /// An artefact was added.
    ArtefactAdded {
        /// Its canonical URL.
        url: String,
    },
    /// An artefact was removed.
    ArtefactRemoved {
        /// Its canonical URL.
        url: String,
    },
    /// An artefact took a new version.
    ArtefactVersion {
        /// Its canonical URL.
        url: String,
        /// The version before.
        from: String,
        /// The version after.
        to: String,
    },
    /// A dependency was added.
    DependencyAdded {
        /// Its name.
        name: String,
    },
    /// A dependency was removed.
    DependencyRemoved {
        /// Its name.
        name: String,
    },
    /// A dependency was pinned to another version.
    DependencyVersion {
        /// Its name.
        name: String,
        /// The version before.
        from: String,
        /// The version after.
        to: String,
    },
    /// A file was added.
    FileAdded {
        /// Its path.
        path: String,
    },
    /// A file was removed.
    FileRemoved {
        /// Its path.
        path: String,
    },
    /// A file changed.
    FileChanged {
        /// Its path.
        path: String,
    },
}

/// Why a version may not follow the latest published one.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BumpProblem {
    /// Not newer than the latest published version.
    #[error("{next} is not newer than the published {latest}")]
    NotNewer {
        /// The latest published version.
        latest: Version,
        /// The version checked.
        next: Version,
    },
    /// The version understates the changes.
    #[error("{next} is a {actual} bump from {latest}, but its changes need a {required} bump")]
    Understated {
        /// The latest published version.
        latest: Version,
        /// The version checked.
        next: Version,
        /// The bump taken.
        actual: Bump,
        /// The bump the changes need.
        required: Bump,
    },
    /// An artefact's own version understates the changes in its content.
    #[error(
        "{url} went from {from} to {to}, a {actual} bump, but its changes need a {required} bump"
    )]
    ArtefactUnderstated {
        /// Its canonical URL.
        url: String,
        /// The version before.
        from: String,
        /// The version after.
        to: String,
        /// The bump taken.
        actual: Bump,
        /// The bump its changes need.
        required: Bump,
        /// Every change found, with the bump each needs.
        changes: Vec<ContentChange>,
    },
    /// A published `url|version` with other content.
    #[error("{0} changed without a new version")]
    Rewritten(String),
    /// An artefact or dependency version that went down.
    #[error("{name} went down from {from} to {to}")]
    Downgraded {
        /// The canonical URL or dependency name.
        name: String,
        /// The version before.
        from: String,
        /// The version after.
        to: String,
    },
    /// An artefact or dependency version that is not exact semver.
    #[error("{name}: {problem}")]
    Version {
        /// The canonical URL or dependency name.
        name: String,
        /// What is wrong.
        problem: NotExact,
    },
}

/// What the check found: every change with the bump it needs, and the bump taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    /// Each change and the bump it needs.
    pub changes: Vec<(Change, Bump)>,
    /// The bump the changes need together.
    pub required: Bump,
    /// The bump the version takes.
    pub actual: Bump,
}

fn versioned(name: &str, from: &str, to: &str, problems: &mut Vec<BumpProblem>) -> Option<Bump> {
    let parse = |text: &str| -> Result<Version, BumpProblem> {
        text.parse().map_err(|problem| BumpProblem::Version {
            name: name.to_owned(),
            problem,
        })
    };
    match (parse(from), parse(to)) {
        (Ok(from_version), Ok(to_version)) => {
            if to_version < from_version {
                problems.push(BumpProblem::Downgraded {
                    name: name.to_owned(),
                    from: from.to_owned(),
                    to: to.to_owned(),
                });
                None
            } else {
                Some(Bump::between(&from_version, &to_version))
            }
        }
        (Err(problem), _) | (_, Err(problem)) => {
            problems.push(problem);
            None
        }
    }
}

fn file_bump(path: &str, removed: bool) -> Bump {
    if path.starts_with("tests/") {
        Bump::Patch
    } else if removed {
        Bump::Major
    } else {
        Bump::Minor
    }
}

/// What `required` asks of a step from `from` to `to`: under major version zero, a minor bump
/// covers a major change.
fn needed(from: &Version, to: &Version, required: Bump) -> Bump {
    if required == Bump::Major && from.major == 0 && to.major == 0 {
        Bump::Minor
    } else {
        required
    }
}

/// The bump `required` would need, if going from `from` to `to` understates it. Major version
/// zero takes a minor bump for a major change, and a pre-release-only step takes anything.
fn understated(from: &Version, to: &Version, required: Bump) -> Option<Bump> {
    let required = needed(from, to, required);
    let actual = Bump::between(from, to);
    (actual != Bump::None && actual < required).then_some(required)
}

/// Checks an artefact's own new version against what changed in its content.
fn own_bump(
    url: &str,
    before: &Pinned,
    after: &Pinned,
    changes: &[ContentChange],
    problems: &mut Vec<BumpProblem>,
) {
    let (Ok(from), Ok(to)) = (
        before.version.parse::<Version>(),
        after.version.parse::<Version>(),
    ) else {
        return;
    };
    let required = changes
        .iter()
        .map(|change| change.bump)
        .max()
        .unwrap_or(Bump::None);
    if let Some(required) = understated(&from, &to, required) {
        problems.push(BumpProblem::ArtefactUnderstated {
            url: url.to_owned(),
            from: before.version.clone(),
            to: after.version.clone(),
            actual: Bump::between(&from, &to),
            required,
            changes: changes.to_vec(),
        });
    }
}

/// Compares `next` with `latest`, the latest version published under the same name. `content`
/// holds what changed in each artefact's content, by canonical URL, for the artefacts that took a
/// new version.
pub fn check(
    latest: &Release,
    next: &Release,
    content: &BTreeMap<String, Vec<ContentChange>>,
) -> Result<Diff, Vec<BumpProblem>> {
    let mut problems = Vec::new();
    let mut changes = Vec::new();

    if next.version <= latest.version {
        problems.push(BumpProblem::NotNewer {
            latest: latest.version.clone(),
            next: next.version.clone(),
        });
    }
    if next.kind != latest.kind {
        let change = Change::Kind {
            from: latest.kind,
            to: next.kind,
        };
        changes.push((change, Bump::Major));
    }

    for (url, before) in &latest.artefacts {
        match next.artefacts.get(url) {
            None => changes.push((Change::ArtefactRemoved { url: url.clone() }, Bump::Major)),
            Some(after) if after.version == before.version => {
                if after.fingerprint != before.fingerprint {
                    problems.push(BumpProblem::Rewritten(format!("{url}|{}", after.version)));
                }
            }
            Some(after) => {
                if let Some(bump) = versioned(url, &before.version, &after.version, &mut problems) {
                    if let Some(found) = content.get(url) {
                        own_bump(url, before, after, found, &mut problems);
                    }
                    let change = Change::ArtefactVersion {
                        url: url.clone(),
                        from: before.version.clone(),
                        to: after.version.clone(),
                    };
                    changes.push((change, bump));
                }
            }
        }
    }
    for (url, after) in &next.artefacts {
        if !latest.artefacts.contains_key(url) {
            if let Err(problem) = after.version.parse::<Version>() {
                problems.push(BumpProblem::Version {
                    name: url.clone(),
                    problem,
                });
            }
            changes.push((Change::ArtefactAdded { url: url.clone() }, Bump::Minor));
        }
    }

    for (name, before) in &latest.dependencies {
        match next.dependencies.get(name) {
            None => changes.push((
                Change::DependencyRemoved { name: name.clone() },
                Bump::Major,
            )),
            Some(after) if after == before => {}
            Some(after) => {
                if let Some(bump) = versioned(name, before, after, &mut problems) {
                    let change = Change::DependencyVersion {
                        name: name.clone(),
                        from: before.clone(),
                        to: after.clone(),
                    };
                    changes.push((change, bump));
                }
            }
        }
    }
    for name in next.dependencies.keys() {
        if !latest.dependencies.contains_key(name) {
            changes.push((Change::DependencyAdded { name: name.clone() }, Bump::Minor));
        }
    }

    for (path, before) in &latest.files {
        match next.files.get(path) {
            None => changes.push((
                Change::FileRemoved { path: path.clone() },
                file_bump(path, true),
            )),
            Some(after) if after == before => {}
            Some(_) => changes.push((
                Change::FileChanged { path: path.clone() },
                file_bump(path, false),
            )),
        }
    }
    for path in next.files.keys() {
        if !latest.files.contains_key(path) {
            changes.push((
                Change::FileAdded { path: path.clone() },
                file_bump(path, false),
            ));
        }
    }

    let required = needed(
        &latest.version,
        &next.version,
        changes
            .iter()
            .map(|(_, bump)| *bump)
            .max()
            .unwrap_or(Bump::None),
    );
    let actual = Bump::between(&latest.version, &next.version);
    if let Some(required) = understated(&latest.version, &next.version, required) {
        problems.push(BumpProblem::Understated {
            latest: latest.version.clone(),
            next: next.version.clone(),
            actual,
            required,
        });
    }

    if problems.is_empty() {
        Ok(Diff {
            changes,
            required,
            actual,
        })
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests;
