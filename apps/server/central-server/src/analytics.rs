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
//! Both read summaries only, never patient-level records, and need [`VIEW_PERMISSION`].

use std::collections::BTreeMap;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_analytics::{Indicator, STANDARD_PERIODS};
use health_analytics_app::explore::{
    ExplorePlace, cells_json, explore, indicator_json, periods_json, read_question,
};
use health_facility_registry::{Place, PlaceLevel, Places};
use health_persistence::Store as _;
use health_publication::Publications;
use health_registries_app::places::places;
use health_surrealdb::SurrealStore;
use serde_json::{Value, json};

/// The use-case permission that reads Indicator Reports (ADR 0070): Reports and dashboards.
pub(crate) const VIEW_PERMISSION: &str = "reporting.view";

#[derive(Clone)]
struct AnalyticsApi {
    store: SurrealStore,
    catalogue: Catalogue,
}

/// The routes over `store`, offering `catalogue`'s indicators. They expect the subject layer's
/// `Subject`.
pub(crate) fn router(store: SurrealStore, catalogue: Catalogue) -> Router {
    Router::new()
        .route("/explorer", get(explorer))
        .route("/explore", post(explore_route))
        .with_state(AnalyticsApi { store, catalogue })
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
    let mut tx = match api.store.begin().await {
        Ok(tx) => tx,
        Err(error) => return unavailable(error),
    };
    let cells = match explore(&mut tx, &question.indicators, &question.periods, &picked).await {
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

// After the routes: the route inventory reads this file up to its first test-only item.
/// Where the indicators offered come from.
#[derive(Clone)]
pub(crate) enum Catalogue {
    /// Every published package version's.
    Published(Option<Publications>),
    /// A fixed list, for tests.
    #[cfg(test)]
    Fixed(Vec<Indicator>),
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
            Self::Fixed(indicators) => Ok(indicators.clone()),
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
    use health_persistence::Transaction as _;
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
            value: None,
        }
    }

    fn visits() -> Indicator {
        indicator("anc1", Calculation::Count)
    }

    fn weeks() -> Indicator {
        indicator("gestational-age", Calculation::Value(Aggregate::Median))
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
            "nhfr_id,name,district,status\n\
             KAM013,Kisenyi HC IV,district-kampala,open\n\
             KAM020,Kawempe HC III,district-kampala,open\n\
             KAM031,Kitebi HC III,district-kampala,open\n\
             WAK001,Wakiso HC IV,district-wakiso,open\n",
        )
        .expect("rows");
        let plan = plan_import(&Places::default(), &areas, &rows).expect("plan");
        health_registries_app::places::apply_import(&store, &NoAudit, "test", &plan)
            .await
            .expect("import");
        let march = Period::parse(PeriodType::Monthly, "2026-03").expect("march");
        for (facility, count) in [("KAM013", 4), ("KAM020", 6), ("WAK001", 50)] {
            let mut tx = store.begin().await.expect("begin");
            for indicator in [visits(), weeks()] {
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
        let app = router(store.clone(), Catalogue::Fixed(vec![visits(), weeks()]))
            .layer(Extension(subject));
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
}
