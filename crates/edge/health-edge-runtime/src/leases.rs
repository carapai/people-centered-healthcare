//! Authority Leases on the edge (#135; write-authority.md § Hand-over rules): the edge grants its
//! enrolled desktops time-boxed write authority over outreach stores, takes it back once the
//! desktop's outbox has drained, freezes a lease that expires unreturned and raises an alert, and
//! lets an admin recover a frozen one. Every grant, return and recovery is audited in the same
//! transaction as the lease.
//!
//! - `GET` / `POST /api/admin/leases` and `POST /api/admin/leases/{id}/recover`: this machine
//!   only, like the other admin routes.
//! - `GET /api/device/leases` and `POST /api/device/leases/{id}/return`: behind the device gate,
//!   for the holder.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_auth::{
    CONSOLE, DeviceAdmin, DeviceGate, DeviceStore as _, EnrolledDevice, SurrealAuthStore,
};
use health_authority_leases::{Lease, LeaseRefused};
use health_identity::DeviceId;
use health_persistence::{Store as _, Transaction as _};
use health_surrealdb::{LeaseRole, SurrealStore};
use health_time::{Clock, SignedDuration, Timestamp};
use serde::Deserialize;
use serde_json::{Value, json};

/// How often expired leases are frozen.
const FREEZE_EVERY: std::time::Duration = std::time::Duration::from_secs(60);

/// The edge's leases.
#[derive(Clone)]
pub(crate) struct Leases {
    store: SurrealStore,
    devices: SurrealAuthStore,
    clock: Arc<dyn Clock>,
    /// The node, as the audit events' observer.
    observer: String,
}

impl Leases {
    pub(crate) fn new(
        store: SurrealStore,
        devices: SurrealAuthStore,
        clock: Arc<dyn Clock>,
        observer: impl Into<String>,
    ) -> Self {
        Self {
            store,
            devices,
            clock,
            observer: observer.into(),
        }
    }

    /// The admin's routes, for the caller to guard.
    pub(crate) fn admin_router(&self) -> Router {
        Router::new()
            .route("/api/admin/leases", get(list).post(grant))
            .route("/api/admin/leases/{id}/recover", post(recover))
            .with_state(self.clone())
    }

    /// The holder's routes, behind `gate`.
    pub(crate) fn device_router(&self, gate: &DeviceGate<SurrealAuthStore>) -> Router {
        gate.layer(
            Router::new()
                .route("/api/device/leases", get(held))
                .route("/api/device/leases/{id}/return", post(give_back))
                .with_state(self.clone()),
        )
    }

    /// Freezes every lease that expired unreturned, raising the alert for each; how many froze.
    pub(crate) async fn freeze(&self) -> Result<usize, String> {
        let now = self.clock.now();
        let mut frozen = 0;
        for mut lease in self
            .store
            .leases(LeaseRole::Granted)
            .await
            .map_err(|e| e.to_string())?
        {
            if lease.expire(now) {
                self.save(&lease, None).await?;
                frozen += 1;
                tracing::warn!(
                    alert = "lease-frozen",
                    lease = %lease.id,
                    holder = %lease.holder,
                    stores = ?lease.stores,
                    "an Authority Lease expired unreturned: its stores are frozen until it is returned or recovered"
                );
            }
        }
        Ok(frozen)
    }

    /// Freezes expired leases every minute, in the background.
    pub(crate) fn spawn_freezer(&self) {
        let leases = self.clone();
        tokio::spawn(async move {
            let mut every = tokio::time::interval(FREEZE_EVERY);
            loop {
                every.tick().await;
                if let Err(e) = leases.freeze().await {
                    tracing::warn!(error = %e, "leases could not be checked for expiry");
                }
            }
        });
    }

    /// Stores `lease`, with `audit` (its AuditEvent) when given, in one transaction.
    async fn save(&self, lease: &Lease, audit: Option<Value>) -> Result<(), String> {
        let mut tx = self.store.begin().await.map_err(|e| e.to_string())?;
        tx.put_lease(LeaseRole::Granted, lease)
            .await
            .map_err(|e| e.to_string())?;
        if let Some(audit) = audit {
            health_audit::append(&mut tx, &audit)
                .await
                .map_err(|e| e.to_string())?;
        }
        tx.commit().await.map_err(|e| e.to_string())
    }

    async fn find(&self, id: &str) -> Result<Option<Lease>, String> {
        Ok(self
            .store
            .leases(LeaseRole::Granted)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|lease| lease.id.to_string() == id))
    }

    fn audit(&self, code: &str, action: &str, lease: &Lease, by: &str, now: Timestamp) -> Value {
        json!({
            "resourceType": "AuditEvent",
            "category": [{ "coding": [{ "system": "http://dicom.nema.org/resources/ontology/DCM", "code": "110137", "display": "User Security Attributes Changed" }] }],
            "code": { "coding": [{ "system": "urn:health:audit-event", "code": code }] },
            "action": action,
            "severity": "notice",
            "recorded": now.to_string(),
            "outcome": { "code": { "system": "http://terminology.hl7.org/CodeSystem/audit-event-outcome", "code": "0" } },
            "agent": [{ "who": { "display": by }, "requestor": true }],
            "source": { "observer": { "display": self.observer } },
            "entity": [{
                "what": { "identifier": { "system": "urn:health:authority-lease", "value": lease.id.to_string() } },
                "detail": [
                    { "type": { "text": "holder" }, "valueString": lease.holder },
                    { "type": { "text": "stores" }, "valueString": lease.stores.iter().cloned().collect::<Vec<_>>().join(", ") },
                    { "type": { "text": "state" }, "valueString": serde_json::to_value(&lease.state).map(|v| v.to_string()).unwrap_or_default() },
                ],
            }],
        })
    }
}

fn problem(status: StatusCode, error: &str, detail: impl Into<String>) -> Response {
    (
        status,
        Json(json!({ "error": error, "detail": detail.into() })),
    )
        .into_response()
}

fn refused(why: &LeaseRefused) -> Response {
    let (status, error) = match why {
        LeaseRefused::NotHolder => (StatusCode::FORBIDDEN, "not-the-holder"),
        LeaseRefused::Overlaps(_) | LeaseRefused::State(_) | LeaseRefused::NotDrained => {
            (StatusCode::CONFLICT, "lease-refused")
        }
        _ => (StatusCode::UNPROCESSABLE_ENTITY, "invalid"),
    };
    problem(status, error, why.to_string())
}

fn failed(why: String) -> Response {
    problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable", why)
}

async fn list(State(leases): State<Leases>) -> Response {
    match leases.store.leases(LeaseRole::Granted).await {
        Ok(all) => Json(all).into_response(),
        Err(e) => failed(e.to_string()),
    }
}

#[derive(Deserialize)]
struct Grant {
    /// The desktop's device id.
    holder: String,
    stores: BTreeSet<String>,
    hours: i64,
}

async fn grant(
    State(leases): State<Leases>,
    admin: Option<Extension<DeviceAdmin>>,
    Json(request): Json<Grant>,
) -> Response {
    let by = admin.map_or_else(|| CONSOLE.to_owned(), |Extension(DeviceAdmin(by))| by);
    let Ok(device) = request.holder.parse().map(DeviceId) else {
        return problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid",
            "the holder is not a device id",
        );
    };
    match leases.devices.device(device).await {
        Ok(Some(found)) if found.active() => {}
        Ok(_) => {
            return problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid",
                "the holder is not an enrolled, active device",
            );
        }
        Err(e) => return failed(e.to_string()),
    }
    let existing = match leases.store.leases(LeaseRole::Granted).await {
        Ok(existing) => existing,
        Err(e) => return failed(e.to_string()),
    };
    let now = leases.clock.now();
    let lease = match Lease::grant(
        uuid::Uuid::now_v7(),
        request.holder,
        request.stores,
        by.clone(),
        now,
        SignedDuration::from_hours(request.hours),
        &existing,
    ) {
        Ok(lease) => lease,
        Err(why) => return refused(&why),
    };
    let audit = leases.audit("lease-granted", "C", &lease, &by, now);
    match leases.save(&lease, Some(audit)).await {
        Ok(()) => {
            tracing::info!(outcome = "lease-granted", lease = %lease.id, holder = %lease.holder, "an Authority Lease was granted");
            (StatusCode::CREATED, Json(lease)).into_response()
        }
        Err(e) => failed(e),
    }
}

#[derive(Deserialize)]
struct Recovery {
    reason: String,
}

async fn recover(
    State(leases): State<Leases>,
    // By name: the edge's `/api/{*rest}` leaves its own parameter in the request too.
    Path(params): Path<std::collections::HashMap<String, String>>,
    admin: Option<Extension<DeviceAdmin>>,
    Json(request): Json<Recovery>,
) -> Response {
    let by = admin.map_or_else(|| CONSOLE.to_owned(), |Extension(DeviceAdmin(by))| by);
    let id = params.get("id").cloned().unwrap_or_default();
    let mut lease = match leases.find(&id).await {
        Ok(Some(lease)) => lease,
        Ok(None) => return problem(StatusCode::NOT_FOUND, "not-found", "no such lease"),
        Err(e) => return failed(e),
    };
    let now = leases.clock.now();
    lease.expire(now);
    if let Err(why) = lease.recover(by.clone(), request.reason, now) {
        return refused(&why);
    }
    let audit = leases.audit("lease-recovered", "U", &lease, &by, now);
    match leases.save(&lease, Some(audit)).await {
        Ok(()) => {
            tracing::warn!(outcome = "lease-recovered", lease = %lease.id, by = %by, "a frozen Authority Lease was recovered by an admin");
            Json(lease).into_response()
        }
        Err(e) => failed(e),
    }
}

async fn held(
    State(leases): State<Leases>,
    Extension(enrolled): Extension<EnrolledDevice>,
) -> Response {
    let holder = enrolled.device.id.to_string();
    let now = leases.clock.now();
    match leases.store.leases(LeaseRole::Granted).await {
        Ok(all) => Json(
            all.into_iter()
                .filter(|lease| lease.holder == holder)
                .map(|mut lease| {
                    // As of now, without waiting for the freezer.
                    lease.expire(now);
                    lease
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(e) => failed(e.to_string()),
    }
}

#[derive(Deserialize)]
struct Return {
    /// Whether the holder's outbox to this edge has drained.
    #[serde(default)]
    drained: bool,
}

async fn give_back(
    State(leases): State<Leases>,
    // By name: the edge's `/api/{*rest}` leaves its own parameter in the request too.
    Path(params): Path<std::collections::HashMap<String, String>>,
    Extension(enrolled): Extension<EnrolledDevice>,
    Json(request): Json<Return>,
) -> Response {
    let holder = enrolled.device.id.to_string();
    let id = params.get("id").cloned().unwrap_or_default();
    let mut lease = match leases.find(&id).await {
        Ok(Some(lease)) => lease,
        Ok(None) => return problem(StatusCode::NOT_FOUND, "not-found", "no such lease"),
        Err(e) => return failed(e),
    };
    let now = leases.clock.now();
    lease.expire(now);
    if lease.holds()
        && !matches!(
            lease.state,
            health_authority_leases::LeaseState::Returning { .. }
        )
    {
        if let Err(why) = lease.request_return(&holder, now) {
            return refused(&why);
        }
    } else if lease.holder != holder {
        return refused(&LeaseRefused::NotHolder);
    }
    let audit = if request.drained {
        if let Err(why) = lease.complete_return(true, now) {
            return refused(&why);
        }
        Some(leases.audit("lease-returned", "U", &lease, &holder, now))
    } else {
        None
    };
    match leases.save(&lease, audit).await {
        Ok(()) => Json(lease).into_response(),
        Err(e) => failed(e),
    }
}

#[cfg(test)]
mod tests {
    use health_identity::SessionPolicy;
    use health_surrealdb::{Connection, MIGRATIONS, Settings, Target, migrate};
    use health_time::FixedClock;

    use super::*;

    #[tokio::test]
    async fn an_unreturned_lease_freezes_once_and_an_admin_recovers_it() {
        let connection = Connection::open(&Settings {
            target: Target::Memory,
            namespace: "leases".into(),
            database: "main".into(),
        })
        .await
        .expect("open");
        migrate(&connection, MIGRATIONS).await.expect("migrate");
        let clock = Arc::new(FixedClock::new(
            "2026-11-12T06:00:00Z".parse().expect("now"),
        ));
        let store = SurrealStore::new(connection, Arc::clone(&clock) as Arc<dyn Clock>);
        let leases = Leases::new(
            store.clone(),
            SurrealAuthStore::new(store.clone(), SessionPolicy::default().hard_cap),
            Arc::clone(&clock) as Arc<dyn Clock>,
            "edge",
        );
        let lease = Lease::grant(
            uuid::Uuid::now_v7(),
            "desktop",
            BTreeSet::from(["Location/outreach-bag".to_owned()]),
            "admin",
            clock.now(),
            SignedDuration::from_hours(8),
            &[],
        )
        .expect("grant");
        leases.save(&lease, None).await.expect("save");

        assert_eq!(leases.freeze().await.expect("freeze"), 0, "not yet");
        clock.advance(SignedDuration::from_hours(9));
        assert_eq!(leases.freeze().await.expect("freeze"), 1);
        assert_eq!(leases.freeze().await.expect("freeze"), 0, "once");
        let mut frozen = leases
            .find(&lease.id.to_string())
            .await
            .expect("find")
            .expect("lease");
        assert!(matches!(
            frozen.state,
            health_authority_leases::LeaseState::Frozen { .. }
        ));
        frozen
            .recover("admin", "laptop lost", clock.now())
            .expect("recover");
        leases
            .save(
                &frozen,
                Some(leases.audit("lease-recovered", "U", &frozen, "admin", clock.now())),
            )
            .await
            .expect("recovered, audited");
    }
}
