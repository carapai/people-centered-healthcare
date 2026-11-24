//! The data explorer on a Facility Edge Node (analytics.md § Dashboards; ADR 0075; #599): the
//! facility's own figures, from the summaries the node computed itself ([`crate::indicators`]),
//! offline. The same questions as central's `/api/analytics/` (central-server's `analytics`),
//! over one place, the facility:
//!
//! - `GET /api/analytics/explorer`: the facility, the active packages' indicators, and the
//!   standard period types.
//! - `POST /api/analytics/explore`: each indicator's figure for each period. A period with no
//!   summary counted nothing here; a median shows, since the facility is facility level. Any
//!   other place is refused (403): districts are explored at central.
//!
//! - `GET /api/analytics/map` (#604): what a map draws here, offline: the facility's own point
//!   from the facility registry's NHFR coordinates. Boundaries are drawn at central, where
//!   districts and regions are explored.
//!
//! - `GET /api/analytics/dashboards` (#879): the Dashboards tab here, offline: the active
//!   Reporting packages' standard Dashboards ([`dashboards_here`]), as central lists them, their
//!   periods the last few up to the facility's today. Their places follow the viewer, which here
//!   is the facility, and each item is answered by `/explore` over the node's own summaries.
//!   Saved Dashboards are central's (user content, #602), so none are listed and none are made.
//!
//! - `GET /api/analytics/reports` and `POST /api/analytics/reports/run` (#557): the active
//!   Reporting packages' Reports that run at facility level ([`reports_here`]), run for the
//!   facility over the node's own summaries, offline ([`run_here`]), as central runs them for a
//!   district. Any other place is refused (403). Line lists wait for #557's second part.
//!
//! A percentage or rate over a Population Estimate type (#553; ADR 0071) is the facility's count
//! over its own estimate for the period: the node keeps only its own place's figures of the
//! active Population packages ([`estimates_here`]), so a facility without a catchment figure
//! shows *no estimate*.
//!
//! Each needs [`VIEW_PERMISSION`].

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_analytics::{
    AgeBandSet, Indicator, Level, Period, PopulationEstimates, STANDARD_PERIODS,
};
use health_analytics_app::dashboards::standard_json;
use health_analytics_app::explore::{
    ExplorePlace, OwnSummaries, cells_json, explore, indicator_json, periods_json, read_question,
};
use health_analytics_app::reports::{RunPlace, RunWith, listed_json, run_report};
use health_files::FsBlobStore;
use health_persistence::Store as _;
use health_registries_app::places::places;
use health_surrealdb::SurrealStore;
use serde_json::{Value, json};

/// The use-case permission that reads Indicator Reports (ADR 0070), as at central.
pub const VIEW_PERMISSION: &str = "reporting.view";

/// A refusal: its status and why.
pub type Refusal = (StatusCode, String);

fn unavailable(error: impl std::fmt::Display) -> Refusal {
    tracing::warn!(%error, "the explorer cannot read the summaries");
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "the indicator summaries are unavailable".into(),
    )
}

/// The facility's name from the facility registry, or its code.
async fn name_of(store: &SurrealStore, facility: &str) -> String {
    let Ok(mut tx) = store.begin().await else {
        return facility.to_owned();
    };
    places(&mut tx)
        .await
        .ok()
        .and_then(|p| p.get(facility).map(|p| p.name.clone()))
        .unwrap_or_else(|| facility.to_owned())
}

/// What `facility` may explore here, offering `indicators`.
pub async fn explorer_here(
    store: &SurrealStore,
    indicators: &[Indicator],
    facility: &str,
) -> Value {
    json!({
        "where": "facility",
        "place": {
            "code": facility,
            "name": name_of(store, facility).await,
            "level": "facility",
            "parent": null,
        },
        "places": [],
        "indicators": indicators.iter().map(indicator_json).collect::<Vec<_>>(),
        "periodTypes": STANDARD_PERIODS,
    })
}

/// What a map of `facility` draws here (#604): its point, when the facility registry holds its
/// coordinates, and no boundaries.
pub async fn map_here(store: &SurrealStore, facility: &str) -> Value {
    let place = match store.begin().await {
        Ok(mut tx) => places(&mut tx)
            .await
            .ok()
            .and_then(|p| p.get(facility).cloned()),
        Err(_) => None,
    };
    let facilities: Vec<Value> = place
        .and_then(|p| {
            let at = p.coordinates?;
            Some(json!({
                "code": p.code,
                "name": p.name,
                "parent": null,
                "latitude": at.latitude,
                "longitude": at.longitude,
            }))
        })
        .into_iter()
        .collect();
    json!({ "areas": [], "facilities": facilities })
}

/// The Population Estimates of the active packages (`packages`, each by its id with its files)
/// that this node keeps: every year's estimate types, and only `facility`'s own figures. Each
/// edge has its own places' figures (ADR 0071); a district's are central's.
pub fn estimates_here<'a>(
    packages: impl IntoIterator<Item = (&'a str, &'a health_packages::Files)>,
    facility: &str,
) -> PopulationEstimates {
    let mut years = Vec::new();
    for (_, files) in packages {
        let population = files
            .get("package.json")
            .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok())
            .is_some_and(|package| package["kind"] == "population");
        if !population {
            continue;
        }
        for (_, read) in health_publication::estimates_in(files) {
            // Validation refused estimates that do not read before they were published.
            if let Ok(definition) = read {
                years.push(definition.for_places(|place| place == facility));
            }
        }
    }
    PopulationEstimates::new(years, |_| None)
}

/// The answer to the question `body` over `facility`'s own summaries in `store`, offering
/// `indicators`, with the facility's own Population Estimates, as of `now`.
pub async fn explore_here(
    store: &SurrealStore,
    indicators: &[Indicator],
    estimates: &PopulationEstimates,
    facility: &str,
    body: &Value,
    now: jiff::Timestamp,
) -> Result<Value, Refusal> {
    let question =
        read_question(body, indicators).map_err(|why| (StatusCode::UNPROCESSABLE_ENTITY, why))?;
    if let Some(other) = question.places.iter().find(|p| *p != facility) {
        return Err((
            StatusCode::FORBIDDEN,
            format!("{other} is explored at central: this node holds {facility}'s figures only"),
        ));
    }
    let mut tx = store.begin().await.map_err(unavailable)?;
    let mut own = OwnSummaries::new(&mut tx, facility, now);
    let place = ExplorePlace {
        code: facility.to_owned(),
        facilities: vec![facility.to_owned()],
    };
    let cells = explore(
        &mut own,
        &question.indicators,
        &question.periods,
        &[place],
        estimates,
    )
    .await
    .map_err(unavailable)?;
    drop(tx);
    let names = [(facility.to_owned(), name_of(store, facility).await)].into();
    Ok(json!({
        "where": "facility",
        "periods": periods_json(&question.periods),
        "cells": cells_json(&cells, &names, question.disaggregation.as_deref()),
    }))
}

/// What the Dashboards tab lists here (#879), from the active packages (`packages`, each by its
/// id, `name@version`, with its files): every Reporting package's standard Dashboards, by
/// package and then file, as of `today`. Nothing here is the viewer's own, and nothing may be
/// authored: saved Dashboards are kept at central.
pub fn dashboards_here<'a>(
    packages: impl IntoIterator<Item = (&'a str, &'a health_packages::Files)>,
    today: jiff::civil::Date,
) -> Value {
    let mut dashboards = Vec::new();
    for (id, files) in packages {
        let reporting = files
            .get("package.json")
            .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok())
            .is_some_and(|package| package["kind"] == "reporting");
        let Some((package, version)) = id.rsplit_once('@').filter(|_| reporting) else {
            continue;
        };
        for (_, read) in health_publication::dashboards_in_files(files) {
            // Validation refused one that does not read before it was published.
            if let Ok(dashboard) = read {
                dashboards.push(standard_json(package, version, &dashboard, today));
            }
        }
    }
    json!({ "canAuthor": false, "dashboards": dashboards, "visualizations": [] })
}

/// The Reports of the active Reporting packages (`packages`, each by its id, `name@version`,
/// with its files), each with its package's name and version: those that run at facility level.
fn facility_reports<'a>(
    packages: impl IntoIterator<Item = (&'a str, &'a health_packages::Files)>,
) -> Vec<(String, String, health_analytics::ReportDefinition)> {
    let mut found = Vec::new();
    for (id, files) in packages {
        let reporting = files
            .get("package.json")
            .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok())
            .is_some_and(|package| package["kind"] == "reporting");
        let Some((package, version)) = id.rsplit_once('@').filter(|_| reporting) else {
            continue;
        };
        for (_, read) in health_publication::reports_in_files(files) {
            // Validation refused one that does not read before it was published.
            if let Ok(report) = read
                && report.levels.contains(&Level::Facility)
            {
                found.push((package.to_owned(), version.to_owned(), report));
            }
        }
    }
    found
}

/// What the Reports page lists here (#557): the active Reporting packages' Reports that run at
/// facility level, by package and then file, each run for `facility` only.
pub async fn reports_here<'a>(
    store: &SurrealStore,
    packages: impl IntoIterator<Item = (&'a str, &'a health_packages::Files)>,
    facility: &str,
) -> Value {
    let place = json!({
        "code": facility,
        "name": name_of(store, facility).await,
        "level": "facility",
        "parent": null,
    });
    let reports: Vec<Value> = facility_reports(packages)
        .iter()
        .map(|(package, version, report)| {
            listed_json(package, version, report, std::slice::from_ref(&place))
        })
        .collect();
    json!({ "where": "facility", "reports": reports })
}

/// What a Report run here reads: the active packages (each by its id with its files), their
/// indicators and the Age Band Sets they pin, and the facility's own Population Estimates.
pub struct Active<'a> {
    /// The active packages.
    pub packages: &'a [(String, health_packages::Files)],
    /// Their indicators.
    pub indicators: &'a [Indicator],
    /// The Age Band Sets their indicators pin.
    pub sets: &'a [AgeBandSet],
    /// The facility's own Population Estimates.
    pub estimates: &'a PopulationEstimates,
}

/// Report `body` (`{"report": "<package>:<id>", "place": <facility>, "period": "2026-03"}`) run
/// for `facility` over its own summaries in `store`, as of `now`.
pub async fn run_here(
    store: &SurrealStore,
    active: &Active<'_>,
    facility: &str,
    body: &Value,
    now: jiff::Timestamp,
) -> Result<Value, Refusal> {
    let (Some(wanted), Some(place), Some(period)) = (
        body["report"].as_str(),
        body["place"].as_str(),
        body["period"].as_str(),
    ) else {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "name the `report`, the `place` and the `period`".into(),
        ));
    };
    if place != facility {
        return Err((
            StatusCode::FORBIDDEN,
            format!("{place} is run at central: this node holds {facility}'s figures only"),
        ));
    }
    let reports = facility_reports(
        active
            .packages
            .iter()
            .map(|(id, files)| (id.as_str(), files)),
    );
    let Some((package, version, report)) = reports
        .iter()
        .find(|(package, _, report)| format!("{package}:{}", report.id) == wanted)
    else {
        return Err((
            StatusCode::NOT_FOUND,
            format!("{wanted} is not a Report run here"),
        ));
    };
    let period = Period::parse(report.period_type, period)
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;
    let name = name_of(store, facility).await;
    let names = [(facility.to_owned(), name.clone())].into();
    let mut tx = store.begin().await.map_err(unavailable)?;
    let mut own = OwnSummaries::new(&mut tx, facility, now);
    let run = RunPlace {
        place: ExplorePlace {
            code: facility.to_owned(),
            facilities: vec![facility.to_owned()],
        },
        name,
        level: Level::Facility,
    };
    let with = RunWith {
        indicators: active.indicators,
        sets: active.sets,
        estimates: active.estimates,
        names: &names,
    };
    let mut answer = run_report(&mut own, report, &run, &period, &with, "facility")
        .await
        .map_err(unavailable)?;
    answer["report"] = json!({
        "id": wanted,
        "title": report.title,
        "package": format!("{package}@{version}"),
    });
    Ok(answer)
}

#[derive(Clone)]
struct Explorer {
    store: SurrealStore,
    blobs: FsBlobStore,
    facility: String,
}

/// The routes over `store`, for `facility`, with the active packages' tarballs in `blobs`. They
/// expect the subject layer's `Subject`.
pub(crate) fn router(store: SurrealStore, blobs: FsBlobStore, facility: String) -> Router {
    Router::new()
        .route("/explorer", get(explorer_route))
        .route("/explore", post(explore_route))
        .route("/map", get(map_route))
        .route("/dashboards", get(dashboards_route))
        .route("/reports", get(reports_route))
        .route("/reports/run", post(run_route))
        .with_state(Explorer {
            store,
            blobs,
            facility,
        })
}

fn refused((status, detail): Refusal) -> Response {
    (status, Json(json!({ "detail": detail }))).into_response()
}

/// The refusal, unless the subject may explore.
fn allowed(subject: Option<Extension<Subject>>) -> Result<(), Refusal> {
    let Some(Extension(subject)) = subject else {
        return Err((StatusCode::UNAUTHORIZED, "sign in first".into()));
    };
    if !subject.permissions.has(VIEW_PERMISSION) {
        return Err((
            StatusCode::FORBIDDEN,
            "exploring indicators needs reporting.view".into(),
        ));
    }
    Ok(())
}

/// The subject allowed to explore, and the active indicators, or the refusal.
async fn ready(
    explorer: &Explorer,
    subject: Option<Extension<Subject>>,
) -> Result<Vec<Indicator>, Refusal> {
    allowed(subject)?;
    let active = crate::indicators::active(&explorer.store, &explorer.blobs, &explorer.facility)
        .await
        .map_err(unavailable)?;
    Ok(active.indicators().to_vec())
}

async fn explorer_route(
    State(explorer): State<Explorer>,
    subject: Option<Extension<Subject>>,
) -> Response {
    match ready(&explorer, subject).await {
        Ok(indicators) => {
            Json(explorer_here(&explorer.store, &indicators, &explorer.facility).await)
                .into_response()
        }
        Err(refusal) => refused(refusal),
    }
}

async fn dashboards_route(
    State(explorer): State<Explorer>,
    subject: Option<Extension<Subject>>,
) -> Response {
    if let Err(refusal) = allowed(subject) {
        return refused(refusal);
    }
    match crate::indicators::active_files(&explorer.store, &explorer.blobs).await {
        Ok(packages) => Json(dashboards_here(
            packages.iter().map(|(id, files)| (id.as_str(), files)),
            jiff::Zoned::now().date(),
        ))
        .into_response(),
        Err(error) => refused(unavailable(error)),
    }
}

async fn map_route(
    State(explorer): State<Explorer>,
    subject: Option<Extension<Subject>>,
) -> Response {
    match allowed(subject) {
        Err(refusal) => refused(refusal),
        Ok(()) => Json(map_here(&explorer.store, &explorer.facility).await).into_response(),
    }
}

async fn explore_route(
    State(explorer): State<Explorer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<Value>,
) -> Response {
    let indicators = match ready(&explorer, subject).await {
        Ok(indicators) => indicators,
        Err(refusal) => return refused(refusal),
    };
    let estimates = match crate::indicators::active_files(&explorer.store, &explorer.blobs).await {
        Ok(packages) => estimates_here(
            packages.iter().map(|(id, files)| (id.as_str(), files)),
            &explorer.facility,
        ),
        Err(error) => return refused(unavailable(error)),
    };
    match explore_here(
        &explorer.store,
        &indicators,
        &estimates,
        &explorer.facility,
        &body,
        jiff::Timestamp::now(),
    )
    .await
    {
        Ok(answer) => Json(answer).into_response(),
        Err(refusal) => refused(refusal),
    }
}

async fn reports_route(
    State(explorer): State<Explorer>,
    subject: Option<Extension<Subject>>,
) -> Response {
    if let Err(refusal) = allowed(subject) {
        return refused(refusal);
    }
    match crate::indicators::active_files(&explorer.store, &explorer.blobs).await {
        Ok(packages) => Json(
            reports_here(
                &explorer.store,
                packages.iter().map(|(id, files)| (id.as_str(), files)),
                &explorer.facility,
            )
            .await,
        )
        .into_response(),
        Err(error) => refused(unavailable(error)),
    }
}

async fn run_route(
    State(explorer): State<Explorer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<Value>,
) -> Response {
    if let Err(refusal) = allowed(subject) {
        return refused(refusal);
    }
    let indicators =
        match crate::indicators::active(&explorer.store, &explorer.blobs, &explorer.facility).await
        {
            Ok(indicators) => indicators,
            Err(error) => return refused(unavailable(error)),
        };
    let packages = match crate::indicators::active_files(&explorer.store, &explorer.blobs).await {
        Ok(packages) => packages,
        Err(error) => return refused(unavailable(error)),
    };
    let estimates = estimates_here(
        packages.iter().map(|(id, files)| (id.as_str(), files)),
        &explorer.facility,
    );
    let active = Active {
        packages: &packages,
        indicators: indicators.indicators(),
        sets: indicators.age_band_sets(),
        estimates: &estimates,
    };
    match run_here(
        &explorer.store,
        &active,
        &explorer.facility,
        &body,
        jiff::Timestamp::now(),
    )
    .await
    {
        Ok(answer) => Json(answer).into_response(),
        Err(refusal) => refused(refusal),
    }
}
