//! The Studio's terminology browser (studio.md § Terminology; #523), at
//! `/api/studio/packages/{id}/terminology/`, over what the package version is authored against:
//! its own CodeSystems and ValueSets as they are now, and the latest published version of every
//! other package (`health_publication::terminology`).
//!
//! - `GET code-systems`: the code systems, each local or external, with its version and concept
//!   count.
//! - `GET concepts`: one level of a code system's hierarchy (`parent`, or the top), or a search
//!   by code or words (`text`), a page at a time. Both run here, so the browser never loads a
//!   large code system such as ICD-11.
//! - `GET concept`: one concept, with its parents and the ValueSets that contain it.
//! - `GET value-sets`: the ValueSets a form of this package may bind, searched by `text`.
//! - `POST expand`: a ValueSet's expansion (one held, by `url`, or an unsaved one sent as
//!   `valueSet`), with its change against the published expansion and the forms that bind it.
//!   The form designer's ValueSet picker uses it and `value-sets` too.

use std::collections::BTreeMap;

use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use health_access_policy::Subject;
use health_configuration::package::PackageKind;
use health_publication::terminology::StudioTerminology;
use health_terminology::{Contains, Content, ExpandRequest, Terminology, compare};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use super::{Studio, StudioProblem, author, failure, problem};

/// Concepts on a page unless asked for fewer.
const CONCEPTS: usize = 50;
/// The most concepts on one page.
const MOST_CONCEPTS: usize = 500;
/// Codes of an expansion unless asked for fewer.
const CODES: usize = 200;
/// The most codes of an expansion, or of each list of its change, in one answer.
const MOST_CODES: usize = 1000;

/// A code system the package version is authored against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioCodeSystem {
    /// Its canonical URL.
    pub(crate) url: String,
    /// Its version.
    pub(crate) version: Option<String>,
    /// Its `title`, or its `name`.
    pub(crate) title: Option<String>,
    /// The package it comes from, `name@version`.
    pub(crate) package: String,
    /// Its file, when it is this package's own.
    pub(crate) path: Option<String>,
    /// Whether it is this package's own.
    pub(crate) own: bool,
    /// Whether it is external: shipped in a terminology package, and read only.
    pub(crate) external: bool,
    /// Whether every concept is held (`content` is `complete`); only then can a ValueSet filter it.
    pub(crate) complete: bool,
    /// How many concepts it holds.
    pub(crate) concepts: u64,
}

/// The code systems, this package's own first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioCodeSystems {
    /// The code systems.
    pub(crate) code_systems: Vec<StudioCodeSystem>,
}

/// A concept, as a row of the hierarchy or of a search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioConceptRow {
    /// Its code.
    pub(crate) code: String,
    /// Its display.
    pub(crate) display: Option<String>,
    /// Whether it is inactive (retired).
    pub(crate) inactive: bool,
    /// How many concepts are directly under it.
    pub(crate) children: u64,
}

/// A page of concepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioConceptPage {
    /// The code system, `url|version`.
    pub(crate) code_system: String,
    /// How many concepts there are before paging.
    pub(crate) total: u64,
    /// This page.
    pub(crate) concepts: Vec<StudioConceptRow>,
}

/// A ValueSet, as a list names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioValueSet {
    /// Its canonical URL.
    pub(crate) url: String,
    /// Its version.
    pub(crate) version: Option<String>,
    /// Its `title`, or its `name`.
    pub(crate) title: Option<String>,
    /// The package it comes from, `name@version`.
    pub(crate) package: String,
    /// Its file in that package.
    pub(crate) path: String,
    /// Whether it is this package's own (a draft, when this package version is one).
    pub(crate) own: bool,
}

/// One concept read in full.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioConcept {
    /// The code system, `url|version`.
    pub(crate) code_system: String,
    /// Its code.
    pub(crate) code: String,
    /// Its display.
    pub(crate) display: Option<String>,
    /// Its definition.
    pub(crate) definition: Option<String>,
    /// Whether it is inactive (retired).
    pub(crate) inactive: bool,
    /// The codes directly above it.
    pub(crate) parents: Vec<String>,
    /// How many concepts are directly under it.
    pub(crate) children: u64,
    /// The ValueSets that contain it.
    pub(crate) value_sets: Vec<StudioValueSet>,
}

/// The ValueSets of the package version and of the published packages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioValueSets {
    /// The ValueSets, this package's own first.
    pub(crate) value_sets: Vec<StudioValueSet>,
}

/// Which ValueSet to expand.
#[derive(Debug, Clone, PartialEq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioExpandInput {
    /// A ValueSet held, by canonical URL.
    pub(crate) url: Option<String>,
    /// Its version; the latest held when there is none.
    pub(crate) version: Option<String>,
    /// An unsaved ValueSet resource of this package, expanded in place of the one it replaces.
    #[schema(value_type = Option<Object>)]
    pub(crate) value_set: Option<Value>,
    /// Only codes whose code or display holds this text.
    pub(crate) filter: Option<String>,
    /// Skip this many codes.
    pub(crate) offset: Option<u64>,
    /// At most this many codes (200 unless asked, 1000 at most).
    pub(crate) count: Option<u64>,
}

/// One code of an expansion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioCode {
    /// Its code system.
    pub(crate) system: String,
    /// The code system's version.
    pub(crate) version: Option<String>,
    /// The code.
    pub(crate) code: String,
    /// Its display.
    pub(crate) display: Option<String>,
    /// Whether it is inactive.
    pub(crate) inactive: bool,
}

impl From<&Contains> for StudioCode {
    fn from(c: &Contains) -> Self {
        Self {
            system: c.system.clone(),
            version: c.version.clone(),
            code: c.code.clone(),
            display: c.display.clone(),
            inactive: c.inactive,
        }
    }
}

/// How the expansion changed against the published one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioExpansionChange {
    /// The published ValueSet compared against, `url|version`; none when it was never published,
    /// so every code is added.
    pub(crate) published: Option<String>,
    /// How many codes were added.
    pub(crate) added_total: u64,
    /// The codes added (the first 1000).
    pub(crate) added: Vec<StudioCode>,
    /// How many codes were removed.
    pub(crate) removed_total: u64,
    /// The codes removed (the first 1000): records that hold them stay valid against the
    /// version that issued them, and new forms no longer offer them.
    pub(crate) removed: Vec<StudioCode>,
}

/// A form question that binds the ValueSet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioBinding {
    /// The package, `name@version`.
    pub(crate) package: String,
    /// The Questionnaire's file.
    pub(crate) path: String,
    /// The Questionnaire's canonical URL.
    pub(crate) questionnaire: Option<String>,
    /// Its title.
    pub(crate) title: Option<String>,
    /// The question's `linkId`.
    pub(crate) link_id: String,
    /// The binding as written, `url` or `url|version`.
    pub(crate) value_set: String,
}

/// A ValueSet's expansion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioExpansion {
    /// The ValueSet expanded, `url|version`.
    pub(crate) value_set: String,
    /// How many codes it has (after `filter`), before paging.
    pub(crate) total: u64,
    /// The codes on this page.
    pub(crate) contains: Vec<StudioCode>,
    /// Its change against the published expansion, for this package's own ValueSets.
    pub(crate) change: Option<StudioExpansionChange>,
    /// The form questions that bind it, in this package and the published ones.
    pub(crate) bound_by: Vec<StudioBinding>,
}

/// The query string's parameters, by name.
fn params(query: Option<&str>) -> BTreeMap<String, String> {
    form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect()
}

fn number(params: &BTreeMap<String, String>, name: &str) -> Result<Option<usize>, Box<Response>> {
    params
        .get(name)
        .map(|text| {
            text.parse().map_err(|_| {
                Box::new(problem(
                    StatusCode::BAD_REQUEST,
                    format!("{name} must be a whole number"),
                    None,
                ))
            })
        })
        .transpose()
}

fn wrong(error: &health_terminology::Error) -> Response {
    use health_terminology::Error as E;
    let status = match error {
        E::UnknownCodeSystem(_) | E::UnknownValueSet(_) | E::UnknownCode { .. } => {
            StatusCode::NOT_FOUND
        }
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    };
    problem(status, error.to_string(), None)
}

async fn sources(
    studio: &Studio,
    subject: Option<Extension<Subject>>,
    id: &str,
) -> Result<StudioTerminology, Box<Response>> {
    author(subject)?;
    studio
        .publications
        .studio_terminology(id)
        .await
        .map_err(|error| Box::new(failure(&error)))
}

fn title(name: Option<&String>, title: Option<&String>) -> Option<String> {
    title.or(name).cloned()
}

/// Every ValueSet of `sources`, this package's own first.
fn value_sets(sources: &StudioTerminology) -> Vec<StudioValueSet> {
    sources
        .parts()
        .flat_map(|part| {
            let own = part.id == sources.own.id;
            part.catalogue
                .entries
                .iter()
                .filter(|e| e.resource_type == "ValueSet")
                .map(move |e| StudioValueSet {
                    url: e.url.clone(),
                    version: e.version.clone(),
                    title: title(e.name.as_ref(), e.title.as_ref()),
                    package: part.id.clone(),
                    path: e.path.clone(),
                    own,
                })
        })
        .collect()
}

fn reference(url: &str, version: Option<&str>) -> String {
    match version {
        Some(version) => format!("{url}|{version}"),
        None => url.to_owned(),
    }
}

/// Documents and serves the code systems.
#[utoipa::path(get, path = "/api/studio/packages/{id}/terminology/code-systems", tag = "studio", operation_id = "listStudioCodeSystems",
    description = "The code systems package version `id` is authored against: its own, and those of the latest published version of every other package. Each is local or external (shipped in a terminology package, read only), with its version and concept count.",
    params(("id" = String, Path, description = "`name@version`")),
    responses(
        (status = 200, description = "The code systems", body = StudioCodeSystems),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version", body = StudioProblem),
        (status = 422, description = "A package's terminology cannot be read", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn code_systems(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
) -> Response {
    let sources = match sources(&studio, subject, &id).await {
        Ok(sources) => sources,
        Err(response) => return *response,
    };
    let mut rows = Vec::new();
    for part in sources.parts() {
        let own = part.id == sources.own.id;
        for summary in part.terminology.code_systems() {
            let entry = part.catalogue.entries.iter().find(|e| {
                e.resource_type == "CodeSystem"
                    && e.url == summary.url
                    && e.version == summary.version
            });
            rows.push(StudioCodeSystem {
                title: entry.and_then(|e| title(e.name.as_ref(), e.title.as_ref())),
                path: entry.filter(|_| own).map(|e| e.path.clone()),
                url: summary.url,
                version: summary.version,
                package: part.id.clone(),
                own,
                external: part.kind == PackageKind::Terminology,
                complete: summary.content == Content::Complete,
                concepts: summary.concepts as u64,
            });
        }
    }
    Json(StudioCodeSystems { code_systems: rows }).into_response()
}

/// Documents and serves a page of concepts.
#[utoipa::path(get, path = "/api/studio/packages/{id}/terminology/concepts", tag = "studio", operation_id = "listStudioConcepts",
    description = "A page of a code system's concepts, found here so a browser never loads a large code system: with `text`, those whose code, display or a designation holds every word (an exact code first); otherwise those directly under `parent`, or the top-level ones.",
    params(
        ("id" = String, Path, description = "`name@version`"),
        ("system" = String, Query, description = "The code system's canonical URL"),
        ("version" = Option<String>, Query, description = "Its version; the latest held when there is none"),
        ("parent" = Option<String>, Query, description = "The concept whose children to list"),
        ("text" = Option<String>, Query, description = "Words or a code to search for"),
        ("offset" = Option<u64>, Query, description = "Skip this many concepts"),
        ("count" = Option<u64>, Query, description = "At most this many (50 unless asked, 500 at most)"),
    ),
    responses(
        (status = 200, description = "The concepts", body = StudioConceptPage),
        (status = 400, description = "No system, or a number that is not one", body = StudioProblem),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version, code system or parent", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn concepts(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let params = params(query.as_deref());
    let Some(system) = params.get("system") else {
        return problem(StatusCode::BAD_REQUEST, "say which code system", None);
    };
    let (offset, count) = match (number(&params, "offset"), number(&params, "count")) {
        (Ok(offset), Ok(count)) => (
            offset.unwrap_or(0),
            count.unwrap_or(CONCEPTS).min(MOST_CONCEPTS),
        ),
        (Err(response), _) | (_, Err(response)) => return *response,
    };
    let sources = match sources(&studio, subject, &id).await {
        Ok(sources) => sources,
        Err(response) => return *response,
    };
    let terminology = sources.current();
    let version = params.get("version").map(String::as_str);
    let text = params
        .get("text")
        .map(|t| t.trim())
        .filter(|t| !t.is_empty());
    let found = match text {
        Some(text) => terminology.search(system, version, text, offset, Some(count)),
        None => terminology.children(
            system,
            version,
            params.get("parent").map(String::as_str),
            offset,
            Some(count),
        ),
    };
    match found {
        Ok(page) => Json(StudioConceptPage {
            code_system: page.code_system,
            total: page.total as u64,
            concepts: page
                .concepts
                .into_iter()
                .map(|row| StudioConceptRow {
                    code: row.code,
                    display: row.display,
                    inactive: row.inactive,
                    children: row.children as u64,
                })
                .collect(),
        })
        .into_response(),
        Err(error) => wrong(&error),
    }
}

/// Whether ValueSet `reference` holds `code` of `system`, by its full expansion.
fn holds(terminology: &Terminology, reference: &str, system: &str, code: &str) -> bool {
    terminology
        .expand(reference, &ExpandRequest::default())
        .is_ok_and(|expansion| {
            expansion
                .contains
                .iter()
                .any(|c| c.system == system && c.code == code)
        })
}

/// Documents and serves one concept.
#[utoipa::path(get, path = "/api/studio/packages/{id}/terminology/concept", tag = "studio", operation_id = "getStudioConcept",
    description = "One concept: its display, definition, the codes directly above it, and the ValueSets (this package's and the published ones) whose expansion contains it.",
    params(
        ("id" = String, Path, description = "`name@version`"),
        ("system" = String, Query, description = "The code system's canonical URL"),
        ("version" = Option<String>, Query, description = "Its version; the latest held when there is none"),
        ("code" = String, Query, description = "The code"),
    ),
    responses(
        (status = 200, description = "The concept", body = StudioConcept),
        (status = 400, description = "No system or code", body = StudioProblem),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version, code system or code", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn concept(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let params = params(query.as_deref());
    let (Some(system), Some(code)) = (params.get("system"), params.get("code")) else {
        return problem(StatusCode::BAD_REQUEST, "say which system and code", None);
    };
    let sources = match sources(&studio, subject, &id).await {
        Ok(sources) => sources,
        Err(response) => return *response,
    };
    let terminology = sources.current();
    let version = params.get("version").map(String::as_str);
    let found = match terminology.lookup(system, version, code) {
        Ok(found) => found,
        Err(error) => return wrong(&error),
    };
    let containing = value_sets(&sources)
        .into_iter()
        .filter(|v| {
            holds(
                &terminology,
                &reference(&v.url, v.version.as_deref()),
                &found.system,
                &found.concept.code,
            )
        })
        .collect();
    Json(StudioConcept {
        code_system: reference(&found.system, found.version.as_deref()),
        inactive: found.concept.is_inactive(),
        code: found.concept.code,
        display: found.concept.display,
        definition: found.concept.definition,
        parents: found.parents,
        children: found.children.len() as u64,
        value_sets: containing,
    })
    .into_response()
}

/// Documents and serves the ValueSets.
#[utoipa::path(get, path = "/api/studio/packages/{id}/terminology/value-sets", tag = "studio", operation_id = "listStudioValueSets",
    description = "The ValueSets a form of package version `id` may bind: its own (drafts, in a draft) and those of the latest published version of every other package, with `text` matching a URL, name or title.",
    params(
        ("id" = String, Path, description = "`name@version`"),
        ("text" = Option<String>, Query, description = "Words to match, ignoring case"),
    ),
    responses(
        (status = 200, description = "The ValueSets", body = StudioValueSets),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version", body = StudioProblem),
        (status = 422, description = "A package's terminology cannot be read", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn list_value_sets(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let params = params(query.as_deref());
    let sources = match sources(&studio, subject, &id).await {
        Ok(sources) => sources,
        Err(response) => return *response,
    };
    let text = params.get("text").map(|t| t.to_lowercase());
    let words: Vec<&str> = text.as_deref().unwrap_or("").split_whitespace().collect();
    let found = value_sets(&sources)
        .into_iter()
        .filter(|v| {
            let haystack = format!(
                "{}\n{}\n{}",
                v.url,
                v.title.as_deref().unwrap_or(""),
                v.path
            )
            .to_lowercase();
            words.iter().all(|word| haystack.contains(word))
        })
        .collect();
    Json(StudioValueSets { value_sets: found }).into_response()
}

fn codes(contains: &[Contains]) -> Vec<StudioCode> {
    contains
        .iter()
        .take(MOST_CODES)
        .map(StudioCode::from)
        .collect()
}

/// Documents and serves an expansion.
#[utoipa::path(post, path = "/api/studio/packages/{id}/terminology/expand", tag = "studio", operation_id = "expandStudioValueSet",
    description = "A ValueSet's expansion over what package version `id` is authored against: one held, by `url` (and `version`), or an unsaved one of this package sent as `valueSet`. For this package's own ValueSets it carries the change against the expansion in the package's previous published version: codes added, and codes removed, which new forms no longer offer. It also lists the form questions that bind it.",
    params(("id" = String, Path, description = "`name@version`")),
    request_body = StudioExpandInput,
    responses(
        (status = 200, description = "The expansion", body = StudioExpansion),
        (status = 400, description = "Neither a url nor a ValueSet", body = StudioProblem),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version, ValueSet or code system", body = StudioProblem),
        (status = 422, description = "The ValueSet cannot be read or expanded here", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn expand(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(input): Json<StudioExpandInput>,
) -> Response {
    let sources = match sources(&studio, subject, &id).await {
        Ok(sources) => sources,
        Err(response) => return *response,
    };
    let mut terminology = sources.current();
    let (url, version, own) = match &input.value_set {
        Some(resource) => {
            let read = match health_packages::terminology::value_set("valueSet", resource) {
                Ok(read) => read,
                Err(error) => {
                    return problem(StatusCode::UNPROCESSABLE_ENTITY, error.to_string(), None);
                }
            };
            let named = (read.url.clone(), read.version.clone());
            terminology.add_value_set(read);
            (named.0, named.1, true)
        }
        None => {
            let Some(url) = input.url.clone() else {
                return problem(
                    StatusCode::BAD_REQUEST,
                    "say which ValueSet: a url or the valueSet itself",
                    None,
                );
            };
            let own = sources
                .own
                .catalogue
                .entries
                .iter()
                .any(|e| e.resource_type == "ValueSet" && e.url == url);
            (url, input.version.clone(), own)
        }
    };
    let named = reference(&url, version.as_deref());
    let full = match terminology.expand(&named, &ExpandRequest::default()) {
        Ok(full) => full,
        Err(error) => return wrong(&error),
    };
    let request = ExpandRequest {
        filter: input.filter.clone().filter(|f| !f.trim().is_empty()),
        active_only: false,
        offset: input.offset.unwrap_or(0) as usize,
        count: Some((input.count.unwrap_or(CODES as u64) as usize).min(MOST_CODES)),
    };
    let page = match terminology.expand(&named, &request) {
        Ok(page) => page,
        Err(error) => return wrong(&error),
    };
    let change = if own {
        let published = sources.previous.as_ref().and_then(|previous| {
            previous
                .catalogue
                .entries
                .iter()
                .find(|e| e.resource_type == "ValueSet" && e.url == url)
                .map(|e| reference(&e.url, e.version.as_deref()))
        });
        let before = match &published {
            Some(published) => match sources
                .published()
                .expand(published, &ExpandRequest::default())
            {
                Ok(expansion) => expansion.contains,
                Err(error) => return wrong(&error),
            },
            None => Vec::new(),
        };
        let found = compare(&before, &full.contains);
        Some(StudioExpansionChange {
            published,
            added_total: found.added.len() as u64,
            added: codes(&found.added),
            removed_total: found.removed.len() as u64,
            removed: codes(&found.removed),
        })
    } else {
        None
    };
    let bound_by = sources
        .parts()
        .flat_map(|part| {
            part.catalogue
                .bindings
                .iter()
                .filter(|b| b.value_set.split('|').next() == Some(url.as_str()))
                .map(|b| StudioBinding {
                    package: part.id.clone(),
                    path: b.path.clone(),
                    questionnaire: b.questionnaire.clone(),
                    title: b.title.clone(),
                    link_id: b.link_id.clone(),
                    value_set: b.value_set.clone(),
                })
        })
        .collect();
    Json(StudioExpansion {
        value_set: page.value_set,
        total: page.total as u64,
        contains: codes(&page.contains),
        change,
        bound_by,
    })
    .into_response()
}
