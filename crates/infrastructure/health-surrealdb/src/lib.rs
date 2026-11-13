//! SurrealDB adapter (persistence.md; ADRs 0007, 0017, 0018).
//!
//! - [`connect`] opens the one database a node uses: embedded RocksDB on the edge and desktop
//!   (always `sync=every`), a server at central, or the in-memory engine in tests. Which engines
//!   are compiled in is chosen with the `embedded`, `remote` and `memory` features.
//! - [`migrate`] applies the forward-only [`MIGRATIONS`] before the node reports ready, and
//!   refuses to start if an applied migration was modified.
//!
//! - [`SurrealStore`] implements the `health-persistence` unit of work on SurrealDB interactive
//!   transactions: heads, history, guards, counters, the outbox and search index entries.
//! - [`SurrealTx::version`] and [`SurrealTx::history`] read stored versions for `vread` and
//!   `_history`.
//! - [`SurrealStore::cursor`], [`SurrealStore::changes_after`] and [`SurrealStore::pending_for`]
//!   read the sync change log every write appends to (ADR 0019); [`new_epoch`] follows a restore.
//! - [`search`] runs FHIR searches as record-id range scans and backfills new parameters.
//! - [`ownership`] declares which Owner each table and record belongs to.

mod auth;
mod blocking;
mod change_log;
mod charges;
mod connection;
mod consent;
mod coverage;
mod devices;
mod encounters;
mod history;
mod inbound;
mod laboratory;
mod leases;
mod local;
mod migrations;
mod nodes;
pub mod ownership;
mod packages;
mod payments;
mod phone;
mod pricing;
mod review;
pub mod search;
mod stock;
mod store;
mod sync;
mod workflow;

pub use auth::{AuthSessionRow, BreakGlassReviewRow, BreakGlassRow};
/// The thread stack every runtime that runs SurrealDB needs: SurrealDB 4 parses and executes
/// SurrealQL by plain recursion (it dropped `reblessive`, RUSTSEC-2026-0454), so the default
/// 2 MiB stack overflows on deep statements. SurrealDB recommends 10 MiB (ADR 0060).
pub const THREAD_STACK: usize = 10 * 1024 * 1024;

pub use change_log::{Change, Cursor, new_epoch};
pub use connection::{
    Connection, ConnectionError, MAX_CONCURRENT_TRANSACTIONS, Settings, Target, connect,
};
pub use devices::DeviceRow;
pub use history::{HistoryEntry, HistoryScope};
pub use inbound::{InboundEntry, InboundKind, InboundStatus};
pub use leases::LeaseRole;
pub use migrations::{MIGRATIONS, Migration, MigrationError, MigrationReport, migrate};
pub use payments::PaymentException;
pub use search::{DEFAULT_COUNT, ReindexJob, ReindexProgress, SearchFailure, SearchPage};
pub use store::{SharedRegistry, SurrealStore, SurrealTx};
pub use sync::{
    ConflictRecord, DOWN_GLOBAL, LEDGERS, Outgoing, Resolution, SNAPSHOT_PAGE, STOCK_LEDGER,
    SnapshotPart, SurrealAuthority, SurrealReplica, SyncScope, TransferMatch, TransferRefused,
    UP_GLOBAL, WORKFLOW_CANDIDATES, WORKFLOW_INSTANCES, class_of,
};
