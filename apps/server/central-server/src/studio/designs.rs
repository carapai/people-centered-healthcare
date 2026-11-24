//! What the Studio designs, across packages (#855; studio.md § The Studio home), at
//! `GET /api/studio/designs`: every form, workflow, decision table, code system, value set,
//! program, Age Band Set, indicator, Report and standard Dashboard, each in the package version
//! it opens in, and each package's published version and open draft, so the client can say
//! where a design stands, start or reuse a draft for *Edit*, and offer a package for *New*. `?kind=` keeps the designs of one kind.

use axum::extract::{RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use health_access_policy::Subject;
use health_publication::designs::{Design, DesignKind, DesignPackage, Designs};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{PackageStage, Studio, StudioPackageKind, StudioProblem, author, failure, problem};

/// What kind of design an artefact is: the Studio's sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StudioDesignKind {
    Form,
    Workflow,
    DecisionTable,
    CodeSystem,
    ValueSet,
    Program,
    AgeBandSet,
    Indicator,
    Report,
    Dashboard,
    Population,
}

impl From<DesignKind> for StudioDesignKind {
    fn from(kind: DesignKind) -> Self {
        match kind {
            DesignKind::Form => Self::Form,
            DesignKind::Workflow => Self::Workflow,
            DesignKind::DecisionTable => Self::DecisionTable,
            DesignKind::CodeSystem => Self::CodeSystem,
            DesignKind::ValueSet => Self::ValueSet,
            DesignKind::Program => Self::Program,
            DesignKind::AgeBandSet => Self::AgeBandSet,
            DesignKind::Indicator => Self::Indicator,
            DesignKind::Report => Self::Report,
            DesignKind::Dashboard => Self::Dashboard,
            DesignKind::Population => Self::Population,
        }
    }
}

/// One designed artefact, in the package version it opens in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDesign {
    /// Its kind.
    pub(crate) kind: StudioDesignKind,
    /// Its title, else its name, id or path; a decision table's id.
    pub(crate) title: String,
    /// Its file in the package; a decision table's is its Workflow Definition's.
    pub(crate) path: String,
    /// A decision table's id in its Workflow Definition.
    pub(crate) table: Option<String>,
    /// Its canonical URL.
    pub(crate) url: Option<String>,
    /// Its version.
    pub(crate) version: Option<String>,
    /// The package name.
    pub(crate) package: String,
    /// The package version it opens in (`name@version`): the open draft, else the latest
    /// published version.
    pub(crate) package_id: String,
    /// A year's Population Estimates' types, given and derived, in order: what an indicator's
    /// denominator may name (#553, #864). Empty for other designs.
    pub(crate) estimate_types: Vec<StudioEstimateType>,
}

/// An estimate type a Population package declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioEstimateType {
    /// Its code, what a denominator names (`expected-pregnancies`).
    pub(crate) code: String,
    /// What a person reads.
    pub(crate) title: String,
}

impl From<&Design> for StudioDesign {
    fn from(design: &Design) -> Self {
        Self {
            kind: design.kind.into(),
            title: design.title.clone(),
            path: design.path.clone(),
            table: design.table.clone(),
            url: design.url.clone(),
            version: design.version.clone(),
            package: design.package.clone(),
            package_id: design.package_id.clone(),
            estimate_types: design
                .estimate_types
                .iter()
                .map(|t| StudioEstimateType {
                    code: t.code.clone(),
                    title: t.title.clone(),
                })
                .collect(),
        }
    }
}

/// A package's open draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDesignDraft {
    /// `name@version`.
    pub(crate) id: String,
    /// Where it stands: draft to approval.
    pub(crate) stage: PackageStage,
    /// Its Draft Holder (an account id).
    pub(crate) holder: String,
    /// Whether the signed-in person holds it.
    pub(crate) mine: bool,
}

/// A package that holds designs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioDesignPackage {
    /// The package name.
    pub(crate) name: String,
    /// Its title.
    pub(crate) title: Option<String>,
    /// Its kind.
    pub(crate) kind: StudioPackageKind,
    /// Its latest published version (`name@version`).
    pub(crate) published: Option<String>,
    /// Its open draft.
    pub(crate) draft: Option<StudioDesignDraft>,
    /// The canonical base new artefacts take, from its manifest's URL.
    pub(crate) canonical: Option<String>,
    /// When the version its designs are read from last changed.
    pub(crate) changed: Option<String>,
}

impl From<&DesignPackage> for StudioDesignPackage {
    fn from(package: &DesignPackage) -> Self {
        Self {
            name: package.name.clone(),
            title: package.title.clone(),
            kind: package.kind.into(),
            published: package.published.clone(),
            draft: package.draft.as_ref().map(|draft| StudioDesignDraft {
                id: draft.id.clone(),
                stage: draft.stage.into(),
                holder: draft.holder.clone(),
                mine: draft.mine,
            }),
            canonical: package.canonical.clone(),
            changed: package.changed.map(|at| at.to_string()),
        }
    }
}

/// Every design across packages, and the packages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct StudioDesigns {
    /// Every package with an open draft or a published version, by name.
    pub(crate) packages: Vec<StudioDesignPackage>,
    /// The designs, by package, then path.
    pub(crate) designs: Vec<StudioDesign>,
}

impl StudioDesigns {
    fn of(designs: &Designs, kind: Option<StudioDesignKind>) -> Self {
        Self {
            packages: designs
                .packages
                .iter()
                .map(StudioDesignPackage::from)
                .collect(),
            designs: designs
                .designs
                .iter()
                .map(StudioDesign::from)
                .filter(|d| kind.is_none_or(|k| d.kind == k))
                .collect(),
        }
    }
}

/// Documents and serves the designs.
#[utoipa::path(get, path = "/api/studio/designs", tag = "studio", operation_id = "listStudioDesigns",
    description = "What the Studio designs, across packages (central only): every form, workflow, decision table, code system, value set and program, each read from its package's open draft when it has one and otherwise from its latest published version; and every such package with its latest published version and its open draft, saying whether the signed-in person holds it. Needs a `metadata.*` or `configuration.activate` permission.",
    params(("kind" = Option<StudioDesignKind>, Query, description = "Only the designs of this kind")),
    responses(
        (status = 200, description = "The designs", body = StudioDesigns),
        (status = 400, description = "Not a kind of design", body = StudioProblem),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn list(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    RawQuery(query): RawQuery,
) -> Response {
    let subject = match author(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let kind = form_urlencoded::parse(query.as_deref().unwrap_or("").as_bytes())
        .find(|(name, _)| name == "kind")
        .map(|(_, value)| value.into_owned());
    let kind = match kind {
        None => None,
        Some(text) => match serde_json::from_value(serde_json::Value::String(text.clone())) {
            Ok(kind) => Some(kind),
            Err(_) => {
                return problem(
                    StatusCode::BAD_REQUEST,
                    format!("{text:?} is not a kind of design"),
                    None,
                );
            }
        },
    };
    match studio.publications.studio_designs(&subject.account).await {
        Ok(designs) => Json(StudioDesigns::of(&designs, kind)).into_response(),
        Err(error) => failure(&error),
    }
}
