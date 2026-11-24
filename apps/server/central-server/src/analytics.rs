//! The data explorer at central (analytics.md § Dashboards; ADR 0075, ADR 0094; #599), under
//! `/api/analytics/`, for district, regional and national users (and a facility's, online):
//!
//! - `GET /explorer` answers what the session may explore: its place and every place below it
//!   (its area), the published indicators, and the standard period types.
//! - `POST /explore` answers a question (`health_analytics_app::explore::read_question`): each
//!   indicator's figure for each period and place, rolled up from the summaries the facilities'
//!   edges synced, with the facilities reported out of expected and the missing ones with their
//!   last sync, never counted as zero; a median or a distinct count above facility level has no
//!   figure and says why. A place outside the session's area is refused (403).
//!
//!   An indicator whose denominator is a Population Estimate type (#553; ADR 0071) is each
//!   place's summed count over the place's estimate for the period, pro rata, from the newest
//!   published version of the year's Population package, the figures summed upward through the
//!   facility registry's hierarchy; a place below the figures given has *no estimate*.
//!
//! - `GET /map` (#604) answers what a map of the area draws: each district's and region's
//!   boundary from the national core package, and each open facility's NHFR coordinates.
//!
//! They read summaries only, never patient-level records, and need [`VIEW_PERMISSION`].
//!
//! Saved Visualizations and Dashboards (#602; ADR 0075), user content kept here at central:
//!
//! - `GET /dashboards` lists the Dashboards the session may open (its own, and those shared with
//!   it as a person, by a role it holds, or by its place's level), each with its Visualizations,
//!   the session's own Visualizations on none of them, and whether it may author.
//! - `POST /dashboards` makes one (a title and shares) and `PUT /dashboards/{id}` changes its
//!   title and shares, its owner only.
//! - `POST /visualizations` saves the explorer's setup with a title, optionally onto one of the
//!   session's own Dashboards. Its places must be in the session's area; one that follows the
//!   viewer shows each viewer their own place, or each area under it.
//!
//! Each needs [`VIEW_PERMISSION`], and every change [`AUTHOR_PERMISSION`] too. Sharing never
//! widens what a viewer reads: the explorer still answers only for their own area.
//!
//! Reports (#557; ADR 0070, ADR 0072) are run here for any place in the session's area at a level
//! the Report runs for, over the facilities' synced summaries, so running one is instant:
//!
//! - `GET /reports` lists the Reports of the newest published version of each Reporting package,
//!   each with the places in the session's area it may be run for.
//! - `POST /reports/run` runs one (`{"report": "<package>:<id>", "place": "...", "period":
//!   "2026-03"}`): each table row's figure for the period and, with % change, the one before,
//!   the whole and each band, rolled up from summed parts with the completeness of the period's
//!   figures (`health_analytics_app::reports`). A place outside the area is refused (403), and
//!   a line list has no rows here: it is patient-level, so it runs on the facility's edge.
//!
//! Standard Dashboards (#603; ADR 0075) ship in Reporting packages: `GET /dashboards` lists, to
//! every session, those of the newest published version of each package, first, their periods
//! as of today and their places following the viewer. They are never changed here (a new
//! package version changes them); `POST /dashboards` with `copyOf` *saves as* the session's own
//! Dashboard, its Visualizations copied as the session sees them.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_analytics::{
    AgeBandSet, EstimatesDefinition, Indicator, Period, PopulationEstimates, STANDARD_PERIODS,
    SavedDashboard, SavedDashboards as _, SavedVisualization, Share, Viewer,
};
use health_analytics_app::dashboards::{
    AUTHOR_PERMISSION, EACH_AREA_UNDER, MOST_ITEMS, copied_setup, dashboard_json, read_setup,
    read_shares, read_title, standard_id, standard_json, unplaced_json,
};
use health_analytics_app::explore::{
    ExplorePlace, cells_json, explore, indicator_json, periods_json, read_question,
};
use health_analytics_app::reports::{RunPlace, RunWith, level_of, listed_json, run_report};
use health_auth::SessionRoles;
use health_facility_registry::{AdministrativeBoundaries, Place, PlaceLevel, Places};
use health_persistence::{Store as _, Transaction as _};
use health_publication::{Publications, PublishedDashboard, PublishedReport};
use health_registries_app::places::places;
use health_surrealdb::SurrealStore;
use health_time::Clock;
use jiff::civil::Date;
use jiff::tz::TimeZone;

use crate::studio::Accounts;
use serde_json::{Value, json};

/// The use-case permission that reads Indicator Reports (ADR 0070): Reports and dashboards.
pub(crate) const VIEW_PERMISSION: &str = "reporting.view";

#[derive(Clone)]
struct AnalyticsApi {
    store: SurrealStore,
    catalogue: Catalogue,
    accounts: Accounts,
    clock: Arc<dyn Clock>,
}

/// The routes over `store`, offering `catalogue`'s indicators and standard Dashboards (their
/// periods as of `clock`'s today), sharing with the people `accounts` finds by username. They
/// expect the subject layer's `Subject` and `SessionRoles`.
pub(crate) fn router(
    store: SurrealStore,
    catalogue: Catalogue,
    accounts: Accounts,
    clock: Arc<dyn Clock>,
) -> Router {
    Router::new()
        .route("/explorer", get(explorer))
        .route("/explore", post(explore_route))
        .route("/map", get(map))
        .route("/dashboards", get(dashboards).post(new_dashboard))
        .route("/dashboards/{id}", put(change_dashboard))
        .route("/visualizations", post(save_visualization))
        .route("/reports", get(reports))
        .route("/reports/run", post(run_report_route))
        .with_state(AnalyticsApi {
            store,
            catalogue,
            accounts,
            clock,
        })
}

fn problem(status: StatusCode, detail: impl Into<String>) -> Response {
    (status, Json(json!({ "detail": detail.into() }))).into_response()
}

fn unavailable(error: impl std::fmt::Display) -> Response {
    tracing::warn!(%error, "the explorer cannot read its summaries");
    problem(
        StatusCode::SERVICE_UNAVAILABLE,
        "the indicator summaries are unavailable",
    )
}

/// The signed-in subject allowed to explore, or the refusal.
fn viewer(subject: Option<Extension<Subject>>) -> Result<Subject, Box<Response>> {
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(problem(StatusCode::UNAUTHORIZED, "sign in first")));
    };
    if !subject.permissions.has(VIEW_PERMISSION) {
        return Err(Box::new(problem(
            StatusCode::FORBIDDEN,
            "exploring indicators needs reporting.view",
        )));
    }
    Ok(subject)
}

/// The session's area: its place first, then every place below it. A facility the registry does
/// not hold yet is still its own area, as before ADR 0094.
fn area(registry: &Places, subject: &Subject) -> Vec<Place> {
    let code = subject.facility.as_str();
    let area: Vec<Place> = registry.area(code).into_iter().cloned().collect();
    if area.is_empty() && !subject.area {
        return vec![Place {
            code: code.to_owned(),
            name: code.to_owned(),
            level: PlaceLevel::Facility,
            parent: None,
            active: true,
            care_level: None,
            ownership: None,
            coordinates: None,
        }];
    }
    area
}

fn place_json(place: &Place) -> Value {
    json!({
        "code": place.code,
        "name": place.name,
        "level": place.level.code(),
        "parent": place.parent,
    })
}

async fn registry(store: &SurrealStore) -> Result<Places, Box<Response>> {
    let mut tx = store.begin().await.map_err(|e| Box::new(unavailable(e)))?;
    places(&mut tx).await.map_err(|e| Box::new(unavailable(e)))
}

/// `GET /explorer`.
async fn explorer(
    State(api): State<AnalyticsApi>,
    subject: Option<Extension<Subject>>,
) -> Response {
    let subject = match viewer(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let registry = match registry(&api.store).await {
        Ok(registry) => registry,
        Err(response) => return *response,
    };
    let indicators = match api.catalogue.indicators().await {
        Ok(indicators) => indicators,
        Err(error) => return unavailable(error),
    };
    let area = area(&registry, &subject);
    Json(json!({
        "where": "central",
        "place": area.first().map(place_json),
        "places": area.iter().skip(1).map(place_json).collect::<Vec<_>>(),
        "indicators": indicators.iter().map(indicator_json).collect::<Vec<_>>(),
        "periodTypes": STANDARD_PERIODS,
    }))
    .into_response()
}

/// `POST /explore`.
async fn explore_route(
    State(api): State<AnalyticsApi>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<Value>,
) -> Response {
    let subject = match viewer(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let offered = match api.catalogue.indicators().await {
        Ok(indicators) => indicators,
        Err(error) => return unavailable(error),
    };
    let question = match read_question(&body, &offered) {
        Ok(question) => question,
        Err(why) => return problem(StatusCode::UNPROCESSABLE_ENTITY, why),
    };
    let registry = match registry(&api.store).await {
        Ok(registry) => registry,
        Err(response) => return *response,
    };
    let area = area(&registry, &subject);
    let mut picked = Vec::new();
    for code in &question.places {
        let Some(place) = area.iter().find(|p| p.code == *code) else {
            return problem(StatusCode::FORBIDDEN, format!("{code} is not in your area"));
        };
        let facilities = if place.level == PlaceLevel::Facility {
            vec![place.code.clone()]
        } else {
            registry.facilities_in(&place.code)
        };
        picked.push(ExplorePlace {
            code: place.code.clone(),
            facilities,
        });
    }
    let estimates = if question.indicators.iter().any(|i| i.estimate.is_some()) {
        match api.catalogue.estimates().await {
            Ok(years) => estimates_over(years, &registry),
            Err(error) => return unavailable(error),
        }
    } else {
        PopulationEstimates::default()
    };
    let mut tx = match api.store.begin().await {
        Ok(tx) => tx,
        Err(error) => return unavailable(error),
    };
    let cells = match explore(
        &mut tx,
        &question.indicators,
        &question.periods,
        &picked,
        &estimates,
    )
    .await
    {
        Ok(cells) => cells,
        Err(error) => return unavailable(error),
    };
    let names: BTreeMap<String, String> = registry
        .iter()
        .map(|p| (p.code.clone(), p.name.clone()))
        .collect();
    Json(json!({
        "where": "central",
        "periods": periods_json(&question.periods),
        "cells": cells_json(&cells, &names, question.disaggregation.as_deref()),
    }))
    .into_response()
}

/// `years` of Population Estimates over the facility registry's hierarchy: each figure summed
/// up through the places above it.
fn estimates_over(years: Vec<EstimatesDefinition>, registry: &Places) -> PopulationEstimates {
    let parents: BTreeMap<String, String> = registry
        .iter()
        .filter_map(|p| p.parent.clone().map(|parent| (p.code.clone(), parent)))
        .collect();
    PopulationEstimates::new(years, |code| parents.get(code).cloned())
}

/// `GET /map` (#604): what a map of the session's area draws, from the facility registry and the
/// national core package: the boundary of each district and region in the area that has one, and
/// each open facility in it with its NHFR coordinates. The figures come from `POST /explore`.
async fn map(State(api): State<AnalyticsApi>, subject: Option<Extension<Subject>>) -> Response {
    let subject = match viewer(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let registry = match registry(&api.store).await {
        Ok(registry) => registry,
        Err(response) => return *response,
    };
    let boundaries = match api.catalogue.boundaries().await {
        Ok(boundaries) => boundaries.unwrap_or_default(),
        Err(error) => return unavailable(error),
    };
    Json(map_json(&area(&registry, &subject), &boundaries)).into_response()
}

/// The map of `area`: its regions' and districts' `boundaries`, and its open facilities' points.
pub(crate) fn map_json(area: &[Place], boundaries: &AdministrativeBoundaries) -> Value {
    // Regions first, so a district is drawn over its region.
    let areas: Vec<Value> = [PlaceLevel::Region, PlaceLevel::District]
        .iter()
        .flat_map(|level| area.iter().filter(move |p| p.level == *level))
        .filter_map(|p| {
            let boundary = boundaries.of(&p.code)?;
            Some(json!({
                "code": p.code,
                "name": p.name,
                "level": p.level.code(),
                "geometry": boundary.geometry,
            }))
        })
        .collect();
    let facilities: Vec<Value> = area
        .iter()
        .filter(|p| p.level == PlaceLevel::Facility && p.active)
        .filter_map(|p| {
            let at = p.coordinates.as_ref()?;
            Some(json!({
                "code": p.code,
                "name": p.name,
                "parent": p.parent,
                "latitude": at.latitude,
                "longitude": at.longitude,
            }))
        })
        .collect();
    json!({ "areas": areas, "facilities": facilities })
}

/// The place levels a Dashboard may be shared with.
const LEVELS: [PlaceLevel; 4] = [
    PlaceLevel::Facility,
    PlaceLevel::District,
    PlaceLevel::Region,
    PlaceLevel::National,
];

/// Who is looking, at their place's level.
fn viewer_of(subject: &Subject, roles: Option<Extension<SessionRoles>>, area: &[Place]) -> Viewer {
    Viewer {
        account: subject.account.clone(),
        roles: roles.map(|Extension(r)| r.0).unwrap_or_default(),
        level: area
            .first()
            .map_or(PlaceLevel::Facility, |p| p.level)
            .code()
            .to_owned(),
    }
}

/// The session's area and who it is, or the refusal.
async fn looking(
    api: &AnalyticsApi,
    subject: Option<Extension<Subject>>,
    roles: Option<Extension<SessionRoles>>,
    author: bool,
) -> Result<(Subject, Vec<Place>, Viewer), Box<Response>> {
    let subject = viewer(subject)?;
    if author && !subject.permissions.has(AUTHOR_PERMISSION) {
        return Err(Box::new(problem(
            StatusCode::FORBIDDEN,
            "saving Visualizations and Dashboards needs analytics.author",
        )));
    }
    let registry = registry(&api.store).await?;
    let area = area(&registry, &subject);
    let viewer = viewer_of(&subject, roles, &area);
    Ok((subject, area, viewer))
}

/// Every saved Dashboard and Visualization.
async fn saved(
    store: &SurrealStore,
) -> Result<(Vec<SavedDashboard>, Vec<SavedVisualization>), Box<Response>> {
    let refused = |e: health_persistence::StoreError| Box::new(unavailable(e));
    let mut tx = store.begin().await.map_err(refused)?;
    let dashboards = tx.dashboards().await.map_err(refused)?;
    let visualizations = tx.visualizations().await.map_err(refused)?;
    Ok((dashboards, visualizations))
}

/// `GET /dashboards`.
async fn dashboards(
    State(api): State<AnalyticsApi>,
    subject: Option<Extension<Subject>>,
    roles: Option<Extension<SessionRoles>>,
) -> Response {
    let (subject, _, viewer) = match looking(&api, subject, roles, false).await {
        Ok(looking) => looking,
        Err(response) => return *response,
    };
    let (dashboards, visualizations) = match saved(&api.store).await {
        Ok(saved) => saved,
        Err(response) => return *response,
    };
    let standard = match standard(&api).await {
        Ok(standard) => standard,
        Err(response) => return *response,
    };
    let saved = dashboards
        .iter()
        .filter(|d| d.visible_to(&viewer))
        .map(|d| dashboard_json(d, &visualizations, &viewer));
    Json(json!({
        "where": "central",
        "canAuthor": subject.permissions.has(AUTHOR_PERMISSION),
        "dashboards": standard.into_iter().chain(saved).collect::<Vec<_>>(),
        "visualizations": unplaced_json(&dashboards, &visualizations, &viewer),
    }))
    .into_response()
}

/// Today, which standard Dashboards' periods run up to.
fn today(clock: &dyn Clock) -> Date {
    clock.now().to_zoned(TimeZone::UTC).date()
}

/// The standard Dashboards every session reads, as of today.
async fn standard(api: &AnalyticsApi) -> Result<Vec<Value>, Box<Response>> {
    let published = api
        .catalogue
        .dashboards()
        .await
        .map_err(|e| Box::new(unavailable(e)))?;
    let today = today(api.clock.as_ref());
    Ok(published
        .iter()
        .map(|p| standard_json(&p.package, &p.version, &p.dashboard, today))
        .collect())
}

/// The refusal to change standard Dashboard `id`, if it is one.
async fn not_standard(api: &AnalyticsApi, id: &str) -> Result<(), Box<Response>> {
    let published = api
        .catalogue
        .dashboards()
        .await
        .map_err(|e| Box::new(unavailable(e)))?;
    if published
        .iter()
        .any(|p| standard_id(&p.package, &p.dashboard.id) == id)
    {
        return Err(Box::new(problem(
            StatusCode::FORBIDDEN,
            "a standard Dashboard changes only in a new version of its Reporting package: Save as makes your own copy",
        )));
    }
    Ok(())
}

/// The shares `body` asks for, the people named found by username.
async fn shares_of(api: &AnalyticsApi, body: &Value) -> Result<Vec<Share>, Box<Response>> {
    let levels = LEVELS.map(PlaceLevel::code);
    let asked = read_shares(body, "shares", &levels)
        .map_err(|why| Box::new(problem(StatusCode::UNPROCESSABLE_ENTITY, why)))?;
    let mut shares = asked.shares;
    for username in asked.people {
        match (api.accounts)(username.clone()).await {
            Ok(Some(account)) => shares.push(Share::Person { account, username }),
            Ok(None) => {
                return Err(Box::new(problem(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("nobody is signed up as {username}"),
                )));
            }
            Err(error) => return Err(Box::new(unavailable(error))),
        }
    }
    Ok(shares)
}

async fn put_dashboard(
    store: &SurrealStore,
    dashboard: &SavedDashboard,
) -> Result<(), Box<Response>> {
    let refused = |e: health_persistence::WriteError| Box::new(unavailable(e));
    let mut tx = store.begin().await.map_err(|e| Box::new(unavailable(e)))?;
    tx.put_dashboard(dashboard).await.map_err(refused)?;
    tx.commit().await.map_err(refused)
}

/// `POST /dashboards`: `{title, shares, copyOf?}`; `copyOf` names a standard Dashboard whose
/// Visualizations it copies (*Save as*).
async fn new_dashboard(
    State(api): State<AnalyticsApi>,
    subject: Option<Extension<Subject>>,
    roles: Option<Extension<SessionRoles>>,
    Json(body): Json<Value>,
) -> Response {
    let (_, area, viewer) = match looking(&api, subject, roles, true).await {
        Ok(looking) => looking,
        Err(response) => return *response,
    };
    let title = match read_title(&body, "title") {
        Ok(title) => title,
        Err(why) => return problem(StatusCode::UNPROCESSABLE_ENTITY, why),
    };
    let shares = match shares_of(&api, &body).await {
        Ok(shares) => shares,
        Err(response) => return *response,
    };
    let copies = match &body["copyOf"] {
        Value::Null => Vec::new(),
        Value::String(id) => match copies_of(&api, id, &area, &viewer).await {
            Ok(copies) => copies,
            Err(response) => return *response,
        },
        _ => return problem(StatusCode::UNPROCESSABLE_ENTITY, "`copyOf` is not an id"),
    };
    let dashboard = SavedDashboard {
        id: uuid::Uuid::now_v7().to_string(),
        owner: viewer.account.clone(),
        title,
        shares,
        items: copies.iter().map(|v| v.id.clone()).collect(),
    };
    let refused = |e: health_persistence::WriteError| unavailable(e);
    let mut tx = match api.store.begin().await {
        Ok(tx) => tx,
        Err(error) => return unavailable(error),
    };
    for copy in &copies {
        if let Err(error) = tx.put_visualization(copy).await {
            return refused(error);
        }
    }
    if let Err(error) = tx.put_dashboard(&dashboard).await {
        return refused(error);
    }
    if let Err(error) = tx.commit().await {
        return refused(error);
    }
    (
        StatusCode::CREATED,
        Json(dashboard_json(&dashboard, &copies, &viewer)),
    )
        .into_response()
}

/// The places a copied Visualization that follows its viewer starts at, as `area`'s session
/// sees it: their own place, or each area directly under it (their own place when none is).
fn own_places(follow: Option<&str>, area: &[Place]) -> Vec<String> {
    let Some(own) = area.first() else {
        return Vec::new();
    };
    let under: Vec<String> = area
        .iter()
        .filter(|p| p.parent.as_deref() == Some(own.code.as_str()))
        .map(|p| p.code.clone())
        .collect();
    if follow == Some(EACH_AREA_UNDER) && !under.is_empty() {
        under
    } else {
        vec![own.code.clone()]
    }
}

/// Standard Dashboard `id`'s Visualizations, copied as the session's own (*Save as*).
async fn copies_of(
    api: &AnalyticsApi,
    id: &str,
    area: &[Place],
    viewer: &Viewer,
) -> Result<Vec<SavedVisualization>, Box<Response>> {
    let unavailable_now = |e: String| Box::new(unavailable(e));
    let standard = standard(api).await?;
    let Some(dashboard) = standard.iter().find(|d| d["id"] == id) else {
        return Err(Box::new(problem(
            StatusCode::NOT_FOUND,
            "no such standard Dashboard",
        )));
    };
    let offered = api.catalogue.indicators().await.map_err(unavailable_now)?;
    let mut copies = Vec::new();
    for item in dashboard["items"].as_array().into_iter().flatten() {
        let follow = item["visualization"]["follow"].as_str();
        let setup = read_setup(&copied_setup(item, &own_places(follow, area)), &offered)
            .map_err(|why| Box::new(problem(StatusCode::UNPROCESSABLE_ENTITY, why)))?;
        copies.push(SavedVisualization {
            id: uuid::Uuid::now_v7().to_string(),
            owner: viewer.account.clone(),
            title: item["title"].as_str().unwrap_or_default().to_owned(),
            setup: setup.json.to_string(),
        });
    }
    Ok(copies)
}

/// The session's own Dashboard `id`, or the refusal: 404 when it may not see it, 403 when it
/// may but is not its owner.
fn own<'a>(
    dashboards: &'a [SavedDashboard],
    id: &str,
    viewer: &Viewer,
) -> Result<&'a SavedDashboard, Box<Response>> {
    match dashboards
        .iter()
        .find(|d| d.id == id && d.visible_to(viewer))
    {
        None => Err(Box::new(problem(
            StatusCode::NOT_FOUND,
            "no such Dashboard",
        ))),
        Some(d) if d.owner != viewer.account => Err(Box::new(problem(
            StatusCode::FORBIDDEN,
            "only its owner changes a Dashboard",
        ))),
        Some(d) => Ok(d),
    }
}

/// `PUT /dashboards/{id}`: `{title, shares}`, its owner only.
async fn change_dashboard(
    State(api): State<AnalyticsApi>,
    subject: Option<Extension<Subject>>,
    roles: Option<Extension<SessionRoles>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let (_, _, viewer) = match looking(&api, subject, roles, true).await {
        Ok(looking) => looking,
        Err(response) => return *response,
    };
    if let Err(response) = not_standard(&api, &id).await {
        return *response;
    }
    let title = match read_title(&body, "title") {
        Ok(title) => title,
        Err(why) => return problem(StatusCode::UNPROCESSABLE_ENTITY, why),
    };
    let shares = match shares_of(&api, &body).await {
        Ok(shares) => shares,
        Err(response) => return *response,
    };
    let (dashboards, visualizations) = match saved(&api.store).await {
        Ok(saved) => saved,
        Err(response) => return *response,
    };
    let mut dashboard = match own(&dashboards, &id, &viewer) {
        Ok(dashboard) => dashboard.clone(),
        Err(response) => return *response,
    };
    dashboard.title = title;
    dashboard.shares = shares;
    if let Err(response) = put_dashboard(&api.store, &dashboard).await {
        return *response;
    }
    Json(dashboard_json(&dashboard, &visualizations, &viewer)).into_response()
}

/// `POST /visualizations`: `{title, visualization, dashboard?}`.
async fn save_visualization(
    State(api): State<AnalyticsApi>,
    subject: Option<Extension<Subject>>,
    roles: Option<Extension<SessionRoles>>,
    Json(body): Json<Value>,
) -> Response {
    let (_, area, viewer) = match looking(&api, subject, roles, true).await {
        Ok(looking) => looking,
        Err(response) => return *response,
    };
    let title = match read_title(&body, "title") {
        Ok(title) => title,
        Err(why) => return problem(StatusCode::UNPROCESSABLE_ENTITY, why),
    };
    let offered = match api.catalogue.indicators().await {
        Ok(indicators) => indicators,
        Err(error) => return unavailable(error),
    };
    let setup = match read_setup(&body["visualization"], &offered) {
        Ok(setup) => setup,
        Err(why) => return problem(StatusCode::UNPROCESSABLE_ENTITY, why),
    };
    if let Some(code) = setup
        .places
        .iter()
        .find(|code| !area.iter().any(|p| p.code == **code))
    {
        return problem(StatusCode::FORBIDDEN, format!("{code} is not in your area"));
    }
    let visualization = SavedVisualization {
        id: uuid::Uuid::now_v7().to_string(),
        owner: viewer.account.clone(),
        title,
        setup: setup.json.to_string(),
    };
    let mut tx = match api.store.begin().await {
        Ok(tx) => tx,
        Err(error) => return unavailable(error),
    };
    let onto = match &body["dashboard"] {
        Value::Null => None,
        Value::String(id) => {
            if let Err(response) = not_standard(&api, id).await {
                return *response;
            }
            let dashboards = match tx.dashboards().await {
                Ok(dashboards) => dashboards,
                Err(error) => return unavailable(error),
            };
            let mut dashboard = match own(&dashboards, id, &viewer) {
                Ok(dashboard) => dashboard.clone(),
                Err(response) => return *response,
            };
            if dashboard.items.len() >= MOST_ITEMS {
                return problem(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("a Dashboard holds at most {MOST_ITEMS} Visualizations"),
                );
            }
            dashboard.items.push(visualization.id.clone());
            Some(dashboard)
        }
        _ => return problem(StatusCode::UNPROCESSABLE_ENTITY, "`dashboard` is not an id"),
    };
    if let Err(error) = tx.put_visualization(&visualization).await {
        return unavailable(error);
    }
    if let Some(dashboard) = &onto
        && let Err(error) = tx.put_dashboard(dashboard).await
    {
        return unavailable(error);
    }
    if let Err(error) = tx.commit().await {
        return unavailable(error);
    }
    (
        StatusCode::CREATED,
        Json(json!({
            "id": visualization.id,
            "title": visualization.title,
            "visualization": setup.json,
            "dashboard": onto.map(|d| d.id),
        })),
    )
        .into_response()
}

/// The places of `area` a Report running for `levels` may be run for.
fn runnable<'a>(area: &'a [Place], levels: &[health_analytics::Level]) -> Vec<&'a Place> {
    area.iter()
        .filter(|p| level_of(p.level.code()).is_some_and(|l| levels.contains(&l)))
        .collect()
}

/// `GET /reports`.
async fn reports(State(api): State<AnalyticsApi>, subject: Option<Extension<Subject>>) -> Response {
    let subject = match viewer(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let registry = match registry(&api.store).await {
        Ok(registry) => registry,
        Err(response) => return *response,
    };
    let published = match api.catalogue.reports().await {
        Ok(published) => published,
        Err(error) => return unavailable(error),
    };
    let area = area(&registry, &subject);
    let reports: Vec<Value> = published
        .iter()
        .filter_map(|p| {
            let places: Vec<Value> = runnable(&area, &p.report.levels)
                .into_iter()
                .map(place_json)
                .collect();
            (!places.is_empty()).then(|| listed_json(&p.package, &p.version, &p.report, &places))
        })
        .collect();
    Json(json!({ "where": "central", "reports": reports })).into_response()
}

/// `POST /reports/run`.
async fn run_report_route(
    State(api): State<AnalyticsApi>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<Value>,
) -> Response {
    let subject = match viewer(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let (Some(wanted), Some(code), Some(period)) = (
        body["report"].as_str(),
        body["place"].as_str(),
        body["period"].as_str(),
    ) else {
        return problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "name the `report`, the `place` and the `period`",
        );
    };
    let published = match api.catalogue.reports().await {
        Ok(published) => published,
        Err(error) => return unavailable(error),
    };
    let Some(found) = published
        .iter()
        .find(|p| format!("{}:{}", p.package, p.report.id) == wanted)
    else {
        return problem(
            StatusCode::NOT_FOUND,
            format!("{wanted} is not a published Report"),
        );
    };
    let report = &found.report;
    let period = match Period::parse(report.period_type, period) {
        Ok(period) => period,
        Err(error) => return problem(StatusCode::UNPROCESSABLE_ENTITY, error.to_string()),
    };
    let registry = match registry(&api.store).await {
        Ok(registry) => registry,
        Err(response) => return *response,
    };
    let area = area(&registry, &subject);
    let Some(place) = area.iter().find(|p| p.code == code) else {
        return problem(StatusCode::FORBIDDEN, format!("{code} is not in your area"));
    };
    let Some(level) = level_of(place.level.code()).filter(|l| report.levels.contains(l)) else {
        return problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{} does not run for a {}", report.title, place.level.code()),
        );
    };
    let facilities = if place.level == PlaceLevel::Facility {
        vec![place.code.clone()]
    } else {
        registry.facilities_in(&place.code)
    };
    let indicators = match api.catalogue.indicators().await {
        Ok(indicators) => indicators,
        Err(error) => return unavailable(error),
    };
    let sets = match api.catalogue.age_band_sets().await {
        Ok(sets) => sets,
        Err(error) => return unavailable(error),
    };
    let estimates = match api.catalogue.estimates().await {
        Ok(years) => estimates_over(years, &registry),
        Err(error) => return unavailable(error),
    };
    let names: BTreeMap<String, String> = registry
        .iter()
        .map(|p| (p.code.clone(), p.name.clone()))
        .collect();
    let mut tx = match api.store.begin().await {
        Ok(tx) => tx,
        Err(error) => return unavailable(error),
    };
    let run = RunPlace {
        place: ExplorePlace {
            code: place.code.clone(),
            facilities,
        },
        name: place.name.clone(),
        level,
    };
    let with = RunWith {
        indicators: &indicators,
        sets: &sets,
        estimates: &estimates,
        names: &names,
    };
    match run_report(&mut tx, report, &run, &period, &with, "central").await {
        Ok(mut answer) => {
            answer["report"] = json!({
                "id": wanted,
                "title": report.title,
                "package": format!("{}@{}", found.package, found.version),
            });
            Json(answer).into_response()
        }
        Err(error) => unavailable(error),
    }
}

// After the routes: the route inventory reads this file up to its first test-only item.
/// Where the indicators offered, and the standard Dashboards, come from.
#[derive(Clone)]
pub(crate) enum Catalogue {
    /// Every published package version's indicators, and the newest published version's
    /// standard Dashboards of each package.
    Published(Option<Publications>),
    /// Fixed lists, and boundaries, for tests.
    #[cfg(test)]
    Fixed(
        Vec<Indicator>,
        Vec<PublishedDashboard>,
        Option<AdministrativeBoundaries>,
        Vec<EstimatesDefinition>,
        Vec<PublishedReport>,
    ),
}

impl Catalogue {
    async fn indicators(&self) -> Result<Vec<Indicator>, String> {
        match self {
            Self::Published(None) => Ok(Vec::new()),
            Self::Published(Some(publications)) => publications
                .published_indicators()
                .await
                .map_err(|e| e.to_string()),
            #[cfg(test)]
            Self::Fixed(indicators, _, _, _, _) => Ok(indicators.clone()),
        }
    }

    async fn dashboards(&self) -> Result<Vec<PublishedDashboard>, String> {
        match self {
            Self::Published(None) => Ok(Vec::new()),
            Self::Published(Some(publications)) => publications
                .published_dashboards()
                .await
                .map_err(|e| e.to_string()),
            #[cfg(test)]
            Self::Fixed(_, dashboards, _, _, _) => Ok(dashboards.clone()),
        }
    }

    /// The Population Estimates of the newest published version of each Population package
    /// (#553).
    async fn estimates(&self) -> Result<Vec<EstimatesDefinition>, String> {
        match self {
            Self::Published(None) => Ok(Vec::new()),
            Self::Published(Some(publications)) => publications
                .published_estimates()
                .await
                .map_err(|e| e.to_string()),
            #[cfg(test)]
            Self::Fixed(_, _, _, estimates, _) => Ok(estimates.clone()),
        }
    }

    /// The Reports of the newest published version of each package (#557).
    async fn reports(&self) -> Result<Vec<PublishedReport>, String> {
        match self {
            Self::Published(None) => Ok(Vec::new()),
            Self::Published(Some(publications)) => publications
                .published_reports()
                .await
                .map_err(|e| e.to_string()),
            #[cfg(test)]
            Self::Fixed(_, _, _, _, reports) => Ok(reports.clone()),
        }
    }

    /// Every published Age Band Set, to label a Report's bands (#557).
    async fn age_band_sets(&self) -> Result<Vec<AgeBandSet>, String> {
        match self {
            Self::Published(None) => Ok(Vec::new()),
            Self::Published(Some(publications)) => publications
                .age_band_sets()
                .await
                .map(|sets| sets.into_iter().map(|p| p.set).collect())
                .map_err(|e| e.to_string()),
            #[cfg(test)]
            Self::Fixed(..) => Ok(Vec::new()),
        }
    }

    /// The region and district boundaries of the newest published package holding any (#604).
    async fn boundaries(&self) -> Result<Option<AdministrativeBoundaries>, String> {
        match self {
            Self::Published(None) => Ok(None),
            Self::Published(Some(publications)) => publications
                .administrative_boundaries()
                .await
                .map_err(|e| e.to_string()),
            #[cfg(test)]
            Self::Fixed(_, _, boundaries, _, _) => Ok(boundaries.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::Request;
    use health_access_policy::Permissions;
    use health_analytics::{
        Aggregate, Calculation, Counts, Period, PeriodType, StoredSummary, Summary,
    };
    use health_analytics_app::report::measure_report;
    use health_facility_registry::{
        AdministrativeAreas, Area, District, parse_nhfr_csv, plan_import,
    };
    use health_surrealdb::{Connection, MIGRATIONS, Settings, Target, migrate};
    use health_sync_protocol::IndicatorSummary;
    use health_time::SystemClock;
    use http_body_util::BodyExt as _;
    use rust_decimal::Decimal;
    use tower::ServiceExt as _;

    use super::*;

    struct NoAudit;
    impl health_registries_app::AuditTrail<health_surrealdb::SurrealTx> for NoAudit {
        async fn written(
            &self,
            _tx: &mut health_surrealdb::SurrealTx,
            _actor: &str,
            _reason: &str,
            _json: &health_persistence::RawJson,
        ) -> Result<(), health_persistence::WriteError> {
            Ok(())
        }
        async fn recorded(
            &self,
            _tx: &mut health_surrealdb::SurrealTx,
            _actor: &str,
            _reason: &str,
            _what: &str,
        ) -> Result<(), health_persistence::WriteError> {
            Ok(())
        }
    }

    fn indicator(name: &str, calculation: Calculation) -> Indicator {
        Indicator {
            url: format!("https://x/Measure/{name}"),
            version: "1.0.0".into(),
            title: name.into(),
            scoring: calculation.scoring(),
            calculation,
            basis: "Encounter".into(),
            counts: Counts::People,
            cumulative: false,
            period_date: "Encounter.actualPeriod.start".into(),
            populations: BTreeMap::new(),
            references: BTreeSet::new(),
            disaggregations: Vec::new(),
            thresholds: Vec::new(),
            estimate: None,
            value: None,
        }
    }

    fn visits() -> Indicator {
        indicator("anc1", Calculation::Count)
    }

    fn weeks() -> Indicator {
        indicator("gestational-age", Calculation::Value(Aggregate::Median))
    }

    /// ANC 1st visit coverage (#553): first visits over the place's expected pregnancies.
    fn coverage() -> Indicator {
        Indicator {
            estimate: Some("expected-pregnancies".into()),
            ..indicator("anc1-coverage", Calculation::Percentage)
        }
    }

    /// Kampala (KAM013, KAM020, KAM031) and Wakiso (WAK001), with KAM013's and KAM020's March
    /// synced and KAM031 never synced.
    async fn central() -> SurrealStore {
        let connection = Connection::open(&Settings {
            target: Target::Memory,
            namespace: "central".into(),
            database: "explorer".into(),
        })
        .await
        .expect("open");
        migrate(&connection, MIGRATIONS).await.expect("migrate");
        let store = SurrealStore::new(connection, Arc::new(SystemClock));
        let areas = AdministrativeAreas {
            national: Some("Ministry of Health".into()),
            regions: vec![Area {
                code: "region-central".into(),
                name: "Central".into(),
            }],
            districts: ["kampala", "wakiso"]
                .map(|d| District {
                    code: format!("district-{d}"),
                    name: d.into(),
                    region: "region-central".into(),
                })
                .to_vec(),
        };
        let rows = parse_nhfr_csv(
            "nhfr_id,name,district,status,latitude,longitude\n\
             KAM013,Kisenyi HC IV,district-kampala,open,0.3136,32.5664\n\
             KAM020,Kawempe HC III,district-kampala,open,,\n\
             KAM031,Kitebi HC III,district-kampala,open,0.2870,32.5590\n\
             WAK001,Wakiso HC IV,district-wakiso,open,0.4044,32.4594\n",
        )
        .expect("rows");
        let plan = plan_import(&Places::default(), &areas, &rows).expect("plan");
        health_registries_app::places::apply_import(&store, &NoAudit, "test", &plan)
            .await
            .expect("import");
        let march = Period::parse(PeriodType::Monthly, "2026-03").expect("march");
        for (facility, count) in [("KAM013", 4), ("KAM020", 6), ("WAK001", 50)] {
            let mut tx = store.begin().await.expect("begin");
            for indicator in [visits(), weeks(), coverage()] {
                let summary = Summary {
                    calculation: indicator.calculation,
                    value: Some(Decimal::from(count)),
                    count,
                    denominator: None,
                    values: None,
                    disaggregations: Vec::new(),
                };
                let report = measure_report(
                    &indicator.canonical(),
                    &march,
                    facility,
                    &summary,
                    "2026-04-01T06:00:00Z",
                );
                let stored = StoredSummary {
                    measure: indicator.canonical(),
                    period: march.clone(),
                    facility: facility.into(),
                    report: report.to_string(),
                };
                let sent = IndicatorSummary {
                    measure: stored.measure,
                    period_type: "monthly".into(),
                    period: "2026-03".into(),
                    facility: facility.into(),
                    hash: health_surrealdb::figures_hash(&stored.report),
                    report: Some(report),
                };
                tx.receive_indicator_summary("node", &sent)
                    .await
                    .expect("receive");
            }
            tx.indicator_summaries_synced(facility, "node")
                .await
                .expect("synced");
            tx.commit().await.expect("commit");
        }
        store
    }

    fn subject<const N: usize>(place: &str, area: bool, permissions: [&str; N]) -> Subject {
        Subject {
            account: "okot".into(),
            facility: place.into(),
            permissions: Permissions::from(permissions),
            programs: BTreeSet::new(),
            purpose: None,
            break_glass: None,
            area,
        }
    }

    async fn ask(
        store: &SurrealStore,
        subject: Subject,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        ask_as(store, subject, &[], method, path, body).await
    }

    /// Asks as `subject` holding `roles`; each username is its own account id, and `nobody`
    /// has none.
    async fn ask_as(
        store: &SurrealStore,
        subject: Subject,
        roles: &[&str],
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let catalogue = Catalogue::Fixed(
            vec![visits(), weeks()],
            Vec::new(),
            None,
            Vec::new(),
            Vec::new(),
        );
        ask_in(store, catalogue, subject, roles, method, path, body).await
    }

    /// Asks as `ask_as` does, offering `catalogue`.
    async fn ask_in(
        store: &SurrealStore,
        catalogue: Catalogue,
        subject: Subject,
        roles: &[&str],
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let accounts: Accounts = Arc::new(|username: String| {
            Box::pin(async move { Ok((username != "nobody").then_some(username)) })
        });
        let app = router(store.clone(), catalogue, accounts, Arc::new(SystemClock))
            .layer(Extension(subject))
            .layer(Extension(SessionRoles(
                roles.iter().map(|r| (*r).to_owned()).collect(),
            )));
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
            .expect("request");
        let response = app.oneshot(request).await.expect("response");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    fn question(places: &[&str]) -> Value {
        json!({
            "indicators": [visits().canonical(), weeks().canonical()],
            "periodType": "monthly",
            "from": "2026-03",
            "to": "2026-03",
            "places": places,
        })
    }

    /// #553: ANC 1st visit coverage at district level uses the district's expected pregnancies
    /// from the published Population package, summed up the facility registry's hierarchy; a
    /// facility without a figure of its own, and a district without one, show *no estimate*.
    #[tokio::test]
    async fn coverage_at_district_level_uses_the_district_s_expected_pregnancies() {
        use health_analytics::{EstimateTypeDefinition, Figure};
        let store = central().await;
        let estimates = EstimatesDefinition {
            year: 2026,
            types: vec![
                EstimateTypeDefinition {
                    code: "total-population".into(),
                    title: "Total population".into(),
                    derived_from: None,
                    percent: None,
                },
                EstimateTypeDefinition {
                    code: "expected-pregnancies".into(),
                    title: "Expected pregnancies".into(),
                    derived_from: Some("total-population".into()),
                    percent: Some("5".into()),
                },
            ],
            // Kampala's figures are its sub-counties'; nothing above them is given.
            figures: vec![Figure {
                place: "district-kampala".into(),
                kind: "total-population".into(),
                value: 24_000,
            }],
        };
        let catalogue = Catalogue::Fixed(
            vec![coverage()],
            Vec::new(),
            None,
            vec![estimates],
            Vec::new(),
        );
        let national = subject("central", true, [VIEW_PERMISSION]);
        let question = json!({
            "indicators": [coverage().canonical()],
            "periodType": "monthly",
            "from": "2026-03",
            "to": "2026-03",
            "places": ["district-kampala", "KAM013", "district-wakiso", "region-central"],
        });
        let (status, explored) = ask_in(
            &store,
            catalogue,
            national,
            &[],
            "POST",
            "/explore",
            Some(question),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{explored}");
        let cells = explored["cells"].as_array().expect("cells");
        // 4 + 6 first visits over 5% of 24,000 / 12 = 100 expected pregnancies in March.
        assert_eq!(cells[0]["value"], "10", "{explored}");
        assert_eq!(cells[0]["estimate"]["value"], "100");
        assert_eq!(cells[0]["estimate"]["title"], "Expected pregnancies");
        assert_eq!(cells[0]["count"], 10);
        for at in [1, 2] {
            assert_eq!(cells[at]["value"], Value::Null, "{}", cells[at]);
            assert_eq!(cells[at]["estimate"]["value"], Value::Null);
        }
        // The region sums Kampala's figure: Wakiso's 50 and Kampala's 10 over 100.
        assert_eq!(cells[3]["value"], "60");
    }

    #[tokio::test]
    async fn a_district_user_sees_their_district_and_nothing_outside_it() {
        let store = central().await;
        let district = subject("district-kampala", true, [VIEW_PERMISSION]);
        let (status, explorer) = ask(&store, district.clone(), "GET", "/explorer", None).await;
        assert_eq!(status, StatusCode::OK, "{explorer}");
        assert_eq!(explorer["where"], "central");
        assert_eq!(explorer["place"]["code"], "district-kampala");
        let codes: Vec<&str> = explorer["places"]
            .as_array()
            .expect("places")
            .iter()
            .filter_map(|p| p["code"].as_str())
            .collect();
        assert_eq!(codes, ["KAM013", "KAM020", "KAM031"]);
        assert_eq!(explorer["indicators"][1]["rollsUp"], false);
        assert_eq!(
            explorer["periodTypes"],
            json!(["weekly", "monthly", "quarterly", "financialJuly"])
        );

        let (status, explored) = ask(
            &store,
            district.clone(),
            "POST",
            "/explore",
            Some(question(&["district-kampala", "KAM013"])),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{explored}");
        let cells = &explored["cells"];
        // The district: 4 + 6, Wakiso's 50 not counted, KAM031 missing rather than zero.
        assert_eq!(cells[0]["place"], "district-kampala");
        assert_eq!(cells[0]["value"], "10");
        assert_eq!(
            (cells[0]["reported"].clone(), cells[0]["expected"].clone()),
            (json!(2), json!(3))
        );
        assert_eq!(cells[0]["missing"][0]["name"], "Kitebi HC III");
        assert_eq!(cells[0]["missing"][0]["lastSync"], Value::Null);
        assert_eq!(cells[1]["place"], "KAM013");
        assert_eq!(cells[1]["value"], "4");
        // The median: none for the district, with the reason; KAM013's own at facility level.
        assert_eq!(cells[2]["facilityOnly"], "median");
        assert_eq!(cells[2]["value"], Value::Null);
        assert_eq!(cells[3]["value"], "4");
        assert_eq!(explored["periods"][0]["id"], "2026-03");

        // Another district's place is refused, alone or among its own.
        for places in [
            &["WAK001"][..],
            &["district-kampala", "district-wakiso"][..],
        ] {
            let (status, body) = ask(
                &store,
                district.clone(),
                "POST",
                "/explore",
                Some(question(places)),
            )
            .await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        }
        // An arbitrary date range is not a question.
        let mut dates = question(&["district-kampala"]);
        dates["from"] = json!("2026-03-05");
        let (status, _) = ask(&store, district, "POST", "/explore", Some(dates)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    /// The national core's boundaries of Central region and Kampala (none for Wakiso).
    fn boundaries() -> AdministrativeBoundaries {
        let square = |x: f64, y: f64, size: f64| {
            health_facility_registry::Geometry::Polygon(vec![vec![
                [x, y],
                [x + size, y],
                [x + size, y + size],
                [x, y + size],
                [x, y],
            ]])
        };
        AdministrativeBoundaries {
            features: [
                ("region-central", square(31.0, -1.0, 2.5)),
                ("district-kampala", square(32.5, 0.25, 0.15)),
            ]
            .map(|(code, geometry)| health_facility_registry::Boundary {
                properties: health_facility_registry::BoundaryProperties {
                    code: code.into(),
                    name: None,
                },
                geometry,
            })
            .to_vec(),
        }
    }

    async fn map_for(store: &SurrealStore, subject: Subject) -> (StatusCode, Value) {
        let catalogue = Catalogue::Fixed(
            vec![visits()],
            Vec::new(),
            Some(boundaries()),
            Vec::new(),
            Vec::new(),
        );
        ask_in(store, catalogue, subject, &[], "GET", "/map", None).await
    }

    #[tokio::test]
    async fn a_map_draws_the_area_s_boundaries_and_facility_points_only() {
        let store = central().await;
        // A national user: every region and district in the area that has a boundary, and every
        // facility with NHFR coordinates; a region's figure comes from /explore like any place.
        let (status, map) = map_for(&store, subject("central", true, [VIEW_PERMISSION])).await;
        assert_eq!(status, StatusCode::OK);
        let codes = |key: &str| -> Vec<String> {
            map[key]
                .as_array()
                .expect(key)
                .iter()
                .map(|p| p["code"].as_str().unwrap_or_default().to_owned())
                .collect()
        };
        assert_eq!(codes("areas"), ["region-central", "district-kampala"]);
        assert_eq!(map["areas"][0]["level"], "region");
        assert_eq!(map["areas"][0]["geometry"]["type"], "Polygon");
        assert_eq!(codes("facilities"), ["KAM013", "KAM031", "WAK001"]);
        assert_eq!(map["facilities"][0]["latitude"], "0.3136");
        assert_eq!(map["facilities"][0]["parent"], "district-kampala");
        let (status, answer) = ask(
            &store,
            subject("central", true, [VIEW_PERMISSION]),
            "POST",
            "/explore",
            Some(json!({
                "indicators": [visits().canonical()],
                "periodType": "monthly",
                "from": "2026-03",
                "to": "2026-03",
                "places": ["region-central"],
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(answer["cells"][0]["place"], "region-central");
        assert_eq!(answer["cells"][0]["value"], "60");

        // A district user: their district only, never the region around it or a neighbour.
        let (_, map) = map_for(&store, subject("district-kampala", true, [VIEW_PERMISSION])).await;
        assert_eq!(codes_of(&map["areas"]), ["district-kampala"]);
        assert_eq!(codes_of(&map["facilities"]), ["KAM013", "KAM031"]);

        let (status, _) = map_for(&store, subject("central", true, ["Organization.rs"])).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    fn codes_of(list: &Value) -> Vec<&str> {
        list.as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p["code"].as_str())
            .collect()
    }

    #[tokio::test]
    async fn exploring_needs_reporting_view() {
        let store = central().await;
        let (status, _) = ask(
            &store,
            subject("district-kampala", true, ["Organization.rs"]),
            "GET",
            "/explorer",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        // A facility session's area is its facility.
        let (status, explorer) = ask(
            &store,
            subject("KAM020", false, [VIEW_PERMISSION]),
            "GET",
            "/explorer",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(explorer["place"]["code"], "KAM020");
        assert_eq!(explorer["places"], json!([]));
    }

    fn author(place: &str, account: &str) -> Subject {
        Subject {
            account: account.into(),
            ..subject(place, true, [VIEW_PERMISSION, AUTHOR_PERMISSION])
        }
    }

    fn saved_setup(places: &[&str], follow: Option<&str>) -> Value {
        let mut setup = json!({
            "type": "column",
            "layout": "placesByIndicators",
            "series": "place",
            "question": question(places),
        });
        if let Some(follow) = follow {
            setup["follow"] = json!(follow);
        }
        setup
    }

    #[tokio::test]
    async fn a_district_user_shares_a_dashboard_with_every_district() {
        let store = central().await;
        let kampala = author("district-kampala", "okot");
        let (status, made) = ask(
            &store,
            kampala.clone(),
            "POST",
            "/dashboards",
            Some(json!({ "title": "ANC", "shares": [{ "with": "level", "level": "district" }] })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{made}");
        let id = made["id"].as_str().expect("id").to_owned();
        let (status, saved) = ask(
            &store,
            kampala.clone(),
            "POST",
            "/visualizations",
            Some(json!({
                "title": "ANC 1st visits by area",
                "dashboard": id,
                "visualization": saved_setup(&["KAM013", "KAM020", "KAM031"], Some("eachAreaUnder")),
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{saved}");
        assert_eq!(saved["dashboard"], id.as_str());

        // Another district's user, who may not author, opens it: the setup follows them.
        let wakiso = Subject {
            account: "nakato".into(),
            ..subject("district-wakiso", true, [VIEW_PERMISSION])
        };
        let (status, listed) = ask(&store, wakiso.clone(), "GET", "/dashboards", None).await;
        assert_eq!(status, StatusCode::OK, "{listed}");
        assert_eq!(listed["canAuthor"], false);
        let dashboard = &listed["dashboards"][0];
        assert_eq!(dashboard["title"], "ANC");
        assert_eq!(dashboard["mine"], false);
        assert_eq!(
            dashboard["shares"],
            Value::Null,
            "only its owner reads its shares"
        );
        let item = &dashboard["items"][0];
        assert_eq!(item["title"], "ANC 1st visits by area");
        assert_eq!(item["visualization"]["follow"], "eachAreaUnder");
        // Their own area's figures: Wakiso HC IV's, never Kampala's.
        let mut asked = item["visualization"]["question"].clone();
        asked["places"] = json!(["WAK001"]);
        let (status, answer) = ask(&store, wakiso.clone(), "POST", "/explore", Some(asked)).await;
        assert_eq!(status, StatusCode::OK, "{answer}");
        assert_eq!(answer["cells"][0]["value"], "50");

        // A facility session is not a district, so it is not shared with it.
        let facility = Subject {
            account: "akello".into(),
            ..subject("KAM013", false, [VIEW_PERMISSION])
        };
        let (_, listed) = ask(&store, facility, "GET", "/dashboards", None).await;
        assert_eq!(listed["dashboards"], json!([]));

        // Its owner reads its shares and may change them; nobody else may.
        let (_, own) = ask(&store, kampala.clone(), "GET", "/dashboards", None).await;
        assert_eq!(own["canAuthor"], true);
        assert_eq!(own["dashboards"][0]["shares"][0]["level"], "district");
        let change = json!({ "title": "ANC, shared", "shares": [{ "with": "person", "username": "nakato" }] });
        let other_author = author("district-wakiso", "nakato");
        let (status, _) = ask(
            &store,
            other_author,
            "PUT",
            &format!("/dashboards/{id}"),
            Some(change.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, changed) = ask(
            &store,
            kampala,
            "PUT",
            &format!("/dashboards/{id}"),
            Some(change),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{changed}");
        assert_eq!(
            changed["shares"],
            json!([{ "with": "person", "account": "nakato", "username": "nakato" }])
        );
        let (_, listed) = ask(&store, wakiso, "GET", "/dashboards", None).await;
        assert_eq!(listed["dashboards"][0]["title"], "ANC, shared");
    }

    #[tokio::test]
    async fn a_dashboard_shared_with_a_role_reaches_whoever_holds_it() {
        let store = central().await;
        let (_, made) = ask(
            &store,
            author("district-kampala", "okot"),
            "POST",
            "/dashboards",
            Some(json!({ "title": "HMIS", "shares": [{ "with": "role", "role": "in-charge" }] })),
        )
        .await;
        let viewer = Subject {
            account: "akello".into(),
            ..subject("KAM013", false, [VIEW_PERMISSION])
        };
        let (_, listed) = ask_as(
            &store,
            viewer.clone(),
            &["in-charge"],
            "GET",
            "/dashboards",
            None,
        )
        .await;
        assert_eq!(listed["dashboards"][0]["id"], made["id"]);
        let (_, listed) = ask_as(&store, viewer, &["clerk"], "GET", "/dashboards", None).await;
        assert_eq!(listed["dashboards"], json!([]));
    }

    #[tokio::test]
    async fn saving_needs_analytics_author_and_places_in_the_area() {
        let store = central().await;
        let viewer = subject("district-kampala", true, [VIEW_PERMISSION]);
        let save = json!({
            "title": "ANC",
            "visualization": saved_setup(&["district-kampala"], Some("myPlace")),
        });
        let (status, _) = ask(
            &store,
            viewer.clone(),
            "POST",
            "/visualizations",
            Some(save.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = ask(
            &store,
            viewer,
            "POST",
            "/dashboards",
            Some(json!({ "title": "ANC" })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        let kampala = author("district-kampala", "okot");
        let (status, saved) = ask(
            &store,
            kampala.clone(),
            "POST",
            "/visualizations",
            Some(save),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{saved}");
        // Saved onto no Dashboard, it is listed as the author's own.
        let (_, listed) = ask(&store, kampala.clone(), "GET", "/dashboards", None).await;
        assert_eq!(listed["visualizations"][0]["id"], saved["id"]);

        let outside = json!({ "title": "Wakiso", "visualization": saved_setup(&["WAK001"], None) });
        let (status, _) = ask(
            &store,
            kampala.clone(),
            "POST",
            "/visualizations",
            Some(outside),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        for wrong in [
            json!({ "title": "", "visualization": saved_setup(&["KAM013"], None) }),
            json!({ "title": "x", "visualization": { "type": "pie" } }),
            json!({ "title": "x", "visualization": saved_setup(&["KAM013"], None), "dashboard": 3 }),
        ] {
            let (status, _) = ask(
                &store,
                kampala.clone(),
                "POST",
                "/visualizations",
                Some(wrong),
            )
            .await;
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        }
        let (status, _) = ask(
            &store,
            kampala.clone(),
            "POST",
            "/visualizations",
            Some(json!({ "title": "x", "visualization": saved_setup(&["KAM013"], None), "dashboard": "none" })),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = ask(
            &store,
            kampala,
            "POST",
            "/dashboards",
            Some(json!({ "title": "x", "shares": [{ "with": "person", "username": "nobody" }] })),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    /// A published Reporting package's *ANC overview*: ANC 1st visits in each area under the
    /// viewer's place, over the last three months.
    fn with_standard() -> Catalogue {
        let dashboard = serde_json::from_value(json!({
            "id": "anc-overview",
            "title": "ANC overview",
            "items": [{
                "id": "first-visits",
                "title": "ANC 1st visits by area",
                "visualization": {
                    "type": "column",
                    "question": {
                        "indicators": [visits().canonical()],
                        "periodType": "monthly",
                        "last": 3
                    },
                    "layout": "placesByIndicators",
                    "series": "place",
                    "follow": "eachAreaUnder"
                }
            }]
        }))
        .expect("a standard Dashboard");
        Catalogue::Fixed(
            vec![visits(), weeks()],
            vec![PublishedDashboard {
                package: "ug.moh.hmis".into(),
                version: "1.0.0".into(),
                dashboard,
            }],
            None,
            Vec::new(),
            Vec::new(),
        )
    }

    async fn ask_standard(
        store: &SurrealStore,
        subject: Subject,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        ask_in(store, with_standard(), subject, &[], method, path, body).await
    }

    #[tokio::test]
    async fn a_published_reporting_package_adds_its_dashboard_for_every_user() {
        let store = central().await;
        let today = super::today(&SystemClock);
        let last = Period::of(PeriodType::Monthly, today);
        let first = last
            .previous()
            .and_then(|p| p.previous())
            .expect("two months back");
        for (viewer, place) in [
            ("nakato", "district-wakiso"),
            ("okot", "district-kampala"),
            ("akello", "KAM013"),
        ] {
            let subject = Subject {
                account: viewer.into(),
                ..subject(place, place.starts_with("district"), [VIEW_PERMISSION])
            };
            let (status, listed) = ask_standard(&store, subject, "GET", "/dashboards", None).await;
            assert_eq!(status, StatusCode::OK, "{listed}");
            let standard = &listed["dashboards"][0];
            assert_eq!(standard["id"], "ug.moh.hmis:anc-overview", "{viewer}");
            assert_eq!(standard["title"], "ANC overview");
            assert_eq!(
                (standard["kind"].clone(), standard["mine"].clone()),
                (json!("standard"), json!(false))
            );
            assert_eq!(standard["package"], "ug.moh.hmis@1.0.0");
            let shown = &standard["items"][0]["visualization"];
            assert_eq!(shown["follow"], "eachAreaUnder");
            assert_eq!(
                (
                    shown["question"]["from"].clone(),
                    shown["question"]["to"].clone()
                ),
                (json!(first.id), json!(last.id))
            );
        }
    }

    #[tokio::test]
    async fn a_standard_dashboard_is_not_edited_in_place_and_save_as_copies_it() {
        let store = central().await;
        let kampala = author("district-kampala", "okot");
        let id = "ug.moh.hmis:anc-overview";
        let change = json!({ "title": "Mine now", "shares": [] });
        let (status, refused) = ask_standard(
            &store,
            kampala.clone(),
            "PUT",
            &format!("/dashboards/{id}"),
            Some(change),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            refused["detail"],
            "a standard Dashboard changes only in a new version of its Reporting package: Save as makes your own copy"
        );
        let (status, _) = ask_standard(
            &store,
            kampala.clone(),
            "POST",
            "/visualizations",
            Some(json!({
                "title": "More",
                "dashboard": id,
                "visualization": saved_setup(&["KAM013"], None),
            })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        let (status, copy) = ask_standard(
            &store,
            kampala.clone(),
            "POST",
            "/dashboards",
            Some(json!({ "title": "ANC overview (mine)", "shares": [], "copyOf": id })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{copy}");
        assert_eq!(
            (copy["kind"].clone(), copy["mine"].clone()),
            (json!("saved"), json!(true))
        );
        let item = &copy["items"][0];
        assert_eq!(item["title"], "ANC 1st visits by area");
        assert_eq!(item["visualization"]["follow"], "eachAreaUnder");
        assert_eq!(
            item["visualization"]["question"]["places"],
            json!(["KAM013", "KAM020", "KAM031"])
        );
        let (_, listed) = ask_standard(&store, kampala.clone(), "GET", "/dashboards", None).await;
        let titles: Vec<&str> = listed["dashboards"]
            .as_array()
            .expect("dashboards")
            .iter()
            .filter_map(|d| d["title"].as_str())
            .collect();
        assert_eq!(titles, ["ANC overview", "ANC overview (mine)"]);
        // The standard one is unchanged.
        assert_eq!(listed["dashboards"][0]["kind"], "standard");

        let (status, _) = ask_standard(
            &store,
            kampala,
            "POST",
            "/dashboards",
            Some(json!({ "title": "Nothing", "copyOf": "ug.moh.hmis:gone" })),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let viewer = Subject {
            account: "nakato".into(),
            ..subject("district-wakiso", true, [VIEW_PERMISSION])
        };
        let (status, _) = ask_standard(
            &store,
            viewer,
            "POST",
            "/dashboards",
            Some(json!({ "title": "Copy", "copyOf": id })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "Save as needs analytics.author"
        );
    }

    /// A published Reporting package's ANC report: first visits and the median gestational age,
    /// run for facilities and districts; and a facility-only monitoring Report.
    fn with_reports() -> Catalogue {
        let anc: health_analytics::ReportDefinition = serde_json::from_value(json!({
            "id": "anc",
            "title": "ANC",
            "periodType": "monthly",
            "levels": ["facility", "district"],
            "sections": [{
                "kind": "table",
                "id": "attendance",
                "title": "Attendance",
                "rows": [
                    { "id": "first", "indicator": visits().canonical(), "label": "ANC 1st visits" },
                    { "id": "weeks", "indicator": weeks().canonical(), "label": "Weeks" },
                ],
                "change": true,
            }],
        }))
        .expect("report");
        let monitoring = health_analytics::ReportDefinition {
            id: "monitoring".into(),
            title: "Monitoring".into(),
            levels: vec![health_analytics::Level::Facility],
            ..anc.clone()
        };
        let published = |report| PublishedReport {
            package: "ug.moh.hmis".into(),
            version: "1.0.0".into(),
            report,
        };
        Catalogue::Fixed(
            vec![visits(), weeks()],
            Vec::new(),
            None,
            Vec::new(),
            vec![published(anc), published(monitoring)],
        )
    }

    /// #557: a district user runs a Report at central for the places in their area only, from
    /// the facilities' synced summaries; without `reporting.view` they cannot run it at all.
    #[tokio::test]
    async fn a_district_user_runs_a_report_for_places_in_their_area_only() {
        let store = central().await;
        let run = |place: &str| json!({ "report": "ug.moh.hmis:anc", "place": place, "period": "2026-03" });

        // Without reporting.view: neither listed nor run.
        let unauthorised = subject("district-kampala", true, ["Organization.rs"]);
        for (method, path, body) in [
            ("GET", "/reports", None),
            ("POST", "/reports/run", Some(run("district-kampala"))),
        ] {
            let (status, _) = ask_in(
                &store,
                with_reports(),
                unauthorised.clone(),
                &[],
                method,
                path,
                body,
            )
            .await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        }

        // With it: the Reports offered, each for the places in Kampala at its levels.
        let district = subject("district-kampala", true, [VIEW_PERMISSION]);
        let (status, listed) = ask_in(
            &store,
            with_reports(),
            district.clone(),
            &[],
            "GET",
            "/reports",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{listed}");
        let reports = listed["reports"].as_array().expect("reports");
        assert_eq!(reports[0]["id"], "ug.moh.hmis:anc");
        assert_eq!(reports[0]["package"], "ug.moh.hmis@1.0.0");
        assert_eq!(reports[0]["definition"]["sections"][0]["id"], "attendance");
        let places: Vec<&Value> = reports[0]["places"]
            .as_array()
            .expect("places")
            .iter()
            .map(|p| &p["code"])
            .collect();
        assert_eq!(
            places,
            ["district-kampala", "KAM013", "KAM020", "KAM031"],
            "Kampala and its facilities, never Wakiso's"
        );
        // The facility-only Report is offered for the facilities, not the district.
        let places: Vec<&Value> = reports[1]["places"]
            .as_array()
            .expect("places")
            .iter()
            .map(|p| &p["code"])
            .collect();
        assert_eq!(places, ["KAM013", "KAM020", "KAM031"]);

        // Run for the district: 4 + 6 first visits, KAM031 missing rather than zero, and the
        // median refused above facility level.
        let (status, answer) = ask_in(
            &store,
            with_reports(),
            district.clone(),
            &[],
            "POST",
            "/reports/run",
            Some(run("district-kampala")),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{answer}");
        assert_eq!(answer["where"], "central");
        assert_eq!(answer["report"]["title"], "ANC");
        assert_eq!(answer["place"]["name"], "kampala");
        let rows = &answer["sections"][0]["rows"];
        assert_eq!(rows[0]["current"]["value"], "10");
        assert_eq!(rows[1]["facilityOnly"], "median");
        assert_eq!(answer["completeness"]["reported"], 2);
        assert_eq!(answer["completeness"]["expected"], 3);
        assert_eq!(
            answer["completeness"]["missing"][0]["name"],
            "Kitebi HC III"
        );
        assert_eq!(answer["previous"]["id"], "2026-02");

        // A facility of the area: its own figure, and its median.
        let (status, answer) = ask_in(
            &store,
            with_reports(),
            district.clone(),
            &[],
            "POST",
            "/reports/run",
            Some(run("KAM013")),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{answer}");
        assert_eq!(answer["sections"][0]["rows"][0]["current"]["value"], "4");
        assert_eq!(answer["sections"][0]["rows"][1]["current"]["value"], "4");

        // Another district's place is refused; a level the Report does not run for, and a
        // period not of its type, cannot be run.
        for (body, wanted) in [
            (run("WAK001"), StatusCode::FORBIDDEN),
            (run("district-wakiso"), StatusCode::FORBIDDEN),
            (
                json!({ "report": "ug.moh.hmis:monitoring", "place": "district-kampala", "period": "2026-03" }),
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
            (
                json!({ "report": "ug.moh.hmis:anc", "place": "district-kampala", "period": "2026Q1" }),
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
            (
                json!({ "report": "ug.moh.hmis:nothing", "place": "district-kampala", "period": "2026-03" }),
                StatusCode::NOT_FOUND,
            ),
        ] {
            let (status, refused) = ask_in(
                &store,
                with_reports(),
                district.clone(),
                &[],
                "POST",
                "/reports/run",
                Some(body.clone()),
            )
            .await;
            assert_eq!(status, wanted, "{body} {refused}");
        }
    }
}
