//! Caregiver delegation over HTTP (#113; patient-identity.md § Caregiver delegation).
//!
//! - **Staff** (`caregiver_router`, behind `subject_layer`, needing [`CAREGIVER_PERMISSION`]):
//!   `GET /?patient={id}` lists a patient's caregivers; `POST /` records one, verified against
//!   the document checked; `POST /{id}/revoke` revokes it; `POST /{id}/adolescent-consent`
//!   records the adolescent's consent, given or withdrawn.
//! - **Patients** (`dependents_router`, behind `patient_layer`): `GET /dependents` lists the
//!   people the signed-in caregiver cares for.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::{PatientSubject, Subject};
use health_audit::{Agent, Auditor, Entity, Interaction};
use health_consent::{
    Authority, CaregiverGrant, Confidentiality, Day, Revoker, Scope, Verification,
};
use health_identity::PhoneNumber;
use health_identity_app::caregivers::{CaregiverAudit, CaregiverError, Caregivers, NewGrant};
use health_persistence::{RawJson, ResourceType, Store, Transaction, WriteError};
use health_surrealdb::{SurrealStore, SurrealTx};
use health_time::Timestamp;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::store::AuthStore;

/// The use-case permission a member of staff's roles must map to, to manage caregivers.
pub const CAREGIVER_PERMISSION: &str = "caregiver.manage";

/// Audits each version written, naming the request's agent.
pub(crate) struct CaregiverAuditor {
    auditor: Option<Arc<Auditor>>,
    agent: Agent,
}

impl CaregiverAudit<SurrealTx> for CaregiverAuditor {
    async fn written(
        &self,
        tx: &mut SurrealTx,
        _actor: &str,
        _reason: &str,
        json: &RawJson,
    ) -> Result<(), WriteError> {
        let Some(auditor) = &self.auditor else {
            return Ok(());
        };
        let value: Value = serde_json::from_str(json.as_str()).unwrap_or(Value::Null);
        let Some(entity) = Entity::of(&value) else {
            return Ok(());
        };
        let patient = value["subject"]["reference"]
            .as_str()
            .or_else(|| value["patient"]["reference"].as_str())
            .and_then(|r| r.strip_prefix("Patient/"))
            .map(str::to_owned);
        let event = auditor.event(
            &self.agent,
            Interaction::Update,
            &[entity],
            patient.as_deref(),
            Timestamp::now(),
            None,
        );
        auditor.append(tx, &event).await.map(|_| ())
    }
}

/// What the caregiver routes need.
pub struct CaregiverRoutes<S> {
    store: SurrealStore,
    accounts: S,
    auditor: Option<Arc<Auditor>>,
    policy: Confidentiality,
    country_code: String,
}

impl<S> Clone for CaregiverRoutes<S>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            accounts: self.accounts.clone(),
            auditor: self.auditor.clone(),
            policy: self.policy.clone(),
            country_code: self.country_code.clone(),
        }
    }
}

type UseCases = Caregivers<SurrealStore, CaregiverAuditor>;

impl<S: AuthStore + Clone + 'static> CaregiverRoutes<S> {
    /// The routes over the FHIR `store`, finding caregivers' phone accounts in `accounts`.
    pub fn new(
        store: SurrealStore,
        accounts: S,
        auditor: Option<Arc<Auditor>>,
        policy: Confidentiality,
        country_code: impl Into<String>,
    ) -> Self {
        Self {
            store,
            accounts,
            auditor,
            policy,
            country_code: country_code.into(),
        }
    }

    fn use_cases(&self, agent: Agent) -> UseCases {
        Caregivers::new(
            self.store.clone(),
            CaregiverAuditor {
                auditor: self.auditor.clone(),
                agent,
            },
            self.policy.clone(),
        )
    }

    /// The staff routes; mount them behind `subject_layer`.
    pub fn caregiver_router(&self) -> Router {
        Router::new()
            .route("/", get(list::<S>).post(record::<S>))
            .route("/{id}/revoke", post(revoke::<S>))
            .route("/{id}/adolescent-consent", post(adolescent::<S>))
            .with_state(self.clone())
    }

    /// The portal's dependents; mount it behind `patient_layer`.
    pub fn dependents_router(&self) -> Router {
        Router::new()
            .route("/dependents", get(dependents::<S>))
            .with_state(self.clone())
    }
}

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn failed(error: &CaregiverError) -> Response {
    match error {
        CaregiverError::NotFound => refused(StatusCode::NOT_FOUND, "no such caregiver grant"),
        CaregiverError::AlreadyGranted => refused(StatusCode::CONFLICT, &error.to_string()),
        CaregiverError::Invalid(message) => refused(StatusCode::UNPROCESSABLE_ENTITY, message),
        CaregiverError::Refused(refusal) => refused(StatusCode::FORBIDDEN, &refusal.to_string()),
        CaregiverError::Store(_) => refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
    }
}

/// The member of staff, or a refusal.
fn staff(
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
) -> Result<(Subject, Agent), Box<Response>> {
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(refused(StatusCode::UNAUTHORIZED, "sign in")));
    };
    if !subject.permissions.has(CAREGIVER_PERMISSION) {
        return Err(Box::new(refused(
            StatusCode::FORBIDDEN,
            "managing caregivers needs the caregiver.manage permission",
        )));
    }
    let agent = agent.map_or_else(
        || Agent {
            display: format!("account {}", subject.account),
            ..Agent::unauthenticated()
        },
        |Extension(agent)| agent,
    );
    Ok((subject, agent))
}

fn authority_code(authority: Authority) -> &'static str {
    match authority {
        Authority::Guardian => "guardian",
        Authority::Delegated => "delegated",
    }
}

/// A grant as the screens see it.
fn grant_json(grant: &CaregiverGrant, now: Timestamp) -> Value {
    let status = if grant.revoked.is_some() {
        "revoked"
    } else if grant.verification.is_none() {
        "unverified"
    } else if now >= grant.expires {
        "expired"
    } else {
        "active"
    };
    json!({
        "id": grant.id,
        "caregiver": grant.caregiver,
        "patient": grant.patient,
        "authority": authority_code(grant.authority),
        "scopes": grant.scopes.iter().map(|s| s.code()).collect::<Vec<_>>(),
        "expires": grant.expires.to_string(),
        "verified": match &grant.verification {
            Some(Verification::StaffChecked { document, .. }) => json!(document),
            Some(Verification::Delivery { .. }) => json!("delivery"),
            None => Value::Null,
        },
        "adolescent_consents": grant.adolescent_consents,
        "status": status,
    })
}

#[derive(Debug, Deserialize)]
struct ByPatient {
    patient: String,
}

async fn list<S: AuthStore + Clone + 'static>(
    State(routes): State<CaregiverRoutes<S>>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
    Query(query): Query<ByPatient>,
) -> Response {
    let agent = match staff(subject, agent) {
        Ok((_, agent)) => agent,
        Err(response) => return *response,
    };
    let now = Timestamp::now();
    match routes.use_cases(agent).of_patient(&query.patient).await {
        Ok(grants) => Json(Value::Array(
            grants.iter().map(|g| grant_json(g, now)).collect(),
        ))
        .into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Debug, Deserialize)]
struct RecordBody {
    patient: String,
    phone: String,
    relationship: String,
    authority: String,
    scopes: Vec<String>,
    /// The last day of access (`YYYY-MM-DD`).
    expires: String,
    document: String,
}

async fn record<S: AuthStore + Clone + 'static>(
    State(routes): State<CaregiverRoutes<S>>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
    Json(body): Json<RecordBody>,
) -> Response {
    let (subject, agent) = match staff(subject, agent) {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let Ok(phone) = PhoneNumber::parse(&body.phone, &routes.country_code) else {
        return refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "the phone number cannot be read",
        );
    };
    let account = match routes.accounts.phone_account(&phone).await {
        Ok(Some(account)) => account,
        Ok(None) => {
            return refused(
                StatusCode::UNPROCESSABLE_ENTITY,
                "no portal account has this phone: the caregiver signs in to the portal once first",
            );
        }
        Err(_) => return refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
    };
    let authority = match body.authority.as_str() {
        "guardian" => Authority::Guardian,
        "delegated" => Authority::Delegated,
        _ => {
            return refused(
                StatusCode::UNPROCESSABLE_ENTITY,
                "authority is guardian or delegated",
            );
        }
    };
    let Some(scopes) = body
        .scopes
        .iter()
        .map(|code| Scope::from_code(code))
        .collect::<Option<Vec<_>>>()
    else {
        return refused(StatusCode::UNPROCESSABLE_ENTITY, "an unknown scope");
    };
    // Access lasts to the end of its last day.
    let Ok(expires) = format!("{}T23:59:59Z", body.expires).parse::<Timestamp>() else {
        return refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "expires is a date (YYYY-MM-DD)",
        );
    };
    let new = NewGrant {
        caregiver: account.id.to_string(),
        phone: phone.to_string(),
        patient: body.patient,
        relationship: body.relationship,
        authority,
        scopes,
        expires,
        document: body.document,
    };
    match routes.use_cases(agent).record(&subject.account, new).await {
        Ok(grant) => (
            StatusCode::CREATED,
            Json(grant_json(&grant, Timestamp::now())),
        )
            .into_response(),
        Err(error) => failed(&error),
    }
}

/// Today, by UTC (staff revocation does not depend on the patient's age).
fn today(now: Timestamp) -> Day {
    let date = now.to_zoned(jiff::tz::TimeZone::UTC).date();
    Day::new(
        i32::from(date.year()),
        date.month().unsigned_abs(),
        date.day().unsigned_abs(),
    )
    .unwrap_or(Day {
        year: 1970,
        month: 1,
        day: 1,
    })
}

async fn revoke<S: AuthStore + Clone + 'static>(
    State(routes): State<CaregiverRoutes<S>>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
    Path(id): Path<String>,
) -> Response {
    let (subject, agent) = match staff(subject, agent) {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let now = Timestamp::now();
    let by = Revoker::Staff {
        staff: subject.account.clone(),
    };
    match routes
        .use_cases(agent)
        .revoke(&id, by, &subject.account, None, today(now), now)
        .await
    {
        Ok(grant) => Json(grant_json(&grant, now)).into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Debug, Deserialize)]
struct ConsentBody {
    consents: bool,
}

async fn adolescent<S: AuthStore + Clone + 'static>(
    State(routes): State<CaregiverRoutes<S>>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
    Path(id): Path<String>,
    Json(body): Json<ConsentBody>,
) -> Response {
    let (subject, agent) = match staff(subject, agent) {
        Ok(found) => found,
        Err(response) => return *response,
    };
    match routes
        .use_cases(agent)
        .adolescent_consent(&id, body.consents, &subject.account)
        .await
    {
        Ok(grant) => Json(grant_json(&grant, Timestamp::now())).into_response(),
        Err(error) => failed(&error),
    }
}

/// The patient's name, for the caregiver's list.
async fn patient_name(store: &SurrealStore, id: &str) -> Option<String> {
    let kind: ResourceType = "Patient".parse().ok()?;
    let mut tx = store.begin().await.ok()?;
    let stored = tx.read(&kind, id.parse().ok()?).await.ok()??;
    let json: Value = serde_json::from_str(stored.json.as_str()).ok()?;
    let name = &json["name"][0];
    let given = name["given"]
        .as_array()
        .map(|g| {
            g.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let full = format!("{given} {}", name["family"].as_str().unwrap_or_default());
    Some(full.trim().to_owned()).filter(|n| !n.is_empty())
}

async fn dependents<S: AuthStore + Clone + 'static>(
    State(routes): State<CaregiverRoutes<S>>,
    patient: Option<Extension<PatientSubject>>,
) -> Response {
    let Some(Extension(caregiver)) = patient else {
        return refused(StatusCode::UNAUTHORIZED, "sign in with your phone");
    };
    let agent = Agent {
        display: format!("patient account {}", caregiver.account),
        ..Agent::unauthenticated()
    };
    let now = Timestamp::now();
    match routes
        .use_cases(agent)
        .of_caregiver(&caregiver.account)
        .await
    {
        Ok(grants) => {
            let mut list = Vec::with_capacity(grants.len());
            for grant in grants.iter().filter(|g| g.revoked.is_none()) {
                let mut item = grant_json(grant, now);
                item["name"] = json!(patient_name(&routes.store, &grant.patient).await);
                // From the adolescent age, their sensitive records stay with them unless they
                // consent.
                item["confidential_withheld"] =
                    json!(grant.authority == Authority::Guardian && !grant.adolescent_consents);
                list.push(item);
            }
            Json(Value::Array(list)).into_response()
        }
        Err(error) => failed(&error),
    }
}
