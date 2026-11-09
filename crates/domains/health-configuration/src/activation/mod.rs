//! Scoped activation on nodes (configuration.md § Distribution and activation on nodes,
//! § Rollback; ADRs 0024, 0025).
//!
//! Publication makes a package version available; an [`Activation`] makes it the one a scope
//! uses from an effective time. A node's active set ([`active_set`]) is the merge of the
//! activations along its [`Chain`]: national, then its district, then its facility, plus the
//! facility's program activations. For each package the most specific scope wins, and within a
//! scope the latest issued activation already in effect. Rollback is a new activation of an
//! earlier version, so nothing is mutated or deleted.
//!
//! Before a node switches, [`check`] holds the set to what it needs: every package staged, each
//! dependency active at exactly the version it was published against (ADR 0024 rejects floating
//! ranges at activation), and the running release new enough and able to read the package's
//! format. [`switch`] moves to the new set whole or not at all: a failed activation keeps the
//! previous set and reports why.
//!
//! [`retained`] lists every version ever activated in the node's scope, which it keeps as long as
//! data refers to it (metadata.md); records keep their pinned versions after a rollback.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use health_time::Timestamp;
use serde::{Deserialize, Serialize};

use crate::package::{PackageJson, Version};

#[cfg(test)]
mod tests;

/// Where an activation applies.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "level", rename_all = "kebab-case")]
pub enum Scope {
    /// Every node.
    National,
    /// The nodes of one district.
    District {
        /// The district's code.
        district: String,
    },
    /// One facility's nodes.
    Facility {
        /// The facility's code.
        facility: String,
    },
    /// One program at one facility.
    Program {
        /// The facility's code.
        facility: String,
        /// The program's code.
        program: String,
    },
}

impl Scope {
    /// How specific it is: a more specific scope's activation wins.
    fn rank(&self) -> u8 {
        match self {
            Self::National => 0,
            Self::District { .. } => 1,
            Self::Facility { .. } => 2,
            Self::Program { .. } => 3,
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::National => f.write_str("national"),
            Self::District { district } => write!(f, "district {district}"),
            Self::Facility { facility } => write!(f, "facility {facility}"),
            Self::Program { facility, program } => {
                write!(f, "program {program} at facility {facility}")
            }
        }
    }
}

/// An activation record: package `name@version` is active in `scope` from `effective_at`. It
/// travels to nodes in the metadata lane and is never changed; a rollback is another one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activation {
    /// Its id.
    pub id: String,
    /// The package's name.
    pub package: String,
    /// The version activated.
    pub version: Version,
    /// Where.
    pub scope: Scope,
    /// From when.
    pub effective_at: Timestamp,
    /// When it was issued: the later of two in one scope wins.
    pub issued_at: Timestamp,
    /// Who issued it.
    pub by: String,
    /// Why, e.g. the reason for a rollback.
    #[serde(default)]
    pub reason: Option<String>,
}

/// An activation as it travels to nodes in the metadata lane: the record, with the published
/// tarball's sha-256 (its blob) and detached signature, so a node can fetch the package and
/// verify it before staging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationRecord {
    /// The activation.
    pub activation: Activation,
    /// The tarball's sha-256, hex.
    pub digest: String,
    /// Its detached signature (configuration.md § Signing).
    pub signature: String,
}

/// A node's place in the scope chain.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chain {
    /// Its district, if any.
    pub district: Option<String>,
    /// Its facility, if any (central has none).
    pub facility: Option<String>,
    /// The programs its facility runs.
    pub programs: BTreeSet<String>,
}

impl Chain {
    /// Whether an activation in `scope` applies to this node.
    pub fn covers(&self, scope: &Scope) -> bool {
        match scope {
            Scope::National => true,
            Scope::District { district } => self.district.as_ref() == Some(district),
            Scope::Facility { facility } => self.facility.as_ref() == Some(facility),
            Scope::Program { facility, program } => {
                self.facility.as_ref() == Some(facility) && self.programs.contains(program)
            }
        }
    }
}

/// A node's active set: the version of each package it uses.
pub type ActiveSet = BTreeMap<String, Version>;

/// The active set the activations give a node on `chain` at `now`: for each package, the most
/// specific scope's latest issued activation that is in effect.
pub fn active_set(activations: &[Activation], chain: &Chain, now: Timestamp) -> ActiveSet {
    let mut winners: BTreeMap<&str, &Activation> = BTreeMap::new();
    for activation in activations
        .iter()
        .filter(|a| a.effective_at <= now && chain.covers(&a.scope))
    {
        let key = |a: &Activation| (a.scope.rank(), a.issued_at, a.id.clone());
        let entry = winners
            .entry(activation.package.as_str())
            .or_insert(activation);
        if key(activation) > key(entry) {
            *entry = activation;
        }
    }
    winners
        .into_iter()
        .map(|(name, a)| (name.to_owned(), a.version.clone()))
        .collect()
}

/// The software a node runs: its release and the package format versions it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The running software's version.
    pub software: Version,
    /// The package format versions it reads.
    pub formats: BTreeSet<u32>,
}

/// Why a set cannot be activated.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Problem {
    /// The package is not staged on the node: its blob has not arrived, or it failed
    /// verification.
    #[error("{package}@{version} is not staged here")]
    NotStaged {
        /// The package.
        package: String,
        /// The version.
        version: Version,
    },
    /// A dependency is not in the set.
    #[error("{package} needs {dependency}@{wanted}, which is not active")]
    MissingDependency {
        /// The package.
        package: String,
        /// What it depends on.
        dependency: String,
        /// The version it was published against.
        wanted: String,
    },
    /// A dependency is active at another version.
    #[error("{package} needs {dependency}@{wanted}, but {active} would be active")]
    WrongDependency {
        /// The package.
        package: String,
        /// What it depends on.
        dependency: String,
        /// The version it was published against.
        wanted: String,
        /// The version that would be active.
        active: Version,
    },
    /// The running software is older than the package needs.
    #[error("{package} needs software {needs} or later; this node runs {running}")]
    SoftwareTooOld {
        /// The package.
        package: String,
        /// Its `minSoftware`.
        needs: String,
        /// The running release.
        running: Version,
    },
    /// The running software cannot read the package's format.
    #[error("{package} is in package format {format}, which this release does not read")]
    Format {
        /// The package.
        package: String,
        /// Its `formatVersion`.
        format: u32,
    },
}

/// What stops `set` being activated on a node running `release`, given the packages `staged`
/// there (verified, with their `package.json`). Empty when it may be activated.
pub fn check(set: &ActiveSet, staged: &[PackageJson], release: &Release) -> Vec<Problem> {
    let mut problems = Vec::new();
    for (name, version) in set {
        let Some(package) = staged.iter().find(|p| {
            &p.name == name && p.version.parse::<Version>().ok().as_ref() == Some(version)
        }) else {
            problems.push(Problem::NotStaged {
                package: name.clone(),
                version: version.clone(),
            });
            continue;
        };
        for (dependency, wanted) in &package.dependencies {
            match set.get(dependency) {
                None => problems.push(Problem::MissingDependency {
                    package: name.clone(),
                    dependency: dependency.clone(),
                    wanted: wanted.clone(),
                }),
                Some(active) if wanted.parse::<Version>().ok().as_ref() != Some(active) => {
                    problems.push(Problem::WrongDependency {
                        package: name.clone(),
                        dependency: dependency.clone(),
                        wanted: wanted.clone(),
                        active: active.clone(),
                    });
                }
                Some(_) => {}
            }
        }
        let needs = &package.platform.min_software;
        if needs
            .parse::<Version>()
            .map_or(true, |needs| needs > release.software)
        {
            problems.push(Problem::SoftwareTooOld {
                package: name.clone(),
                needs: needs.clone(),
                running: release.software.clone(),
            });
        }
        if !release.formats.contains(&package.platform.format_version) {
            problems.push(Problem::Format {
                package: name.clone(),
                format: package.platform.format_version,
            });
        }
    }
    problems
}

/// One package's change in a switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The package.
    pub package: String,
    /// The version it had, if it was active.
    pub from: Option<Version>,
    /// The version it has, if it stays active.
    pub to: Option<Version>,
}

/// What a switch did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Switch {
    /// The activations give the set already active.
    Unchanged,
    /// The node moved to `active`.
    Activated {
        /// The new active set.
        active: ActiveSet,
        /// What changed, by package; the caller reindexes when a changed package carries
        /// SearchParameters.
        changes: Vec<Change>,
    },
    /// The new set cannot be activated: the previous set stays, and this is reported.
    Failed {
        /// The set that was wanted.
        wanted: ActiveSet,
        /// Why not.
        problems: Vec<Problem>,
    },
}

/// The changes from `previous` to `next`, by package.
pub fn changes(previous: &ActiveSet, next: &ActiveSet) -> Vec<Change> {
    let names: BTreeSet<&String> = previous.keys().chain(next.keys()).collect();
    names
        .into_iter()
        .filter_map(|name| {
            let (from, to) = (previous.get(name), next.get(name));
            (from != to).then(|| Change {
                package: name.clone(),
                from: from.cloned(),
                to: to.cloned(),
            })
        })
        .collect()
}

/// Moves a node from `current` to the set its activations give at `now`, whole or not at all.
pub fn switch(
    current: &ActiveSet,
    activations: &[Activation],
    chain: &Chain,
    now: Timestamp,
    staged: &[PackageJson],
    release: &Release,
) -> Switch {
    let wanted = active_set(activations, chain, now);
    if &wanted == current {
        return Switch::Unchanged;
    }
    let problems = check(&wanted, staged, release);
    if !problems.is_empty() {
        return Switch::Failed { wanted, problems };
    }
    Switch::Activated {
        changes: changes(current, &wanted),
        active: wanted,
    }
}

/// Every package version ever activated in the node's scope, effective or not: the node keeps
/// each while data refers to it.
pub fn retained(activations: &[Activation], chain: &Chain) -> BTreeSet<(String, Version)> {
    activations
        .iter()
        .filter(|a| chain.covers(&a.scope))
        .map(|a| (a.package.clone(), a.version.clone()))
        .collect()
}
