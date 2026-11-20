//! The Studio's authoring API (studio.md § The authoring API; #492), at central only, under
//! `/api/studio/`, acting as the signed-in account. An edge or a desktop never mounts it: this
//! module is part of the central server alone.
//!
//! - `GET /packages` is the package list: one row per package, its newest version, with the
//!   actions the signed-in person may take and the reason for each one refused.
//! - `GET /packages/{id}` is one package version (`name@version`) read from top to bottom: what
//!   changed, the stored check results, who must act, and the history.
//! - `POST /packages/{id}/{action}` takes a lifecycle step (`submit`, `review`,
//!   `request-changes` with a `reason`, `approve`, `publish`, `activate` with a `scope`, and
//!   `check`, *Check now*) and answers the page as it is afterwards.
//!
//! The Draft Holder (#493): only the holder edits, checks, submits, hands over or withdraws a
//! draft. `hand-over` offers it `to` a username, `accept-handover` makes the person offered it
//! its holder, and `decline-handover` calls the offer off (the holder or the person offered).
//! `reassign`, with `metadata.manage`, gives it `to` a username at once, for a `reason`.
//! `withdraw` ends it and answers its page under the id it is kept as; its `name@version` is
//! free again. The lifecycle records account ids, so a username is looked up here.
//!
//! Editing a draft (#494):
//!
//! - `POST /drafts` starts one: a copy of a published version (`copyOf`) under the next minor
//!   version marked `-draft`, or a blank new package.
//! - The page and each file carry the draft's revision as an `ETag`. `GET`, `PUT` and `DELETE
//!   /packages/{id}/files/{*path}` read, write and remove one file; `PUT /packages/{id}/version`
//!   gives the draft another version. Each change must send `If-Match` with the revision it
//!   read: one that is missing or stale is refused with 412, and nothing is kept.
//!
//! The terminology browser (#523) reads under `/packages/{id}/terminology/` ([`terminology`]).
//!
//! The Workflow Studio (#498) reads the release's typed actions from `GET /workflow-actions`
//! ([`actions`]). Its decision-table editor (#516) tries a patient and runs its checks under
//! `/packages/{id}/decision-tables/` ([`decisions`]).
//!
//! The list of allowed actions is advisory. Each step is checked again: the permission here, and
//! the stage and the separation of duties by the lifecycle itself (ADR 0025).

pub(crate) mod actions;
pub(crate) mod decisions;
mod dto;
pub(crate) mod terminology;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_fhir_validation::OctofhirEngine;
use health_publication::editing::Blank;
use health_publication::studio::{AUTHOR_PERMISSION, PERMISSIONS, Refusal, StudioAction};
use health_publication::{Error, LocalSigner, Publications};

pub(crate) use dto::*;

/// The account id of the person with a username, `None` if there is none, `Err` if the account
/// store failed.
pub(crate) type Accounts = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<Option<String>, String>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub(crate) struct Studio {
    publications: Publications,
    signer: Option<Arc<LocalSigner>>,
    accounts: Accounts,
    /// The FHIRPath engine decision tables are tried on, built on first use.
    engine: Arc<OctofhirEngine>,
}

/// The Studio's routes over `publications`, publishing with `signer` when central has the
/// publication-signing key, and finding who a draft goes to by username in `accounts`. They
/// expect the subject layer's `Subject`.
pub(crate) fn router(
    publications: Publications,
    signer: Option<Arc<LocalSigner>>,
    accounts: Accounts,
) -> Router {
    Router::new()
        .route("/packages", get(list))
        .route("/packages/{id}", get(page))
        .route("/packages/{id}/{action}", post(act))
        .route(
            "/packages/{id}/files/{*path}",
            get(read_file).put(write_file).delete(remove_file),
        )
        .route("/packages/{id}/version", put(set_version))
        .route("/packages/{id}/note", put(set_note))
        .route(
            "/packages/{id}/terminology/code-systems",
            get(terminology::code_systems),
        )
        .route(
            "/packages/{id}/terminology/concepts",
            get(terminology::concepts),
        )
        .route(
            "/packages/{id}/terminology/concept",
            get(terminology::concept),
        )
        .route(
            "/packages/{id}/terminology/value-sets",
            get(terminology::list_value_sets),
        )
        .route(
            "/packages/{id}/terminology/expand",
            post(terminology::expand),
        )
        .route(
            "/packages/{id}/decision-tables/try",
            post(decisions::try_table),
        )
        .route(
            "/packages/{id}/decision-tables/check",
            post(decisions::check),
        )
        .route("/drafts", post(start))
        .route("/workflow-actions", get(actions::list))
        .with_state(Studio {
            publications,
            signer,
            accounts,
            engine: Arc::new(OctofhirEngine::new()),
        })
}

fn problem(status: StatusCode, detail: impl Into<String>, refusal: Option<&Refusal>) -> Response {
    let body = StudioProblem {
        detail: detail.into(),
        refusal: refusal.map(ActionRefusal::from),
    };
    (status, Json(body)).into_response()
}

/// The signed-in author, if their roles carry any Studio permission.
fn author(subject: Option<Extension<Subject>>) -> Result<Subject, Box<Response>> {
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(problem(
            StatusCode::UNAUTHORIZED,
            "the Studio needs a signed-in account",
            None,
        )));
    };
    if !PERMISSIONS.iter().any(|p| subject.permissions.has(p)) {
        return Err(Box::new(problem(
            StatusCode::FORBIDDEN,
            "the Studio needs a metadata permission",
            None,
        )));
    }
    Ok(subject)
}

fn failure(error: &Error) -> Response {
    match error {
        Error::NotFound(detail) => problem(StatusCode::NOT_FOUND, detail.clone(), None),
        Error::Refused(refused) => problem(
            StatusCode::CONFLICT,
            refused.to_string(),
            Some(&Refusal::from(refused)),
        ),
        Error::Conflict(detail) => problem(StatusCode::CONFLICT, detail.clone(), None),
        Error::Stale { .. } => problem(
            StatusCode::PRECONDITION_FAILED,
            "the draft changed since you read it: reload it before saving",
            None,
        ),
        Error::Invalid(detail) => problem(StatusCode::UNPROCESSABLE_ENTITY, detail.clone(), None),
        Error::Package(error) => problem(StatusCode::UNPROCESSABLE_ENTITY, error.to_string(), None),
        Error::Store(_) => {
            tracing::error!(error.kind = "store", "studio request failed");
            problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the package store failed",
                None,
            )
        }
    }
}

/// Documents and serves the package list.
#[utoipa::path(get, path = "/api/studio/packages", tag = "studio", operation_id = "listStudioPackages",
    description = "The Studio's package list (central only): one row per package, its newest version, with the actions the signed-in person may take and why each other one is refused. Needs a `metadata.*` or `configuration.activate` permission.",
    responses(
        (status = 200, description = "The packages", body = StudioPackageList),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn list(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
) -> Response {
    let subject = match author(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let has = |permission: &str| subject.permissions.has(permission);
    match studio
        .publications
        .studio_packages(&subject.account, &has)
        .await
    {
        Ok(rows) => Json(StudioPackageList {
            packages: rows.iter().map(StudioPackage::from).collect(),
        })
        .into_response(),
        Err(error) => failure(&error),
    }
}

/// Documents and serves one package version's page.
#[utoipa::path(get, path = "/api/studio/packages/{id}", tag = "studio", operation_id = "getStudioPackage",
    description = "One package version read from top to bottom: what changed against the previous published version, the stored check results, who must act and the history.",
    params(("id" = String, Path, description = "`name@version`")),
    responses(
        (status = 200, description = "The package version", body = StudioPackagePage, headers(("ETag" = String, description = "The draft's revision, for `If-Match`"))),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn page(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
) -> Response {
    let subject = match author(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let has = |permission: &str| subject.permissions.has(permission);
    match studio
        .publications
        .studio_package(&id, &subject.account, &has)
        .await
    {
        Ok(page) => tagged(page.revision, Json(StudioPackagePage::from(&page))),
        Err(error) => failure(&error),
    }
}

/// `body`, with the draft's `revision` as its `ETag`.
fn tagged(revision: u64, body: impl IntoResponse) -> Response {
    let mut response = body.into_response();
    if let Ok(value) = HeaderValue::from_str(&format!("\"{revision}\"")) {
        response.headers_mut().insert(header::ETAG, value);
    }
    response
}

/// The revision `If-Match` names: one strong or weak entity tag. Missing, `*`, or a list is
/// not a revision, so the change is refused (412).
fn if_match(headers: &HeaderMap) -> Option<u64> {
    let text = headers.get(header::IF_MATCH)?.to_str().ok()?.trim();
    let text = text.strip_prefix("W/").unwrap_or(text);
    text.strip_prefix('"')?.strip_suffix('"')?.parse().ok()
}

/// The signed-in account, if its roles carry the author permission that editing needs.
fn editor(subject: Option<Extension<Subject>>) -> Result<Subject, Box<Response>> {
    let subject = author(subject)?;
    if subject.permissions.has(AUTHOR_PERMISSION) {
        Ok(subject)
    } else {
        Err(Box::new(problem(
            StatusCode::FORBIDDEN,
            format!("editing a draft needs the {AUTHOR_PERMISSION} permission"),
            Some(&Refusal::permission(AUTHOR_PERMISSION)),
        )))
    }
}

/// The page of `id` for `subject` after a change, with its `ETag`; `status` for a new draft.
async fn answer(studio: &Studio, subject: &Subject, id: &str, status: StatusCode) -> Response {
    let has = |permission: &str| subject.permissions.has(permission);
    match studio
        .publications
        .studio_package(id, &subject.account, &has)
        .await
    {
        Ok(page) => tagged(
            page.revision,
            (status, Json(StudioPackagePage::from(&page))),
        ),
        Err(error) => failure(&error),
    }
}

fn precondition_failed() -> Response {
    problem(
        StatusCode::PRECONDITION_FAILED,
        "send If-Match with the draft's revision (its ETag) to change it",
        None,
    )
}

/// Documents and serves one file of a package version.
#[utoipa::path(get, path = "/api/studio/packages/{id}/files/{path}", tag = "studio", operation_id = "getStudioFile",
    description = "One file of a package version, for an editor, with the draft's revision as the `ETag` to send back as `If-Match` when saving.",
    params(
        ("id" = String, Path, description = "`name@version`"),
        ("path" = String, Path, description = "The file's path in the package, e.g. `Questionnaire-anc-visit.json`"),
    ),
    responses(
        (status = 200, description = "The file", body = StudioFile, headers(("ETag" = String, description = "The draft's revision"))),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
        (status = 404, description = "No such package version or file", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn read_file(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path((id, path)): Path<(String, String)>,
) -> Response {
    if let Err(response) = author(subject) {
        return *response;
    }
    match studio.publications.draft_files(&id).await {
        Ok((files, revision)) => match files.get(&path) {
            Some(bytes) => tagged(
                revision,
                Json(StudioFile {
                    path,
                    content: String::from_utf8_lossy(bytes).into_owned(),
                    revision,
                }),
            ),
            None => problem(
                StatusCode::NOT_FOUND,
                format!("{id} has no file {path}"),
                None,
            ),
        },
        Err(error) => failure(&error),
    }
}

/// Documents and saves one file of a draft.
#[utoipa::path(put, path = "/api/studio/packages/{id}/files/{path}", tag = "studio", operation_id = "saveStudioFile",
    description = "Writes one file of a draft (a new one is added) and repacks the draft. `If-Match` must carry the revision read (the `ETag`): a save without it, or after someone else's, is refused with 412 and nothing is kept. `package.json` keeps its name and version; the version changes through `PUT …/version`. Needs `metadata.author`. Answers the page as it is now.",
    params(
        ("id" = String, Path, description = "`name@version`"),
        ("path" = String, Path, description = "The file's path in the package"),
        ("If-Match" = String, Header, description = "The draft's revision, as its ETag"),
    ),
    request_body = StudioFileInput,
    responses(
        (status = 200, description = "Saved; the page as it is now", body = StudioPackagePage, headers(("ETag" = String, description = "The draft's new revision"))),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No metadata.author permission", body = StudioProblem),
        (status = 404, description = "No such package version", body = StudioProblem),
        (status = 409, description = "The draft is no longer in Draft", body = StudioProblem),
        (status = 412, description = "If-Match is missing or stale: reload", body = StudioProblem),
        (status = 422, description = "The file or its path cannot be saved in the package", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn write_file(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path((id, path)): Path<(String, String)>,
    headers: HeaderMap,
    Json(input): Json<StudioFileInput>,
) -> Response {
    let subject = match editor(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let Some(expected) = if_match(&headers) else {
        return precondition_failed();
    };
    let saved = studio
        .publications
        .save_file(
            &id,
            &path,
            Some(input.content.into_bytes()),
            &subject.account,
            expected,
        )
        .await;
    match saved {
        Ok(_) => answer(&studio, &subject, &id, StatusCode::OK).await,
        Err(error) => failure(&error),
    }
}

/// Documents and removes one file of a draft.
#[utoipa::path(delete, path = "/api/studio/packages/{id}/files/{path}", tag = "studio", operation_id = "removeStudioFile",
    description = "Removes one file of a draft and repacks it, with `If-Match` as for a save. `package.json` and the manifest stay. Needs `metadata.author`.",
    params(
        ("id" = String, Path, description = "`name@version`"),
        ("path" = String, Path, description = "The file's path in the package"),
        ("If-Match" = String, Header, description = "The draft's revision, as its ETag"),
    ),
    responses(
        (status = 200, description = "Removed; the page as it is now", body = StudioPackagePage, headers(("ETag" = String, description = "The draft's new revision"))),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No metadata.author permission", body = StudioProblem),
        (status = 404, description = "No such package version or file", body = StudioProblem),
        (status = 409, description = "The draft is no longer in Draft", body = StudioProblem),
        (status = 412, description = "If-Match is missing or stale: reload", body = StudioProblem),
        (status = 422, description = "That file cannot be removed", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn remove_file(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path((id, path)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let subject = match editor(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let Some(expected) = if_match(&headers) else {
        return precondition_failed();
    };
    match studio
        .publications
        .save_file(&id, &path, None, &subject.account, expected)
        .await
    {
        Ok(_) => answer(&studio, &subject, &id, StatusCode::OK).await,
        Err(error) => failure(&error),
    }
}

/// Documents and changes a draft's version.
#[utoipa::path(put, path = "/api/studio/packages/{id}/version", tag = "studio", operation_id = "setStudioVersion",
    description = "Gives a draft another version until it is submitted, where the semver check confirms it: `package.json` and the manifest take it, and the draft becomes `name@version` with its history. `If-Match` as for a save. Needs `metadata.author`. Answers the page under its new id.",
    params(
        ("id" = String, Path, description = "`name@version`"),
        ("If-Match" = String, Header, description = "The draft's revision, as its ETag"),
    ),
    request_body = StudioVersionInput,
    responses(
        (status = 200, description = "Renumbered; the page under its new id", body = StudioPackagePage, headers(("ETag" = String, description = "The draft's new revision"))),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No metadata.author permission", body = StudioProblem),
        (status = 404, description = "No such package version", body = StudioProblem),
        (status = 409, description = "Not in Draft, or that version is drafted or published already", body = StudioProblem),
        (status = 412, description = "If-Match is missing or stale: reload", body = StudioProblem),
        (status = 422, description = "Not an exact version", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn set_version(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<StudioVersionInput>,
) -> Response {
    let subject = match editor(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let Some(expected) = if_match(&headers) else {
        return precondition_failed();
    };
    match studio
        .publications
        .change_version(&id, input.version.trim(), &subject.account, expected)
        .await
    {
        Ok((renamed, _)) => answer(&studio, &subject, &renamed, StatusCode::OK).await,
        Err(error) => failure(&error),
    }
}

/// Documents and keeps a draft's change note.
#[utoipa::path(put, path = "/api/studio/packages/{id}/note", tag = "studio", operation_id = "setStudioNote",
    description = "Keeps the holder's one change note for the whole draft, shown in *What changed* above the generated summaries; an empty note clears it. It is not content: the fingerprint and the history stay as they were. `If-Match` as for a save. Needs `metadata.author`, and only the Draft Holder may.",
    params(
        ("id" = String, Path, description = "`name@version`"),
        ("If-Match" = String, Header, description = "The draft's revision, as its ETag"),
    ),
    request_body = StudioNoteInput,
    responses(
        (status = 200, description = "Kept; the page", body = StudioPackagePage, headers(("ETag" = String, description = "The draft's new revision"))),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No metadata.author permission", body = StudioProblem),
        (status = 404, description = "No such package version", body = StudioProblem),
        (status = 409, description = "Not in Draft, or not the Draft Holder", body = StudioProblem),
        (status = 412, description = "If-Match is missing or stale: reload", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn set_note(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<StudioNoteInput>,
) -> Response {
    let subject = match editor(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let Some(expected) = if_match(&headers) else {
        return precondition_failed();
    };
    match studio
        .publications
        .set_note(&id, input.note, &subject.account, expected)
        .await
    {
        Ok(_) => answer(&studio, &subject, &id, StatusCode::OK).await,
        Err(error) => failure(&error),
    }
}

/// Documents and starts a draft.
#[utoipa::path(post, path = "/api/studio/drafts", tag = "studio", operation_id = "startStudioDraft",
    description = "Starts a draft as the signed-in account: with `copyOf`, a copy of that published version under the next minor version marked `-draft` (`2.0.0` gives `2.1.0-draft`), its artefacts as they were; otherwise a blank new package from `name`, `kind`, `title`, `version` and `canonical`. Needs `metadata.author`.",
    request_body = StudioNewDraft,
    responses(
        (status = 201, description = "Started; its page", body = StudioPackagePage, headers(("ETag" = String, description = "The draft's revision"))),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No metadata.author permission", body = StudioProblem),
        (status = 404, description = "`copyOf` is not published", body = StudioProblem),
        (status = 409, description = "That version is drafted or published already", body = StudioProblem),
        (status = 422, description = "A field is missing or not valid", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
    ))]
pub(crate) async fn start(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Json(input): Json<StudioNewDraft>,
) -> Response {
    let subject = match editor(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let by = subject.account.as_str();
    let started = match input.copy_of.as_deref().map(str::trim) {
        Some(copy_of) if !copy_of.is_empty() => studio.publications.start_copy(copy_of, by).await,
        _ => {
            let blank = match input.blank() {
                Ok(blank) => blank,
                Err(detail) => return problem(StatusCode::UNPROCESSABLE_ENTITY, detail, None),
            };
            studio.publications.start_blank(&blank, by).await
        }
    };
    match started {
        Ok(lifecycle) => {
            let id = health_publication::lifecycle_id(&lifecycle);
            answer(&studio, &subject, &id, StatusCode::CREATED).await
        }
        Err(error) => failure(&error),
    }
}

impl StudioNewDraft {
    /// A blank draft from these fields, or what is missing.
    fn blank(&self) -> Result<Blank, String> {
        let text = |value: &Option<String>, what: &str| {
            value
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| format!("a blank draft needs its {what}"))
        };
        let kind = self.kind.ok_or("a blank draft needs its kind")?;
        let version = text(&self.version, "version")?;
        Ok(Blank {
            name: text(&self.name, "name")?,
            kind: kind.into(),
            title: text(&self.title, "title")?,
            version: version.parse().map_err(|e| format!("the version: {e}"))?,
            canonical: text(&self.canonical, "canonical base URL")?,
            min_software: env!("CARGO_PKG_VERSION").to_owned(),
        })
    }
}

/// Documents and takes one lifecycle step.
#[utoipa::path(post, path = "/api/studio/packages/{id}/{action}", tag = "studio", operation_id = "takeStudioAction",
    description = "Takes one lifecycle step as the signed-in account, checked again here whatever the list said: the permission, the stage, the Draft Holder and the separation of duties. `request-changes` needs a `reason`, `activate` a `scope` (`national`, `district:<code>`, `facility:<code>` or `program:<facility>:<program>`). `check` (*Check now*) runs validation and simulation on a draft and keeps the results, leaving it in Draft. `hand-over` offers the draft `to` a username, who holds it once they `accept-handover`; `decline-handover` calls the offer off. `reassign` (`metadata.manage`) gives it `to` a username at once, for a `reason`. `withdraw` ends an unpublished draft and keeps it under another id, freeing its `name@version`. Answers the page as it is afterwards, under the id it is kept as.",
    params(
        ("id" = String, Path, description = "`name@version`"),
        ("action" = StudioActionName, Path, description = "The step"),
    ),
    request_body = StudioActionInput,
    responses(
        (status = 200, description = "Done; the page as it is now", body = StudioPackagePage),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "The step's permission is missing", body = StudioProblem),
        (status = 404, description = "No such package version or step", body = StudioProblem),
        (status = 409, description = "The lifecycle refused the step, or it changed meanwhile", body = StudioProblem),
        (status = 422, description = "A reason, a scope or who it goes to is missing or not valid", body = StudioProblem),
        (status = 500, description = "The package store failed", body = StudioProblem),
        (status = 503, description = "Central has no publication-signing key", body = StudioProblem),
    ))]
pub(crate) async fn act(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
    Path((id, action)): Path<(String, String)>,
    input: Option<Json<StudioActionInput>>,
) -> Response {
    let subject = match author(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let Some(action) = StudioActionName::parse(&action) else {
        return problem(StatusCode::NOT_FOUND, format!("no step {action}"), None);
    };
    let action = StudioAction::from(action);
    let permission = action.permission();
    if !subject.permissions.has(permission) {
        return problem(
            StatusCode::FORBIDDEN,
            format!("this step needs the {permission} permission"),
            Some(&Refusal::permission(permission)),
        );
    }
    let input = input.map(|Json(input)| input).unwrap_or_default();
    let by = subject.account.as_str();
    let publications = &studio.publications;
    let mut id = id;
    let reason = input.reason.as_deref().unwrap_or("").trim().to_owned();
    let to = match action {
        StudioAction::HandOver | StudioAction::Reassign => {
            match recipient(&studio, input.to.as_deref()).await {
                Ok(to) => to,
                Err(response) => return *response,
            }
        }
        _ => String::new(),
    };
    let done = match action {
        StudioAction::HandOver => publications.hand_over(&id, by, &to).await.map(|_| ()),
        StudioAction::AcceptHandover => publications.accept_handover(&id, by).await.map(|_| ()),
        StudioAction::DeclineHandover => publications.decline_handover(&id, by).await.map(|_| ()),
        StudioAction::Reassign => {
            if reason.is_empty() {
                return problem(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "say why the draft is reassigned",
                    None,
                );
            }
            publications
                .reassign(&id, by, &to, &reason)
                .await
                .map(|_| ())
        }
        StudioAction::Withdraw => publications.withdraw(&id, by).await.map(|(kept, _)| {
            id = kept;
        }),
        StudioAction::Submit => publications.submit(&id, by).await.map(|_| ()),
        StudioAction::Check => publications.check_now(&id, by).await.map(|_| ()),
        StudioAction::Review => publications.review(&id, by).await.map(|_| ()),
        StudioAction::Approve => publications.approve(&id, by).await.map(|_| ()),
        StudioAction::RequestChanges => {
            if reason.is_empty() {
                return problem(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "say what needs to change",
                    None,
                );
            }
            publications
                .request_changes(&id, by, &reason)
                .await
                .map(|_| ())
        }
        StudioAction::Publish => {
            let Some(signer) = &studio.signer else {
                return problem(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "central has no publication-signing key (HEALTH_PUBLICATION_KEY_FILE)",
                    None,
                );
            };
            publications
                .publish(&id, by, signer.as_ref())
                .await
                .map(|_| ())
        }
        StudioAction::Activate => {
            let Some((package, version)) = id
                .rsplit_once('@')
                .and_then(|(name, version)| Some((name, version.parse().ok()?)))
            else {
                return problem(
                    StatusCode::NOT_FOUND,
                    format!("{id} is not name@version"),
                    None,
                );
            };
            let scope = match crate::package::scope(input.scope.as_deref().unwrap_or("")) {
                Ok(scope) => scope,
                Err(_) => {
                    return problem(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "the scope is national, district:<code>, facility:<code> or program:<facility>:<program>",
                        None,
                    );
                }
            };
            let reason = Some(reason.as_str()).filter(|r| !r.is_empty());
            // Only a published version is activated; the lifecycle says whether it is.
            match publications.lifecycle(&id).await {
                Ok((lifecycle, _))
                    if lifecycle.stage != health_configuration::lifecycle::Stage::Published =>
                {
                    let refusal = Refusal::not_published();
                    return problem(
                        StatusCode::CONFLICT,
                        refusal.message.clone(),
                        Some(&refusal),
                    );
                }
                Ok(_) => {}
                Err(error) => return failure(&error),
            }
            publications
                .activate(package, &version, scope, None, by, reason)
                .await
                .map(|_| ())
        }
    };
    if let Err(error) = done {
        return failure(&error);
    }
    answer(&studio, &subject, &id, StatusCode::OK).await
}

/// The account id of the person a draft goes to, by the username given.
async fn recipient(studio: &Studio, to: Option<&str>) -> Result<String, Box<Response>> {
    let username = to
        .map(str::trim)
        .filter(|to| !to.is_empty())
        .ok_or_else(|| {
            Box::new(problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "say who the draft goes to, by their username",
                None,
            ))
        })?;
    match (studio.accounts)(username.to_owned()).await {
        Ok(Some(account)) => Ok(account),
        Ok(None) => Err(Box::new(problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("no account is named {username}"),
            None,
        ))),
        Err(_) => {
            tracing::error!(error.kind = "store", "the account store failed");
            Err(Box::new(problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the account store failed",
                None,
            )))
        }
    }
}

#[cfg(test)]
mod tests;
