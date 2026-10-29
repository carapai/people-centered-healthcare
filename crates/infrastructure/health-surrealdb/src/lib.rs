//! SurrealDB adapter (persistence.md; ADRs 0007, 0017, 0018).
//!
//! - [`connect`] opens the one database a node uses: embedded RocksDB on the edge and desktop
//!   (always `sync=every`), a server at central, or the in-memory engine in tests. Which engines
//!   are compiled in is chosen with the `embedded`, `remote` and `memory` features.
//! - [`migrate`] applies the forward-only [`MIGRATIONS`] before the node reports ready, and
//!   refuses to start if an applied migration was modified.
//!
//! The `health-persistence` unit of work over heads, history and index entries lands with
//! [Implement head/history storage and the write transaction](https://github.com/carapai/people-centered-healthcare/issues/56).

mod connection;
mod migrations;

pub use connection::{Connection, ConnectionError, Settings, Target, connect};
pub use migrations::{MIGRATIONS, Migration, MigrationError, MigrationReport, migrate};
