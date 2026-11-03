//! Device enrollment and the device gate (lan-security.md § Device enrollment, § Denying
//! unauthorized LAN devices; ADR 0009; #74).
//!
//! - **Enrolling** (`POST /api/enrollment`, open to the LAN): a device sends a DPoP proof made
//!   with its non-extractable key, which proves it holds the key, and a label. It gets back a
//!   pairing code to show. Requests are rate-limited per source address and expire after 10
//!   minutes. `GET /api/enrollment`, with a proof by the same key, says how the request stands.
//! - **Approving and revoking** (`/api/admin/…`): an admin enters the code the device shows,
//!   names the device and assigns it a location; or revokes a device. Both are audited. The
//!   caller guards these routes (an edge serves them to its own console).
//! - **The gate** ([`DeviceGate::layer`]): every other API request must carry a fresh, unreplayed
//!   DPoP proof by an enrolled, active key. Revoking a device refuses its next request.

use std::collections::HashMap;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{ConnectInfo, Path, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use health_identity::{
    Device, DeviceId, DeviceStatus, EnrollmentPolicy, EnrollmentRefused, EnrollmentRequest,
};
use health_time::{Clock, Timestamp};
use rand_core::RngCore;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::jose::{DpopProof, verify_dpop};
use crate::store::{AuthStore, MemoryStore, StoreError};

/// The device registry and pending enrollments, behind a port.
pub trait DeviceStore: Send + Sync {
    /// Stores a pending request, replacing any earlier one for the same key; false if another
    /// pending request holds its code.
    fn put_enrollment(
        &self,
        request: &EnrollmentRequest,
        now: Timestamp,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;
    /// The pending request holding `code`, expired or not.
    fn enrollment_by_code(
        &self,
        code: &str,
    ) -> impl Future<Output = Result<Option<EnrollmentRequest>, StoreError>> + Send;
    /// The pending request for key `thumbprint`, expired or not.
    fn enrollment_by_thumbprint(
        &self,
        thumbprint: &str,
    ) -> impl Future<Output = Result<Option<EnrollmentRequest>, StoreError>> + Send;
    /// The requests still pending at `now`.
    fn enrollments(
        &self,
        now: Timestamp,
    ) -> impl Future<Output = Result<Vec<EnrollmentRequest>, StoreError>> + Send;
    /// Takes request `request` and records `device` with its AuditEvent, together; false if the
    /// request was already taken or the key already belongs to a device.
    fn enroll(
        &self,
        request: Uuid,
        device: &Device,
        audit: &Value,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;
    /// The device with key `thumbprint`.
    fn device_by_thumbprint(
        &self,
        thumbprint: &str,
    ) -> impl Future<Output = Result<Option<Device>, StoreError>> + Send;
    /// The device with `id`.
    fn device(
        &self,
        id: DeviceId,
    ) -> impl Future<Output = Result<Option<Device>, StoreError>> + Send;
    /// Every device, revoked ones too.
    fn devices(&self) -> impl Future<Output = Result<Vec<Device>, StoreError>> + Send;
    /// Replaces `device`, if its stored version is still `before`, with its AuditEvent,
    /// together; false if it changed meanwhile.
    fn update_device(
        &self,
        device: &Device,
        before: u64,
        audit: &Value,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;
}

impl DeviceStore for MemoryStore {
    async fn put_enrollment(
        &self,
        request: &EnrollmentRequest,
        now: Timestamp,
    ) -> Result<bool, StoreError> {
        let mut state = self.lock();
        state
            .enrollments
            .retain(|_, r| !r.expired(now) && r.thumbprint != request.thumbprint);
        if state.enrollments.values().any(|r| r.code == request.code) {
            return Ok(false);
        }
        state.enrollments.insert(request.id, request.clone());
        Ok(true)
    }

    async fn enrollment_by_code(
        &self,
        code: &str,
    ) -> Result<Option<EnrollmentRequest>, StoreError> {
        Ok(self
            .lock()
            .enrollments
            .values()
            .find(|r| r.code == code)
            .cloned())
    }

    async fn enrollment_by_thumbprint(
        &self,
        thumbprint: &str,
    ) -> Result<Option<EnrollmentRequest>, StoreError> {
        Ok(self
            .lock()
            .enrollments
            .values()
            .find(|r| r.thumbprint == thumbprint)
            .cloned())
    }

    async fn enrollments(&self, now: Timestamp) -> Result<Vec<EnrollmentRequest>, StoreError> {
        let mut pending: Vec<_> = self
            .lock()
            .enrollments
            .values()
            .filter(|r| !r.expired(now))
            .cloned()
            .collect();
        pending.sort_by_key(|r| r.expires);
        Ok(pending)
    }

    async fn enroll(
        &self,
        request: Uuid,
        device: &Device,
        _audit: &Value,
    ) -> Result<bool, StoreError> {
        let mut state = self.lock();
        if state
            .devices
            .values()
            .any(|d| d.thumbprint == device.thumbprint)
            || state.enrollments.remove(&request).is_none()
        {
            return Ok(false);
        }
        state.devices.insert(device.id, device.clone());
        Ok(true)
    }

    async fn device_by_thumbprint(&self, thumbprint: &str) -> Result<Option<Device>, StoreError> {
        Ok(self
            .lock()
            .devices
            .values()
            .find(|d| d.thumbprint == thumbprint)
            .cloned())
    }

    async fn device(&self, id: DeviceId) -> Result<Option<Device>, StoreError> {
        Ok(self.lock().devices.get(&id).cloned())
    }

    async fn devices(&self) -> Result<Vec<Device>, StoreError> {
        let mut devices: Vec<_> = self.lock().devices.values().cloned().collect();
        devices.sort_by_key(|d| d.id);
        Ok(devices)
    }

    async fn update_device(
        &self,
        device: &Device,
        before: u64,
        _audit: &Value,
    ) -> Result<bool, StoreError> {
        let mut state = self.lock();
        match state.devices.get_mut(&device.id) {
            Some(stored) if stored.version == before => {
                *stored = device.clone();
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

/// How the gate and the enrollment routes are configured.
#[derive(Debug, Clone)]
pub struct DeviceConfig {
    /// The origins (`scheme://host[:port]`) clients reach this node by; a proof's `htu` must be
    /// one of them followed by the request's path.
    pub origins: Vec<String>,
    /// The enrollment rules.
    pub policy: EnrollmentPolicy,
    /// How old a proof's `iat` may be, and how far ahead of our clock.
    pub dpop_window: Duration,
    /// Enrollment requests allowed per source address in [`rate_window`](Self::rate_window).
    pub rate_limit: usize,
    /// The window [`rate_limit`](Self::rate_limit) counts over.
    pub rate_window: Duration,
    /// The node, as the audit events' observer.
    pub observer: String,
}

impl DeviceConfig {
    /// The defaults for a node reached at `origins`: 10-minute requests, a 300 s proof window,
    /// 10 enrollment requests per address per 10 minutes.
    pub fn new(origins: Vec<String>, observer: impl Into<String>) -> Self {
        Self {
            origins: origins
                .into_iter()
                .map(|o| o.trim_end_matches('/').to_owned())
                .collect(),
            policy: EnrollmentPolicy::default(),
            dpop_window: Duration::from_secs(300),
            rate_limit: 10,
            rate_window: Duration::from_secs(600),
            observer: observer.into(),
        }
    }
}

/// The device a request was made from, put in its extensions by the gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrolledDevice {
    /// The device.
    pub device: Device,
    /// The proof the request carried, already checked for freshness and replay.
    pub proof: DpopProof,
}

/// Who approves or revokes on the admin routes: put in the request's extensions by whatever
/// guards them. Without one, the actor is the node's own console.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceAdmin(pub String);

/// The actor when no [`DeviceAdmin`] is given.
pub const CONSOLE: &str = "edge-console";

/// Enrollment, the registry's admin routes and the gate, over `store`.
pub struct DeviceGate<S> {
    inner: Arc<Inner<S>>,
}

impl<S> Clone for DeviceGate<S> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

struct Inner<S> {
    store: S,
    config: DeviceConfig,
    clock: Arc<dyn Clock>,
    /// Enrollment requests per source address, by when they were made.
    recent: Mutex<HashMap<IpAddr, Vec<Timestamp>>>,
}

/// Why a request was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refusal {
    /// No proof, or a bad, stale or replayed one.
    Proof,
    /// The key is not an enrolled device's.
    Unenrolled,
    /// The device is revoked.
    Revoked,
    /// The store failed.
    Unavailable,
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        let (status, error) = match self {
            Self::Proof => (StatusCode::UNAUTHORIZED, "invalid_dpop_proof"),
            Self::Unenrolled => (StatusCode::FORBIDDEN, "device-not-enrolled"),
            Self::Revoked => (StatusCode::FORBIDDEN, "device-revoked"),
            Self::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
        };
        let mut response = (status, no_store(), Json(json!({ "error": error }))).into_response();
        if self == Self::Proof {
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_static(r#"DPoP algs="ES256", error="invalid_dpop_proof""#),
            );
        }
        response
    }
}

impl From<StoreError> for Refusal {
    fn from(error: StoreError) -> Self {
        tracing::error!(error.kind = %error, "device store unavailable");
        Self::Unavailable
    }
}

fn no_store() -> [(header::HeaderName, HeaderValue); 1] {
    [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))]
}

fn error(status: StatusCode, error: &str, detail: impl Into<String>) -> Response {
    (
        status,
        no_store(),
        Json(json!({ "error": error, "detail": detail.into() })),
    )
        .into_response()
}

impl<S: AuthStore + DeviceStore + 'static> DeviceGate<S> {
    /// The gate and routes over `store`.
    pub fn new(store: S, config: DeviceConfig, clock: Arc<dyn Clock>) -> Self {
        Self {
            inner: Arc::new(Inner {
                store,
                config,
                clock,
                recent: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// `POST` and `GET /api/enrollment`, open to the LAN.
    pub fn enrollment_router(&self) -> Router {
        Router::new()
            .route(
                "/api/enrollment",
                post(request_enrollment::<S>).get(enrollment_status::<S>),
            )
            .with_state(self.clone())
    }

    /// The admin routes, for the caller to guard: `GET /api/admin/enrollments`,
    /// `POST /api/admin/enrollments/approve`, `GET /api/admin/devices` and
    /// `POST /api/admin/devices/{id}/revoke`.
    pub fn admin_router(&self) -> Router {
        Router::new()
            .route("/api/admin/enrollments", get(pending::<S>))
            .route("/api/admin/enrollments/approve", post(approve::<S>))
            .route("/api/admin/devices", get(devices::<S>))
            .route("/api/admin/devices/{id}/revoke", post(revoke::<S>))
            .with_state(self.clone())
    }

    /// `GET /api/device`: the calling device, as the registry has it. Behind the gate.
    pub fn device_router(&self) -> Router {
        self.layer(Router::new().route("/api/device", get(this_device)))
    }

    /// The check the authorization server makes at `/token` (`AuthConfig::devices`): whether a
    /// key thumbprint is an enrolled, active device's.
    pub fn check(&self) -> crate::server::DeviceCheck {
        let gate = self.clone();
        crate::server::DeviceCheck(Arc::new(move |jkt| {
            let gate = gate.clone();
            Box::pin(async move {
                matches!(gate.inner.store.device_by_thumbprint(&jkt).await, Ok(Some(d)) if d.active())
            })
        }))
    }

    /// `routes` behind the gate: each request must carry a DPoP proof by an enrolled, active
    /// device, which the handlers find as an [`EnrolledDevice`] extension.
    pub fn layer(&self, routes: Router) -> Router {
        routes.layer(axum::middleware::from_fn_with_state(
            self.clone(),
            gate::<S>,
        ))
    }

    /// Checks the request's DPoP proof for `method` on `path`: signature, `htm`, `htu` on one of
    /// our origins, freshness and replay.
    async fn proof(
        &self,
        headers: &HeaderMap,
        method: &str,
        path: &str,
    ) -> Result<DpopProof, Refusal> {
        let inner = &self.inner;
        // With a token, the proof's replay is checked once, where the token is verified
        // (`AccessVerifier`); alone, it is checked here.
        let remember = !headers.contains_key(header::AUTHORIZATION);
        let proof = headers
            .get("dpop")
            .and_then(|v| v.to_str().ok())
            .ok_or(Refusal::Proof)?;
        let verified = inner
            .config
            .origins
            .iter()
            .find_map(|origin| verify_dpop(proof, method, &format!("{origin}{path}")).ok())
            .ok_or(Refusal::Proof)?;
        let now = inner.clock.now();
        let window = i64::try_from(inner.config.dpop_window.as_secs()).unwrap_or(i64::MAX);
        let age = now.as_second() - verified.iat;
        if age > window || age < -window {
            return Err(Refusal::Proof);
        }
        let until = now.checked_add(inner.config.dpop_window).unwrap_or(now);
        if !remember || inner.store.remember_jti(&verified.jti, until, now).await? {
            Ok(verified)
        } else {
            Err(Refusal::Proof)
        }
    }

    /// The enrolled, active device that signed `proof`.
    async fn device_of(&self, proof: &DpopProof) -> Result<Device, Refusal> {
        match self.inner.store.device_by_thumbprint(&proof.jkt).await? {
            Some(device) if device.active() => Ok(device),
            Some(_) => Err(Refusal::Revoked),
            None => Err(Refusal::Unenrolled),
        }
    }

    /// Whether `address` may make another enrollment request now; counts it if so.
    fn admit(&self, address: Option<IpAddr>, now: Timestamp) -> bool {
        let Some(address) = address else {
            return true;
        };
        let config = &self.inner.config;
        let since = now.checked_sub(config.rate_window).unwrap_or(now);
        let mut recent = self.inner.recent.lock().unwrap_or_else(|e| e.into_inner());
        recent.retain(|_, times| {
            times.retain(|t| *t > since);
            !times.is_empty()
        });
        let times = recent.entry(address).or_default();
        if times.len() >= config.rate_limit {
            return false;
        }
        times.push(now);
        true
    }

    fn audit(&self, code: &str, action: &str, device: &Device, by: &str, now: Timestamp) -> Value {
        device_audit(&self.inner.config.observer, code, action, device, by, now)
    }
}

/// The AuditEvent of a change to the registry: who enrolled or revoked which device.
fn device_audit(
    observer: &str,
    code: &str,
    action: &str,
    device: &Device,
    by: &str,
    now: Timestamp,
) -> Value {
    json!({
        "resourceType": "AuditEvent",
        "category": [{ "coding": [{ "system": "http://dicom.nema.org/resources/ontology/DCM", "code": "110137", "display": "User Security Attributes Changed" }] }],
        "code": { "coding": [{ "system": "urn:health:audit-event", "code": code }] },
        "action": action,
        "severity": "notice",
        "recorded": now.to_string(),
        "outcome": { "code": { "system": "http://terminology.hl7.org/CodeSystem/audit-event-outcome", "code": "0" } },
        "agent": [{ "who": { "display": by }, "requestor": true }],
        "source": { "observer": { "display": observer } },
        "entity": [{
            "what": { "identifier": { "system": "urn:health:device-id", "value": device.id.to_string() }, "display": device.name },
            "detail": [
                { "type": { "text": "location" }, "valueString": device.location },
                { "type": { "text": "thumbprint" }, "valueString": device.thumbprint },
            ],
        }],
    })
}

/// A device as the admin page and the device itself see it.
fn device_json(device: &Device) -> Value {
    json!({
        "id": device.id.to_string(),
        "name": device.name,
        "location": device.location,
        "label": device.label,
        "enrolled_by": device.enrolled_by,
        "enrolled_at": device.enrolled_at.to_string(),
        "status": match device.status {
            DeviceStatus::Active => "active",
            DeviceStatus::Revoked => "revoked",
        },
        "revoked_by": device.revoked.as_ref().map(|(by, _)| by),
        "revoked_at": device.revoked.as_ref().map(|(_, at)| at.to_string()),
    })
}

fn peer(request: &Request) -> Option<IpAddr> {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip())
}

/// The most a device's label may hold.
const LABEL_LIMIT: usize = 200;

async fn request_enrollment<S: AuthStore + DeviceStore + 'static>(
    State(gate): State<DeviceGate<S>>,
    request: Request,
) -> Response {
    let now = gate.inner.clock.now();
    let address = peer(&request);
    if !gate.admit(address, now) {
        tracing::warn!(alert = "enrollment-rate-limited", peer = ?address, "too many enrollment requests");
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            "rate-limited",
            "too many enrollment requests: wait and try again",
        );
    }
    let (parts, body) = request.into_parts();
    let proof = match gate.proof(&parts.headers, "POST", parts.uri.path()).await {
        Ok(proof) => proof,
        Err(refusal) => {
            tracing::warn!(alert = "enrollment-refused", peer = ?address, "enrollment request without a valid proof");
            return refusal.into_response();
        }
    };
    let Ok(bytes) = axum::body::to_bytes(body, 4096).await else {
        return error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "invalid",
            "the request is too large",
        );
    };
    let label = serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|v| v["label"].as_str().map(str::trim).map(str::to_owned))
        .unwrap_or_default();
    if label.is_empty() || label.chars().count() > LABEL_LIMIT {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid",
            "the device needs a label of at most 200 characters",
        );
    }
    let store = &gate.inner.store;
    match store.device_by_thumbprint(&proof.jkt).await {
        Ok(Some(device)) if device.active() => {
            return error(
                StatusCode::CONFLICT,
                "already-enrolled",
                "this device is already enrolled",
            );
        }
        Ok(Some(_)) => {
            return error(
                StatusCode::FORBIDDEN,
                "device-revoked",
                "this device was revoked: clear its key and enroll again",
            );
        }
        Ok(None) => {}
        Err(e) => return Refusal::from(e).into_response(),
    }
    let policy = gate.inner.config.policy;
    // A code another pending request holds is drawn again; collisions are rare.
    for _ in 0..8 {
        let code = policy.code(rand_core::OsRng.next_u64());
        let pending = policy.request(Uuid::now_v7(), proof.jkt.clone(), label.clone(), code, now);
        match store.put_enrollment(&pending, now).await {
            Ok(true) => {
                tracing::info!(outcome = "enrollment-requested", request = %pending.id, "a device asked to enroll");
                return (
                    StatusCode::CREATED,
                    no_store(),
                    Json(json!({
                        "request": pending.id.to_string(),
                        "code": pending.code,
                        "expires": pending.expires.to_string(),
                    })),
                )
                    .into_response();
            }
            Ok(false) => {}
            Err(e) => return Refusal::from(e).into_response(),
        }
    }
    Refusal::Unavailable.into_response()
}

async fn enrollment_status<S: AuthStore + DeviceStore + 'static>(
    State(gate): State<DeviceGate<S>>,
    headers: HeaderMap,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> Response {
    let proof = match gate.proof(&headers, "GET", uri.path()).await {
        Ok(proof) => proof,
        Err(refusal) => return refusal.into_response(),
    };
    let store = &gate.inner.store;
    let now = gate.inner.clock.now();
    let body = match store.device_by_thumbprint(&proof.jkt).await {
        Ok(Some(device)) if device.active() => {
            json!({ "status": "enrolled", "device": device_json(&device) })
        }
        Ok(Some(_)) => json!({ "status": "revoked" }),
        Ok(None) => match store.enrollment_by_thumbprint(&proof.jkt).await {
            Ok(Some(pending)) if !pending.expired(now) => json!({
                "status": "pending",
                "code": pending.code,
                "expires": pending.expires.to_string(),
            }),
            Ok(_) => {
                return (
                    StatusCode::NOT_FOUND,
                    no_store(),
                    Json(json!({ "status": "none" })),
                )
                    .into_response();
            }
            Err(e) => return Refusal::from(e).into_response(),
        },
        Err(e) => return Refusal::from(e).into_response(),
    };
    (no_store(), Json(body)).into_response()
}

async fn pending<S: AuthStore + DeviceStore + 'static>(
    State(gate): State<DeviceGate<S>>,
) -> Response {
    match gate.inner.store.enrollments(gate.inner.clock.now()).await {
        // The codes are not listed: the admin types the one the device shows.
        Ok(requests) => (
            no_store(),
            Json(json!(
                requests
                    .iter()
                    .map(|r| json!({
                        "id": r.id.to_string(),
                        "label": r.label,
                        "requested_at": r.requested_at.to_string(),
                        "expires": r.expires.to_string(),
                    }))
                    .collect::<Vec<_>>()
            )),
        )
            .into_response(),
        Err(e) => Refusal::from(e).into_response(),
    }
}

#[derive(Deserialize)]
struct Approval {
    code: String,
    name: String,
    location: String,
}

fn actor(admin: Option<&DeviceAdmin>) -> String {
    admin.map_or_else(|| CONSOLE.to_owned(), |a| a.0.clone())
}

async fn approve<S: AuthStore + DeviceStore + 'static>(
    State(gate): State<DeviceGate<S>>,
    admin: Option<axum::Extension<DeviceAdmin>>,
    Json(approval): Json<Approval>,
) -> Response {
    let by = actor(admin.as_ref().map(|a| &a.0));
    let now = gate.inner.clock.now();
    let store = &gate.inner.store;
    let request = match store.enrollment_by_code(approval.code.trim()).await {
        Ok(Some(request)) => request,
        Ok(None) => {
            tracing::warn!(
                alert = "enrollment-refused",
                "approval with an unknown pairing code"
            );
            return error(
                StatusCode::NOT_FOUND,
                "no-such-code",
                EnrollmentRefused::NoSuchCode.to_string(),
            );
        }
        Err(e) => return Refusal::from(e).into_response(),
    };
    let device = match request.approve(
        DeviceId(Uuid::now_v7()),
        &approval.name,
        &approval.location,
        &by,
        now,
    ) {
        Ok(device) => device,
        Err(refused @ EnrollmentRefused::Expired) => {
            return error(StatusCode::GONE, "expired", refused.to_string());
        }
        Err(refused) => {
            return error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid",
                refused.to_string(),
            );
        }
    };
    let audit = gate.audit("device-enrolled", "C", &device, &by, now);
    match store.enroll(request.id, &device, &audit).await {
        Ok(true) => {
            tracing::info!(outcome = "device-enrolled", device = %device.id, "device enrolled");
            (StatusCode::CREATED, no_store(), Json(device_json(&device))).into_response()
        }
        Ok(false) => error(
            StatusCode::CONFLICT,
            "conflict",
            "the request was already approved",
        ),
        Err(e) => Refusal::from(e).into_response(),
    }
}

async fn devices<S: AuthStore + DeviceStore + 'static>(
    State(gate): State<DeviceGate<S>>,
) -> Response {
    match gate.inner.store.devices().await {
        Ok(devices) => (
            no_store(),
            Json(json!(devices.iter().map(device_json).collect::<Vec<_>>())),
        )
            .into_response(),
        Err(e) => Refusal::from(e).into_response(),
    }
}

async fn revoke<S: AuthStore + DeviceStore + 'static>(
    State(gate): State<DeviceGate<S>>,
    admin: Option<axum::Extension<DeviceAdmin>>,
    // By name: a caller that routes to this router through its own path parameters (the edge's
    // `/api/{*rest}`) leaves those in the request too.
    Path(params): Path<HashMap<String, String>>,
) -> Response {
    let by = actor(admin.as_ref().map(|a| &a.0));
    let Some(Ok(id)) = params.get("id").map(|id| id.parse().map(DeviceId)) else {
        return error(StatusCode::NOT_FOUND, "not-found", "no such device");
    };
    let store = &gate.inner.store;
    let mut device = match store.device(id).await {
        Ok(Some(device)) => device,
        Ok(None) => return error(StatusCode::NOT_FOUND, "not-found", "no such device"),
        Err(e) => return Refusal::from(e).into_response(),
    };
    let now = gate.inner.clock.now();
    let before = device.version;
    if let Err(refused) = device.revoke(&by, now) {
        return error(StatusCode::CONFLICT, "already-revoked", refused.to_string());
    }
    let audit = gate.audit("device-revoked", "U", &device, &by, now);
    match store.update_device(&device, before, &audit).await {
        Ok(true) => {
            tracing::warn!(alert = "device-revoked", device = %device.id, "device revoked");
            (no_store(), Json(device_json(&device))).into_response()
        }
        Ok(false) => error(
            StatusCode::CONFLICT,
            "conflict",
            "the device changed meanwhile: reload",
        ),
        Err(e) => Refusal::from(e).into_response(),
    }
}

async fn gate<S: AuthStore + DeviceStore + 'static>(
    State(gate): State<DeviceGate<S>>,
    mut request: Request,
    next: Next,
) -> Response {
    let address = peer(&request);
    let method = request.method().as_str().to_owned();
    let path = request.uri().path().to_owned();
    // Cloned, so the check holds no borrow of the (non-`Sync`) body across an await.
    let headers = request.headers().clone();
    let checked = async {
        let proof = gate.proof(&headers, &method, &path).await?;
        let device = gate.device_of(&proof).await?;
        Ok::<_, Refusal>(EnrolledDevice { device, proof })
    }
    .await;
    match checked {
        Ok(enrolled) => {
            request.extensions_mut().insert(enrolled);
            next.run(request).await
        }
        Err(refusal) => {
            if matches!(refusal, Refusal::Unenrolled | Refusal::Revoked) {
                tracing::warn!(alert = "unenrolled-device", peer = ?address, %path, "a request from a key that is not an enrolled device");
            }
            refusal.into_response()
        }
    }
}

async fn this_device(axum::Extension(enrolled): axum::Extension<EnrolledDevice>) -> Response {
    (no_store(), Json(device_json(&enrolled.device))).into_response()
}
