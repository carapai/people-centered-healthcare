//! Telling the service manager where the node is (facility-edge.md § OS service).

use std::time::Duration;

/// The service manager the node runs under: told when the node is ready, when it stops, and
/// when a long migration or drain needs more time than the start or stop timeout allows.
pub trait ServiceManager: Send + Sync + 'static {
    /// Startup is complete: the store is open and migrated.
    fn ready(&self);
    /// The node has begun to stop.
    fn stopping(&self);
    /// The current start or stop needs at least `by` more.
    fn extend(&self, by: Duration);
    /// The node is alive (a watchdog keep-alive).
    fn watchdog(&self);
    /// How often [`ServiceManager::watchdog`] must be called, if the manager watches at all.
    fn watchdog_interval(&self) -> Option<Duration>;
}

/// No service manager: development, in a terminal.
#[derive(Debug, Default, Clone, Copy)]
pub struct Foreground;

impl ServiceManager for Foreground {
    fn ready(&self) {
        tracing::info!(outcome = "ready", "facility edge ready");
    }
    fn stopping(&self) {}
    fn extend(&self, _by: Duration) {}
    fn watchdog(&self) {}
    fn watchdog_interval(&self) -> Option<Duration> {
        None
    }
}

/// systemd, `Type=notify`: messages go to `$NOTIFY_SOCKET` (sd_notify(3)).
#[derive(Debug, Clone)]
pub struct Systemd {
    socket: String,
    watchdog: Option<Duration>,
}

impl Systemd {
    /// From the service environment, if systemd set `NOTIFY_SOCKET`. The watchdog interval is
    /// `WATCHDOG_USEC`, when `WATCHDOG_PID` is unset or names this process.
    pub fn from_env(lookup: impl Fn(&str) -> Option<String>) -> Option<Self> {
        let socket = lookup("NOTIFY_SOCKET").filter(|s| !s.is_empty())?;
        let ours = lookup("WATCHDOG_PID").is_none_or(|pid| pid == std::process::id().to_string());
        let watchdog = lookup("WATCHDOG_USEC")
            .and_then(|usec| usec.parse::<u64>().ok())
            .filter(|usec| ours && *usec > 0)
            .map(Duration::from_micros);
        Some(Self { socket, watchdog })
    }

    fn send(&self, message: &str) {
        if let Err(error) = send(&self.socket, message) {
            tracing::warn!(error.kind = ?error.kind(), "service manager notification failed");
        }
    }
}

impl ServiceManager for Systemd {
    fn ready(&self) {
        self.send("READY=1\nSTATUS=ready");
        tracing::info!(outcome = "ready", "facility edge ready");
    }
    fn stopping(&self) {
        self.send("STOPPING=1\nSTATUS=stopping");
    }
    fn extend(&self, by: Duration) {
        self.send(&format!("EXTEND_TIMEOUT_USEC={}", by.as_micros()));
    }
    fn watchdog(&self) {
        self.send("WATCHDOG=1");
    }
    fn watchdog_interval(&self) -> Option<Duration> {
        self.watchdog
    }
}

/// The Windows Service Control Manager: each report carries a wait hint, and a long migration or
/// drain advances the checkpoint so the SCM keeps waiting (facility-edge.md § OS service).
#[cfg(windows)]
pub struct WindowsScm {
    status: std::sync::Mutex<ScmStatus>,
}

#[cfg(windows)]
struct ScmStatus {
    handle: windows_service::service_control_handler::ServiceStatusHandle,
    state: windows_service::service::ServiceState,
    checkpoint: u32,
}

#[cfg(windows)]
impl WindowsScm {
    /// Reports through `handle`, starting in `StartPending`.
    pub fn new(handle: windows_service::service_control_handler::ServiceStatusHandle) -> Self {
        let scm = Self {
            status: std::sync::Mutex::new(ScmStatus {
                handle,
                state: windows_service::service::ServiceState::StartPending,
                checkpoint: 0,
            }),
        };
        scm.report(
            windows_service::service::ServiceState::StartPending,
            Duration::from_secs(30),
            0,
        );
        scm
    }

    fn report(&self, state: windows_service::service::ServiceState, wait: Duration, exit: u32) {
        use windows_service::service::{
            ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
        };
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let pending = matches!(
            state,
            ServiceState::StartPending | ServiceState::StopPending
        );
        status.checkpoint = if pending && status.state == state {
            status.checkpoint + 1
        } else {
            0
        };
        status.state = state;
        let report = ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: if state == ServiceState::Running {
                ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN
            } else {
                ServiceControlAccept::empty()
            },
            exit_code: if exit == 0 {
                ServiceExitCode::Win32(0)
            } else {
                ServiceExitCode::ServiceSpecific(exit)
            },
            checkpoint: if pending { status.checkpoint } else { 0 },
            wait_hint: if pending { wait } else { Duration::ZERO },
            process_id: None,
        };
        if status.handle.set_service_status(report).is_err() {
            tracing::warn!(error.kind = "scm-report", "service status report failed");
        }
    }

    /// The node has stopped with `exit` (its `facility-edge` exit code; 0 for a clean stop). A
    /// non-zero code is a service-specific error, which the SCM's failure actions act on.
    pub fn stopped(&self, exit: u32) {
        self.report(
            windows_service::service::ServiceState::Stopped,
            Duration::ZERO,
            exit,
        );
    }
}

#[cfg(windows)]
impl ServiceManager for WindowsScm {
    fn ready(&self) {
        self.report(
            windows_service::service::ServiceState::Running,
            Duration::ZERO,
            0,
        );
        tracing::info!(outcome = "ready", "facility edge ready");
    }
    fn stopping(&self) {
        self.report(
            windows_service::service::ServiceState::StopPending,
            Duration::from_secs(30),
            0,
        );
    }
    fn extend(&self, by: Duration) {
        let state = self
            .status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state;
        if matches!(
            state,
            windows_service::service::ServiceState::StartPending
                | windows_service::service::ServiceState::StopPending
        ) {
            self.report(state, by, 0);
        }
    }
    fn watchdog(&self) {}
    fn watchdog_interval(&self) -> Option<Duration> {
        None
    }
}

#[cfg(unix)]
fn send(socket: &str, message: &str) -> std::io::Result<()> {
    let datagram = std::os::unix::net::UnixDatagram::unbound()?;
    // A leading `@` names a Linux abstract socket.
    #[cfg(target_os = "linux")]
    if let Some(name) = socket.strip_prefix('@') {
        use std::os::linux::net::SocketAddrExt;
        let address = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes())?;
        return datagram
            .send_to_addr(message.as_bytes(), &address)
            .map(drop);
    }
    datagram.send_to(message.as_bytes(), socket).map(drop)
}

#[cfg(not(unix))]
fn send(_socket: &str, _message: &str) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systemd_is_used_only_with_a_notify_socket_and_watches_only_this_process() {
        let env = |vars: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                vars.iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| (*v).to_owned())
            }
        };
        assert!(Systemd::from_env(env(&[])).is_none());
        let s = Systemd::from_env(env(&[
            ("NOTIFY_SOCKET", "/run/n"),
            ("WATCHDOG_USEC", "4000000"),
        ]))
        .expect("systemd");
        assert_eq!(s.watchdog_interval(), Some(Duration::from_secs(4)));
        let other = Systemd::from_env(env(&[
            ("NOTIFY_SOCKET", "/run/n"),
            ("WATCHDOG_USEC", "4000000"),
            ("WATCHDOG_PID", "1"),
        ]))
        .expect("systemd");
        assert_eq!(other.watchdog_interval(), None);
    }

    #[cfg(unix)]
    #[test]
    fn messages_reach_the_socket() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("notify");
        let listener = std::os::unix::net::UnixDatagram::bind(&path).expect("bind");
        let systemd = Systemd {
            socket: path.to_string_lossy().into_owned(),
            watchdog: None,
        };
        systemd.extend(Duration::from_secs(30));
        systemd.ready();
        let mut buf = [0_u8; 256];
        let n = listener.recv(&mut buf).expect("recv");
        assert_eq!(&buf[..n], b"EXTEND_TIMEOUT_USEC=30000000");
        let n = listener.recv(&mut buf).expect("recv");
        assert!(buf[..n].starts_with(b"READY=1"));
    }
}
