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
//! Each needs [`VIEW_PERMISSION`].

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_analytics::{Indicator, STANDARD_PERIODS};
use health_analytics_app::explore::{
    ExplorePlace, OwnSummaries, cells_json, explore, indicator_json, periods_json, read_question,
};
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

/// The answer to the question `body` over `facility`'s own summaries in `store`, offering
/// `indicators`, as of `now`.
pub async fn explore_here(
    store: &SurrealStore,
    indicators: &[Indicator],
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
    let cells = explore(&mut own, &question.indicators, &question.periods, &[place])
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
        .with_state(Explorer {
            store,
            blobs,
            facility,
        })
}

fn refused((status, detail): Refusal) -> Response {
    (status, Json(json!({ "detail": detail }))).into_response()
}

/// The subject allowed to explore, and the active indicators, or the refusal.
async fn ready(
    explorer: &Explorer,
    subject: Option<Extension<Subject>>,
) -> Result<Vec<Indicator>, Refusal> {
    let Some(Extension(subject)) = subject else {
        return Err((StatusCode::UNAUTHORIZED, "sign in first".into()));
    };
    if !subject.permissions.has(VIEW_PERMISSION) {
        return Err((
            StatusCode::FORBIDDEN,
            "exploring indicators needs reporting.view".into(),
        ));
    }
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

async fn map_route(
    State(explorer): State<Explorer>,
    subject: Option<Extension<Subject>>,
) -> Response {
    match subject {
        None => refused((StatusCode::UNAUTHORIZED, "sign in first".into())),
        Some(Extension(subject)) if !subject.permissions.has(VIEW_PERMISSION) => refused((
            StatusCode::FORBIDDEN,
            "exploring indicators needs reporting.view".into(),
        )),
        Some(_) => Json(map_here(&explorer.store, &explorer.facility).await).into_response(),
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
    match explore_here(
        &explorer.store,
        &indicators,
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
