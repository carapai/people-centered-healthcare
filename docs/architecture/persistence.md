# Persistence: FHIR resources, versions and history in SurrealDB

Status: **accepted** (ADRs 0017, 0018). Produced while resolving the Wayfinder ticket [How are FHIR resources, versions and history persisted in SurrealDB?](https://github.com/carapai/people-centered-healthcare/issues/16). Background: [docs/research/surrealdb.md](../research/surrealdb.md). Related decisions:
- the model, validation and search semantics are in [fhir-r5-model.md](fhir-r5-model.md);
- ids are in [identifiers.md](identifiers.md);
- authority is in [write-authority.md](write-authority.md);
- the change log, outbox, `seq` counter and wire format are in [synchronization.md](synchronization.md) (ADR 0019).

A validated schema sketch is in [persistence-sketch.surql](persistence-sketch.surql).

## Engine, version and process

| Runtime | Process | Engine | Durability |
|---|---|---|---|
| Facility Edge Node | Embedded in `facility-edge` (ADR 0007) | RocksDB (`kv-rocksdb`) | `sync=every` |
| Standalone Tauri desktop | Embedded in-process, one shared handle | RocksDB | `sync=every` |
| Central | Separate `surreal start rocksdb://…` server, single node, on localhost or a private network | RocksDB | `sync=every` |

- **Version pinning:** every runtime uses one exact SurrealDB **3.3.x** version (SDK and server), upgraded together and only deliberately, with the FHIR query regression suite as the gate.
- **SurrealKV** is revisited only once upstream recommends it and it has some release history.
- **Central high availability** (Enterprise or TiKV) and the BSL "Database Service" clause are in the map's Not yet specified.

## Namespace, database and ownership

- **Layout:** one **namespace per tenant** (a country deployment, e.g. `ug`) and one database `main` per node. Every Module's tables live in that one database, so ADR 0004 transactions can span Owners.
- **Ownership is enforced in code:**
  - each table belongs to exactly one Owner's repository (table → Owner follows [resource-ownership.md](resource-ownership.md));
  - the architecture check rejects a repository that touches another Owner's table;
  - shared infrastructure tables (`idx_*`, `change_log`, `outbox`, `conflict`, `_migration`) are written only through `health-surrealdb`'s unit of work.

## Tables per resource type

**Head table `<type>`** (e.g. `observation`): `SCHEMAFULL`, one row per resource, record id = the resource's UUID.

| Field | Content |
|---|---|
| `json` | The stored bytes of the resource of record: the validated input JSON stamped with `id` and `meta`, as a string (ADR 0051). Storing a SurrealDB object would lose decimal text and key order |
| `version_id` | UUID; equal to `meta.versionId`, which is the `op_id` of the write (ADR 0018) |
| `version_seq` | Integer assigned by the Authority Node, monotonic per resource |
| `last_updated` | `meta.lastUpdated` |
| `deleted` | Tombstone flag |
| `facility`, `authority_node` | Scope and authority (ADR 0012) |
| `provisional` | For provisional records, e.g. Patients |
| `security_labels`, `profiles` | Copied from `meta`, used by the policy decision point and validation |
| `index_keys` | The record ids of this resource's current search index entries |
| hot projections | Typed, indexed fields for the most frequent query paths, declared per type and filled by the search extractor. For example, Observation has `patient`, `encounter`, `code`, `effective_at`, `status`, and Encounter has `patient`, `status`, `period`, `location` |

**History table `<type>_history`**: record id `[resource_uuid, version_seq]`, so an instance's history is one range scan.

| Field | Content |
|---|---|
| `json` | The full resource for that version as stored bytes (never a diff) |
| `version_id`, `parent_version_id` | For sync ancestry |
| `op_id`, `origin_node`, `hlc`, author | Provenance of the write |
| `last_updated`, `deleted` | |

- History for a type, and system-wide `_history`, use an index on `last_updated` (plus `version_seq` for ties).
- Unaccepted Candidate Versions are stored in the `conflict` table, **never** in history.
- History is kept at least as long as the sync tombstone horizon (the maximum offline window is in Not yet specified). Pruning beyond that is a retention policy, not a default.

**References in projections** are `record<…>` links held as single values or arrays, never `set<record>` (SurrealDB #7621).

## Search index tables

Each supported SearchParameter writes entries into one table per search type. Each table's record id is an **array ordered for range scans**:

| Table | Record id |
|---|---|
| `idx_token` | `[type, param, system, code, resource_uuid]` |
| `idx_string` | `[type, param, folded_value, resource_uuid]` |
| `idx_date` | `[type, param, start, end, resource_uuid]`, plus a secondary index on `end` for overlap queries |
| `idx_reference` | `[type, param, target_type, target_id, resource_uuid]` |
| `idx_quantity` | `[type, param, system, canonical_unit, value, resource_uuid]` |
| `idx_number` | `[type, param, value, resource_uuid]` |
| `idx_uri` | `[type, param, value, resource_uuid]` |
| `idx_composite` | `[type, param, component tuple…, resource_uuid]` |

- **How a search runs.** A search compiles to **record range scans** over these ids. Several parameters are intersected, preferring the most selective. Hot projections serve the common paths directly.
- **Testing.** The persistence test suite includes `EXPLAIN` checks for each main query shape, so a full scan fails a test instead of passing silently.
- **Writes.** On every write, the transaction deletes the entries listed in the head's `index_keys` and writes the new ones. No scan is needed.
- **Reindex.** Activating a configuration package that adds or changes a SearchParameter starts a **resumable background reindex job**. The parameter becomes searchable only when the job completes.

### In code

`health-fhir-search` (contracts) extracts and parses; `health-surrealdb::search` stores and compiles. Migration `0002_search_index` defines the tables.

- **Keys.** Each table's record id is the array above, ending with the resource id. Dates are SurrealDB datetimes and numbers and quantity values are decimals, so the ids sort by value. `rt` and `param` fields repeat the first two elements so secondary indexes can lead with them: `idx_token(rt, param, code)` for a code without a system, `idx_date(rt, param, end)`, `idx_reference(rt, param, target_id)` for an id without a type, and `idx_quantity(rt, param, value)` and `(rt, param, code, value)` for a quantity whose unit system or unit is left open. `idx_string` carries the exact text for `:exact`.
- **`:missing` and `:not`.** A supported parameter with no value writes a marker in `idx_missing` (`[type, param, resource_uuid]`), so `:missing=true` is a range scan too, and `:not` is (entries ∪ markers) minus the matches.
- **Composites.** A composite's leading components must give their whole key (a `system|code` token, a typed reference or a uri); its last component may be any value or range. Other composite searches are refused.
- **Results.** Each clause's scans return resource ids, a clause's values are unioned, and clauses are intersected smallest first. Pages are keyset by resource id. `total` is exact for every indexed search; the unfiltered listing of a type is a record-id range over the head table and does not count.
- **Policy.** Clauses from the policy decision point (`SearchPolicy`) are ANDed into every search, including the unfiltered listing, so `total` never counts what the caller cannot see.
- **Reindex.** New parameters enter the registry as `Reindexing`: every write from then on indexes them, and searches using them are refused. The job walks the head table in id order, one batch per transaction, and records the last id in `reindex_job`; a restart resumes there. When it reports done, the parameter is activated.

## The write transaction

Every accepted write is **one** `BEGIN … COMMIT`, issued through the `health-persistence` unit of work.
- **Contract.** `health-persistence` defines the `Store` and `Transaction` traits: read, write with a `Precondition`, delete as a tombstone, claim and release guards, `next_sequence`, append to the outbox, and commit. It also defines `stamp_meta`, `RawJson` and `RetryPolicy`.
- **Test fake.** `health-persistence-memory` (testing layer) implements the contract with snapshot isolation and first-committer-wins, for domain and application tests.

The write sequence:

```mermaid
sequenceDiagram
  participant UC as Use case
  participant UoW as Unit of work (health-surrealdb)
  participant DB as SurrealDB
  UC->>UoW: save(resource, If-Match)
  UoW->>DB: BEGIN
  UoW->>DB: read head (version_id, version_seq, index_keys)
  alt head.version_id ≠ If-Match
    UoW-->>UC: 412 Precondition Failed
  else matches (or create)
    UoW->>DB: guard records / UNIQUE checks
    UoW->>DB: upsert head
    UoW->>DB: create history row
    UoW->>DB: delete old index entries; create new ones
    UoW->>DB: append change_log + outbox rows
    UoW->>DB: COMMIT
  end
  Note over UoW,DB: write-write conflict → retry ≤3 with jitter; on retry the If-Match check yields 412 if the other writer won
```

- **Implementation (#56).** `health-surrealdb::SurrealStore` runs each unit of work as one SurrealDB interactive transaction.
  - **Preconditions** are checked against the head read inside the transaction.
  - **Conflicts.** A concurrent commit to the same head, guard or counter fails the later commit, which surfaces as `WriteError::Conflict`. The rerun then sees the winner and gets 412.
  - **Guard records** (`guard`) and **counters** (`counter`, advanced by `SequenceState::next`) are records in that same transaction, never read-then-check.
  - **Measured on 3.3.5.** 8 concurrent writers on one counter minted 200 values with no duplicates or gaps, but retried about 1,200 conflicts, roughly 6 per value. That exceeds `RetryPolicy`'s default of 4 attempts. One hot counter per facility is fine at clinic rates; a hotter one would need sharding or a longer retry budget.
  - **Tables.** Migration `0001_resource_tables` creates head and history tables for the 84 resource types in the ownership registry (`health-surrealdb::ownership`), plus `guard`, `counter` and `outbox`. Not yet included: `facility`, `authority_node`, `security_labels`, `index_keys` and hot projections, which arrive with search (#57) and authorization.
- **`If-Match` is required** on updates from our clients. Creates use `PUT [type]/[client-uuid]` (update-as-create, `updateCreate=true`). `POST` create works for external clients, and the server then mints a UUIDv7.
- **Concurrent writers.** Two writers of the same resource both write its head record, so SurrealDB snapshot isolation reports a retryable conflict.
- **Multi-record invariants** are never enforced by read-then-check, because snapshot isolation allows write skew. They use **guard records** or **UNIQUE indexes** written in the same transaction. Examples: one active MRN per facility, one open Encounter per patient per service point, unique business identifiers within their system.

## REST semantics implemented here

- `versioned-update` capability.
- Weak ETag `W/"<versionId>"` and `Last-Modified` from `meta.lastUpdated`.
- `If-Match` mismatch → 412.
- Deleted → 410 (read and vread); unknown → 404.
- Conditional create, update and delete follow the R5 matrices in [docs/research/fhir-r5.md](../research/fhir-r5.md). They are resolved through the search index inside the same transaction.
- `_history` at instance, type and system level: Bundle type `history`, oldest last, deletes included, `_since`, `_at` and `_count`.
- `transaction` Bundles are one SurrealDB transaction, all or nothing. `batch` entries are independent transactions with per-entry results.

### In code

`health-fhir-server` serves these interactions over `health-surrealdb`:
- read, vread, create, update and delete;
- instance, type and system `_history`;
- `search-type` over GET and POST.

Its CapabilityStatement is generated from what is implemented, and a test exercises every interaction it declares. These rules are fixed:
- **Updates.** Updating an existing resource without `If-Match` is 428. `PUT` to an unknown UUID creates it (`updateCreate`).
- **Create.** `POST` ignores the client's `id` and mints a UUIDv7.
- **Delete.** `DELETE` writes a tombstone and returns 204. Deleting again is 204, and deleting an unknown resource is 404. A deleted resource is not recreated by `PUT` (410).
- **Conflicts.** A write that loses a write-write conflict reruns the whole use case, as `RetryPolicy` allows.
- **History.** Bundles are newest first and take `_since` and `_count`; `_at` is refused for now. A version's `request.method` is inferred: DELETE for a tombstone, POST for the first version, PUT after that.
- **Search.** `next` links use `_cursor` (keyset by resource id).
- **Bodies.** Reads, search and history embed the stored bytes as they are stored.
- **Not done here:** conditional interactions (#157), transaction and batch Bundles (#158) and reference integrity (#159). Until then they are not declared and their requests are refused.

## Reference integrity

Checked in application code on write, not with SurrealDB `REFERENCE` constraints, because some dangling references are legitimate.

| Reference to | Rule |
|---|---|
| A resource in the same transaction or Bundle | Must resolve |
| In-scope facility records (Patient, Encounter, …) | Must resolve |
| Global replicas (Practitioner, Location, products, Published Definitions) | Must resolve against the local replica |
| Records outside this node's scope, e.g. another facility's Encounter in a referral | Allowed; flagged `unresolved` and checked at central |

- **Deletion.** A resource that live records still reference cannot be deleted (409). Clinical corrections use `entered-in-error`, not delete.
- **Physical purge** happens only when a node's scope shrinks or a retention period expires. It runs as an audited background job.

## Migrations

We write our own migrations runner as a module of `health-surrealdb`. SurrealKit needs a custom build for embedded engines, and its third-party alternatives are unverified on 3.3.

- **Schema migrations** are versioned, forward-only `.surql` files embedded in the binary. CI validates them with `surreal validate` and formats them with `surql-fmt`.
- **Data migrations** (resource rewrites) are written in Rust.
- **Startup.** Pending migrations are applied before the node reports ready (ADR 0007) and recorded in `_migration` with checksums. If an already-applied migration has been modified, startup fails.
  - **Implementation.** `health-surrealdb::migrate` creates the `_migration` ledger itself (`SCHEMAFULL`: `name`, `checksum` as SHA-256 hex, `applied_at`).
  - **Refusals before any change:** a modified applied migration, an applied migration the binary does not know (the binary is older than the data), or an out-of-order list.
  - **Per-migration transaction.** Each pending migration runs in one transaction with its ledger row, so a failing migration applies nothing.
  - **Layout.** Files live in `crates/infrastructure/health-surrealdb/migrations/NNNN_name.surql` and are listed in `MIGRATIONS`. A test checks the folder and the list match.
  - **Snapshot.** The schema after migrating a fresh database is compared with `tests/fixtures/schema.snapshot`.
- **Engines.** `Target::RocksDb` always opens `rocksdb://<path>?sync=every`. `Target::Remote` is central's server over WebSocket, with root sign-in. `Target::Memory` is for tests. Features `embedded`, `remote` and `memory` choose what is compiled in.
- **Block cache.** The RocksDB block cache is capped through the service environment (`SURREAL_ROCKSDB_BLOCK_CACHE_SIZE`); SDK 3.3.5 reads RocksDB tuning from its config map, not the connection string. Whether the embedded engine honours the variable is unverified, and is to be measured with the edge hardware spikes.
- **Reindexing** is a separate resumable job, never a migration.
- **Testing.** Every migration also runs on the export → import round-trip fixture ([backup-and-recovery.md](backup-and-recovery.md)).
