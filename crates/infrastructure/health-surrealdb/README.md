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
- **Devices** (migration `0020_devices`; #74): `auth_device`, the edge's device registry, unique by key thumbprint, changed only by compare-and-set on `version` and never deleted; and `auth_enrollment`, pending requests, unique by key and by pairing code, with expired ones dropped before each new request and by `purge_auth`. Used by `health_auth::SurrealAuthStore`.
- **PINs** (migration `0021_pins`; #75): `auth_pin`, keyed by account and device key thumbprint, holds a sealed PIN hash and its wrong tries. `auth_session` gains `account` and `jkt` columns, indexed, so `device_auth_sessions` finds a user's live session on a device.
- **Provisional accounts** (migration `0023_provisional`; #75): `auth_local_provisional`, one record per account created offline.
- **Nodes** (migration `0024_nodes`; #268): `node_provisioning_code` and `node`, used by `health-provisioning`.
  - A code is stored by the sha-256 of its text, never the text itself.
  - Unique indexes allow one node per code and one short code per facility.
  - A node row is never deleted.
- **Revocations** (migration `0026_node_revocations`; #268): one record, `node_revocations:issuing`, holding the issuing CA's revoked facility intermediates and its last CRL number.
- **Publication lifecycle** (migration `0027_package_lifecycle`; #88; owned by configuration): `package_lifecycle`, one row per `name@version`, changed only by compare-and-set on `revision`; `package_lifecycle_record`, its transitions, appended with UUIDv7 ids; and `published_package`, created once per published version. Tarballs are blobs. Used by `health-publication`.
- **Workflow** (migration `0028_workflow`; #97; owned by workflow): `health_workflow::WorkflowStore`. `workflow_instance`, changed only by compare-and-set on `version`; `workflow_history`, keyed `instance|seq`, created once with READONLY fields; `workflow_timer`, by `instance|timer`; and `workflow_inbox`, the domain events consumed, by event id. Migration `0029_workflow_tasks` adds `workflow_task`, the worklists' read model: each instance's projected Tasks, replaced in the transaction that changes the instance, listed by worklist, `rank` (priority) and arrival.
- **Pricing** (migration `0030_pricing`; #120; owned by pricing): `health_pricing::PricingStore`. `price_book` and `price_rule`, one row per version keyed `id|version`. Every version is kept.
- **Charges** (migration `0031_charges`; #121; owned by billing): `health_billing::ChargeStore`. `charge`, one row per ChargeItem keyed by its deterministic id. `add_charge` keeps a charge only once. Migration `0032_billing_accounts` adds `billing_deferral`, `billing_settlement` and `billing_waiver` for `health_billing::AccountStore`, each appended once.
- **Encounters and queues** (migration `0033_encounters_and_queues`; #102): `encounter_state` (owned by encounters, `health_encounters::EncounterStore`) and `queue_entry` (owned by patient-flow, `health_patient_flow::QueueStore`, keyed by derived ids, so `add_entry` keeps an entry once).
- **Recovery keys** (migration `0025_recovery_keys`; #268): `node_recovery_key`, one row per node and volume (`node:volume`). It holds the record with its read-outs, and the key as the KMS wrapped it, never in the clear.
- **Offline administration** (migration `0022_local_admin`; #75): `auth_local_grant` (the grant's JSON, its JWS signed with the node's key, and its account, facility and expiry, indexed), `auth_local_revocation` keyed by account, and `auth_local_config`, one record holding the country's role mappings and grantable roles.
- **Registration review** (migration `0013_registration_review`): the review queue also takes `registration-request` items (#111).
- **Caregiver grants** (migration `0014_caregiver_grant`): `health_consent::GrantStore`, one record per grant, indexed by patient and by caregiver.
- **Transfers and Reconciliation Records** (migration `0015_stock_transfers`): `stock_transfer`, and `stock_reconciliation` indexed by whether it is resolved and when it was opened.
- **Stock counts** (migration `0016_stock_counts`): `stock_count`, and `units_of_store` by the balance records' key prefix.
- **Receiving** (migration `0017_receiving`): `stock_goods_received` and `stock_requisition`.
- **Payments** (migration `0018_payments`): `ledger_posting` (`health_financial_ledger::PostingStore`, each posting created once), and `payment` and `cash_shift` (`health_payments::PaymentStore`).
- **Provenance** (migration `0019_provenance`): `provenance`, whose `owner` the store sets to the Owner of the target's head in the same transaction, refusing a missing or unknown target; `provenance_of_owner` lists a node's Provenance for sync (#196).
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
