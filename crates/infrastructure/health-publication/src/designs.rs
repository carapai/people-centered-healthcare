//! What the Studio designs, across packages (#855; studio.md § The Studio home): every form,
//! workflow, decision table, code system, value set, program, Age Band Set, indicator and
//! Report and standard Dashboard, each in the package version a person would open it in, and each package's published
//! version and open draft. The Studio starts from these; the package workspace stays behind
//! them.
//!
//! A package's artefacts are read from its open draft when it has one (Draft to Approval), and
//! otherwise from its latest published version. A package with neither (only withdrawn drafts)
//! is left out.

use std::collections::BTreeMap;

use health_configuration::lifecycle::{Lifecycle, Stage};
use health_configuration::package::{MANIFEST, PackageKind};
use health_packages::Packed;
use health_time::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use health_analytics::EstimateTypeDefinition;
use health_analytics_app::is_age_band_set;

use crate::dashboards::dashboards_in;
use crate::population::estimates_in;
use crate::reports::reports_in;
use crate::workflows::workflow_definitions;
use crate::{Error, Publications, failed, lifecycle_id};

/// What kind of design an artefact is: the Studio's sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesignKind {
    /// A Questionnaire, in the form designer.
    Form,
    /// A Workflow Definition Library, in the Workflow Studio.
    Workflow,
    /// A decision table inside a Workflow Definition.
    DecisionTable,
    /// A CodeSystem, in the terminology browser.
    CodeSystem,
    /// A ValueSet, in the terminology browser.
    ValueSet,
    /// A program's PlanDefinition, in the Program Designer.
    Program,
    /// An Age Band Set Library, in the age band table (#551).
    AgeBandSet,
    /// An indicator's Measure, in the indicator editor (#537).
    Indicator,
    /// A Report in a Reporting package, `configuration/reports/<id>.json` (#556).
    Report,
    /// A standard Dashboard in a Reporting package, `configuration/dashboards/<id>.json`
    /// (#603, #879).
    Dashboard,
    /// A year's Population Estimates in a Population package, `configuration/population.json`
    /// (#553).
    Population,
}

/// One designed artefact, in the package version it opens in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Design {
    /// Its kind.
    pub kind: DesignKind,
    /// Its title (else its name, else its id, else its path; a decision table's id).
    pub title: String,
    /// Its file in the package; a decision table's is its Workflow Definition's.
    pub path: String,
    /// A decision table's id in its Workflow Definition.
    pub table: Option<String>,
    /// Its canonical URL, if it has one.
    pub url: Option<String>,
    /// Its version, if it has one.
    pub version: Option<String>,
    /// The package name.
    pub package: String,
    /// The package version it is read from (`name@version`).
    pub package_id: String,
    /// A year's Population Estimates' types, given and derived, in order: what an indicator's
    /// denominator may name (#553, #864). None for other designs.
    #[serde(default)]
    pub estimate_types: Vec<EstimateTypeDefinition>,
}

/// A package's open draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenDraft {
    /// `name@version`.
    pub id: String,
    /// Where it stands (Draft to Approval).
    pub stage: Stage,
    /// Its Draft Holder (an account id).
    pub holder: String,
    /// Whether the person asking holds it.
    pub mine: bool,
}

/// A package, as the Studio's sections need it: to say where an artefact stands, to start or
/// reuse a draft for *Edit*, and to offer a package for *New*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignPackage {
    /// The package name.
    pub name: String,
    /// Its title, from the manifest of the version read.
    pub title: Option<String>,
    /// Its kind.
    pub kind: PackageKind,
    /// Its latest published version (`name@version`), if any.
    pub published: Option<String>,
    /// Its open draft, if any.
    pub draft: Option<OpenDraft>,
    /// The canonical base of the version read, from its manifest's URL (`…/Library/x` gives
    /// `…`), for new artefacts.
    pub canonical: Option<String>,
    /// When the version read last changed: its newest history entry.
    pub changed: Option<Timestamp>,
}

/// Every design and every package that holds them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Designs {
    /// The packages, by name.
    pub packages: Vec<DesignPackage>,
    /// The designs, by package, then path.
    pub designs: Vec<Design>,
}

/// Whether a stage is an open draft's: not yet published, nor withdrawn.
fn open(stage: Stage) -> bool {
    !matches!(stage, Stage::Published | Stage::Withdrawn)
}

/// The title a resource shows under.
fn title_of(resource: &Value, path: &str) -> String {
    ["title", "name", "id"]
        .iter()
        .find_map(|key| resource[*key].as_str().filter(|t| !t.trim().is_empty()))
        .unwrap_or(path)
        .to_owned()
}

/// The manifest's title and canonical base.
fn manifest(packed: &Packed) -> (Option<String>, Option<String>) {
    let Some(manifest) = packed
        .files
        .get(MANIFEST)
        .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok())
    else {
        return (None, None);
    };
    let title = manifest["title"].as_str().map(str::to_owned);
    let canonical = manifest["url"]
        .as_str()
        .and_then(|url| url.rsplit_once("/Library/"))
        .map(|(base, _)| base.to_owned());
    (title, canonical)
}

/// The designs in `packed`, read as package version `id` of `package`.
pub fn designs_in(packed: &Packed, package: &str, id: &str) -> Vec<Design> {
    let design = |kind, title: String, path: &str, table, url, version| Design {
        kind,
        title,
        path: path.to_owned(),
        table,
        url,
        version,
        package: package.to_owned(),
        package_id: id.to_owned(),
        estimate_types: Vec::new(),
    };
    let (workflows, _) = workflow_definitions(&packed.files);
    let mut found = Vec::new();
    for artefact in &packed.contents.artefacts {
        let path = artefact.path.as_str();
        if path.contains('/') || path == MANIFEST {
            continue;
        }
        let kind = match artefact.resource_type.as_str() {
            "Questionnaire" => DesignKind::Form,
            "CodeSystem" => DesignKind::CodeSystem,
            "ValueSet" => DesignKind::ValueSet,
            "PlanDefinition" => DesignKind::Program,
            "Measure" => DesignKind::Indicator,
            "Library" if workflows.iter().any(|(p, _)| p == path) => DesignKind::Workflow,
            "Library" => DesignKind::AgeBandSet,
            _ => continue,
        };
        let resource = packed
            .files
            .get(path)
            .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok())
            .unwrap_or(Value::Null);
        if kind == DesignKind::AgeBandSet && !is_age_band_set(&resource) {
            continue;
        }
        found.push(design(
            kind,
            title_of(&resource, path),
            path,
            None,
            artefact.url.clone(),
            artefact.version.clone(),
        ));
    }
    for (path, report) in reports_in(packed) {
        let title = report
            .ok()
            .map(|r| r.title)
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| path.clone());
        found.push(design(DesignKind::Report, title, &path, None, None, None));
    }
    for (path, estimates) in estimates_in(&packed.files) {
        let estimates = estimates.ok();
        let title = estimates
            .as_ref()
            .map(|e| format!("Population estimates {}", e.year))
            .unwrap_or_else(|| path.clone());
        found.push(Design {
            estimate_types: estimates.map(|e| e.types).unwrap_or_default(),
            ..design(DesignKind::Population, title, &path, None, None, None)
        });
    }
    for (path, dashboard) in dashboards_in(packed) {
        let title = dashboard
            .ok()
            .map(|d| d.title)
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| path.clone());
        found.push(design(
            DesignKind::Dashboard,
            title,
            &path,
            None,
            None,
            None,
        ));
    }
    for (path, definition) in &workflows {
        for table in &definition.decision_tables {
            found.push(design(
                DesignKind::DecisionTable,
                table.id.clone(),
                path,
                Some(table.id.clone()),
                Some(definition.url.clone()),
                Some(definition.version.clone()),
            ));
        }
    }
    found.sort_by(|a, b| (&a.path, &a.table).cmp(&(&b.path, &b.table)));
    found
}

impl Publications {
    /// Every design across packages, for `by`: each package's open draft when it has one, else
    /// its latest published version.
    pub async fn studio_designs(&self, by: &str) -> Result<Designs, Error> {
        let mut tx = self.begin().await?;
        let mut by_package: BTreeMap<String, Vec<Lifecycle>> = BTreeMap::new();
        for json in tx.package_lifecycles().await.map_err(failed)? {
            let lifecycle: Lifecycle = serde_json::from_str(&json).map_err(failed)?;
            by_package
                .entry(lifecycle.package.clone())
                .or_default()
                .push(lifecycle);
        }
        drop(tx);
        let mut out = Designs::default();
        for (name, lifecycles) in by_package {
            let newest = |wanted: &dyn Fn(Stage) -> bool| {
                lifecycles
                    .iter()
                    .filter(|l| wanted(l.stage))
                    .max_by(|a, b| a.version.cmp(&b.version))
            };
            let published = newest(&|stage| stage == Stage::Published);
            let draft = newest(&open);
            let Some(read) = draft.or(published) else {
                continue;
            };
            let id = lifecycle_id(read);
            let packed = self.packed(&read.content).await?;
            let (title, canonical) = manifest(&packed);
            let (_, history) = self.lifecycle(&id).await?;
            out.designs.extend(designs_in(&packed, &name, &id));
            out.packages.push(DesignPackage {
                name,
                title,
                kind: read.kind,
                published: published.map(lifecycle_id),
                draft: draft.map(|d| OpenDraft {
                    id: lifecycle_id(d),
                    stage: d.stage,
                    holder: d.holder.clone(),
                    mine: d.holder == by,
                }),
                canonical,
                changed: history.last().map(|r| r.at),
            });
        }
        Ok(out)
    }
}
