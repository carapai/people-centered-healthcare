//! Enforces the crate-layer dependency rules in `docs/architecture/dependency-rules.md`.
//!
//! The checks read `cargo metadata --no-deps` for the dependency graph and each member's raw
//! `Cargo.toml` for the workspace-hygiene rules, so they run offline and without a lockfile.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use cargo_metadata::{DependencyKind, MetadataCommand, Package};

/// A crate layer, as declared in `[package.metadata.health] layer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Typed identifiers, errors and shared value types.
    Foundation,
    /// Pure FHIR, sync and persistence contracts, and per-context contract crates.
    Contracts,
    /// One crate per Module.
    Domain,
    /// One crate per Bounded Context.
    Application,
    /// Adapters that implement ports.
    Infrastructure,
    /// Composition roots.
    Runtime,
    /// Fixtures and fakes, used only as dev-dependencies.
    Testing,
    /// Workspace tooling binaries; nothing may depend on them.
    Tooling,
}

impl Layer {
    fn parse(tag: &str) -> Option<Self> {
        Some(match tag {
            "foundation" => Self::Foundation,
            "contracts" => Self::Contracts,
            "domain" => Self::Domain,
            "application" => Self::Application,
            "infrastructure" => Self::Infrastructure,
            "runtime" => Self::Runtime,
            "testing" => Self::Testing,
            "tooling" => Self::Tooling,
            _ => return None,
        })
    }

    /// Position in the stack; testing and tooling sit beside it.
    fn rank(self) -> Option<u8> {
        match self {
            Self::Foundation => Some(1),
            Self::Contracts => Some(2),
            Self::Domain => Some(3),
            Self::Application => Some(4),
            Self::Infrastructure => Some(5),
            Self::Runtime => Some(6),
            Self::Testing | Self::Tooling => None,
        }
    }
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tag = match self {
            Self::Foundation => "foundation",
            Self::Contracts => "contracts",
            Self::Domain => "domain",
            Self::Application => "application",
            Self::Infrastructure => "infrastructure",
            Self::Runtime => "runtime",
            Self::Testing => "testing",
            Self::Tooling => "tooling",
        };
        f.write_str(tag)
    }
}

/// The rule a [`Violation`] breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rule {
    /// The crate has no valid `[package.metadata.health] layer`.
    MissingLayerTag,
    /// A domain or application crate has no `context`.
    MissingContext,
    /// A dependency points to the same or a higher layer (rule 1).
    UpwardEdge,
    /// A domain crate depends on another domain crate (rule 2).
    DomainToDomain,
    /// An application crate depends on another application crate (rule 3).
    ApplicationToApplication,
    /// Something depends on a runtime crate other than a composition root in `apps/` (rule 4).
    RuntimeIsLeaf,
    /// Something depends on a tooling crate.
    ToolingIsLeaf,
    /// A testing crate is used as a normal or build dependency.
    TestingAsNormalDependency,
    /// A domain crate uses an external crate outside the allowlist (rule 5).
    DomainExternal,
    /// A domain or application crate declares `[features]` (rule 6).
    FeaturesInDomainOrApplication,
    /// A dependency does not use `{ workspace = true }` (rule 8).
    InlineVersion,
    /// The crate does not inherit `[workspace.lints]` (rule 8).
    MissingWorkspaceLints,
    /// The normal and build dependency graph has a cycle.
    Cycle,
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::MissingLayerTag => "missing-layer-tag",
            Self::MissingContext => "missing-context",
            Self::UpwardEdge => "upward-edge",
            Self::DomainToDomain => "domain-to-domain",
            Self::ApplicationToApplication => "application-to-application",
            Self::RuntimeIsLeaf => "runtime-is-leaf",
            Self::ToolingIsLeaf => "tooling-is-leaf",
            Self::TestingAsNormalDependency => "testing-as-normal-dependency",
            Self::DomainExternal => "domain-external",
            Self::FeaturesInDomainOrApplication => "features-in-domain-or-application",
            Self::InlineVersion => "inline-version",
            Self::MissingWorkspaceLints => "missing-workspace-lints",
            Self::Cycle => "cycle",
        };
        f.write_str(name)
    }
}

/// One broken rule, naming the crate and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Violation {
    /// The rule broken.
    pub rule: Rule,
    /// The crate at fault.
    pub krate: String,
    /// A human-readable explanation, naming the edge or item.
    pub detail: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.rule, self.krate, self.detail)
    }
}

/// The reviewed policy lists from `allowlist.toml`.
#[derive(Debug, Clone, Default)]
pub struct Allowlist {
    /// External crates a domain crate may use.
    pub domain_externals: BTreeSet<String>,
    /// Externals explicitly forbidden in domain crates (for clearer messages).
    pub domain_forbidden: BTreeSet<String>,
    /// Crates allowed not to inherit `[workspace.lints]`.
    pub lint_exempt: BTreeSet<String>,
}

impl Allowlist {
    /// Reads an allowlist file.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading allowlist {}", path.display()))?;
        let value: toml::Table =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        let list = |key: &str| -> Result<BTreeSet<String>> {
            match value.get(key) {
                None => Ok(BTreeSet::new()),
                Some(toml::Value::Array(items)) => items
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .map(str::to_owned)
                            .with_context(|| format!("`{key}` must list strings"))
                    })
                    .collect(),
                Some(_) => bail!("`{key}` must be an array"),
            }
        };
        Ok(Self {
            domain_externals: list("domain-externals")?,
            domain_forbidden: list("domain-forbidden")?,
            lint_exempt: list("lint-exempt")?,
        })
    }
}

struct Member {
    name: String,
    layer: Option<Layer>,
    context: Option<String>,
    dir: PathBuf,
    in_apps: bool,
}

/// Checks the workspace whose root manifest is `manifest_path` and returns every violation,
/// sorted by rule and crate.
pub fn check(manifest_path: &Path, allowlist: &Allowlist) -> Result<Vec<Violation>> {
    let metadata = MetadataCommand::new()
        .manifest_path(manifest_path)
        .no_deps()
        .exec()
        .with_context(|| format!("running cargo metadata on {}", manifest_path.display()))?;
    let root = metadata.workspace_root.as_std_path().to_path_buf();
    let packages: Vec<&Package> = metadata.workspace_packages();

    let members: BTreeMap<String, Member> = packages
        .iter()
        .map(|package| {
            let health = package.metadata.get("health");
            let tag = |key: &str| {
                health
                    .and_then(|h| h.get(key))
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            };
            let dir = package
                .manifest_path
                .parent()
                .map(|p| p.as_std_path().to_path_buf())
                .unwrap_or_default();
            let in_apps = dir
                .strip_prefix(&root)
                .is_ok_and(|rel| rel.starts_with("apps"));
            let member = Member {
                name: package.name.to_string(),
                layer: tag("layer").as_deref().and_then(Layer::parse),
                context: tag("context"),
                dir,
                in_apps,
            };
            (member.name.clone(), member)
        })
        .collect();

    let mut violations = Vec::new();
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for package in &packages {
        let me = &members[package.name.as_str()];
        let name = me.name.as_str();
        let mut flag = |rule, detail: String| {
            violations.push(Violation {
                rule,
                krate: name.to_owned(),
                detail,
            });
        };

        match me.layer {
            None => flag(
                Rule::MissingLayerTag,
                "add `[package.metadata.health] layer = \"…\"` (foundation, contracts, domain, \
                 application, infrastructure, runtime, testing or tooling)"
                    .to_owned(),
            ),
            Some(Layer::Domain | Layer::Application) if me.context.is_none() => flag(
                Rule::MissingContext,
                "domain and application crates declare `context` in `[package.metadata.health]`"
                    .to_owned(),
            ),
            _ => {}
        }

        if matches!(me.layer, Some(Layer::Domain | Layer::Application))
            && !package.features.is_empty()
        {
            let features: Vec<&str> = package.features.keys().map(String::as_str).collect();
            flag(
                Rule::FeaturesInDomainOrApplication,
                format!(
                    "declares features {features:?}; runtime differences are injected through \
                     ports and configuration"
                ),
            );
        }

        for dep in &package.dependencies {
            let internal = dep
                .path
                .as_ref()
                .and_then(|p| members.values().find(|m| m.dir == p.as_std_path()))
                .or_else(|| members.get(dep.name.as_str()));
            let dev = dep.kind == DependencyKind::Development;
            match internal {
                Some(target) if !dev => {
                    edges
                        .entry(name.to_owned())
                        .or_default()
                        .insert(target.name.clone());
                    if let Some((rule, detail)) = edge_violation(me, target) {
                        flag(rule, detail);
                    }
                }
                Some(_) => {}
                None if !dev
                    && me.layer == Some(Layer::Domain)
                    && !allowlist.domain_externals.contains(&dep.name) =>
                {
                    let why = if allowlist.domain_forbidden.contains(&dep.name) {
                        "is forbidden in domain crates"
                    } else {
                        "is not on the domain allowlist (tools/architecture-checks/allowlist.toml)"
                    };
                    flag(
                        Rule::DomainExternal,
                        format!("depends on external `{}`, which {why}", dep.name),
                    );
                }
                None => {}
            }
        }

        let manifest = package.manifest_path.as_std_path();
        let raw: toml::Table = std::fs::read_to_string(manifest)
            .with_context(|| format!("reading {}", manifest.display()))?
            .parse()
            .with_context(|| format!("parsing {}", manifest.display()))?;
        for (table, dep) in inline_dependencies(&raw) {
            flag(
                Rule::InlineVersion,
                format!(
                    "`{dep}` in [{table}] must be `{{ workspace = true }}`; declare it once in the root [workspace.dependencies]"
                ),
            );
        }
        let inherits_lints = raw
            .get("lints")
            .and_then(|l| l.get("workspace"))
            .and_then(toml::Value::as_bool)
            == Some(true);
        if !inherits_lints && !allowlist.lint_exempt.contains(name) {
            flag(
                Rule::MissingWorkspaceLints,
                "add `[lints] workspace = true`".to_owned(),
            );
        }
    }

    for cycle in cycles(&edges) {
        violations.push(Violation {
            rule: Rule::Cycle,
            krate: cycle[0].clone(),
            detail: format!("dependency cycle {}", cycle.join(" → ")),
        });
    }

    violations.sort();
    violations.dedup();
    Ok(violations)
}

fn edge_violation(from: &Member, to: &Member) -> Option<(Rule, String)> {
    let (Some(a), Some(b)) = (from.layer, to.layer) else {
        return None;
    };
    let edge = format!("`{}` ({a}) → `{}` ({b})", from.name, to.name);
    if b == Layer::Testing {
        return Some((
            Rule::TestingAsNormalDependency,
            format!("{edge}: testing crates are dev-dependencies only"),
        ));
    }
    if b == Layer::Tooling {
        return Some((
            Rule::ToolingIsLeaf,
            format!("{edge}: nothing depends on tooling crates"),
        ));
    }
    if b == Layer::Runtime {
        return if a == Layer::Runtime && from.in_apps {
            None
        } else {
            Some((
                Rule::RuntimeIsLeaf,
                format!("{edge}: only composition roots in apps/ may depend on runtime crates"),
            ))
        };
    }
    if matches!(a, Layer::Testing | Layer::Tooling) {
        return None;
    }
    if a == Layer::Domain && b == Layer::Domain {
        return Some((
            Rule::DomainToDomain,
            format!(
                "{edge}: declare a port in the dependent domain and implement it in the application layer"
            ),
        ));
    }
    if a == Layer::Application && b == Layer::Application {
        return Some((
            Rule::ApplicationToApplication,
            format!("{edge}: compose across contexts in a runtime crate or through events"),
        ));
    }
    let (ra, rb) = (a.rank()?, b.rank()?);
    if rb < ra {
        return None;
    }
    let same_layer_allowed = ra == rb
        && match a {
            // Per-context contract crates may use the shared FHIR and foundation contracts.
            Layer::Contracts => from.context.is_some() && to.context.is_none(),
            Layer::Infrastructure => true,
            _ => false,
        };
    if same_layer_allowed {
        return None;
    }
    let direction = if ra == rb {
        "same layer"
    } else {
        "higher layer"
    };
    Some((
        Rule::UpwardEdge,
        format!("{edge}: depends on a {direction}"),
    ))
}

/// Dependencies in a raw manifest that are not `{ workspace = true }`, as `(table, name)`.
fn inline_dependencies(raw: &toml::Table) -> Vec<(String, String)> {
    const KINDS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];
    let mut tables: Vec<(String, &toml::Table)> = KINDS
        .iter()
        .filter_map(|kind| raw.get(*kind)?.as_table().map(|t| ((*kind).to_owned(), t)))
        .collect();
    if let Some(targets) = raw.get("target").and_then(toml::Value::as_table) {
        for (cfg, target) in targets {
            for kind in KINDS {
                if let Some(t) = target.get(kind).and_then(toml::Value::as_table) {
                    tables.push((format!("target.{cfg}.{kind}"), t));
                }
            }
        }
    }
    let mut found = Vec::new();
    for (table, deps) in tables {
        for (name, spec) in deps {
            let inherited = spec.get("workspace").and_then(toml::Value::as_bool) == Some(true);
            if !inherited {
                found.push((table.clone(), name.clone()));
            }
        }
    }
    found
}

/// Every elementary cycle reachable by depth-first search, each reported once.
fn cycles(edges: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<String>> {
    fn visit(
        node: &str,
        edges: &BTreeMap<String, BTreeSet<String>>,
        stack: &mut Vec<String>,
        done: &mut BTreeSet<String>,
        found: &mut BTreeSet<Vec<String>>,
    ) {
        if let Some(pos) = stack.iter().position(|n| n == node) {
            let mut cycle = stack[pos..].to_vec();
            let start = cycle
                .iter()
                .enumerate()
                .min_by(|x, y| x.1.cmp(y.1))
                .map_or(0, |(i, _)| i);
            cycle.rotate_left(start);
            cycle.push(cycle[0].clone());
            found.insert(cycle);
            return;
        }
        if done.contains(node) {
            return;
        }
        stack.push(node.to_owned());
        for next in edges.get(node).into_iter().flatten() {
            visit(next, edges, stack, done, found);
        }
        stack.pop();
        done.insert(node.to_owned());
    }

    let mut found = BTreeSet::new();
    let mut done = BTreeSet::new();
    for node in edges.keys() {
        visit(node, edges, &mut Vec::new(), &mut done, &mut found);
    }
    found.into_iter().collect()
}
