//! The Facility Edge Node's runtime (facility-edge.md, ADR 0007): one process embedding
//! SurrealDB, run by a service manager.
//!
//! [`run`] is the whole life of the node:
//! 1. The HTTP listener, at once: `/health` answers, and `/ready` reports `not-ready` until the
//!    store is open and migrated. The embedded web app ([`web`]) is served from the start, so
//!    browsers load the shell while the store opens.
//! 2. The embedded store (SurrealKV, `sync=every`; ADR 0060), opened once; the process owns its
//!    directory.
//! 3. Migrations, before ready, extending the service manager's start timeout while they run.
//!    An old binary refuses a store migrated by a newer one.
//! 4. Ready: the service manager is told, the watchdog starts, and supervised tasks run.
//!    A pending update ([`update`]) copies the store before its first open, and a failed
//!    migration rolls it back.
//! 5. A stop (a signal, a UPS low-battery signal, or a task that keeps failing): `/ready`
//!    reports `not-ready`, in-flight requests drain, and the store closes.
//!
//! Once ready, the node also takes its scheduled backups ([`backup`]) and logs its health
//! indicators, which `/api/admin/status` serves to the admin page and the telemetry buffer ships to
//! central ([`admin`]). Its API is open only to enrolled LAN devices (`enrollment`, #74), over
//! TLS with a leaf it issues itself under its facility intermediate (`tls`, #74); before
//! provisioning it serves plain HTTP on the loopback address only.

pub mod activation;
pub mod admin;
mod auth;
pub mod backup;
mod clinical;
mod config;
mod discovery;
mod enrollment;
mod node;
mod notify;
pub mod seed;
mod supervisor;
pub mod sync;
mod tls;
pub mod update;
mod ups;
pub mod web;

pub use auth::{CLIENT_ID, INTERMEDIATE_KEY, PEPPER_KEY, REDIRECT_PATH, Secrets, TOKEN_KEY};
pub use config::{ConfigError, EdgeConfig};
pub use node::{EdgeError, Stop, run, shutdown_signal};
#[cfg(windows)]
pub use notify::WindowsScm;
pub use notify::{Foreground, ServiceManager, Systemd};
pub use supervisor::{RestartPolicy, Restarts, Supervisor};
pub use tls::Certificate;
pub use ups::{Power, PowerState, Ups, UpsError};
