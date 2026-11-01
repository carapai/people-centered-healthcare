//! The authorization server, mounted at `/auth` (offline-auth.md; ADR 0010), and the FHIR API
//! behind it at `/fhir` (ADR 0056). Central is the identity provider of record; its routes come
//! up once the database is migrated, and answer 503 until then.

use std::convert::Infallible;
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use health_access_policy::{Label, LabelRule, NodeScope, Pdp};
use health_audit::Auditor;
use health_auth::{
    AuthConfig, AuthServer, CaregiverRoutes, Client, Credentials, OtpSendError, OtpSender,
    PhoneSignIn, SendFuture, SnapshotGate, SnapshotSigner, SnapshotVerifier, Subjects,
    SurrealAuthStore, TokenSigner, patient_layer, subject_layer,
};
use health_consent::Confidentiality;
use health_fhir_search::Unrestricted;
use health_fhir_server::{FacilityTag, FhirServer};
use health_identity::PhoneNumber;
use health_identity::SessionPolicy;
use health_identity::snapshot::{RolePermissions, SnapshotPolicy};
use health_surrealdb::{Connection, SurrealStore};
use health_time::SystemClock;
use serde::Deserialize;
use tokio::sync::OnceCell;
use tower::ServiceExt;

/// What the authorization server needs, read at startup.
#[derive(Clone)]
pub(crate) struct AuthSettings {
    /// The issuer URL, e.g. `https://central.example/auth`.
    pub(crate) issuer: String,
    /// The signing keys: the first signs, the rest (rotated out) still verify.
    pub(crate) keys: Vec<[u8; 32]>,
    /// The Argon2id pepper (empty for none).
    pub(crate) pepper: Vec<u8>,
    /// The registered clients.
    pub(crate) clients: Vec<Client>,
    /// The web client's sign-in page, if it fronts `/authorize`.
    pub(crate) login_page: Option<String>,
    /// Central's snapshot-signing keys (the active one first): with them, a session acts for
    /// one facility, chosen at sign-in (ADR 0056).
    pub(crate) snapshot_keys: Vec<[u8; 32]>,
    /// The role mappings and label rules: with them and snapshot keys, `/fhir` is mounted.
    pub(crate) policy: Option<AccessPolicy>,
    /// The NIN identifier system: with it, `/fhir` serves the client registry's operations.
    pub(crate) registry_nin_system: Option<String>,
    /// This node's short code, in the MRNs it mints.
    pub(crate) node_code: health_identifiers::ShortCode,
    /// The patient portal's phone sign-in (#111), with a portal client among `clients`.
    pub(crate) portal: Option<PortalSettings>,
}

/// The patient portal's phone sign-in.
#[derive(Debug, Clone)]
pub(crate) struct PortalSettings {
    /// The country code of national phone numbers.
    pub(crate) country_code: String,
    /// The development outbox file one-time codes are appended to (#235).
    pub(crate) otp_outbox: std::path::PathBuf,
}

/// One-time codes appended to a file, one `{phone} {code}` line each: the development and e2e
/// sender until an SMS provider is chosen (#235). Nothing reaches a phone.
struct FileOutbox(std::path::PathBuf);

impl OtpSender for FileOutbox {
    fn send<'a>(&'a self, phone: &'a PhoneNumber, code: &'a str) -> SendFuture<'a> {
        Box::pin(async move {
            use std::io::Write;
            let line = format!("{phone} {code}\n");
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.0)
                .and_then(|mut file| file.write_all(line.as_bytes()))
                .map_err(|e| OtpSendError(e.to_string()))
        })
    }
}

/// The access policy file (`HEALTH_ACCESS_POLICY_FILE`): role → permission mappings and the
/// sensitivity label rules, from the country configuration package.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct AccessPolicy {
    /// Role → permission codes (`{Type|*}.{cruds}`, `break-glass`, …).
    pub(crate) roles: RolePermissions,
    /// The labels that restrict records, and the program that opens each.
    #[serde(default)]
    pub(crate) labels: Vec<LabelFile>,
}

/// One label rule in the policy file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct LabelFile {
    system: String,
    code: String,
    program: String,
}

/// Reads the access policy file.
pub(crate) fn read_policy(path: &Path) -> Result<AccessPolicy, &'static str> {
    let text =
        std::fs::read_to_string(path).map_err(|_| "cannot read HEALTH_ACCESS_POLICY_FILE")?;
    serde_json::from_str(&text).map_err(|_| "HEALTH_ACCESS_POLICY_FILE is not a valid policy")
}

impl std::fmt::Debug for AuthSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthSettings")
            .field("issuer", &self.issuer)
            .field("keys", &self.keys.len())
            .field("clients", &self.clients)
            .field("snapshot_keys", &self.snapshot_keys.len())
            .field("policy", &self.policy.is_some())
            .finish_non_exhaustive()
    }
}

/// A key file: one hex-encoded 32-byte P-256 secret per line, the active key first. Blank lines
/// and `#` comments are ignored.
pub(crate) fn read_keys(path: &Path) -> Result<Vec<[u8; 32]>, &'static str> {
    let text = std::fs::read_to_string(path).map_err(|_| "cannot read HEALTH_AUTH_KEYS_FILE")?;
    let keys: Vec<[u8; 32]> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|line| {
            let bytes: Vec<u8> = (0..line.len())
                .step_by(2)
                .map(|i| {
                    line.get(i..i + 2)
                        .and_then(|h| u8::from_str_radix(h, 16).ok())
                })
                .collect::<Option<_>>()
                .ok_or("HEALTH_AUTH_KEYS_FILE holds a line that is not hex")?;
            bytes
                .try_into()
                .map_err(|_| "HEALTH_AUTH_KEYS_FILE keys are 32 bytes")
        })
        .collect::<Result<_, _>>()?;
    if keys.is_empty() {
        return Err("HEALTH_AUTH_KEYS_FILE holds no key");
    }
    Ok(keys)
}

/// The authorization server's routes over `connection`.
pub(crate) fn routes(
    connection: Connection,
    settings: &AuthSettings,
) -> Result<Router, &'static str> {
    let signer = TokenSigner::from_secrets(&settings.keys)
        .map_err(|_| "HEALTH_AUTH_KEYS_FILE holds an invalid key")?;
    let credentials =
        Credentials::new(settings.pepper.clone()).map_err(|_| "the pepper cannot be used")?;
    let policy = SessionPolicy::default();
    let fhir_store = SurrealStore::new(connection, Arc::new(SystemClock));
    let store = SurrealAuthStore::new(fhir_store.clone(), policy.hard_cap);
    let mut config = AuthConfig::new(settings.issuer.as_str(), "central", false);
    config.clients.clone_from(&settings.clients);
    config.login_page.clone_from(&settings.login_page);
    if let Some(portal) = &settings.portal {
        tracing::warn!("the patient portal sends one-time codes to a development outbox file");
        config.phone_sign_in = Some(PhoneSignIn::new(
            portal.country_code.as_str(),
            Arc::new(FileOutbox(portal.otp_outbox.clone())),
        ));
    }
    if !settings.snapshot_keys.is_empty() {
        let keys = settings
            .snapshot_keys
            .iter()
            .map(|k| SnapshotSigner::from_secret(k).map(|s| s.public()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "HEALTH_AUTH_SNAPSHOT_KEYS_FILE holds an invalid key")?;
        config.snapshots = Some(SnapshotGate {
            facility: None,
            verifier: SnapshotVerifier::new(&keys)
                .map_err(|_| "HEALTH_AUTH_SNAPSHOT_KEYS_FILE holds an invalid key")?,
            policy: SnapshotPolicy::default(),
        });
    }
    let accounts = store.clone();
    let server = AuthServer::new(config, signer, credentials, store, Arc::new(SystemClock));
    let mut api = server.session_route();
    let mut router = Router::new().nest("/auth", server.router());
    if let (Some(policy), false) = (&settings.policy, settings.snapshot_keys.is_empty()) {
        let subjects = Subjects::new(server.verifier(), policy.roles.clone())
            .with_auditor(Auditor::new("central", sensitive(policy)));
        api = api.merge(subjects.routes());
        escalate_hourly(subjects.clone());
        // Caregiver delegation (#113): staff manage grants; patients see their dependents. The
        // policy's labels are the sensitive ones an adolescent's guardian does not see.
        let caregivers = CaregiverRoutes::new(
            fhir_store.clone(),
            accounts,
            Some(Arc::new(Auditor::new("central", sensitive(policy)))),
            Confidentiality {
                sensitive: policy
                    .labels
                    .iter()
                    .map(|l| (l.system.clone(), l.code.clone()))
                    .collect(),
                ..Confidentiality::default()
            },
            settings
                .portal
                .as_ref()
                .map_or("256", |p| p.country_code.as_str()),
        );
        api = api.nest(
            "/caregivers",
            caregivers
                .caregiver_router()
                .layer(axum::middleware::from_fn_with_state(
                    subjects.clone(),
                    subject_layer,
                )),
        );
        let (fhir, reviews, portal) = fhir(fhir_store, settings, policy, subjects);
        router = router.nest("/fhir", fhir);
        if let Some(reviews) = reviews {
            api = api.nest("/registry/reviews", reviews);
        }
        // The portal's self-registration (#111), for patients signed in by phone.
        if let (Some(portal), Some(_)) = (portal, &settings.portal) {
            api = api.nest(
                "/portal",
                portal.merge(caregivers.dependents_router()).layer(
                    axum::middleware::from_fn_with_state(
                        server.verifier(),
                        patient_layer::<SurrealAuthStore>,
                    ),
                ),
            );
        }
    }
    Ok(router.nest("/api", api))
}

/// The FHIR API: every interaction decided for the request's subject at central's scope, and
/// audited (ADR 0056).
/// The client registry's configuration and the platform's default Matching Rules (until the
/// configuration package supplies them), when `HEALTH_REGISTRY_NIN_SYSTEM` is set.
fn registry_rules(
    settings: &AuthSettings,
) -> Option<(
    health_registries_app::fhir::RegistryConfig,
    health_client_registry::matching::MatchingRules,
)> {
    let nin = settings.registry_nin_system.as_deref()?;
    Some((
        health_registries_app::fhir::RegistryConfig {
            nin_system: nin.to_owned(),
        },
        health_client_registry::matching::MatchingRules::platform_default(
            nin,
            &[nin, health_registries_app::fhir::ECID_SYSTEM],
        ),
    ))
}

/// Escalates overdue break-glass reviews every hour (offline-auth.md § Break-glass).
fn escalate_hourly(subjects: Subjects<SurrealAuthStore>) {
    tokio::spawn(async move {
        let mut every = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            every.tick().await;
            match subjects.escalate_overdue().await {
                Ok(0) => {}
                Ok(escalated) => tracing::warn!(
                    escalated,
                    outcome = "break-glass-reviews-overdue",
                    "break-glass reviews are overdue"
                ),
                Err(_) => tracing::warn!(
                    error.kind = "escalation-failed",
                    "break-glass reviews not checked"
                ),
            }
        }
    });
}

/// Recomputes block counts every hour (ADR 0055), starting now.
fn recount_hourly(store: SurrealStore) {
    tokio::spawn(async move {
        let mut every = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            every.tick().await;
            match store.recount_blocks().await {
                Ok(blocks) => tracing::info!(
                    blocks,
                    outcome = "blocks-recounted",
                    "block counts recomputed"
                ),
                Err(_) => {
                    tracing::warn!(error.kind = "recount-failed", "block counts not recomputed")
                }
            }
        }
    });
}

/// The search parameters central always supports: `_tag` on every facility-scope type, which
/// facility isolation searches with (ADR 0057). The rest come with the configuration package.
fn facility_registry() -> health_fhir_search::Registry {
    let tag = serde_json::json!({
        "resourceType": "SearchParameter",
        "url": "http://hl7.org/fhir/SearchParameter/Resource-tag",
        "code": "_tag",
        "base": ["Resource"],
        "type": "token",
        "expression": "Resource.meta.tag",
    });
    let catalog = health_fhir_search::Catalog::from_definitions(std::iter::once(&tag));
    let mut registry = health_fhir_search::Registry::new(jiff::tz::Offset::UTC);
    for resource_type in health_access_policy::FACILITY_SCOPE_TYPES {
        if registry
            .support(
                &catalog,
                resource_type,
                "_tag",
                health_fhir_search::ParamState::Active,
            )
            .is_err()
        {
            tracing::warn!(resource_type, "no _tag index for a facility-scope type");
        }
    }
    registry
}

/// The policy's sensitive labels, as the auditor takes them.
fn sensitive(policy: &AccessPolicy) -> Vec<(String, String)> {
    policy
        .labels
        .iter()
        .map(|l| (l.system.clone(), l.code.clone()))
        .collect()
}

fn fhir(
    store: SurrealStore,
    settings: &AuthSettings,
    policy: &AccessPolicy,
    subjects: Subjects<SurrealAuthStore>,
) -> (Router, Option<Router>, Option<Router>) {
    // Facility-scope records are searched by their facility tag (ADR 0057).
    let store = store.with_registry(facility_registry());
    // With the registry, source Patients carry their blocking keys, and block counts are kept.
    let store = match registry_rules(settings) {
        Some((config, rules)) => {
            let store = store.with_blocking(Arc::new(
                health_registries_app::matching::RegistryBlocking::new(rules, config),
            ));
            recount_hourly(store.clone());
            store
        }
        None => store,
    };
    let rules: Vec<LabelRule> = policy
        .labels
        .iter()
        .map(|l| LabelRule {
            label: Label::new(&l.system, &l.code),
            program: l.program.clone(),
        })
        .collect();
    let origin = settings
        .issuer
        .split('/')
        .take(3)
        .collect::<Vec<_>>()
        .join("/");
    let mut fhir = FhirServer::new(store, format!("{origin}/fhir"), Arc::new(Unrestricted))
        .with_access_control(
            Pdp::new(rules).with_scope(NodeScope::Central),
            Arc::new(FacilityTag),
        )
        .with_audit(Auditor::new("central", sensitive(policy)));
    if let Some(rules) = registry_rules(settings) {
        fhir = fhir
            .with_client_registry(rules.0, rules.1)
            .with_registration(health_registries_app::register::MrnSettings {
                node: settings.node_code.clone(),
                format: health_identifiers::NumberFormat {
                    padding: 6,
                    ..health_identifiers::NumberFormat::default()
                },
                system: "urn:health:mrn".into(),
            });
    }
    let layer = axum::middleware::from_fn_with_state(subjects, subject_layer);
    // The registry review queue (#203), with the registry.
    let reviews =
        registry_rules(settings).map(|_| fhir.registry_review_router().layer(layer.clone()));
    // The portal, with the registry; its patients pass `patient_layer`, not the subject layer.
    let portal = registry_rules(settings).map(|_| fhir.portal_router());
    (fhir.router().layer(layer), reviews, portal)
}

/// `/auth`, `/api/session` and `/fhir`, forwarded (paths intact) to the routes once they are
/// built, and 503 until then.
pub(crate) fn gate(ready: Arc<OnceCell<Router>>) -> Router {
    let service = tower::service_fn(move |request: Request<Body>| {
        let ready = Arc::clone(&ready);
        async move {
            let response = match ready.get() {
                Some(router) => {
                    // A fresh request: the gate's own route parameters must not reach the
                    // routes' extractors.
                    let (parts, body) = request.into_parts();
                    let mut fresh = Request::new(body);
                    *fresh.method_mut() = parts.method;
                    *fresh.uri_mut() = parts.uri;
                    *fresh.version_mut() = parts.version;
                    *fresh.headers_mut() = parts.headers;
                    router.clone().oneshot(fresh).await.into_response()
                }
                None => (StatusCode::SERVICE_UNAVAILABLE, "sign-in is starting").into_response(),
            };
            Ok::<Response, Infallible>(response)
        }
    });
    Router::new()
        .route_service("/auth/{*rest}", service.clone())
        .route_service("/api/session", service.clone())
        .route_service("/api/break-glass", service.clone())
        .route_service("/api/events", service.clone())
        .route_service("/api/registry/reviews", service.clone())
        .route_service("/api/registry/reviews/{*rest}", service.clone())
        .route_service("/api/portal/{*rest}", service.clone())
        .route_service("/api/caregivers", service.clone())
        .route_service("/api/caregivers/{*rest}", service.clone())
        .route_service("/api/break-glass/{*rest}", service.clone())
        .route_service("/fhir", service.clone())
        .route_service("/fhir/{*rest}", service)
}

#[cfg(test)]
mod tests {
    use health_surrealdb::{MIGRATIONS, Settings, Target, migrate};
    use http_body_util::BodyExt;

    use super::*;

    fn settings() -> AuthSettings {
        AuthSettings {
            issuer: "https://central.test/auth".into(),
            keys: vec![[9u8; 32], [8u8; 32]],
            pepper: Vec::new(),
            clients: vec![Client {
                id: "health-client".into(),
                redirect_uris: vec!["https://central.test/callback".into()],
                patient: false,
            }],
            login_page: Some("/sign-in".into()),
            snapshot_keys: Vec::new(),
            policy: None,
            registry_nin_system: None,
            portal: None,
            node_code: "C1".parse().expect("node code"),
        }
    }

    #[tokio::test]
    async fn the_gate_answers_503_until_the_database_is_ready() {
        let ready = Arc::new(OnceCell::new());
        let app = gate(Arc::clone(&ready));
        let discovery = || {
            Request::get("/auth/.well-known/openid-configuration")
                .body(Body::empty())
                .expect("request")
        };
        assert_eq!(
            app.clone()
                .oneshot(discovery())
                .await
                .expect("response")
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );

        let connection = Connection::open(&Settings {
            target: Target::Memory,
            namespace: "central".into(),
            database: "main".into(),
        })
        .await
        .expect("open");
        migrate(&connection, MIGRATIONS).await.expect("migrate");
        let _ = ready.set(routes(connection, &settings()).expect("routes"));
        let response = app.clone().oneshot(discovery()).await.expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let metadata: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(
            metadata["token_endpoint"],
            "https://central.test/auth/token"
        );
        // Both keys are published: tokens signed before a rotation stay verifiable.
        let jwks = app
            .clone()
            .oneshot(
                Request::get("/auth/jwks")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let keys: serde_json::Value =
            serde_json::from_slice(&jwks.into_body().collect().await.expect("body").to_bytes())
                .expect("json");
        assert_eq!(keys["keys"].as_array().map(Vec::len), Some(2));
        // The authorized route answers 401 without a token, and /authorize sends the browser to
        // the web client's sign-in page.
        let session = app
            .clone()
            .oneshot(
                Request::get("/api/session")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(session.status(), StatusCode::UNAUTHORIZED);
        let authorize = app
            .oneshot(Request::get("/auth/authorize?response_type=code&client_id=health-client&redirect_uri=https%3A%2F%2Fcentral.test%2Fcallback&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256").body(Body::empty()).expect("request"))
            .await
            .expect("response");
        assert_eq!(authorize.status(), StatusCode::SEE_OTHER);
        assert!(
            authorize.headers()["location"]
                .to_str()
                .expect("location")
                .starts_with("/sign-in?")
        );
    }

    #[tokio::test]
    async fn the_portal_signs_patients_in_by_phone_and_registers_them() {
        let connection = Connection::open(&Settings {
            target: Target::Memory,
            namespace: "central".into(),
            database: "main".into(),
        })
        .await
        .expect("open");
        migrate(&connection, MIGRATIONS).await.expect("migrate");
        let dir = tempfile::tempdir().expect("dir");
        let outbox = dir.path().join("otp");
        let mut settings = settings();
        settings.login_page = None;
        settings.snapshot_keys = vec![[7u8; 32]];
        settings.registry_nin_system = Some("urn:health:nin".into());
        settings.policy = Some(AccessPolicy::default());
        settings.clients.push(Client {
            id: "health-portal".into(),
            redirect_uris: vec!["https://central.test/portal/callback".into()],
            patient: true,
        });
        settings.portal = Some(PortalSettings {
            country_code: "256".into(),
            otp_outbox: outbox.clone(),
        });
        let ready = Arc::new(OnceCell::new());
        let app = gate(Arc::clone(&ready));
        let _ = ready.set(routes(connection, &settings).expect("routes"));

        let request = [
            ("response_type", "code"),
            ("client_id", "health-portal"),
            ("redirect_uri", "https://central.test/portal/callback"),
            ("scope", "openid"),
            ("state", "s"),
            (
                "code_challenge",
                "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
            ),
            ("code_challenge_method", "S256"),
        ];
        let with = |extra: &[(&'static str, String)]| {
            let mut fields: Vec<(&str, String)> =
                request.iter().map(|(k, v)| (*k, (*v).to_owned())).collect();
            fields.extend(extra.iter().cloned());
            fields
        };
        let post = |path: &str, fields: Vec<(&str, String)>| {
            let pairs: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
            form(path, &pairs)
        };
        let (status, _, _) = send(
            &app,
            post("/auth/otp", with(&[("phone", "0772 123456".into())])),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let line = std::fs::read_to_string(&outbox).expect("outbox");
        let code = line
            .strip_prefix("+256772123456 ")
            .expect("the code went to the outbox")
            .trim()
            .to_owned();
        let (_, _, location) = send(
            &app,
            post(
                "/auth/authorize",
                with(&[("phone", "0772123456".into()), ("code", code)]),
            ),
        )
        .await;
        let location = location.expect("redirect");
        let code = form_urlencoded::parse(location.split_once('?').expect("query").1.as_bytes())
            .find(|(k, _)| k == "code")
            .expect("code")
            .1
            .into_owned();
        let (status, body, _) = send(
            &app,
            form(
                "/auth/token",
                &[
                    ("grant_type", "authorization_code"),
                    ("code", &code),
                    ("redirect_uri", "https://central.test/portal/callback"),
                    ("client_id", "health-portal"),
                    (
                        "code_verifier",
                        "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
                    ),
                ],
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let tokens: serde_json::Value = serde_json::from_str(&body).expect("json");
        let bearer = format!("Bearer {}", tokens["access_token"].as_str().expect("token"));
        let portal = |method: &str, body: Option<serde_json::Value>, auth: bool| {
            let mut request = Request::builder()
                .method(method)
                .uri("/api/portal/registration");
            if auth {
                request = request.header("authorization", bearer.as_str());
            }
            if body.is_some() {
                request = request.header("content-type", "application/json");
            }
            request
                .body(body.map_or(Body::empty(), |b| Body::from(b.to_string())))
                .expect("request")
        };
        assert_eq!(
            send(&app, portal("GET", None, false)).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            send(&app, portal("GET", None, true)).await.0,
            StatusCode::NOT_FOUND
        );
        let text = |link: &str, value: &str| serde_json::json!({ "linkId": link, "answer": [{ "valueString": value }] });
        let response = serde_json::json!({
            "resourceType": "QuestionnaireResponse",
            "questionnaire": "urn:health:questionnaire/self-registration",
            "status": "completed",
            "item": [
                text("given", "Grace"),
                text("family", "Nakato"),
                { "linkId": "birthDate", "answer": [{ "valueDate": "1990-04-12" }] },
                { "linkId": "gender", "answer": [{ "valueCoding": { "code": "female" } }] },
            ],
        });
        let (status, body, _) = send(&app, portal("POST", Some(response), true)).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let registered: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_eq!(registered["status"], "completed");
        assert_eq!(registered["access"], "self-submitted");
    }

    async fn send(app: &Router, request: Request<Body>) -> (StatusCode, String, Option<String>) {
        let response = app.clone().oneshot(request).await.expect("response");
        let location = response
            .headers()
            .get(axum::http::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        (
            status,
            String::from_utf8_lossy(&body).into_owned(),
            location,
        )
    }

    fn form(path: &str, fields: &[(&str, &str)]) -> Request<Body> {
        let body = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(fields)
            .finish();
        Request::post(path)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .expect("request")
    }

    #[tokio::test]
    async fn fhir_is_served_to_signed_in_users_with_their_roles_permissions() {
        use health_auth::SnapshotSigner;
        use health_identity::snapshot::{PermissionSnapshot, RoleAssignment};
        use health_identity::{AccountId, AccountStatus, Lockout, LoginAccount};

        let connection = Connection::open(&Settings {
            target: Target::Memory,
            namespace: "central".into(),
            database: "main".into(),
        })
        .await
        .expect("open");
        migrate(&connection, MIGRATIONS).await.expect("migrate");
        let snapshot_key = [7u8; 32];
        let policy: AccessPolicy = serde_json::from_value(serde_json::json!({
            "roles": { "clinician": ["Patient.crs"], "cashier": ["Invoice.crus"] },
            "labels": [{ "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode", "code": "HIV", "program": "hiv" }],
        }))
        .expect("policy");
        // Provision a clinician at KAM013, as central's workforce registry will.
        let provisioning = SurrealAuthStore::new(
            SurrealStore::new(connection.clone(), Arc::new(SystemClock)),
            SessionPolicy::default().hard_cap,
        );
        let credentials = Credentials::new(Vec::new()).expect("credentials");
        let account = AccountId(uuid::Uuid::now_v7());
        provisioning
            .put_account(&LoginAccount {
                id: account,
                username: "nakato".into(),
                credential: credentials.hash("correct horse battery").expect("hash"),
                status: AccountStatus::Active,
                lockout: Lockout::default(),
            })
            .await
            .expect("account");
        let issued = health_time::Timestamp::now();
        let snapshot = PermissionSnapshot {
            account,
            facility: "KAM013".into(),
            roles: vec![RoleAssignment {
                role: "clinician".into(),
                locations: vec![],
            }],
            programs: vec![],
            issued_at: issued,
            not_after: SnapshotPolicy::default().not_after(issued),
            snapshot_version: 1,
        };
        let jws = SnapshotSigner::from_secret(&snapshot_key)
            .expect("key")
            .sign(&snapshot, &policy.roles)
            .expect("sign");
        provisioning
            .put_snapshot(account, "KAM013", &jws)
            .await
            .expect("snapshot");

        let mut settings = settings();
        settings.login_page = None;
        settings.snapshot_keys = vec![snapshot_key];
        settings.registry_nin_system = Some("urn:health:nin".into());
        settings.policy = Some(policy);
        let ready = Arc::new(OnceCell::new());
        let app = gate(Arc::clone(&ready));
        let _ = ready.set(routes(connection, &settings).expect("routes"));

        // Sign in: one facility, so none need be named.
        let (_, _, location) = send(
            &app,
            form(
                "/auth/authorize",
                &[
                    ("response_type", "code"),
                    ("client_id", "health-client"),
                    ("redirect_uri", "https://central.test/callback"),
                    ("scope", "openid"),
                    ("state", "s"),
                    (
                        "code_challenge",
                        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
                    ),
                    ("code_challenge_method", "S256"),
                    ("username", "nakato"),
                    ("password", "correct horse battery"),
                ],
            ),
        )
        .await;
        let location = location.expect("redirect");
        let code = form_urlencoded::parse(location.split_once('?').expect("query").1.as_bytes())
            .find(|(k, _)| k == "code")
            .expect("code")
            .1
            .into_owned();
        let (status, body, _) = send(
            &app,
            form(
                "/auth/token",
                &[
                    ("grant_type", "authorization_code"),
                    ("code", &code),
                    ("redirect_uri", "https://central.test/callback"),
                    ("client_id", "health-client"),
                    (
                        "code_verifier",
                        "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
                    ),
                ],
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let tokens: serde_json::Value = serde_json::from_str(&body).expect("json");
        let bearer = format!("Bearer {}", tokens["access_token"].as_str().expect("token"));
        let fhir = |method: &str,
                    path: &str,
                    body: Option<serde_json::Value>,
                    purpose: Option<&str>,
                    auth: bool| {
            let mut request = Request::builder().method(method).uri(path);
            if auth {
                request = request.header("authorization", bearer.as_str());
            }
            if let Some(purpose) = purpose {
                request = request.header("x-purpose-of-use", purpose);
            }
            if body.is_some() {
                request = request.header("content-type", "application/fhir+json");
            }
            request
                .body(body.map_or(Body::empty(), |b| Body::from(b.to_string())))
                .expect("request")
        };

        // No route serves FHIR data without a signed-in user; the CapabilityStatement is open.
        assert_eq!(
            send(
                &app,
                fhir(
                    "GET",
                    "/fhir/Patient/018f0000-0000-7000-8000-000000000000",
                    None,
                    None,
                    false
                )
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            send(&app, fhir("GET", "/fhir/metadata", None, None, false))
                .await
                .0,
            StatusCode::OK
        );
        // The clinician's role creates and reads Patients.
        let (status, body, _) = send(
            &app,
            fhir(
                "POST",
                "/fhir/Patient",
                Some(serde_json::json!({ "resourceType": "Patient" })),
                None,
                true,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let id = serde_json::from_str::<serde_json::Value>(&body).expect("json")["id"]
            .as_str()
            .expect("id")
            .to_owned();
        let (status, _, _) = send(
            &app,
            fhir(
                "GET",
                &format!("/fhir/Patient/{id}"),
                None,
                Some("TREAT"),
                true,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        // But not Invoices, which another role holds.
        let (status, _, _) = send(
            &app,
            fhir(
                "POST",
                "/fhir/Invoice",
                Some(serde_json::json!({ "resourceType": "Invoice", "status": "draft" })),
                None,
                true,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        // Patient/$match ranks candidates for anyone who may search Patients.
        let (status, body, _) = send(
            &app,
            fhir(
                "POST",
                "/fhir/Patient/$match",
                Some(serde_json::json!({ "resourceType": "Parameters", "parameter": [
                    { "name": "resource", "resource": { "resourceType": "Patient", "name": [{ "family": "Nakato", "given": ["Grace"] }], "birthDate": "1990-04-02", "gender": "female" } },
                ] })),
                None,
                true,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        // Reception registers a new patient, with this node's MRN.
        let (status, body, _) = send(
            &app,
            fhir(
                "POST",
                "/fhir/Patient/$register",
                Some(serde_json::json!({ "resourceType": "Parameters", "parameter": [
                    { "name": "resource", "resource": { "resourceType": "Patient", "name": [{ "family": "Achen", "given": ["Ruth"] }], "birthDate": "1985-01-20", "gender": "female" } },
                    { "name": "confirmedNew", "valueBoolean": true },
                ] })),
                None,
                true,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert!(body.contains("KAM013-C1-000001"), "{body}");
        // The registry review queue is served, to reviewers only.
        let (status, body, _) =
            send(&app, fhir("GET", "/api/registry/reviews", None, None, true)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        // The registry's operations are served, to reviewers only.
        let (status, body, _) = send(
            &app,
            fhir(
                "POST",
                &format!("/fhir/Patient/{id}/$link"),
                None,
                None,
                true,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        // An unknown purpose of use is refused.
        let (status, body, _) = send(
            &app,
            fhir(
                "GET",
                &format!("/fhir/Patient/{id}"),
                None,
                Some("SNOOPING"),
                true,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }

    /// Provisions `username` at KAM013 with `roles`, as the workforce registry will.
    async fn provision(
        connection: &Connection,
        key: &[u8; 32],
        policy: &AccessPolicy,
        username: &str,
        roles: &[&str],
    ) {
        use health_auth::SnapshotSigner;
        use health_identity::snapshot::{PermissionSnapshot, RoleAssignment};
        use health_identity::{AccountId, AccountStatus, Lockout, LoginAccount};
        let store = SurrealAuthStore::new(
            SurrealStore::new(connection.clone(), Arc::new(SystemClock)),
            SessionPolicy::default().hard_cap,
        );
        let credentials = Credentials::new(Vec::new()).expect("credentials");
        let account = AccountId(uuid::Uuid::now_v7());
        store
            .put_account(&LoginAccount {
                id: account,
                username: username.into(),
                credential: credentials.hash("correct horse battery").expect("hash"),
                status: AccountStatus::Active,
                lockout: Lockout::default(),
            })
            .await
            .expect("account");
        let issued = health_time::Timestamp::now();
        let snapshot = PermissionSnapshot {
            account,
            facility: "KAM013".into(),
            roles: roles
                .iter()
                .map(|r| RoleAssignment {
                    role: (*r).into(),
                    locations: vec![],
                })
                .collect(),
            programs: vec![],
            issued_at: issued,
            not_after: SnapshotPolicy::default().not_after(issued),
            snapshot_version: 1,
        };
        let jws = SnapshotSigner::from_secret(key)
            .expect("key")
            .sign(&snapshot, &policy.roles)
            .expect("sign");
        store
            .put_snapshot(account, "KAM013", &jws)
            .await
            .expect("snapshot");
    }

    /// Signs `username` in: the `Authorization` header value.
    async fn sign_in(app: &Router, username: &str) -> String {
        let (_, _, location) = send(
            app,
            form(
                "/auth/authorize",
                &[
                    ("response_type", "code"),
                    ("client_id", "health-client"),
                    ("redirect_uri", "https://central.test/callback"),
                    ("scope", "openid"),
                    ("state", "s"),
                    (
                        "code_challenge",
                        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
                    ),
                    ("code_challenge_method", "S256"),
                    ("username", username),
                    ("password", "correct horse battery"),
                ],
            ),
        )
        .await;
        let location = location.expect("redirect");
        let code = form_urlencoded::parse(location.split_once('?').expect("query").1.as_bytes())
            .find(|(k, _)| k == "code")
            .expect("code")
            .1
            .into_owned();
        let (status, body, _) = send(
            app,
            form(
                "/auth/token",
                &[
                    ("grant_type", "authorization_code"),
                    ("code", &code),
                    ("redirect_uri", "https://central.test/callback"),
                    ("client_id", "health-client"),
                    (
                        "code_verifier",
                        "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
                    ),
                ],
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let tokens: serde_json::Value = serde_json::from_str(&body).expect("json");
        format!("Bearer {}", tokens["access_token"].as_str().expect("token"))
    }

    fn authorized(
        method: &str,
        path: &str,
        bearer: &str,
        body: Option<serde_json::Value>,
    ) -> Request<Body> {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", bearer);
        if body.is_some() {
            request = request.header("content-type", "application/json");
        }
        request
            .body(body.map_or(Body::empty(), |b| Body::from(b.to_string())))
            .expect("request")
    }

    #[tokio::test]
    async fn break_glass_opens_one_patient_audits_and_alerts_the_in_charge() {
        const ACT: &str = "http://terminology.hl7.org/CodeSystem/v3-ActCode";
        let connection = Connection::open(&Settings {
            target: Target::Memory,
            namespace: "central".into(),
            database: "glass".into(),
        })
        .await
        .expect("open");
        migrate(&connection, MIGRATIONS).await.expect("migrate");
        let key = [5u8; 32];
        let policy: AccessPolicy = serde_json::from_value(serde_json::json!({
            "roles": {
                "clinician": ["Patient.crs", "Observation.crs", "break-glass"],
                "in-charge": ["break-glass.alerts"],
                "privacy-officer": ["break-glass.review"],
            },
            "labels": [{ "system": ACT, "code": "HIV", "program": "hiv" }],
        }))
        .expect("policy");
        provision(&connection, &key, &policy, "nakato", &["clinician"]).await;
        provision(&connection, &key, &policy, "okello", &["in-charge"]).await;
        provision(&connection, &key, &policy, "achen", &["privacy-officer"]).await;
        // A patient with an HIV-labelled Observation, written as the system would.
        let admin = FhirServer::new(
            SurrealStore::new(connection.clone(), Arc::new(SystemClock)),
            "https://central.test/fhir",
            Arc::new(Unrestricted),
        )
        .router();
        let created = |body: serde_json::Value| {
            let admin = admin.clone();
            async move {
                let request =
                    Request::post(format!("/{}", body["resourceType"].as_str().expect("type")))
                        .header("content-type", "application/fhir+json")
                        .body(Body::from(body.to_string()))
                        .expect("request");
                let (status, body, _) = send(&admin, request).await;
                assert_eq!(status, StatusCode::CREATED, "{body}");
                serde_json::from_str::<serde_json::Value>(&body).expect("json")["id"]
                    .as_str()
                    .expect("id")
                    .to_owned()
            }
        };
        let patient = created(serde_json::json!({ "resourceType": "Patient" })).await;
        let observation = created(serde_json::json!({
            "resourceType": "Observation",
            "status": "final",
            "code": { "text": "viral load" },
            "subject": { "reference": format!("Patient/{patient}") },
            "meta": { "security": [{ "system": ACT, "code": "HIV" }] },
        }))
        .await;

        let mut settings = settings();
        settings.login_page = None;
        settings.snapshot_keys = vec![key];
        settings.policy = Some(policy);
        let ready = Arc::new(OnceCell::new());
        let app = gate(Arc::clone(&ready));
        let _ = ready.set(routes(connection, &settings).expect("routes"));
        let clinician = sign_in(&app, "nakato").await;
        let in_charge = sign_in(&app, "okello").await;
        let privacy = sign_in(&app, "achen").await;
        // The in-charge's event stream, open before the grant.
        let stream = app
            .clone()
            .oneshot(authorized("GET", "/api/events", &in_charge, None))
            .await
            .expect("response");
        assert_eq!(stream.headers()["content-type"], "text/event-stream");
        let mut stream = stream.into_body();
        let read = format!("/fhir/Observation/{observation}");

        // Without break-glass the HIV record does not exist for this clinician.
        assert_eq!(
            send(&app, authorized("GET", &read, &clinician, None))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        // A reason and a justification are required.
        let glass = |justification: &str| serde_json::json!({ "patient": format!("Patient/{patient}"), "reason": "BTG", "justification": justification });
        let (status, _, _) = send(
            &app,
            authorized("POST", "/api/break-glass", &clinician, Some(glass(" "))),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // The in-charge's role may not break glass.
        let (status, _, _) = send(
            &app,
            authorized(
                "POST",
                "/api/break-glass",
                &in_charge,
                Some(glass("unconscious")),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, body, _) = send(
            &app,
            authorized(
                "POST",
                "/api/break-glass",
                &clinician,
                Some(glass("unconscious on arrival")),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let granted: serde_json::Value = serde_json::from_str(&body).expect("json");
        // One grant at a time, never extended.
        let (status, _, _) = send(
            &app,
            authorized("POST", "/api/break-glass", &clinician, Some(glass("again"))),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        // Now the record opens.
        assert_eq!(
            send(&app, authorized("GET", &read, &clinician, None))
                .await
                .0,
            StatusCode::OK
        );

        // The in-charge sees the alert, and acknowledges it; the clinician may not see alerts.
        let (status, body, _) = send(
            &app,
            authorized("GET", "/api/break-glass/alerts", &in_charge, None),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let alerts: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_eq!(alerts.as_array().map(Vec::len), Some(1));
        assert_eq!(alerts[0]["justification"], "unconscious on arrival");
        assert_eq!(alerts[0]["id"], granted["id"]);
        assert_eq!(
            send(
                &app,
                authorized("GET", "/api/break-glass/alerts", &clinician, None)
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        let acknowledge = format!(
            "/api/break-glass/alerts/{}/acknowledge",
            granted["id"].as_str().expect("id")
        );
        assert_eq!(
            send(&app, authorized("POST", &acknowledge, &in_charge, None))
                .await
                .0,
            StatusCode::NO_CONTENT
        );
        let (_, body, _) = send(
            &app,
            authorized("GET", "/api/break-glass/alerts", &in_charge, None),
        )
        .await;
        assert_eq!(body, "[]");

        // The grant reached the in-charge's open stream as an invalidation hint.
        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), stream.frame())
            .await
            .expect("a hint in time")
            .expect("frame")
            .expect("ok");
        let text = String::from_utf8(frame.into_data().expect("data").to_vec()).expect("utf8");
        assert!(
            text.contains("\"worklist\":\"break-glass-alerts\""),
            "{text}"
        );

        // The privacy officer reviews it; other roles may not.
        let (status, body, _) = send(
            &app,
            authorized("GET", "/api/break-glass/reviews", &privacy, None),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let pending: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_eq!(pending.as_array().map(Vec::len), Some(1));
        assert_eq!(pending[0]["overdue"], false);
        assert_eq!(
            send(
                &app,
                authorized("GET", "/api/break-glass/reviews", &in_charge, None)
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        let decide = format!(
            "/api/break-glass/reviews/{}",
            granted["id"].as_str().expect("id")
        );
        let (status, _, _) = send(
            &app,
            authorized(
                "POST",
                &decide,
                &privacy,
                Some(serde_json::json!({ "decision": "inappropriate" })),
            ),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "an inappropriate access needs a note"
        );
        let (status, _, _) = send(&app, authorized("POST", &decide, &privacy, Some(serde_json::json!({ "decision": "appropriate", "note": "confirmed with the ward" })))).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _, _) = send(
            &app,
            authorized(
                "POST",
                &decide,
                &privacy,
                Some(serde_json::json!({ "decision": "appropriate" })),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "a review is made once");
        let (_, body, _) = send(
            &app,
            authorized("GET", "/api/break-glass/reviews", &privacy, None),
        )
        .await;
        assert_eq!(body, "[]");

        // The grant and the read under it are both audited with BTG, and the review is audited.
        let request = Request::get("/AuditEvent?_count=100")
            .body(Body::empty())
            .expect("request");
        let (_, body, _) = send(&admin, request).await;
        let bundle: serde_json::Value = serde_json::from_str(&body).expect("json");
        let events: Vec<&serde_json::Value> = bundle["entry"]
            .as_array()
            .map(|e| e.iter().map(|e| &e["resource"]).collect())
            .unwrap_or_default();
        assert!(
            events
                .iter()
                .any(|e| e["code"]["coding"][0]["code"] == "BTG"),
            "{body}"
        );
        assert!(
            events
                .iter()
                .any(|e| e["code"]["coding"][0]["code"] == "read"
                    && e["authorization"].to_string().contains("BTG")),
            "{body}"
        );
        assert!(
            events
                .iter()
                .any(|e| e["entity"].to_string().contains("break-glass/")),
            "{body}"
        );
    }

    #[test]
    fn every_facility_scope_type_is_searchable_by_its_facility() {
        let tag = serde_json::json!({
            "resourceType": "SearchParameter",
            "url": "http://hl7.org/fhir/SearchParameter/Resource-tag",
            "code": "_tag",
            "base": ["Resource"],
            "type": "token",
            "expression": "Resource.meta.tag",
        });
        let catalog = health_fhir_search::Catalog::from_definitions(std::iter::once(&tag));
        let mut registry = health_fhir_search::Registry::new(jiff::tz::Offset::UTC);
        let failed: Vec<&str> = health_access_policy::FACILITY_SCOPE_TYPES
            .iter()
            .copied()
            .filter(|t| {
                registry
                    .support(&catalog, t, "_tag", health_fhir_search::ParamState::Active)
                    .is_err()
            })
            .collect();
        assert!(failed.is_empty(), "{failed:?}");
        let _ = facility_registry();
    }

    #[test]
    fn key_files_are_hex_lines_active_first() {
        let dir = tempfile::tempdir().expect("tmp");
        let path = dir.path().join("keys");
        std::fs::write(
            &path,
            format!("# active\n{}\n\n{}\n", "09".repeat(32), "08".repeat(32)),
        )
        .expect("write");
        assert_eq!(read_keys(&path), Ok(vec![[9u8; 32], [8u8; 32]]));
        std::fs::write(&path, "zz\n").expect("write");
        assert!(read_keys(&path).is_err());
        std::fs::write(&path, "0909\n").expect("write");
        assert!(read_keys(&path).is_err());
        std::fs::write(&path, "# nothing\n").expect("write");
        assert!(read_keys(&path).is_err());
    }
}
