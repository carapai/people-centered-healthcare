//! The Studio's authoring API (studio.md § The authoring API; #492), at central only, under
//! `/api/studio/`, acting as the signed-in account. An edge or a desktop never mounts it: this
//! module is part of the central server alone.
//!
//! - `GET /packages` is the package list: one row per package, its newest version, with the
//!   actions the signed-in person may take and the reason for each one refused.
//! - `GET /packages/{id}` is one package version (`name@version`) read from top to bottom: what
//!   changed, the stored check results, who must act, and the history.
//! - `POST /packages/{id}/{action}` takes a lifecycle step (`submit`, `review`,
//!   `request-changes` with a `reason`, `approve`, `publish`, `activate` with a `scope`) and
//!   answers the page as it is afterwards.
//!
//! The list of allowed actions is advisory. Each step is checked again: the permission here, and
//! the stage and the separation of duties by the lifecycle itself (ADR 0025).

mod dto;

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_publication::studio::{PERMISSIONS, Refusal, StudioAction};
use health_publication::{Error, LocalSigner, Publications};

pub(crate) use dto::*;

#[derive(Clone)]
pub(crate) struct Studio {
    publications: Publications,
    signer: Option<Arc<LocalSigner>>,
}

/// The Studio's routes over `publications`, publishing with `signer` when central has the
/// publication-signing key. They expect the subject layer's `Subject`.
pub(crate) fn router(publications: Publications, signer: Option<Arc<LocalSigner>>) -> Router {
    Router::new()
        .route("/packages", get(list))
        .route("/packages/{id}", get(page))
        .route("/packages/{id}/{action}", post(act))
        .with_state(Studio {
            publications,
            signer,
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
        (status = 200, description = "The package version", body = StudioPackagePage),
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
        Ok(page) => Json(StudioPackagePage::from(&page)).into_response(),
        Err(error) => failure(&error),
    }
}

/// Documents and takes one lifecycle step.
#[utoipa::path(post, path = "/api/studio/packages/{id}/{action}", tag = "studio", operation_id = "takeStudioAction",
    description = "Takes one lifecycle step as the signed-in account, checked again here whatever the list said: the permission, the stage and the separation of duties. `request-changes` needs a `reason`, `activate` a `scope` (`national`, `district:<code>`, `facility:<code>` or `program:<facility>:<program>`). Answers the page as it is afterwards.",
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
        (status = 422, description = "A reason or a scope is missing or not valid", body = StudioProblem),
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
    let done = match action {
        StudioAction::Submit => publications.submit(&id, by).await.map(|_| ()),
        StudioAction::Review => publications.review(&id, by).await.map(|_| ()),
        StudioAction::Approve => publications.approve(&id, by).await.map(|_| ()),
        StudioAction::RequestChanges => {
            let reason = input.reason.as_deref().unwrap_or("").trim();
            if reason.is_empty() {
                return problem(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "say what needs to change",
                    None,
                );
            }
            publications
                .request_changes(&id, by, reason)
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
            let reason = input
                .reason
                .as_deref()
                .map(str::trim)
                .filter(|r| !r.is_empty());
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
    let has = |permission: &str| subject.permissions.has(permission);
    match publications.studio_package(&id, by, &has).await {
        Ok(page) => Json(StudioPackagePage::from(&page)).into_response(),
        Err(error) => failure(&error),
    }
}

#[cfg(test)]
mod tests;
