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
//! - `POST /population/preview` (#553; ADR 0071) reads the HMIS lead's Population Estimates CSV
//!   against a Population package draft's estimates (`{"csv": "…", "estimates": {…}}`, the
//!   draft's `configuration/population.json`): each place code must be one the registry holds
//!   (the national core's areas and the NHFR's facilities), and a refused file names each row.
//!   It answers the estimates with the file's figures, for the Studio to save to the draft, and
//!   each district's annual totals, given and derived, to read before approval. Nothing is
//!   written. It needs the Studio's `metadata.author`.
//! - `GET /reporting` (#560; ADR 0069) answers each facility in the session's area with its HMIS
//!   reporting settings: its HMIS org unit ID, the NHFR's, what is wrong with it against the
//!   NHFR (`health_facility_registry::org_unit_flags`), and the forms (DHIS2 data sets, from
//!   central's recorded copy of the instance's metadata) reported through the platform. It
//!   needs `reporting.view`, so the district sees which forms it no longer keys, or
//!   [`MANAGE_PERMISSION`].
//! - `PUT /reporting/{facility}` sets them (`{"orgUnit": "…" | null, "forms": ["…"]}`), for a
//!   facility in the session's area, audited. It needs [`MANAGE_PERMISSION`]; settings that are
//!   not a DHIS2 ID, name a data set the recorded copy lacks, or report a form without an org
//!   unit ID are refused (422). An ID the NHFR does not know is kept, and flagged.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_analytics::{
    DataSetsExport, Dhis2Metadata, Estimates, EstimatesDefinition, parse_estimates_csv,
};
use health_audit::{Agent, Auditor, Entity, Interaction};
use health_facility_registry::{
    FacilityReporting, FacilityReportingStore as _, ImportPlan, Place, PlaceLevel, Places,
    org_unit_flags, parse_nhfr_csv, plan_import,
};
use health_persistence::{RawJson, Store as _, Transaction as _, WriteError};
use health_publication::Publications;
use health_registries_app::AuditTrail;
use health_registries_app::places::{apply_import, places};
use health_surrealdb::{SurrealStore, SurrealTx};
use health_time::Timestamp;
use serde_json::{Value, json};

/// The use-case permission that imports the NHFR's facility list, and sets each facility's HMIS
/// reporting settings.
pub(crate) const MANAGE_PERMISSION: &str = "places.manage";
/// The use-case permission that reads Reports, and so each facility's HMIS reporting settings.
const REPORTING_VIEW: &str = "reporting.view";

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
        .route("/population/preview", post(population_preview))
        .route("/reporting", get(reporting))
        .route("/reporting/{facility}", put(set_reporting))
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

/// `POST /population/preview`.
async fn population_preview(
    State(api): State<PlacesApi>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<Value>,
) -> Response {
    let subject = match signed_in(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    if !subject
        .permissions
        .has(health_publication::studio::AUTHOR_PERMISSION)
    {
        return problem(
            StatusCode::FORBIDDEN,
            "importing Population Estimates needs metadata.author",
        );
    }
    let not_estimates = |problems: Vec<String>| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "detail": "the Population Estimates cannot be imported",
                "problems": problems,
            })),
        )
            .into_response()
    };
    let Some(csv) = body["csv"].as_str() else {
        return not_estimates(vec!["the body names no csv".into()]);
    };
    let mut written = body["estimates"].clone();
    let definition: EstimatesDefinition = match serde_json::from_value(written.clone()) {
        Ok(definition) => definition,
        Err(e) => {
            return not_estimates(vec![format!("the draft's estimates cannot be read: {e}")]);
        }
    };
    let registry = match registry(&api.store).await {
        Ok(registry) => registry,
        Err(response) => return *response,
    };
    let figures = match parse_estimates_csv(csv, &definition, |code| registry.get(code).is_some()) {
        Ok(figures) => figures,
        Err(problems) => return not_estimates(problems),
    };
    let imported = EstimatesDefinition {
        figures,
        ..definition
    };
    written["figures"] = serde_json::to_value(&imported.figures).unwrap_or(Value::Null);
    let problems = imported.problems();
    let estimates = Estimates::new(imported, |code| {
        registry.get(code).and_then(|p| p.parent.clone())
    });
    Json(json!({
        "estimates": written,
        "figures": estimates.definition().figures.len(),
        "districts": district_totals(&registry, &estimates),
        "problems": problems,
    }))
    .into_response()
}

/// Each district with an estimate, by code: its name, its region and its annual figure for each
/// estimate type, given or derived, or none (*no estimate*).
fn district_totals(registry: &Places, estimates: &Estimates) -> Vec<Value> {
    registry
        .iter()
        .filter(|place| place.level == PlaceLevel::District)
        .filter_map(|district| {
            let totals: Vec<Value> = estimates
                .definition()
                .types
                .iter()
                .map(|kind| {
                    json!({
                        "type": kind.code,
                        "title": kind.title,
                        "annual": estimates
                            .annual(&district.code, &kind.code)
                            .map(|a| a.round_dp(0).normalize().to_string()),
                    })
                })
                .collect();
            totals.iter().any(|t| !t["annual"].is_null()).then(|| {
                json!({
                    "code": district.code,
                    "name": district.name,
                    "region": district.parent,
                    "totals": totals,
                })
            })
        })
        .collect()
}

/// Central's recorded copy of the DHIS2 instance's metadata, if one is recorded (#560).
async fn dhis2_metadata(tx: &mut SurrealTx) -> Result<Option<Dhis2Metadata>, Box<Response>> {
    let export = tx
        .dhis2_metadata()
        .await
        .map_err(|e| Box::new(unavailable(e)))?;
    Ok(export.and_then(|text| {
        serde_json::from_str::<DataSetsExport>(&text)
            .ok()
            .map(|e| e.metadata())
    }))
}

/// A facility's row of the HMIS reporting settings: the place, its settings and their flags.
fn reporting_row(place: &Place, all: &[FacilityReporting], registry: &Places) -> Value {
    let setting = all
        .iter()
        .find(|s| s.facility == place.code)
        .cloned()
        .unwrap_or_else(|| FacilityReporting {
            facility: place.code.clone(),
            ..FacilityReporting::default()
        });
    json!({
        "code": place.code,
        "name": place.name,
        "district": place.parent,
        "active": place.active,
        "nhfrOrgUnit": place.org_unit,
        "orgUnit": setting.org_unit,
        "forms": setting.forms,
        "flags": org_unit_flags(registry, all, &setting),
    })
}

/// `GET /reporting`.
async fn reporting(State(api): State<PlacesApi>, subject: Option<Extension<Subject>>) -> Response {
    let subject = match signed_in(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let manage = subject.permissions.has(MANAGE_PERMISSION);
    if !manage && !subject.permissions.has(REPORTING_VIEW) {
        return problem(
            StatusCode::FORBIDDEN,
            "the facilities' HMIS reporting settings need reporting.view or places.manage",
        );
    }
    let registry = match registry(&api.store).await {
        Ok(registry) => registry,
        Err(response) => return *response,
    };
    let mut tx = match api.store.begin().await {
        Ok(tx) => tx,
        Err(e) => return unavailable(e),
    };
    let all = match tx.facility_reporting().await {
        Ok(all) => all,
        Err(e) => return unavailable(e),
    };
    let metadata = match dhis2_metadata(&mut tx).await {
        Ok(metadata) => metadata,
        Err(response) => return *response,
    };
    let forms: Vec<Value> = metadata
        .iter()
        .flat_map(|m| &m.data_sets)
        .map(|(id, set)| json!({ "id": id, "name": set.name, "periodType": set.period_type }))
        .collect();
    let facilities: Vec<Value> = registry
        .area(subject.facility.as_str())
        .into_iter()
        .filter(|p| p.level == PlaceLevel::Facility)
        .map(|place| reporting_row(place, &all, &registry))
        .collect();
    Json(json!({
        "manage": manage,
        "metadataRecorded": metadata.is_some(),
        "forms": forms,
        "facilities": facilities,
    }))
    .into_response()
}

/// The body of `PUT /reporting/{facility}`.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetReporting {
    #[serde(default)]
    org_unit: Option<String>,
    #[serde(default)]
    forms: std::collections::BTreeSet<String>,
}

/// `PUT /reporting/{facility}`.
async fn set_reporting(
    State(api): State<PlacesApi>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
    Path(facility): Path<String>,
    body: Bytes,
) -> Response {
    let subject = match signed_in(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    if !subject.permissions.has(MANAGE_PERMISSION) {
        return problem(
            StatusCode::FORBIDDEN,
            "setting a facility's HMIS reporting needs places.manage",
        );
    }
    let Ok(body) = serde_json::from_slice::<SetReporting>(&body) else {
        return problem(
            StatusCode::BAD_REQUEST,
            "the body is not {\"orgUnit\": …, \"forms\": […]}",
        );
    };
    let registry = match registry(&api.store).await {
        Ok(registry) => registry,
        Err(response) => return *response,
    };
    if !registry.within(&facility, subject.facility.as_str()) {
        return problem(
            StatusCode::FORBIDDEN,
            format!("{facility} is not in your area"),
        );
    }
    let setting = FacilityReporting {
        facility: facility.clone(),
        org_unit: body
            .org_unit
            .map(|o| o.trim().to_owned())
            .filter(|o| !o.is_empty()),
        forms: body.forms,
    };
    let mut tx = match api.store.begin().await {
        Ok(tx) => tx,
        Err(e) => return unavailable(e),
    };
    let metadata = match dhis2_metadata(&mut tx).await {
        Ok(metadata) => metadata,
        Err(response) => return *response,
    };
    let data_sets = metadata
        .map(|m| m.data_sets.into_keys().collect())
        .unwrap_or_default();
    let problems = setting.problems(&registry, &data_sets);
    if !problems.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "detail": "the HMIS reporting settings cannot be kept",
                "problems": problems,
            })),
        )
            .into_response();
    }
    if let Err(e) = tx.put_facility_reporting(&setting, &subject.account).await {
        return unavailable(e);
    }
    let event = reporting_audit(agent, &subject, &setting);
    if let Err(e) = health_audit::append(&mut tx, &event).await {
        return unavailable(e);
    }
    let all = match tx.facility_reporting().await {
        Ok(all) => all,
        Err(e) => return unavailable(e),
    };
    if let Err(e) = tx.commit().await {
        return unavailable(e);
    }
    match registry.get(&facility) {
        Some(place) => Json(reporting_row(place, &all, &registry)).into_response(),
        None => unavailable("a facility in the area is missing"),
    }
}

/// The AuditEvent of setting `setting`, by the signed-in account.
fn reporting_audit(
    agent: Option<Extension<Agent>>,
    subject: &Subject,
    setting: &FacilityReporting,
) -> Value {
    let who = agent.map_or_else(
        || json!({ "display": format!("account {}", subject.account) }),
        |Extension(agent)| {
            let mut who = json!({ "display": agent.display });
            if let Some(reference) = agent.who {
                who["reference"] = json!(reference);
            }
            who
        },
    );
    let forms: Vec<&str> = setting.forms.iter().map(String::as_str).collect();
    json!({
        "resourceType": "AuditEvent",
        "category": [{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/audit-event-type", "code": "rest", "display": "Restful Operation" }] }],
        "code": { "coding": [{ "system": "urn:health:audit-event", "code": "facility-reporting" }] },
        "action": "U",
        "severity": "notice",
        "recorded": Timestamp::now().to_string(),
        "outcome": { "code": { "system": "http://terminology.hl7.org/CodeSystem/audit-event-outcome", "code": "0" } },
        "agent": [{ "who": who, "requestor": true }],
        "source": { "observer": { "display": "central" } },
        "entity": [{
            "what": { "display": format!("facility {}", setting.facility) },
            "detail": [
                { "type": { "text": "org unit" }, "valueString": setting.org_unit.as_deref().unwrap_or("none") },
                { "type": { "text": "forms reported through the platform" }, "valueString": if forms.is_empty() { "none".to_owned() } else { forms.join(", ") } },
            ],
        }],
    })
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
