//! The node's life: listen, open and migrate the store, report ready, run, and stop gracefully.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use health_http::{Readiness, ReadinessCheck, app};
use health_surrealdb::{Connection, MIGRATIONS, Settings, Target, migrate};
use tokio::sync::{Notify, OnceCell};

use crate::config::EdgeConfig;
use crate::notify::ServiceManager;
use crate::supervisor::{RestartPolicy, Supervisor};
use crate::update::{self, Startup, Updates};

/// How often a long migration or drain asks the service manager for more time, and how much.
const EXTEND_EVERY: Duration = Duration::from_secs(10);
const EXTEND_BY: Duration = Duration::from_secs(30);

/// Why the node stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// The service manager or an operator asked it to.
    Requested,
    /// The UPS reported low battery.
    LowBattery,
}

/// Why the node could not run. Each kind has its own exit code in `facility-edge`, so a service
/// manager and the update rollback can tell them apart.
#[derive(Debug, thiserror::Error)]
pub enum EdgeError {
    /// The listener could not bind.
    #[error("cannot listen")]
    Listen(#[source] std::io::Error),
    /// The store could not be opened: another process holds it, or the disk failed.
    #[error("cannot open the store")]
    Store,
    /// A migration failed, or the store was migrated by a newer binary.
    #[error("the store cannot be migrated")]
    Migration,
    /// The pending update kept failing and was rolled back; the next start runs the previous
    /// binary.
    #[error("the update was rolled back")]
    RolledBack,
    /// The pending update's store is restored, but this process may not replace its own
    /// binary: the service manager's privileged rollback must.
    #[error("the update rollback needs the service manager")]
    RollbackPending,
    /// An update step failed: the node needs a person.
    #[error("the update cannot proceed")]
    Update(#[source] crate::update::UpdateError),
    /// A supervised task kept failing.
    #[error("the {0} task keeps failing")]
    Task(&'static str),
    /// Serving failed.
    #[error("serving failed")]
    Serve(#[source] std::io::Error),
}

/// The open, migrated store, once startup has it.
#[derive(Clone, Default)]
struct Store(Arc<OnceCell<Connection>>);

impl ReadinessCheck for Store {
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

/// Fails once the node has begun to stop, so load balancers and clients go elsewhere.
#[derive(Clone, Default)]
struct Serving(Arc<AtomicBool>);

impl ReadinessCheck for Serving {
    fn name(&self) -> &'static str {
        "serving"
    }

    fn check(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let serving = self.0.load(Ordering::SeqCst);
        Box::pin(async move { serving })
    }
}

/// Runs `work` while asking `manager` for more time every [`EXTEND_EVERY`].
async fn extending<T>(manager: &dyn ServiceManager, work: impl Future<Output = T>) -> T {
    let mut work = std::pin::pin!(work);
    loop {
        manager.extend(EXTEND_BY);
        tokio::select! {
            done = &mut work => return done,
            () = tokio::time::sleep(EXTEND_EVERY) => {}
        }
    }
}

/// Opens the store and runs the migrations. An embedded store is opened once: if it fails, the
/// process exits and the service manager restarts it.
async fn open(config: &EdgeConfig) -> Result<Connection, EdgeError> {
    let settings = Settings {
        target: Target::RocksDb {
            path: config.data_dir.clone(),
        },
        namespace: config.tenant.clone(),
        database: config.database.clone(),
    };
    let connection = Connection::open(&settings).await.map_err(|_| {
        tracing::error!(error.kind = "store-unavailable", "cannot open the store");
        EdgeError::Store
    })?;
    match migrate(&connection, MIGRATIONS).await {
        Ok(report) => {
            tracing::info!(
                applied = report.applied.len(),
                outcome = "migrated",
                "store migrated"
            );
            Ok(connection)
        }
        // A newer binary's migrations are unknown to this one, so an old binary stops here
        // rather than running against a migrated store.
        Err(error) => {
            tracing::error!(error.kind = %error, "migration failed");
            Err(EdgeError::Migration)
        }
    }
}

/// Runs the node until `stop` resolves, the UPS reports low battery, or a supervised task keeps
/// failing.
pub async fn run(
    config: &EdgeConfig,
    manager: Arc<dyn ServiceManager>,
    stop: impl Future<Output = ()>,
) -> Result<Stop, EdgeError> {
    let store = Store::default();
    let serving = Serving(Arc::new(AtomicBool::new(true)));
    let readiness = Readiness::new(vec![Arc::new(store.clone()), Arc::new(serving.clone())]);
    let router = app(axum::Router::new(), readiness);
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(EdgeError::Listen)?;
    tracing::info!(outcome = "listening", "facility edge listening");
    let drain = Arc::new(Notify::new());
    let draining = Arc::clone(&drain);
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move { draining.notified().await })
            .await
    });

    let updates = Updates::new(&config.update_dir);
    let binary = std::env::current_exe().map_err(|e| EdgeError::Update(e.into()))?;
    let startup = extending(manager.as_ref(), async {
        update::before_open(&updates, &config.data_dir, &binary)
    })
    .await
    .map_err(EdgeError::Update)?;
    match startup {
        Startup::RolledBack => {
            tracing::error!(
                outcome = "rolled-back",
                "the update failed and was rolled back"
            );
            return Err(EdgeError::RolledBack);
        }
        Startup::RollbackPending => {
            tracing::error!(
                outcome = "rollback-pending",
                "the update failed; the service manager restores the previous binary"
            );
            return Err(EdgeError::RollbackPending);
        }
        Startup::Normal | Startup::Updating => {}
    }
    let connection = match extending(manager.as_ref(), open(config)).await {
        Ok(connection) => connection,
        Err(EdgeError::Migration) if startup == Startup::Updating => {
            update::fail(&updates).map_err(EdgeError::Update)?;
            return Err(EdgeError::Migration);
        }
        Err(error) => return Err(error),
    };
    let _ = store.0.set(connection);
    if startup == Startup::Updating {
        update::commit(&updates).map_err(EdgeError::Update)?;
        tracing::info!(outcome = "updated", "update committed");
    }
    manager.ready();

    let low_battery = Arc::new(Notify::new());
    let mut supervisor = Supervisor::new(RestartPolicy::default());
    if let Some(interval) = manager.watchdog_interval() {
        // On the runtime, so a stalled runtime stops the keep-alives.
        let beat = interval / 2;
        let manager = Arc::clone(&manager);
        supervisor.spawn("watchdog", move || {
            let manager = Arc::clone(&manager);
            async move {
                loop {
                    manager.watchdog();
                    tokio::time::sleep(beat).await;
                }
            }
        });
    }
    if let Some(ups) = &config.ups {
        let (ups, poll, low) = (ups.clone(), config.ups_poll, Arc::clone(&low_battery));
        supervisor.spawn("ups", move || ups.clone().watch(poll, Arc::clone(&low)));
    }

    let outcome = tokio::select! {
        () = stop => Ok(Stop::Requested),
        () = low_battery.notified() => Ok(Stop::LowBattery),
        name = supervisor.failed() => Err(EdgeError::Task(name)),
    };

    // Stop: report not-ready, then let in-flight requests finish within the drain time.
    serving.0.store(false, Ordering::SeqCst);
    manager.stopping();
    drain.notify_one();
    let served = extending(manager.as_ref(), tokio::time::timeout(config.drain, server)).await;
    drop(supervisor);
    match served {
        Ok(Ok(Ok(()))) => tracing::info!(outcome = "stopped", "facility edge stopped"),
        Ok(Ok(Err(error))) => return Err(EdgeError::Serve(error)),
        Ok(Err(_)) => return Err(EdgeError::Serve(std::io::ErrorKind::Other.into())),
        Err(_) => tracing::warn!(
            outcome = "drain-timeout",
            "requests still in flight at stop"
        ),
    }
    outcome
}

/// A stop request: Ctrl-C, or SIGTERM on Unix (systemd's stop). The handlers are installed
/// when this is called, not when the future is first polled, so a stop that arrives while the
/// node starts is never lost to the signal's default action.
pub fn shutdown_signal() -> impl Future<Output = ()> + Send + 'static {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let interrupt = signal(SignalKind::interrupt()).ok();
        let terminate = signal(SignalKind::terminate()).ok();
        async move {
            let wait = |s: Option<tokio::signal::unix::Signal>| async move {
                match s {
                    Some(mut s) => {
                        s.recv().await;
                    }
                    None => std::future::pending::<()>().await,
                }
            };
            tokio::select! {
                () = wait(interrupt) => {}
                () = wait(terminate) => {}
            }
        }
    }
    #[cfg(windows)]
    {
        let mut interrupt = tokio::signal::windows::ctrl_c().ok();
        async move {
            match interrupt.as_mut() {
                Some(interrupt) => {
                    interrupt.recv().await;
                }
                None => std::future::pending::<()>().await,
            }
        }
    }
}
