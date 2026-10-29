//! Each rule is proven on a generated fixture workspace: a valid baseline, then one breakage.

use std::fs;
use std::path::Path;

use architecture_checks::{Allowlist, Rule, check};
use tempfile::TempDir;

/// A fixture crate: path, name, layer tag (`None` omits it), context, and `[dependencies]` lines.
struct Krate {
    path: &'static str,
    name: &'static str,
    layer: Option<&'static str>,
    context: Option<&'static str>,
    deps: Vec<String>,
    dev_deps: Vec<String>,
    extra: String,
    lints: bool,
}

impl Krate {
    fn new(path: &'static str, name: &'static str, layer: &'static str) -> Self {
        Self {
            path,
            name,
            layer: Some(layer),
            context: None,
            deps: Vec::new(),
            dev_deps: Vec::new(),
            extra: String::new(),
            lints: true,
        }
    }

    fn context(mut self, context: &'static str) -> Self {
        self.context = Some(context);
        self
    }

    fn dep(mut self, name: &str) -> Self {
        self.deps.push(format!("{name} = {{ workspace = true }}"));
        self
    }

    fn dev_dep(mut self, name: &str) -> Self {
        self.dev_deps
            .push(format!("{name} = {{ workspace = true }}"));
        self
    }
}

fn baseline() -> Vec<Krate> {
    vec![
        Krate::new(
            "crates/foundation/health-errors",
            "health-errors",
            "foundation",
        ),
        Krate::new(
            "crates/contracts/health-persistence",
            "health-persistence",
            "contracts",
        )
        .dep("health-errors"),
        Krate::new(
            "crates/contracts/health-registries-contract",
            "health-registries-contract",
            "contracts",
        )
        .context("registries")
        .dep("health-persistence"),
        Krate::new(
            "crates/domains/health-client-registry",
            "health-client-registry",
            "domain",
        )
        .context("registries")
        .dep("health-registries-contract")
        .dep("serde")
        .dev_dep("health-fakes"),
        Krate::new(
            "crates/domains/health-facility-registry",
            "health-facility-registry",
            "domain",
        )
        .context("registries"),
        Krate::new(
            "crates/application/health-registries-app",
            "health-registries-app",
            "application",
        )
        .context("registries")
        .dep("health-client-registry")
        .dep("health-facility-registry"),
        Krate::new(
            "crates/infrastructure/health-surrealdb",
            "health-surrealdb",
            "infrastructure",
        )
        .dep("health-persistence"),
        Krate::new(
            "crates/infrastructure/health-http",
            "health-http",
            "infrastructure",
        )
        .dep("health-surrealdb")
        .dep("health-registries-app"),
        Krate::new(
            "crates/edge/health-edge-runtime",
            "health-edge-runtime",
            "runtime",
        )
        .dep("health-http"),
        Krate::new("apps/server/facility-edge", "facility-edge", "runtime")
            .dep("health-edge-runtime"),
        Krate::new("crates/testing/health-fakes", "health-fakes", "testing")
            .dep("health-client-registry"),
        Krate::new(
            "tools/architecture-checks",
            "architecture-checks",
            "tooling",
        )
        .dep("health-errors"),
    ]
}

fn write(dir: &Path, krates: &[Krate]) {
    let members: Vec<String> = krates.iter().map(|k| format!("\"{}\"", k.path)).collect();
    let mut deps: Vec<String> = krates
        .iter()
        .map(|k| format!("{} = {{ path = \"{}\" }}", k.name, k.path))
        .collect();
    deps.extend(
        [
            "serde = \"1\"",
            "tokio = \"1\"",
            "uuid = \"1\"",
            "fhir-r5 = \"=4.2.3\"",
        ]
        .map(str::to_owned),
    );
    fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[workspace]\nresolver = \"3\"\nmembers = [{}]\n\n[workspace.package]\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\n\n[workspace.dependencies]\n{}\n",
            members.join(", "),
            deps.join("\n")
        ),
    )
    .expect("write root manifest");
    for k in krates {
        let crate_dir = dir.join(k.path);
        fs::create_dir_all(crate_dir.join("src")).expect("create crate dir");
        fs::write(crate_dir.join("src/lib.rs"), "").expect("write lib.rs");
        let mut manifest = format!(
            "[package]\nname = \"{}\"\nversion.workspace = true\nedition.workspace = true\n\n",
            k.name
        );
        if let Some(layer) = k.layer {
            manifest.push_str(&format!("[package.metadata.health]\nlayer = \"{layer}\"\n"));
            if let Some(context) = k.context {
                manifest.push_str(&format!("context = \"{context}\"\n"));
            }
            manifest.push('\n');
        }
        if k.lints {
            manifest.push_str("[lints]\nworkspace = true\n\n");
        }
        manifest.push_str(&format!("[dependencies]\n{}\n\n", k.deps.join("\n")));
        manifest.push_str(&format!(
            "[dev-dependencies]\n{}\n\n",
            k.dev_deps.join("\n")
        ));
        manifest.push_str(&k.extra);
        fs::write(crate_dir.join("Cargo.toml"), manifest).expect("write crate manifest");
    }
}

fn allowlist() -> Allowlist {
    Allowlist {
        domain_externals: ["serde", "thiserror", "uuid"].map(str::to_owned).into(),
        domain_forbidden: ["tokio"].map(str::to_owned).into(),
        lint_exempt: Default::default(),
        facades: [("fhir-r5".to_owned(), "health-persistence".to_owned())].into(),
    }
}

/// Runs the checks on `krates` and returns the rules broken, with their crates.
fn rules(krates: &[Krate]) -> Vec<(Rule, String)> {
    let dir = TempDir::new().expect("tempdir");
    write(dir.path(), krates);
    check(&dir.path().join("Cargo.toml"), &allowlist())
        .expect("checks run")
        .into_iter()
        .map(|v| (v.rule, v.krate))
        .collect()
}

fn with(edit: impl FnOnce(&mut Vec<Krate>)) -> Vec<(Rule, String)> {
    let mut krates = baseline();
    edit(&mut krates);
    rules(&krates)
}

fn find<'a>(krates: &'a mut [Krate], name: &str) -> &'a mut Krate {
    krates
        .iter_mut()
        .find(|k| k.name == name)
        .expect("fixture crate")
}

fn only(rule: Rule, krate: &str) -> Vec<(Rule, String)> {
    vec![(rule, krate.to_owned())]
}

#[test]
fn baseline_is_clean() {
    assert_eq!(rules(&baseline()), vec![]);
}

#[test]
fn missing_layer_tag() {
    let found = with(|k| find(k, "health-errors").layer = None);
    assert_eq!(found, only(Rule::MissingLayerTag, "health-errors"));
}

#[test]
fn unknown_layer_tag() {
    let found = with(|k| find(k, "health-errors").layer = Some("domains"));
    assert_eq!(found, only(Rule::MissingLayerTag, "health-errors"));
}

#[test]
fn missing_context() {
    let found = with(|k| find(k, "health-facility-registry").context = None);
    assert_eq!(
        found,
        only(Rule::MissingContext, "health-facility-registry")
    );
}

#[test]
fn upward_edge() {
    let found = with(|k| {
        let errors = find(k, "health-errors");
        *errors = Krate::new(
            "crates/foundation/health-errors",
            "health-errors",
            "foundation",
        )
        .dep("health-persistence");
    });
    assert!(
        found.contains(&(Rule::UpwardEdge, "health-errors".to_owned())),
        "{found:?}"
    );
}

#[test]
fn same_layer_edge() {
    let found = with(|k| {
        let persistence = find(k, "health-persistence");
        persistence
            .deps
            .push("health-registries-contract = { workspace = true }".to_owned());
    });
    assert!(
        found.contains(&(Rule::UpwardEdge, "health-persistence".to_owned())),
        "{found:?}"
    );
}

#[test]
fn domain_to_domain() {
    let found = with(|k| {
        find(k, "health-client-registry")
            .deps
            .push("health-facility-registry = { workspace = true }".to_owned());
    });
    assert_eq!(found, only(Rule::DomainToDomain, "health-client-registry"));
}

#[test]
fn application_to_application() {
    let found = with(|k| {
        k.push(
            Krate::new(
                "crates/application/health-finance-app",
                "health-finance-app",
                "application",
            )
            .context("finance")
            .dep("health-registries-app"),
        );
    });
    assert_eq!(
        found,
        only(Rule::ApplicationToApplication, "health-finance-app")
    );
}

#[test]
fn runtime_is_leaf() {
    let found = with(|k| {
        find(k, "health-http")
            .deps
            .push("health-edge-runtime = { workspace = true }".to_owned());
    });
    assert!(
        found.contains(&(Rule::RuntimeIsLeaf, "health-http".to_owned())),
        "{found:?}"
    );
}

#[test]
fn runtime_library_cannot_depend_on_runtime_library() {
    let found = with(|k| {
        k.push(
            Krate::new(
                "crates/edge/health-node-identity",
                "health-node-identity",
                "runtime",
            )
            .dep("health-edge-runtime"),
        );
    });
    assert_eq!(found, only(Rule::RuntimeIsLeaf, "health-node-identity"));
}

#[test]
fn tooling_is_leaf() {
    let found = with(|k| {
        find(k, "facility-edge")
            .deps
            .push("architecture-checks = { workspace = true }".to_owned());
    });
    assert_eq!(found, only(Rule::ToolingIsLeaf, "facility-edge"));
}

#[test]
fn testing_as_normal_dependency() {
    let found = with(|k| {
        find(k, "health-registries-app")
            .deps
            .push("health-fakes = { workspace = true }".to_owned());
    });
    assert_eq!(
        found,
        only(Rule::TestingAsNormalDependency, "health-registries-app")
    );
}

#[test]
fn domain_external_not_allowlisted() {
    let found = with(|k| {
        find(k, "health-facility-registry")
            .deps
            .push("tokio = { workspace = true }".to_owned());
    });
    assert_eq!(
        found,
        only(Rule::DomainExternal, "health-facility-registry")
    );
}

#[test]
fn only_the_facade_uses_a_faceted_crate() {
    let owner = with(|k| {
        find(k, "health-persistence")
            .deps
            .push("fhir-r5 = { workspace = true }".to_owned());
    });
    assert_eq!(owner, Vec::new(), "the facade itself may");
    let bypass = with(|k| {
        find(k, "health-registries-contract")
            .dev_deps
            .push("fhir-r5 = { workspace = true }".to_owned());
    });
    assert_eq!(
        bypass,
        only(Rule::FacadeBypass, "health-registries-contract"),
        "not even as a dev-dependency"
    );
}

#[test]
fn features_in_domain() {
    let found =
        with(|k| find(k, "health-facility-registry").extra = "[features]\nedge = []\n".to_owned());
    assert_eq!(
        found,
        only(
            Rule::FeaturesInDomainOrApplication,
            "health-facility-registry"
        )
    );
}

#[test]
fn features_in_application() {
    let found =
        with(|k| find(k, "health-registries-app").extra = "[features]\ncentral = []\n".to_owned());
    assert_eq!(
        found,
        only(Rule::FeaturesInDomainOrApplication, "health-registries-app")
    );
}

#[test]
fn features_allowed_in_infrastructure() {
    let found =
        with(|k| find(k, "health-surrealdb").extra = "[features]\nembedded = []\n".to_owned());
    assert_eq!(found, vec![]);
}

#[test]
fn inline_version() {
    let found = with(|k| {
        find(k, "health-surrealdb")
            .deps
            .push("uuid = \"1\"".to_owned())
    });
    assert_eq!(found, only(Rule::InlineVersion, "health-surrealdb"));
}

#[test]
fn inline_path() {
    let found = with(|k| {
        find(k, "health-surrealdb")
            .deps
            .push("health-errors = { path = \"../../foundation/health-errors\" }".to_owned());
    });
    assert_eq!(found, only(Rule::InlineVersion, "health-surrealdb"));
}

#[test]
fn inline_version_in_target_table() {
    let found = with(|k| {
        find(k, "health-surrealdb").extra =
            "[target.'cfg(windows)'.dependencies]\nuuid = \"1\"\n".to_owned();
    });
    assert_eq!(found, only(Rule::InlineVersion, "health-surrealdb"));
}

#[test]
fn missing_workspace_lints() {
    let found = with(|k| find(k, "health-http").lints = false);
    assert_eq!(found, only(Rule::MissingWorkspaceLints, "health-http"));
}

#[test]
fn lint_exemption_is_honoured() {
    let mut krates = baseline();
    find(&mut krates, "health-http").lints = false;
    let dir = TempDir::new().expect("tempdir");
    write(dir.path(), &krates);
    let mut allow = allowlist();
    allow.lint_exempt.insert("health-http".to_owned());
    let found = check(&dir.path().join("Cargo.toml"), &allow).expect("checks run");
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn cycle_between_infrastructure_crates() {
    // Same-layer infrastructure edges are allowed, so only the cycle rule catches this one.
    // `cargo metadata --no-deps` does not reject cycles, so the check must.
    let found = with(|k| {
        find(k, "health-surrealdb")
            .deps
            .push("health-http = { workspace = true }".to_owned());
    });
    assert_eq!(found, only(Rule::Cycle, "health-http"));
}

#[test]
fn dev_dependency_back_edge_is_not_a_cycle() {
    let found = with(|k| {
        find(k, "health-surrealdb")
            .dev_deps
            .push("health-http = { workspace = true }".to_owned());
    });
    assert_eq!(found, vec![]);
}

#[test]
fn real_workspace_is_clean() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let allow =
        Allowlist::load(&root.join("tools/architecture-checks/allowlist.toml")).expect("allowlist");
    let found = check(&root.join("Cargo.toml"), &allow).expect("checks run");
    assert!(found.is_empty(), "{found:#?}");
}
