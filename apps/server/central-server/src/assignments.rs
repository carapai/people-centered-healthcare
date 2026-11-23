//! Role assignment at a place (ADR 0094; #873), under `/api/assignments/`, for an administrator
//! whose session gives [`ADMINISTER`]. A role is held at a place of the facility registry: a
//! facility, a district, a region or the national place (`central`). The administrator assigns
//! only within their own area: the session's place and everything below it.
//!
//! - `GET /options`: the roles the access policy defines, and the places of the administrator's
//!   area, the session's place first.
//! - `GET /users/{username}`: the places the user holds roles at, with the roles and when each
//!   snapshot expires; `manageable` says whether the place lies in the administrator's area.
//! - `PUT /users/{username}/{place}` (`{"roles": [...]}`): replaces the user's roles at the place
//!   with a newly signed permission snapshot, keeping its program assignments and each kept
//!   role's locations. The user signs in for the place from their next sign-in.
//! - `DELETE /users/{username}/{place}`: withdraws every role the user holds there.
//!
//! Each change is audited with its snapshot, in one transaction.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_audit::Agent;
use health_auth::{AuthStore as _, SnapshotSigner, SnapshotVerifier, SurrealAuthStore};
use health_facility_registry::{NATIONAL, Place, Places};
use health_identity::AccountId;
use health_identity::snapshot::{
    ADMINISTER, PermissionSnapshot, RoleAssignment, RolePermissions, SnapshotPolicy,
};
use health_persistence::Store as _;
use health_registries_app::places::places;
use health_surrealdb::SurrealStore;
use health_time::Timestamp;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone)]
struct AssignmentsApi {
    accounts: SurrealAuthStore,
    store: SurrealStore,
    signer: Arc<SnapshotSigner>,
    verifier: Arc<SnapshotVerifier>,
    roles: Arc<RolePermissions>,
}

/// The routes over central's `accounts` and the facility registry in `store`, signing snapshots
/// with `signer` under the policy's `roles` and reading them back with `verifier`. They expect the
/// subject layer's `Subject` and `Agent`.
pub(crate) fn router(
    accounts: SurrealAuthStore,
    store: SurrealStore,
    signer: SnapshotSigner,
    verifier: SnapshotVerifier,
    roles: RolePermissions,
) -> Router {
    Router::new()
        .route("/options", get(options))
        .route("/users/{username}", get(user))
        .route("/users/{username}/{place}", put(assign).delete(withdraw))
        .with_state(AssignmentsApi {
            accounts,
            store,
            signer: Arc::new(signer),
            verifier: Arc::new(verifier),
            roles: Arc::new(roles),
        })
}

fn problem(status: StatusCode, detail: impl Into<String>) -> Response {
    (status, Json(json!({ "detail": detail.into() }))).into_response()
}

fn unavailable(error: impl std::fmt::Display) -> Response {
    tracing::warn!(%error, "role assignment is unavailable");
    problem(
        StatusCode::SERVICE_UNAVAILABLE,
        "role assignment is unavailable",
    )
}

/// The signed-in administrator, or the refusal.
fn administrator(subject: Option<Extension<Subject>>) -> Result<Subject, Box<Response>> {
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(problem(StatusCode::UNAUTHORIZED, "sign in first")));
    };
    if !subject.permissions.has(ADMINISTER) {
        return Err(Box::new(problem(
            StatusCode::FORBIDDEN,
            "assigning roles needs access.administer",
        )));
    }
    Ok(subject)
}

async fn registry(store: &SurrealStore) -> Result<Places, Box<Response>> {
    let mut tx = store.begin().await.map_err(|e| Box::new(unavailable(e)))?;
    places(&mut tx).await.map_err(|e| Box::new(unavailable(e)))
}

/// The place with `code` as the registry holds it, or as a bare place of its level when it does
/// not (the national place before any import, a facility not imported yet).
fn place_of(registry: &Places, code: &str) -> Place {
    registry.get(code).cloned().unwrap_or_else(|| Place {
        code: code.to_owned(),
        name: code.to_owned(),
        level: registry.level_of(code),
        parent: None,
        active: true,
        care_level: None,
        ownership: None,
        coordinates: None,
    })
}

/// Whether `code` is a place the administrator at `session` may assign roles at: in the
/// registry (or the national place), and in the session's area.
fn manageable(registry: &Places, session: &str, code: &str) -> bool {
    (code == NATIONAL || registry.get(code).is_some() || code == session)
        && registry.within(code, session)
}

/// `GET /options`.
async fn options(
    State(api): State<AssignmentsApi>,
    subject: Option<Extension<Subject>>,
) -> Response {
    let subject = match administrator(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let registry = match registry(&api.store).await {
        Ok(registry) => registry,
        Err(response) => return *response,
    };
    let session = subject.facility.as_str();
    let mut area: Vec<Place> = registry.area(session).into_iter().cloned().collect();
    if area.is_empty() {
        area.push(place_of(&registry, session));
    }
    Json(json!({
        "roles": api.roles.0.keys().collect::<Vec<_>>(),
        "places": area,
    }))
    .into_response()
}

/// The account named `username`, or 404.
async fn account(api: &AssignmentsApi, username: &str) -> Result<AccountId, Box<Response>> {
    match api.accounts.account_by_username(username).await {
        Ok(Some(account)) => Ok(account.id),
        Ok(None) => Err(Box::new(problem(
            StatusCode::NOT_FOUND,
            format!("there is no user {username}"),
        ))),
        Err(error) => Err(Box::new(unavailable(error))),
    }
}

/// `account`'s snapshot at `place`, if central holds a valid one.
async fn snapshot(
    api: &AssignmentsApi,
    account: AccountId,
    place: &str,
) -> Result<Option<PermissionSnapshot>, Box<Response>> {
    let jws = api
        .accounts
        .snapshot(account, place)
        .await
        .map_err(|e| Box::new(unavailable(e)))?;
    Ok(jws.and_then(|jws| api.verifier.verify(&jws).ok()))
}

/// `GET /users/{username}`.
async fn user(
    State(api): State<AssignmentsApi>,
    subject: Option<Extension<Subject>>,
    Path(username): Path<String>,
) -> Response {
    let subject = match administrator(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let account = match account(&api, &username).await {
        Ok(account) => account,
        Err(response) => return *response,
    };
    let registry = match registry(&api.store).await {
        Ok(registry) => registry,
        Err(response) => return *response,
    };
    let codes = match api.accounts.facilities(account).await {
        Ok(codes) => codes,
        Err(error) => return unavailable(error),
    };
    let mut assignments = Vec::new();
    for code in codes {
        let held = match snapshot(&api, account, &code).await {
            Ok(held) => held,
            Err(response) => return *response,
        };
        let Some(held) = held else { continue };
        assignments.push(json!({
            "place": place_of(&registry, &code),
            "roles": held.roles.iter().map(|r| r.role.as_str()).collect::<Vec<_>>(),
            "not_after": held.not_after.to_string(),
            "manageable": manageable(&registry, &subject.facility, &code),
        }));
    }
    Json(json!({
        "username": username,
        "account": account.to_string(),
        "assignments": assignments,
    }))
    .into_response()
}

/// The body of `PUT /users/{username}/{place}`.
#[derive(Deserialize)]
struct Assign {
    roles: Vec<String>,
}

/// The audit of an assignment, like an edge's offline administration (health-auth `local_admin`).
fn audit(
    agent: Option<Extension<Agent>>,
    subject: &Subject,
    action: &str,
    account: AccountId,
    detail: &str,
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
    json!({
        "resourceType": "AuditEvent",
        "category": [{ "coding": [{ "system": "http://dicom.nema.org/resources/ontology/DCM", "code": "110137", "display": "User Security Attributes Changed" }] }],
        "code": { "coding": [{ "system": "urn:health:audit-event", "code": "role-assignment" }] },
        "action": action,
        "severity": "notice",
        "recorded": Timestamp::now().to_string(),
        "outcome": { "code": { "system": "http://terminology.hl7.org/CodeSystem/audit-event-outcome", "code": "0" } },
        "agent": [{ "who": who, "requestor": true }],
        "source": { "observer": { "display": "central" } },
        "entity": [{
            "what": { "display": format!("account {account}") },
            "detail": [{ "type": { "text": "change" }, "valueString": detail }],
        }],
    })
}

/// The target of a change: the account and the place, if the administrator may manage it.
async fn target(
    api: &AssignmentsApi,
    subject: &Subject,
    username: &str,
    place: &str,
) -> Result<AccountId, Box<Response>> {
    let account = account(api, username).await?;
    let registry = registry(&api.store).await?;
    if place != NATIONAL && registry.get(place).is_none() && place != subject.facility {
        return Err(Box::new(problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("the facility registry holds no place {place}"),
        )));
    }
    if !manageable(&registry, &subject.facility, place) {
        return Err(Box::new(problem(
            StatusCode::FORBIDDEN,
            format!("{place} is outside your area"),
        )));
    }
    Ok(account)
}

/// `PUT /users/{username}/{place}`.
async fn assign(
    State(api): State<AssignmentsApi>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
    Path((username, place)): Path<(String, String)>,
    Json(body): Json<Assign>,
) -> Response {
    let subject = match administrator(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let mut roles: Vec<String> = body.roles.iter().map(|r| r.trim().to_owned()).collect();
    roles.retain(|r| !r.is_empty());
    roles.sort();
    roles.dedup();
    if roles.is_empty() {
        return problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "name at least one role; withdraw the place to remove them all",
        );
    }
    if let Some(unknown) = roles.iter().find(|r| !api.roles.0.contains_key(*r)) {
        return problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("the access policy defines no role {unknown}"),
        );
    }
    let account = match target(&api, &subject, &username, &place).await {
        Ok(account) => account,
        Err(response) => return *response,
    };
    // What the user holds there now: its programs and each kept role's locations stay.
    let held = match snapshot(&api, account, &place).await {
        Ok(held) => held,
        Err(response) => return *response,
    };
    let issued = Timestamp::now();
    let snapshot = PermissionSnapshot {
        account,
        facility: place.clone(),
        roles: roles
            .iter()
            .map(|role| RoleAssignment {
                role: role.clone(),
                locations: held
                    .as_ref()
                    .and_then(|h| h.roles.iter().find(|r| &r.role == role))
                    .map(|r| r.locations.clone())
                    .unwrap_or_default(),
            })
            .collect(),
        programs: held.map(|h| h.programs).unwrap_or_default(),
        issued_at: issued,
        not_after: SnapshotPolicy::default().not_after(issued),
        snapshot_version: u64::try_from(issued.as_second()).unwrap_or(0),
    };
    let jws = match api.signer.sign(&snapshot, &api.roles) {
        Ok(jws) => jws,
        Err(error) => return unavailable(error),
    };
    let detail = format!("roles at {place}: {}", roles.join(", "));
    let event = audit(agent, &subject, "U", account, &detail);
    if let Err(error) = api
        .accounts
        .assign(account, &place, Some(&jws), &event)
        .await
    {
        return unavailable(error);
    }
    Json(json!({
        "username": username,
        "place": place,
        "roles": roles,
        "not_after": snapshot.not_after.to_string(),
    }))
    .into_response()
}

/// `DELETE /users/{username}/{place}`.
async fn withdraw(
    State(api): State<AssignmentsApi>,
    subject: Option<Extension<Subject>>,
    agent: Option<Extension<Agent>>,
    Path((username, place)): Path<(String, String)>,
) -> Response {
    let subject = match administrator(subject) {
        Ok(subject) => subject,
        Err(response) => return *response,
    };
    let account = match target(&api, &subject, &username, &place).await {
        Ok(account) => account,
        Err(response) => return *response,
    };
    match api.accounts.snapshot(account, &place).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return problem(
                StatusCode::NOT_FOUND,
                format!("{username} holds no role at {place}"),
            );
        }
        Err(error) => return unavailable(error),
    }
    let event = audit(
        agent,
        &subject,
        "D",
        account,
        &format!("every role at {place} withdrawn"),
    );
    match api.accounts.assign(account, &place, None, &event).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => unavailable(error),
    }
}
