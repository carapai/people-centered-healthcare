//! The facility registry's places at central (ADR 0094; #869), under `/api/places/`, acting as
//! the signed-in account:
//!
//! - `GET /area` answers the session's place and its area: every place below it, and the codes
//!   of the facilities in it, which a district's or region's figures roll up over
//!   (`health_analytics_app::roll_up`).
//! - `POST /nhfr/preview` reads an NHFR facility export (CSV) and answers what importing it
//!   would do: the regions and districts of the national core package not yet in the registry,
//!   and the facilities new, changed, closed, unchanged and absent. Nothing is written.
//! - `POST /nhfr/apply` does it, in one transaction, each place's write audited. Both need
//!   [`MANAGE_PERMISSION`], and both refuse (422) a file with a district the national core
//!   package does not hold. Both take the CSV as the body, or, from the client's JSON transport,
//!   as `{"csv": "…"}` with `Content-Type: application/json` (#873).

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_audit::{Agent, Auditor, Entity, Interaction};
use health_facility_registry::{ImportPlan, Places, parse_nhfr_csv, plan_import};
use health_persistence::{RawJson, Store as _, WriteError};
use health_publication::Publications;
use health_registries_app::AuditTrail;
use health_registries_app::places::{apply_import, places};
use health_surrealdb::{SurrealStore, SurrealTx};
use health_time::Timestamp;
use serde_json::{Value, json};

/// The use-case permission that imports the NHFR's facility list.
pub(crate) const MANAGE_PERMISSION: &str = "places.manage";

#[derive(Clone)]
struct PlacesApi {
    store: SurrealStore,
    publications: Option<Publications>,
    auditor: Arc<Auditor>,
}

/// The routes over `store`, reading the national core package's areas from `publications`
/// (without them, central has no packages and imports nothing). They expect the subject layer's
/// `Subject` and `Agent`.
pub(crate) fn router(
    store: SurrealStore,
    publications: Option<Publications>,
    auditor: Auditor,
) -> Router {
    Router::new()
        .route("/area", get(area))
        .route("/nhfr/preview", post(preview))
        .route("/nhfr/apply", post(apply))
        .with_state(PlacesApi {
            store,
            publications,
            auditor: Arc::new(auditor),
        })
}

fn problem(status: StatusCode, detail: impl Into<String>) -> Response {
    (status, Json(json!({ "detail": detail.into() }))).into_response()
}

fn refused(problems: Vec<String>) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "detail": "the NHFR file cannot be imported",
            "problems": problems,
        })),
    )
        .into_response()
}

fn unavailable(error: impl std::fmt::Display) -> Response {
    tracing::warn!(%error, "the facility registry is unavailable");
    problem(
        StatusCode::SERVICE_UNAVAILABLE,
        "the facility registry is unavailable",
    )
}

fn signed_in(subject: Option<Extension<Subject>>) -> Result<Subject, Box<Response>> {
    subject
        .map(|Extension(s)| s)
        .ok_or_else(|| Box::new(problem(StatusCode::UNAUTHORIZED, "sign in first")))
}

async fn registry(store: &SurrealStore) -> Result<Places, Box<Response>> {
    let mut tx = store.begin().await.map_err(|e| Box::new(unavailable(e)))?;
    places(&mut tx).await.map_err(|e| Box::new(unavailable(e)))
}

/// `GET /area`.
async fn area(State(api): State<PlacesApi>, subject: Option<Extension<Subject>>) -> Response {
    let subject = match signed_in(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let registry = match registry(&api.store).await {
        Ok(registry) => registry,
        Err(response) => return *response,
    };
    let code = subject.facility.as_str();
    let area = registry.area(code);
    // A facility the registry does not hold yet is still its own area, as before ADR 0094.
    let facilities = if area.is_empty() && !subject.area {
        vec![code.to_owned()]
    } else {
        registry.facilities_in(code)
    };
    Json(json!({
        "code": code,
        "level": registry.level_of(code).code(),
        "place": area.first(),
        "places": area.iter().skip(1).collect::<Vec<_>>(),
        "facilities": facilities,
    }))
    .into_response()
}

/// The CSV text of a request: the body itself, or its JSON's `csv` field.
fn csv_of(headers: &HeaderMap, body: &Bytes) -> Result<String, &'static str> {
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    if json {
        let value: Value = serde_json::from_slice(body).map_err(|_| "the body is not JSON")?;
        return value["csv"]
            .as_str()
            .map(str::to_owned)
            .ok_or("the body names no csv");
    }
    std::str::from_utf8(body)
        .map(str::to_owned)
        .map_err(|_| "the file is not UTF-8 text")
}

/// The plan of importing the body's NHFR export, or the refusal.
async fn plan(
    api: &PlacesApi,
    subject: &Subject,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<ImportPlan, Box<Response>> {
    if !subject.permissions.has(MANAGE_PERMISSION) {
        return Err(Box::new(problem(
            StatusCode::FORBIDDEN,
            "importing the NHFR needs places.manage",
        )));
    }
    let text = csv_of(headers, body).map_err(|p| Box::new(refused(vec![p.into()])))?;
    let rows = parse_nhfr_csv(&text).map_err(|p| Box::new(refused(p)))?;
    let Some(publications) = &api.publications else {
        return Err(Box::new(refused(vec!["central keeps no packages".into()])));
    };
    let areas = publications
        .administrative_areas()
        .await
        .map_err(|e| Box::new(unavailable(e)))?
        .ok_or_else(|| {
            Box::new(refused(vec![
                "no published package holds the regions and districts".into(),
            ]))
        })?;
    let registry = registry(&api.store).await?;
    plan_import(&registry, &areas, &rows).map_err(|r| Box::new(refused(r.problems)))
}

/// `POST /nhfr/preview`.
async fn preview(
    State(api): State<PlacesApi>,
    subject: Option<Extension<Subject>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let subject = match signed_in(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    match plan(&api, &subject, &headers, &body).await {
        Ok(plan) => Json(plan).into_response(),
        Err(response) => *response,
    }
}

/// `POST /nhfr/apply`.
async fn apply(
    State(api): State<PlacesApi>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let subject = match signed_in(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let plan = match plan(&api, &subject, &headers, &body).await {
        Ok(plan) => plan,
        Err(response) => return *response,
    };
    let audit = PlaceAudit {
        auditor: Arc::clone(&api.auditor),
        agent: agent.map_or_else(
            || Agent {
                who: None,
                display: format!("account {}", subject.account),
                client: None,
                network: None,
                purpose: Vec::new(),
                break_glass: false,
            },
            |Extension(a)| a,
        ),
    };
    match apply_import(&api.store, &audit, &subject.account, &plan).await {
        Ok(written) => Json(json!({ "written": written, "plan": plan })).into_response(),
        Err(health_registries_app::RegistryError::Conflict) => problem(
            StatusCode::CONFLICT,
            "the registry changed while importing; preview again",
        ),
        Err(error) => unavailable(error),
    }
}

/// Audits each place an import writes, as the signed-in account.
struct PlaceAudit {
    auditor: Arc<Auditor>,
    agent: Agent,
}

impl AuditTrail<SurrealTx> for PlaceAudit {
    async fn written(
        &self,
        tx: &mut SurrealTx,
        _actor: &str,
        _reason: &str,
        json: &RawJson,
    ) -> Result<(), WriteError> {
        let value: Value = serde_json::from_str(json.as_str()).unwrap_or(Value::Null);
        let Some(entity) = Entity::of(&value) else {
            return Ok(());
        };
        let event = self.auditor.event(
            &self.agent,
            Interaction::Update,
            &[entity],
            None,
            Timestamp::now(),
            None,
        );
        self.auditor.append(tx, &event).await.map(|_| ())
    }

    async fn recorded(
        &self,
        _tx: &mut SurrealTx,
        _actor: &str,
        _reason: &str,
        _what: &str,
    ) -> Result<(), WriteError> {
        Ok(())
    }
}
