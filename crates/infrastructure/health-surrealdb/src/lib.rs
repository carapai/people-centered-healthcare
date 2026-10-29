//! SurrealDB adapter (persistence.md; ADRs 0007, 0017, 0018).
//!
//! - [`connect`] opens the one database a node uses: embedded RocksDB on the edge and desktop
//!   (always `sync=every`), a server at central, or the in-memory engine in tests. Which engines
//!   are compiled in is chosen with the `embedded`, `remote` and `memory` features.
//! - [`migrate`] applies the forward-only [`MIGRATIONS`] before the node reports ready, and
//!   refuses to start if an applied migration was modified.
//!
//! - [`SurrealStore`] implements the `health-persistence` unit of work on SurrealDB interactive
//!   transactions: heads, history, guards, counters and the outbox. Search index entries arrive
//!   with the search task (#57).
//! - [`ownership`] declares which Owner each table and record belongs to.

mod connection;
mod migrations;
pub mod ownership;
mod store;

pub use connection::{Connection, ConnectionError, Settings, Target, connect};
pub use migrations::{MIGRATIONS, Migration, MigrationError, MigrationReport, migrate};
pub use store::{SurrealStore, SurrealTx};
