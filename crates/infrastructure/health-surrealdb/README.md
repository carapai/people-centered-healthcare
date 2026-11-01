# health-surrealdb

Layer: **infrastructure**. SurrealDB `=3.3.5`. Implements persistence.md, ADRs 0007, 0017 and 0018.

- **`Connection::open(&Settings)`** opens one of three targets:
  - `Target::RocksDb { path }`: embedded, always `?sync=every`. Edge and desktop; feature `embedded`.
  - `Target::Remote { endpoint, … }`: a server over WebSocket, with root sign-in. Central; feature `remote`.
  - `Target::Memory`: tests; feature `memory`, or as a dev-dependency.

  It then selects one namespace per tenant and the node's database. `Debug` never prints credentials.
- **`SurrealStore::new(connection, clock)`** implements `health-persistence::Store` on SurrealDB interactive transactions: heads and history (stored JSON bytes), preconditions, tombstones, `guard` records, `counter` sequences and the `outbox`. A commit conflict is `WriteError::Conflict`. Dropping a transaction cancels it.
- **Search** (persistence.md § Search index tables, § In code). `SurrealStore::with_registry(registry)` makes every version write replace its `idx_*` entries, listed in the head's `index_keys`. `SurrealTx::search(query, policy, after)` compiles a parsed `health-fhir-search` query to record-id range scans (or secondary-index scans where a leading key element is open), intersects clauses smallest first and pages by resource id; `SurrealTx::explain` returns the plans. `start_reindex` and `reindex_batch` backfill new parameters resumably.
- **Blocking index** (ADR 0055; migration `0006_blocking_index`).
  - `SurrealStore::with_blocking(keys)` takes a `health_persistence::DerivedKeys` (for Patients, the registry's `RegistryBlocking`). Every live version then writes `idx_block:[kind, value, id]` rows, listed in `index_keys`, so the next version replaces them and a tombstone removes them.
  - `SurrealTx` implements `BlockingIndex`: `members` reads a block with a record-id range, and `counts` reads `block_count`.
  - `SurrealStore::recount_blocks()` recomputes `block_count` in one transaction. It is the hourly background job, so counts are never written in a write transaction.
- **Inbound** (migration `0005_inbound`): `SurrealTx::put_inbound`, `inbound`, `inbound_pending`, `resolve_inbound` and `purge_inbound` hold the integration module's Inbound Dead Letter and quarantine.
- **Registry review** (migration `0007_registry_review`): `SurrealTx` implements `health_client_registry::ReviewQueue`, storing each item's details as JSON. Recording a decision is a compare-and-set on `decision = NONE`.
- **Export and import:** `Connection::export_sql` returns the snapshot-consistent SurrealQL export (embedded engines). `import_sql` runs one in memory into an empty database. `health-backup` encrypts and restores them.
- **Authorization server state** (migration `0008_auth`): `SurrealTx` methods for Login Accounts and lockout, hashed codes (taken with `DELETE … RETURN`), sessions with compare-and-set refresh rotation and retired hashes, the jti cache, snapshots, the clock floor and `purge_auth`. They are used by `health_auth::SurrealAuthStore`.
- **Break-glass grants** (migration `0009_break_glass`): `put_break_glass`, `active_break_glass` (by session, unexpired), and `break_glass_alerts` and `acknowledge_break_glass` (by facility). `$session` is reserved by SurrealDB, so the bind is `$grant_session`. Migration `0010_break_glass_review` adds the privacy officer's review: `break_glass_reviews`, `review_break_glass` (once) and `escalate_break_glass`.
- **Stock** (migration `0011_stock`, `health_inventory::StockStore`): `stock_balance`, one record per Stock Unit keyed by `StockUnit::key`, is the guard every movement writes; `stock_movement` holds the movements, indexed by unit; the adapter only ever creates them.
- **Phone sign-in** (migration `0012_phone_sign_in`): `auth_phone_account`, unique by phone, and `auth_otp`, each phone's current code challenge; `attempt_phone_otp` counts a try by compare-and-set. Used by `health_auth::SurrealAuthStore`.
- **Registration review** (migration `0013_registration_review`): the review queue also takes `registration-request` items (#111).
- **Caregiver grants** (migration `0014_caregiver_grant`): `health_consent::GrantStore`, one record per grant, indexed by patient and by caregiver.
- **Transfers and Reconciliation Records** (migration `0015_stock_transfers`): `stock_transfer`, and `stock_reconciliation` indexed by whether it is resolved and when it was opened.
- **`ownership`** is the ownership registry: one default Owner per head table, plus category partitions (resource-ownership.md § In code).
- **`migrate(&Connection, MIGRATIONS)`** creates the `_migration` ledger, refuses modified, unknown or out-of-order migrations before changing anything, and then applies each pending migration in its own transaction with its ledger row.
- **Closing:** SDK 3.3.5 has no awaitable close. An embedded datastore shuts down in the background after its last handle drops, so reopening the same RocksDB directory in the same process must wait for its lock. A node opens its database once per process.
- **Block cache:** set `SURREAL_ROCKSDB_BLOCK_CACHE_SIZE` (bytes) in the service environment. Whether the embedded engine honours it is not yet verified.

## Tests

| Command | Covers |
|---|---|
| `cargo test -p health-surrealdb` | Runner semantics, the schema snapshot (`UPDATE_SCHEMA_SNAPSHOT=1` rewrites it), the folder-versus-list check, the repository contract (`tests/store.rs`), search and its `EXPLAIN` plans (`tests/search.rs`), the blocking index (`tests/blocking.rs`) and the ownership architecture test (`tests/ownership.rs`), all on the memory engine |
| `cargo test -p health-surrealdb --features embedded` | RocksDB open, migrate, write, reopen |
| `HEALTH_TEST_SURREALDB_WS=ws://… cargo test -p health-surrealdb --features remote` | The same migrations against `surreal start --user root --pass root` |

CI (#47) also runs `surreal validate` and `surql-fmt --check` over `migrations/`.
