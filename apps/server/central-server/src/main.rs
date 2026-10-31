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

mod auth;
mod config;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use health_http::{Readiness, ReadinessCheck, app, document};
use health_observability::TelemetryConfig;
use health_surrealdb::{Connection, MIGRATIONS, Settings, Target, migrate};
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

/// Central's OpenAPI document: its API routes (none are mounted yet) and the system routes.
fn openapi() -> utoipa::openapi::OpenApi {
    document("central-server", utoipa::openapi::OpenApi::default())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("openapi") {
        let mut json = openapi().to_pretty_json()?;
        json.push('\n');
        std::io::Write::write_all(&mut std::io::stdout(), json.as_bytes())?;
        return Ok(());
    }
    let config = Config::from_lookup(|name| std::env::var(name).ok())?;
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
            let pepper = match &files.pepper_file {
                Some(path) => {
                    std::fs::read(path).map_err(|_| "cannot read HEALTH_AUTH_PEPPER_FILE")?
                }
                None => Vec::new(),
            };
            let settings = auth::AuthSettings {
                issuer: files.issuer.clone(),
                keys: auth::read_keys(&files.keys_file)?,
                pepper,
                clients: vec![health_auth::Client {
                    id: files.client_id.clone(),
                    redirect_uris: files.redirect_uris.clone(),
                }],
            };
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
