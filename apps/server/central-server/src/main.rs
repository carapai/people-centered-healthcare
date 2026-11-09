//! Central server composition root (ADR 0001): one process serving every Bounded Context over
//! the shared `health-http` router, with a separate SurrealDB server (persistence.md).
//!
//! Startup order:
//! 1. Telemetry, through the allowlist (ADR 0047).
//! 2. The HTTP listener, at once: `/health` answers, and `/ready` reports `not-ready` until the
//!    database is connected and migrated.
//! 3. The database, connected and migrated in the background with backoff, so a slow or
//!    restarting SurrealDB never stops central from reporting its state.
//!
//! `central-server openapi` prints central's OpenAPI document and exits, without configuration;
//! `tools/codegen` generates `@health/api-client` from it (client-application.md § Types).

mod activation;
mod auth;
mod config;
mod node;
mod package;
mod provision;
mod recovery;
mod seed;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use health_http::{Readiness, ReadinessCheck, app, document};
use health_observability::TelemetryConfig;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use tokio::sync::OnceCell;

use crate::config::Config;

/// The migrated database, once the background task has opened it.
#[derive(Clone, Default)]
struct Database(Arc<OnceCell<Connection>>);

impl ReadinessCheck for Database {
    fn name(&self) -> &'static str {
        "database"
    }

    fn check(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async {
            match self.0.get() {
                Some(connection) => connection.query("RETURN 1").await.is_ok(),
                None => false,
            }
        })
    }
}

/// Connects and migrates, retrying with capped exponential backoff until it succeeds.
async fn open_database(
    config: Config,
    password: String,
    database: Database,
    auth: Option<(auth::AuthSettings, Arc<OnceCell<axum::Router>>)>,
) {
    let settings = Settings {
        target: Target::Remote {
            endpoint: config.surrealdb_endpoint,
            username: config.surrealdb_user,
            password,
        },
        namespace: config.tenant,
        database: config.database,
    };
    let mut delay = Duration::from_millis(500);
    for attempt in 1_u64.. {
        let opened = match Connection::open(&settings).await {
            Ok(connection) => match migrate(&connection, MIGRATIONS).await {
                Ok(_) => Ok(connection),
                Err(_) => Err("migration-failed"),
            },
            Err(_) => Err("database-unavailable"),
        };
        match opened {
            Ok(connection) => {
                if let Some(path) = &config.dev_fhir_seed {
                    // With central's search registry, so seeded resources are indexed.
                    let store = SurrealStore::new(connection.clone(), Arc::new(SystemClock))
                        .with_registry(auth::search_registry());
                    match seed::run(&store, path).await {
                        Ok(added) => tracing::info!(added, "development FHIR seed written"),
                        Err(reason) => tracing::error!(error.kind = reason, "seed refused"),
                    }
                }
                // Package activation (#89): the national set, and the definitions it serves.
                match (
                    &config.national_root_file,
                    config.auth.as_ref().and_then(|a| a.files_dir.clone()),
                ) {
                    (Some(root), Some(files)) => {
                        tokio::spawn(activation::run(connection.clone(), root.clone(), files));
                    }
                    (Some(_), None) => tracing::warn!(
                        "HEALTH_NATIONAL_ROOT_FILE without HEALTH_FILES_DIR: packages are not activated"
                    ),
                    _ => {}
                }
                if let Some((settings, ready)) = &auth {
                    match auth::routes(connection.clone(), settings) {
                        Ok(router) => {
                            let _ = ready.set(router);
                        }
                        Err(reason) => tracing::error!(error.kind = reason, "sign-in disabled"),
                    }
                }
                let _ = database.0.set(connection);
                tracing::info!(attempt, outcome = "database-ready", "database ready");
                return;
            }
            // Error messages can carry endpoints and other free text, so only a kind is logged.
            Err(kind) => tracing::warn!(attempt, error.kind = kind, "database not ready"),
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(Duration::from_secs(30));
    }
}

async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
}

/// The signed-in session, as `GET /api/session` answers (served by `health-auth`).
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct SessionInfo {
    /// The account id.
    account: String,
    /// The client the session was issued to.
    client_id: String,
    /// The scopes granted, space-separated.
    scope: String,
    /// The session id.
    session: String,
    /// The facility the session acts for (ADR 0056).
    facility: Option<String>,
}

/// Documents `GET /api/session`; the route itself is `health_auth::AuthServer::session_route`.
#[utoipa::path(get, path = "/api/session", tag = "identity", operation_id = "getSession",
    description = "The signed-in session. Needs the access token (`Authorization: Bearer` or `DPoP`).",
    responses(
        (status = 200, description = "The session is live", body = SessionInfo),
        (status = 401, description = "No valid access token, or the session has ended"),
    ))]
#[allow(dead_code)]
async fn session() {}

#[derive(utoipa::OpenApi)]
#[openapi(paths(session), components(schemas(SessionInfo)))]
struct CentralApi;

/// Central's OpenAPI document: its API routes and the system routes. The FHIR API at `/fhir`
/// is described by its CapabilityStatement (`/fhir/metadata`), not here.
fn openapi() -> utoipa::openapi::OpenApi {
    document("central-server", <CentralApi as utoipa::OpenApi>::openapi())
}

/// The authorization server's settings, read from `files` and the secret files they name.
/// The facility issuing CA from its certificate and key files (#268).
fn read_issuing_ca(
    cert: &std::path::Path,
    key: &std::path::Path,
) -> Result<health_pki::Authority, Box<dyn std::error::Error>> {
    let read = |path: &std::path::Path, what: &str| {
        std::fs::read_to_string(path).map_err(|_| format!("cannot read {what}"))
    };
    let authority = health_pki::Authority::from_pem(
        &read(cert, "HEALTH_ISSUING_CA_CERT_FILE")?,
        &read(key, "HEALTH_ISSUING_CA_KEY_FILE")?,
    )
    .map_err(|e| format!("the issuing CA: {e}"))?;
    if authority.tier() != health_pki::Tier::IssuingCa {
        return Err("HEALTH_ISSUING_CA_CERT_FILE is not a facility issuing CA".into());
    }
    Ok(authority)
}

fn auth_settings(
    files: &config::AuthFiles,
) -> Result<auth::AuthSettings, Box<dyn std::error::Error>> {
    let pepper = match &files.pepper_file {
        Some(path) => auth::read_pepper(path)?.to_vec(),
        None => Vec::new(),
    };
    Ok(auth::AuthSettings {
        issuer: files.issuer.clone(),
        keys: auth::read_keys(&files.keys_file)?,
        pepper,
        clients: std::iter::once(health_auth::Client {
            id: files.client_id.clone(),
            redirect_uris: files.redirect_uris.clone(),
            patient: false,
        })
        .chain(files.portal.iter().map(|portal| health_auth::Client {
            id: portal.client_id.clone(),
            redirect_uris: portal.redirect_uris.clone(),
            patient: true,
        }))
        .collect(),
        payments: if files.payment_tills.is_empty() {
            None
        } else {
            Some(health_fhir_server::PaymentConfig {
                currency: health_financial_ledger::Currency::from_code(&files.currency)
                    .map_err(|_| "HEALTH_CURRENCY is not an ISO 4217 code")?,
                tills: files.payment_tills.clone(),
                node: files.node_code.clone(),
                pending_for: health_time::SignedDuration::from_hours(24),
            })
        },
        portal: files.portal.as_ref().map(|portal| auth::PortalSettings {
            country_code: portal.country_code.clone(),
            otp_outbox: portal.otp_outbox.clone(),
        }),
        login_page: files.login_page.clone(),
        snapshot_keys: match &files.snapshot_keys_file {
            Some(path) => auth::read_keys(path)
                .map_err(|_| "HEALTH_AUTH_SNAPSHOT_KEYS_FILE is not a valid key file")?,
            None => Vec::new(),
        },
        registry_nin_system: files.registry_nin_system.clone(),
        files: files
            .files_dir
            .as_ref()
            .map(health_files::FsBlobStore::open)
            .transpose()
            .map_err(|_| "HEALTH_FILES_DIR cannot be opened as an attachment store")?,
        kms: files
            .kms_key_file
            .as_deref()
            .map(auth::read_kms_key)
            .transpose()?
            .map(|key| Arc::new(health_provisioning::LocalKeyWrap::new(&key))),
        pricing: health_fhir_server::PricingConfig {
            currency: health_financial_ledger::Currency::from_code(&files.currency)
                .map_err(|_| "HEALTH_CURRENCY is not an ISO 4217 code")?
                .code()
                .to_owned(),
        },
        workflows: files
            .dev_workflow_definitions
            .as_deref()
            .map(auth::read_workflows)
            .transpose()?,
        issuing_ca: files
            .issuing_ca
            .as_ref()
            .map(|(cert, key)| read_issuing_ca(cert, key))
            .transpose()?
            .map(Arc::new),
        node_code: files
            .node_code
            .parse()
            .map_err(|_| "HEALTH_NODE_CODE is not a short code")?,
        policy: files
            .access_policy_file
            .as_deref()
            .map(auth::read_policy)
            .transpose()?,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // SurrealDB 4 needs a larger stack than Tokio's default (see `THREAD_STACK`).
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(health_surrealdb::THREAD_STACK)
        .build()?
        .block_on(run())
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("openapi") {
        let mut json = openapi().to_pretty_json()?;
        json.push('\n');
        std::io::Write::write_all(&mut std::io::stdout(), json.as_bytes())?;
        return Ok(());
    }
    let config = Config::from_lookup(|name| std::env::var(name).ok())?;
    if std::env::args().nth(1).as_deref() == Some("node") {
        let args: Vec<String> = std::env::args().skip(2).collect();
        return node::run(&config, &args).await;
    }
    if std::env::args().nth(1).as_deref() == Some("package") {
        let args: Vec<String> = std::env::args().skip(2).collect();
        return package::run(&config, &args).await;
    }
    if std::env::args().nth(1).as_deref() == Some("recovery") {
        let args: Vec<String> = std::env::args().skip(2).collect();
        return recovery::run(&config, &args).await;
    }
    if std::env::args().nth(1).as_deref() == Some("provision") {
        let args: Vec<String> = std::env::args().skip(2).collect();
        return provision::run(&config, &args).await;
    }
    let _telemetry = health_observability::init(&TelemetryConfig {
        service: "central-server",
        otlp_endpoint: config.otlp_endpoint.clone(),
        filter: config.log.clone(),
    })?;
    let password = std::fs::read_to_string(&config.surrealdb_password_file)
        .map_err(|_| "cannot read HEALTH_SURREALDB_PASSWORD_FILE")?
        .trim()
        .to_owned();

    let auth = match &config.auth {
        None => None,
        Some(files) => {
            let settings = auth_settings(files)?;
            Some((settings, Arc::new(OnceCell::new())))
        }
    };
    let api = match &auth {
        Some((_, ready)) => auth::gate(Arc::clone(ready)),
        None => axum::Router::new(),
    };

    let database = Database::default();
    tokio::spawn(open_database(
        config.clone(),
        password,
        database.clone(),
        auth,
    ));

    let router = app(api, Readiness::new(vec![Arc::new(database)]));
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    tracing::info!(outcome = "listening", "central-server listening");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!(outcome = "stopped", "central-server stopped");
    Ok(())
}
