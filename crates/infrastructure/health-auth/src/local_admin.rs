//! Offline user and role administration on an edge (offline-auth.md § Offline user and role
//! administration; ADR 0011): routes for a signed-in facility admin whose snapshot grants
//! `access.administer-local` under the country's role mappings, which central provisions in its
//! bundle ([`LocalConfig`](crate::LocalConfig)).
//!
//! - `GET /access/roles`: the roles this edge may grant offline, those on the country's locally
//!   grantable list that carry no never-offline permission.
//! - `GET /access/grants`, `POST /access/grants` (`username`, `role`, `days`): roles granted
//!   locally, only from the country's locally grantable list, never a role carrying a
//!   never-offline permission, for at most 7 days. Each grant is signed with the node's key for
//!   central to ratify, and audited.
//! - `GET /access/revocations`, `POST /access/revocations` (`username`): local access revoked at
//!   once: the user's tokens stop working at their next request, and they cannot sign in here.
//! - `POST /access/accounts` (`username`, `password`, `role`, `days`): a provisional account for
//!   new staff, its role a local grant, lasting as long; its user sets their own password at
//!   their first sign-in.
//!
//! Grants and revocations are listed with their user's `username` where the edge knows it.

use std::sync::Arc;

use axum::Router;
use axum::extract::{OriginalUri, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Form, Json};
use health_identity::AccountId;
use health_identity::snapshot::{
    ADMINISTER_LOCAL, LocalGrant, LocalGrantPolicy, LocalRevocation, NEVER_OFFLINE,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::server::{AccessVerifier, Principal};
use crate::store::AuthStore;

/// Signs a local grant's claims with the node's key (its facility intermediate), for central to
/// ratify. Blocking: it may unseal the key from the platform keystore.
pub type GrantSigner = Arc<dyn Fn(&Value) -> Result<String, String> + Send + Sync>;

/// The routes, over the authorization server's `verifier`.
pub struct LocalAdmin<S> {
    verifier: AccessVerifier<S>,
    signer: GrantSigner,
    observer: String,
}

impl<S> Clone for LocalAdmin<S> {
    fn clone(&self) -> Self {
        Self {
            verifier: self.verifier.clone(),
            signer: Arc::clone(&self.signer),
            observer: self.observer.clone(),
        }
    }
}

fn error(status: StatusCode, error: &str, detail: impl Into<String>) -> Response {
    let no_store = [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))];
    (
        status,
        no_store,
        Json(json!({ "error": error, "detail": detail.into() })),
    )
        .into_response()
}

/// A signed-in facility admin, and where they administer.
struct Admin {
    principal: Principal,
    facility: String,
}

impl<S: AuthStore + 'static> LocalAdmin<S> {
    /// The routes over `verifier`, signing grants with `signer`, audited as `observer`.
    pub fn new(
        verifier: AccessVerifier<S>,
        signer: GrantSigner,
        observer: impl Into<String>,
    ) -> Self {
        Self {
            verifier,
            signer,
            observer: observer.into(),
        }
    }

    /// `/access/roles`, `/access/grants`, `/access/revocations` and `/access/accounts`; nest
    /// them where the API is.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/access/roles", get(roles::<S>))
            .route("/access/grants", get(grants::<S>).post(grant::<S>))
            .route(
                "/access/revocations",
                get(revocations::<S>).post(revoke::<S>),
            )
            .route("/access/accounts", axum::routing::post(provision::<S>))
            .with_state(self.clone())
    }

    /// The request's admin: a live token whose effective snapshot, under the provisioned role
    /// mappings, grants `access.administer-local`.
    async fn admin(
        &self,
        headers: &HeaderMap,
        method: &str,
        path: &str,
    ) -> Result<Admin, Box<Response>> {
        let uri = format!("{}{path}", self.verifier.origin());
        let principal = self
            .verifier
            .verify(headers, method, &uri)
            .await
            .map_err(|e| Box::new(e.into_response()))?;
        let facility = principal.facility.clone().ok_or_else(|| {
            error(
                StatusCode::FORBIDDEN,
                "forbidden",
                "the session has no facility",
            )
        })?;
        let snapshot = self
            .verifier
            .current_snapshot(principal.account, &facility)
            .await
            .map_err(|e| Box::new(e.into_response()))?;
        let config = self
            .verifier
            .store()
            .local_config()
            .await
            .map_err(|_| {
                error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "unavailable",
                    "the store failed",
                )
            })?
            .unwrap_or_default();
        if !config.roles.resolve(&snapshot).contains(ADMINISTER_LOCAL) {
            return Err(Box::new(error(
                StatusCode::FORBIDDEN,
                "forbidden",
                "offline administration needs access.administer-local",
            )));
        }
        Ok(Admin {
            principal,
            facility,
        })
    }

    fn audit(
        &self,
        code: &str,
        action: &str,
        admin: AccountId,
        subject: AccountId,
        detail: &str,
    ) -> Value {
        json!({
            "resourceType": "AuditEvent",
            "category": [{ "coding": [{ "system": "http://dicom.nema.org/resources/ontology/DCM", "code": "110137", "display": "User Security Attributes Changed" }] }],
            "code": { "coding": [{ "system": "urn:health:audit-event", "code": code }] },
            "action": action,
            "severity": "notice",
            "recorded": self.verifier.now().to_string(),
            "outcome": { "code": { "system": "http://terminology.hl7.org/CodeSystem/audit-event-outcome", "code": "0" } },
            "agent": [{ "who": { "display": format!("account {admin}") }, "requestor": true }],
            "source": { "observer": { "display": self.observer } },
            "entity": [{
                "what": { "display": format!("account {subject}") },
                "detail": [{ "type": { "text": "change" }, "valueString": detail }],
            }],
        })
    }

    /// The username of `account`, if the edge knows it.
    async fn username(&self, account: AccountId) -> Option<String> {
        match self.verifier.store().account(account).await {
            Ok(Some(login)) => Some(login.username),
            _ => None,
        }
    }

    async fn account(&self, username: &str) -> Result<AccountId, Box<Response>> {
        match self
            .verifier
            .store()
            .account_by_username(username.trim())
            .await
        {
            Ok(Some(account)) => Ok(account.id),
            Ok(None) => Err(Box::new(error(
                StatusCode::NOT_FOUND,
                "no-such-user",
                "no account by that username",
            ))),
            Err(_) => Err(Box::new(error(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "the store failed",
            ))),
        }
    }
}

/// `GET /access/roles`: the roles this edge may grant offline.
async fn roles<S: AuthStore + 'static>(
    State(admin): State<LocalAdmin<S>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    if let Err(refused) = admin.admin(&headers, "GET", uri.path()).await {
        return *refused;
    }
    match admin.verifier.store().local_config().await {
        Ok(config) => {
            let config = config.unwrap_or_default();
            let grantable: Vec<&str> = config
                .grantable
                .iter()
                .filter(|role| {
                    config
                        .roles
                        .0
                        .get(role.as_str())
                        .is_some_and(|permissions| {
                            !permissions
                                .iter()
                                .any(|p| NEVER_OFFLINE.contains(&p.as_str()))
                        })
                })
                .map(String::as_str)
                .collect();
            Json(json!(grantable)).into_response()
        }
        Err(_) => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "the store failed",
        ),
    }
}

fn grant_json(grant: &LocalGrant, username: Option<&str>) -> Value {
    json!({
        "id": grant.id.to_string(),
        "account": grant.account.to_string(),
        "username": username,
        "role": grant.role,
        "granted_by": grant.granted_by.to_string(),
        "granted_at": grant.granted_at.to_string(),
        "not_after": grant.not_after.to_string(),
    })
}

async fn grants<S: AuthStore + 'static>(
    State(admin): State<LocalAdmin<S>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let who = match admin.admin(&headers, "GET", uri.path()).await {
        Ok(who) => who,
        Err(refused) => return *refused,
    };
    match admin
        .verifier
        .store()
        .local_grants(&who.facility, None, admin.verifier.now())
        .await
    {
        Ok(grants) => {
            let mut listed = Vec::with_capacity(grants.len());
            for grant in &grants {
                let username = admin.username(grant.account).await;
                listed.push(grant_json(grant, username.as_deref()));
            }
            Json(listed).into_response()
        }
        Err(_) => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "the store failed",
        ),
    }
}

#[derive(Deserialize)]
struct GrantForm {
    username: String,
    role: String,
    days: i64,
}

async fn grant<S: AuthStore + 'static>(
    State(admin): State<LocalAdmin<S>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Form(form): Form<GrantForm>,
) -> Response {
    let who = match admin.admin(&headers, "POST", uri.path()).await {
        Ok(who) => who,
        Err(refused) => return *refused,
    };
    let account = match admin.account(&form.username).await {
        Ok(account) => account,
        Err(refused) => return *refused,
    };
    let store = admin.verifier.store();
    // Only staff already provisioned to this facility (offline-auth.md).
    match store.snapshot(account, &who.facility).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "not-provisioned",
                "the user is not provisioned to this facility",
            );
        }
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "the store failed",
            );
        }
    }
    let config = match store.local_config().await {
        Ok(config) => config.unwrap_or_default(),
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "the store failed",
            );
        }
    };
    let now = admin.verifier.now();
    let grant = match LocalGrantPolicy::default().grant(
        form.role.trim(),
        &config.roles,
        &config.grantable,
        account,
        &who.facility,
        who.principal.account,
        form.days,
        now,
    ) {
        Ok(grant) => grant,
        Err(refused) => {
            return error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "refused",
                refused.to_string(),
            );
        }
    };
    let claims = match serde_json::to_value(&grant) {
        Ok(claims) => claims,
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "the grant cannot be encoded",
            );
        }
    };
    let signer = Arc::clone(&admin.signer);
    let jws = match tokio::task::spawn_blocking(move || signer(&claims)).await {
        Ok(Ok(jws)) => jws,
        _ => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "the node's key cannot sign",
            );
        }
    };
    let audit = admin.audit(
        "local-grant",
        "C",
        who.principal.account,
        account,
        &format!("{} until {}", grant.role, grant.not_after),
    );
    match store.put_local_grant(&grant, &jws, &audit).await {
        Ok(()) => {
            tracing::warn!(alert = "local-grant", role = %grant.role, "a role was granted offline, pending ratification");
            (
                StatusCode::CREATED,
                Json(grant_json(&grant, Some(form.username.trim()))),
            )
                .into_response()
        }
        Err(_) => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "the store failed",
        ),
    }
}

fn revocation_json(revocation: &LocalRevocation, username: Option<&str>) -> Value {
    json!({
        "account": revocation.account.to_string(),
        "username": username,
        "revoked_by": revocation.revoked_by.to_string(),
        "revoked_at": revocation.revoked_at.to_string(),
    })
}

async fn revocations<S: AuthStore + 'static>(
    State(admin): State<LocalAdmin<S>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    if let Err(refused) = admin.admin(&headers, "GET", uri.path()).await {
        return *refused;
    }
    match admin.verifier.store().local_revocations().await {
        Ok(all) => {
            let mut listed = Vec::with_capacity(all.len());
            for revocation in &all {
                let username = admin.username(revocation.account).await;
                listed.push(revocation_json(revocation, username.as_deref()));
            }
            Json(listed).into_response()
        }
        Err(_) => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "the store failed",
        ),
    }
}

#[derive(Deserialize)]
struct RevokeForm {
    username: String,
}

async fn revoke<S: AuthStore + 'static>(
    State(admin): State<LocalAdmin<S>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Form(form): Form<RevokeForm>,
) -> Response {
    let who = match admin.admin(&headers, "POST", uri.path()).await {
        Ok(who) => who,
        Err(refused) => return *refused,
    };
    let account = match admin.account(&form.username).await {
        Ok(account) => account,
        Err(refused) => return *refused,
    };
    if account == who.principal.account {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "refused",
            "an admin cannot revoke their own access",
        );
    }
    let revocation = LocalRevocation {
        account,
        revoked_by: who.principal.account,
        revoked_at: admin.verifier.now(),
    };
    let audit = admin.audit(
        "local-revocation",
        "U",
        who.principal.account,
        account,
        "local access revoked",
    );
    match admin
        .verifier
        .store()
        .put_local_revocation(&revocation, &audit)
        .await
    {
        Ok(()) => {
            tracing::warn!(alert = "local-revocation", "local access was revoked");
            (
                StatusCode::CREATED,
                Json(revocation_json(&revocation, Some(form.username.trim()))),
            )
                .into_response()
        }
        Err(_) => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "the store failed",
        ),
    }
}

#[derive(Deserialize)]
struct ProvisionForm {
    username: String,
    password: String,
    role: String,
    days: i64,
}

async fn provision<S: AuthStore + 'static>(
    State(admin): State<LocalAdmin<S>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Form(form): Form<ProvisionForm>,
) -> Response {
    let who = match admin.admin(&headers, "POST", uri.path()).await {
        Ok(who) => who,
        Err(refused) => return *refused,
    };
    let username = form.username.trim();
    if username.is_empty() {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid",
            "the account needs a username",
        );
    }
    let store = admin.verifier.store();
    let config = match store.local_config().await {
        Ok(config) => config.unwrap_or_default(),
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "the store failed",
            );
        }
    };
    let now = admin.verifier.now();
    let account = AccountId(uuid::Uuid::now_v7());
    let grant = match LocalGrantPolicy::default().grant(
        form.role.trim(),
        &config.roles,
        &config.grantable,
        account,
        &who.facility,
        who.principal.account,
        form.days,
        now,
    ) {
        Ok(grant) => grant,
        Err(refused) => {
            return error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "refused",
                refused.to_string(),
            );
        }
    };
    // The admin's password must already meet the policy; the user replaces it at first sign-in.
    let credential = match admin.verifier.credentials().set_password(
        account,
        &health_identity::PasswordPolicy::default(),
        crate::BreachedPasswords::platform(),
        &form.password,
        true,
    ) {
        Ok(credential) => credential,
        Err(refused) => {
            return error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "password_rejected",
                refused.to_string(),
            );
        }
    };
    let provisional = health_identity::snapshot::ProvisionalAccount {
        account,
        facility: who.facility.clone(),
        created_by: who.principal.account,
        created_at: now,
        not_after: grant.not_after,
        must_change_password: true,
    };
    let claims = match serde_json::to_value(&grant) {
        Ok(claims) => claims,
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "the grant cannot be encoded",
            );
        }
    };
    let signer = Arc::clone(&admin.signer);
    let jws = match tokio::task::spawn_blocking(move || signer(&claims)).await {
        Ok(Ok(jws)) => jws,
        _ => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "the node's key cannot sign",
            );
        }
    };
    let login = health_identity::LoginAccount {
        id: account,
        username: username.to_owned(),
        credential,
        status: health_identity::AccountStatus::Active,
        lockout: health_identity::Lockout::default(),
    };
    let audit = admin.audit(
        "provisional-account",
        "C",
        who.principal.account,
        account,
        &format!("{username} as {} until {}", grant.role, grant.not_after),
    );
    match store
        .create_provisional(&login, &provisional, &grant, &jws, &audit)
        .await
    {
        Ok(()) => {
            tracing::warn!(alert = "provisional-account", role = %grant.role, "a provisional account was created offline, pending reconciliation");
            (
                StatusCode::CREATED,
                Json(json!({
                    "account": account.to_string(),
                    "username": username,
                    "role": grant.role,
                    "not_after": grant.not_after.to_string(),
                })),
            )
                .into_response()
        }
        Err(_) => error(
            StatusCode::CONFLICT,
            "taken",
            "an account by that username exists",
        ),
    }
}
