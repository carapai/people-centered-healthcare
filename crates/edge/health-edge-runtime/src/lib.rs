//! The Facility Edge Node's runtime (facility-edge.md, ADR 0007): one process embedding
//! SurrealDB, run by a service manager.
//!
//! [`run`] is the whole life of the node:
//! 1. The HTTP listener, at once: `/health` answers, and `/ready` reports `not-ready` until the
//!    store is open and migrated. The embedded web app ([`web`]) is served from the start, so
//!    browsers load the shell while the store opens.
//! 2. The embedded store (RocksDB, `sync=every`), opened once; the process owns its directory.
//! 3. Migrations, before ready, extending the service manager's start timeout while they run.
//!    An old binary refuses a store migrated by a newer one.
//! 4. Ready: the service manager is told, the watchdog starts, and supervised tasks run.
//!    A pending update ([`update`]) copies the store before its first open, and a failed
//!    migration rolls it back.
//! 5. A stop (a signal, a UPS low-battery signal, or a task that keeps failing): `/ready`
//!    reports `not-ready`, in-flight requests drain, and the store closes.

mod config;
mod node;
mod notify;
mod supervisor;
pub mod update;
mod ups;
pub mod web;

pub use config::{ConfigError, EdgeConfig};
pub use node::{EdgeError, Stop, run, shutdown_signal};
#[cfg(windows)]
pub use notify::WindowsScm;
pub use notify::{Foreground, ServiceManager, Systemd};
pub use supervisor::{RestartPolicy, Supervisor};
pub use ups::{Ups, UpsError};
