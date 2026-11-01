//! `facility-edge --service`: the node under the platform's service manager.

#[cfg(unix)]
use std::process::ExitCode;
#[cfg(unix)]
use std::sync::Arc;

#[cfg(unix)]
use health_edge_runtime::EdgeConfig;

/// systemd, `Type=notify`: notifications go to `$NOTIFY_SOCKET`.
#[cfg(unix)]
pub(crate) fn run(config: EdgeConfig) -> ExitCode {
    match health_edge_runtime::Systemd::from_env(|name| std::env::var(name).ok()) {
        Some(systemd) => ExitCode::from(crate::block_on(&config, Arc::new(systemd), None)),
        None => {
            eprintln!("facility-edge: --service needs a service manager: NOTIFY_SOCKET is unset");
            ExitCode::from(2)
        }
    }
}

#[cfg(windows)]
pub(crate) use windows::run;

/// The Windows Service Control Manager. The dispatcher calls `service_main` on its own thread;
/// the configuration and the exit code pass through statics because the entry point is a plain
/// function.
#[cfg(windows)]
mod windows {
    use std::ffi::OsString;
    use std::process::ExitCode;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::{Arc, OnceLock};

    use health_edge_runtime::{EdgeConfig, WindowsScm};
    use tokio::sync::Notify;
    use windows_service::service::ServiceControl;
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

    /// The service's name, as `install-service.ps1` registers it.
    const NAME: &str = "facility-edge";

    static CONFIG: OnceLock<EdgeConfig> = OnceLock::new();
    static EXIT: AtomicU8 = AtomicU8::new(0);

    windows_service::define_windows_service!(ffi_service_main, service_main);

    pub(crate) fn run(config: EdgeConfig) -> ExitCode {
        let _ = CONFIG.set(config);
        match windows_service::service_dispatcher::start(NAME, ffi_service_main) {
            Ok(()) => ExitCode::from(EXIT.load(Ordering::SeqCst)),
            Err(_) => {
                eprintln!(
                    "facility-edge: --service must be started by the Service Control Manager"
                );
                ExitCode::from(2)
            }
        }
    }

    fn service_main(_arguments: Vec<OsString>) {
        let Some(config) = CONFIG.get() else { return };
        let stop = Arc::new(Notify::new());
        let requested = Arc::clone(&stop);
        let handle = match service_control_handler::register(NAME, move |control| match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                requested.notify_one();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }) {
            Ok(handle) => handle,
            Err(_) => {
                EXIT.store(1, Ordering::SeqCst);
                return;
            }
        };
        let scm = Arc::new(WindowsScm::new(handle));
        let manager: Arc<dyn health_edge_runtime::ServiceManager> = scm.clone();
        let code = crate::block_on(
            config,
            manager,
            Some(Box::pin(async move { stop.notified().await })),
        );
        EXIT.store(code, Ordering::SeqCst);
        scm.stopped(u32::from(code));
    }
}
