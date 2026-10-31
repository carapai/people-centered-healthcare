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
//! - [`search`] runs FHIR searches as record-id range scans and backfills new parameters.
//! - [`ownership`] declares which Owner each table and record belongs to.

mod connection;
mod history;
mod inbound;
mod migrations;
pub mod ownership;
pub mod search;
mod store;

pub use connection::{Connection, ConnectionError, Settings, Target, connect};
pub use history::{HistoryEntry, HistoryScope};
pub use inbound::{InboundEntry, InboundKind, InboundStatus};
pub use migrations::{MIGRATIONS, Migration, MigrationError, MigrationReport, migrate};
pub use search::{DEFAULT_COUNT, ReindexJob, ReindexProgress, SearchFailure, SearchPage};
pub use store::{SurrealStore, SurrealTx};
