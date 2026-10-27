# Research: What do SurrealDB server and embedded engines guarantee today?

Ticket: carapai/people-centered-healthcare#2 · Researched 2026-10-27 · Primary sources only (SurrealDB docs, GitHub releases/source at tag `v3.3.4`, crates.io, SurrealDB issue tracker). `SRC` below = `https://github.com/surrealdb/surrealdb/blob/v3.3.4/`.

## Summary

- **Current stable:** SurrealDB **3.3.4** (server binary and `surrealdb` Rust crate share the version; crate published 2026-10-13, GitHub release re-tagged 2026-10-19). Maintained in parallel: 3.2.6, 3.1.6, 2.7.0. No 3.4 release exists yet.
- **Both persistent engines (RocksDB, SurrealKV) are usable embedded and in the server**, with the same SurrealQL, the same **snapshot isolation (optimistic, commit-time conflict → retryable error)**, and default **`sync=every` (fsync/fdatasync before commit is acknowledged)**. Both take an exclusive OS **`LOCK` file**: one process per data directory, no multi-process sharing.
- **RocksDB is the mature choice today.** SurrealKV is officially **"beta in SurrealDB 3.3 and earlier"**; it had a crash-recovery bug (0-byte SSTable after unclean shutdown) fixed only from 3.3.0, and open memory-ballooning reports. SurrealDB 3.4 is announced to make SurrealKV (now 1.x) the recommended engine, but 3.3.4 still ships SurrealKV 0.21.4.
- **Change feeds exist but are not a safe sync log on their own**: versionstamps on single-node engines derive from a process-local HLC seeded from wall clock, which resets on restart, so a backward clock step after reboot can make new changes sort *below* already-consumed cursors (open issue #7606). Live queries are per-connection, best-effort ordering, not durable, single-node only in Community edition, and have a known lost-notification race (#6983).
- **Backup = logical SurrealQL export only** (single read-transaction, i.e. snapshot-consistent per namespace/database); no engine-level online backup/checkpoint exposed; an open bug makes record-bearing `set` values round-trip lossy (#7621).
- **Licence:** BSL 1.1 (Change Date 2030-01-01 → Apache-2.0). Self-hosting and embedding in our own product is allowed; offering SurrealDB "as a Database Service" to third parties is not.
- **No hard blocker** for the Facility Edge store (server or embedded-in-edge-process, RocksDB) or the standalone Tauri store (embedded, RocksDB), but **sync correctness must not depend on changefeed versionstamps or live queries**; we need our own durable outbox/sequence, and a validation spike for power-loss on Windows.

## 1. Releases and Rust SDK

| Fact | Source |
|---|---|
| Latest GitHub releases: v3.3.4, v3.3.3…v3.3.0 (3.3.0 on 2026-09-28), v3.2.6, v3.1.6, v2.7.0; 3.3.0 betas were pre-releases | `gh api repos/surrealdb/surrealdb/releases` → https://github.com/surrealdb/surrealdb/releases |
| crates.io `surrealdb` max stable = 3.3.4 (2026-10-13); 3.2.6 (2026-10-19); licence field "non-standard" (BSL) | https://crates.io/crates/surrealdb |
| Release page lists 3.3 as latest stable (3.3.4), plus 3.2/3.1/3.0/2.7 lines; no LTS/support policy stated; no 3.4 | https://surrealdb.com/releases |
| 3.3.4 fixes "a set of queries that returned the wrong rows or none at all" (indexed scan filters); 3.2.5 fixes REBUILD INDEX that never finished and vector-search issues | https://surrealdb.com/releases |
| Release assets for 3.3.4: linux amd64/arm64, darwin amd64/arm64/universal, **windows-amd64.exe** (no Windows arm64) | https://github.com/surrealdb/surrealdb/releases/tag/v3.3.4 |
| Workspace is Rust edition 2024, toolchain pinned to 1.95 | `SRC/Cargo.toml`, `SRC/rust-toolchain.toml` |
| SDK feature flags: `kv-mem`, `kv-rocksdb`, `kv-surrealkv`, `kv-tikv`, `kv-indxdb`; default = `protocol-ws`,`rustls`,`parse`; `protocol-http`, `protocol-grpc` optional | `SRC/surrealdb/Cargo.toml` |
| Embedded RocksDB needs a C++ compiler and libclang at build time; SurrealKV is pure Rust | https://surrealdb.com/docs/sdk/rust/embedding |
| RocksDB binding: `surrealdb-rocksdb 0.24.0-surreal.5` / `surrealdb-librocksdb-sys 0.18.3+11.0.0-4` (SurrealDB fork, RocksDB 11.0); SurrealKV `0.21.4` | `SRC/Cargo.lock` |
| Local CLI installed here: `surreal 3.2.4+20260803` (not latest) | `surreal version` |

## 2. Durability and fsync

| Fact | Source |
|---|---|
| Shared `SyncMode`: `never` (OS flushes), `every` (default, "sync on every commit"), or interval > 100 ms; set as `?sync=` on the path or `SURREAL_DATASTORE_SYNC(_DATA)` config key | `SRC/surrealdb/kvs/src/config.rs`; `SRC/surrealdb/kvs-rocksdb/src/cnf.rs` (`datastore_sync_data`, default `SyncMode::Every`); `SRC/surrealdb/kvs-surrealkv/src/cnf.rs` (`datastore_sync` / `datastore_sync_data`, default `Every`) |
| Docs: SurrealKV and RocksDB `sync` default is `every` = "sync before completing and confirming each transaction"; memory backend default `never` | https://surrealdb.com/docs/surrealdb/cli/start |
| RocksDB: `set_use_fsync(false)` (uses **fdatasync**), WAL on, `manual_wal_flush=false`; with `every`, a CommitCoordinator groups fsyncs across concurrent commits (group commit); with interval, a BackgroundFlusher | `SRC/surrealdb/kvs-rocksdb/src/lib.rs` (`Datastore::new`), `commit_coordinator.rs`, `background_flusher.rs` |
| SurrealKV: in `every` mode txns use `Durability::Eventual` and the SurrealDB commit coordinator performs the grouped fsync (grouped-commit timeout default 5 ms, batch cap 4096) | `SRC/surrealdb/kvs-surrealkv/src/lib.rs` l.186; `cnf.rs` |
| SurrealKV standalone crate default is Eventual; SurrealDB overrides — don't assume crate defaults | https://github.com/surrealdb/surrealkv (README "Durability Levels") |
| Windows-specific fsync semantics (FlushFileBuffers etc.) for either engine | **unverified** — no SurrealDB doc covers Windows durability |

## 3. Transactions and isolation

| Fact | Source |
|---|---|
| Every statement is its own transaction; `BEGIN … COMMIT/CANCEL` groups them; "Transactions run under snapshot isolation"; concurrent txns don't block; second committer on same record/unique-index value gets a **transaction conflict error and can be retried** | https://surrealdb.com/docs/surrealql/statements/begin |
| Both on-disk engines "use the same transaction and isolation semantics as every other SurrealDB deployment" | https://surrealdb.com/docs/surrealdb/installation/running/surrealkv |
| RocksDB engine opens `OptimisticTransactionDB` (commit-time validation) | `SRC/surrealdb/kvs-rocksdb/src/lib.rs` |
| SurrealKV maps `TransactionWriteConflict` → `TransactionConflict`; open bug: SurrealKV `TransactionRetry` surfaces as untyped non-retryable failure | `SRC/surrealdb/kvs-surrealkv/src/lib.rs` l.37; https://github.com/surrealdb/surrealdb/issues/7605 |
| SDK exposes `db.begin()` → `Transaction` with `commit()`/`cancel()` (interactive txns) | `SRC/surrealdb/src/method/begin.rs`, `transaction.rs` |
| Documented size/duration limits for txns: none stated. SurrealKV: a single txn larger than `max_memtable_size` errors (64 MiB on <1 GiB RAM, 128 MiB <4 GiB, 256 MiB <16 GiB) | begin docs; `SRC/surrealdb/kvs-surrealkv/src/cnf.rs` |
| Serializable / write-skew protection | **not provided** (snapshot isolation only, per docs) |

## 4. File locking / single-process ownership

| Fact | Source |
|---|---|
| A second process opening the same path fails: "Database at mydb/LOCK is already locked by another process"; also applies **within one process** (second `Surreal::new()` on an open path fails); stop the embedded connection before starting a server on the same path | https://surrealdb.com/docs/sdk/rust/embedding |
| SurrealKV `LockFile` uses OS file locks via `fs2`; lock auto-released by the OS on crash, so stale `LOCK` files don't block restart | https://github.com/surrealdb/surrealkv (`src/lockfile.rs`, crate 0.21.4) |
| SurrealKit (schema tool) warns "Embedded engines are single-process, so run the CLI while your application is stopped" | https://github.com/surrealdb/surrealkit |

## 5. Crash / power-loss recovery

| Fact | Source |
|---|---|
| Committed data survives stop "subject to the backend's sync setting" | https://surrealdb.com/docs/surrealdb/cli/start |
| RocksDB `sync=never` comment: commits go to WAL but OS flushes later; "In the event of a system crash, data may be lost" | `SRC/surrealdb/kvs-rocksdb/src/lib.rs` |
| SurrealKV crash bug: after unexpected shutdown, manifest referenced a 0-byte SSTable → server would not start, no repair path (3.0.5). Maintainer: fixed since SurrealKV 0.21.3 (SurrealDB ≥3.3.0) by syncing compaction outputs + dir entry before manifest. Earlier report #5001 same signature | https://github.com/surrealdb/surrealdb/issues/7426 |
| SurrealKV corruption → HNSW IO errors (3.0.0-beta.3), closed | https://github.com/surrealdb/surrealdb/issues/6872 |
| No documented repair/fsck tool for either engine in SurrealDB | **unverified / none found** |
| Independent power-loss testing results (e.g. on Windows NTFS, consumer SSDs) | **unverified** — none published |

## 6. Change feeds and live queries

| Fact | Source |
|---|---|
| `DEFINE TABLE … CHANGEFEED @duration [INCLUDE ORIGINAL]` (also definable per database); duration = retention; INCLUDE ORIGINAL stores **reverse diffs**; plain feeds emit `{delete:{id}}` only | https://surrealdb.com/docs/surrealql/statements/define/table |
| `SHOW CHANGES FOR TABLE t SINCE <datetime|versionstamp> [LIMIT n]`; "Versionstamps monotonically increase"; sequential only if the table is the DB's only table; `>> 16` to get the sequence; datetime must be after feed definition | https://surrealdb.com/docs/surrealql/statements/show |
| **Open:** on single-node engines (mem/rocksdb/surrealkv) versionstamps come from `HlcTimeStamp::next()` with a process-local `LAST_TIMESTAMP` that resets to 0 on restart; a backward wall-clock step across restart stamps new writes below persisted stamps and `SINCE` cursors silently skip them; no hook to seed it | https://github.com/surrealdb/surrealdb/issues/7606 |
| Live queries: only receive changes on the node holding the subscription (Community); multi-node = Enterprise; only committed txns notify; "Ordering is kept correct on a best-effort basis, but it is not yet strictly guaranteed"; bound to the WebSocket connection, ended on close, client must re-subscribe | https://surrealdb.com/docs/surrealql/statements/live |
| Embedded engine runs a live-notification delivery task (live queries work embedded, gated by capabilities) | `SRC/surrealdb/engine-local/src/lib.rs` l.158–220 |
| **Open:** LIVE SELECT messages lost during a ~0.3–0.8 ms window when another connection writes (2.0–3.0) | https://github.com/surrealdb/surrealdb/issues/6983 |

## 7. Export / backup / restore

| Fact | Source |
|---|---|
| `surreal export` produces a SurrealQL script for one ns/db; `--only` + per-resource flags; `--versions` refused from 3.3.1; `--log none` when piping to stdout; import requires the `OPTION IMPORT` line (3.0.4+), which disables events, live queries, field processing; signing keys are never exported | https://surrealdb.com/docs/surrealdb/cli/export |
| Core export runs inside **one read-only transaction** → snapshot-consistent, online | `SRC/surrealdb/core/src/kvs/ds.rs` (`export_with_config`, `self.transaction(Read)`) |
| SDK has `db.export(target)` / `db.import(...)` with config (users, accesses, params, functions, analyzers, tables/records, sequences, …); works for embedded engines through the same API | `SRC/surrealdb/src/method/export.rs`, `import.rs` |
| **Open bug:** export writes record-bearing `set` values as `{a:1,…}` literals that re-import as empty objects (silent loss) or abort the import (3.3.0) | https://github.com/surrealdb/surrealdb/issues/7621 |
| Fast physical backup/restore (non-SQL path) is an open feature request; no engine checkpoint/hot-backup exposed by SurrealDB | https://github.com/surrealdb/surrealdb/issues/7189; no `checkpoint` in `SRC/surrealdb/kvs-rocksdb/src` |
| SurrealKV crate itself has "Checkpoint and Restore" (not surfaced through SurrealDB) | https://github.com/surrealdb/surrealkv README |

## 8. Schema and migration tooling

| Fact | Source |
|---|---|
| No built-in migration runner in SurrealDB. Official **SurrealKit** (`surrealkit` 0.7.0 on crates.io, Apache-2.0, github.com/surrealdb/surrealkit): declarative sync for dev, phased "rollouts" with manifests + rollback for prod, seeding, type generation, test runner; supports remote and embedded (`rocksdb://`, `surrealkv://`) hosts (embedded engines need a custom build) | https://github.com/surrealdb/surrealkit; https://crates.io/crates/surrealkit |
| SurrealKit caveats: no cross-database transaction for multi-target runs; rolling back `TYPE`/`VALUE`/`DEFAULT` restores schema not data; `DEFINE ACCESS` without fixed key re-keys on overwrite (signs everyone out) | https://github.com/surrealdb/surrealkit |
| Third-party: `surrealdb-migrations` 2.4.0 (MIT, last 2025-11), `surrealdb-migrator` 0.3.0 (Apache-2.0) — compatibility with 3.3 **unverified** | https://crates.io/crates/surrealdb-migrations, https://crates.io/crates/surrealdb-migrator |

## 9. Performance on modest hardware

No official benchmarks for small machines. Defaults scale with RAM:

| Fact | Source |
|---|---|
| RocksDB block cache `max(total_mem/2 − 1 GiB, 16 MiB)`; write buffer 32/64/128 MiB; max open files 256/512/1026; group-commit cap 256/1024/4096 by RAM tier (<1, <4, ≥4 GiB); background jobs capped ~1 per 128 MiB | `SRC/surrealdb/kvs-rocksdb/src/cnf.rs` |
| SurrealKV block cache same formula; memtable 64 MiB–4 GiB by RAM; vlog files 64–512 MiB | `SRC/surrealdb/kvs-surrealkv/src/cnf.rs` |
| **Open:** SurrealKV 3.2.1 RSS grew to 27.6 GB → OOM on a 547 MB dataset; block-cache auto-size ignores cgroup limits | https://github.com/surrealdb/surrealdb/issues/7430 |
| Closed: SurrealKV reclaim task ballooned RSS → OOM (3.2.4) | https://github.com/surrealdb/surrealdb/issues/7477 |
| Open planner issues causing full scans (`LIMIT` without `ORDER BY`; `IN [..]` >32 items) | https://github.com/surrealdb/surrealdb/issues/7631, https://github.com/surrealdb/surrealdb/issues/7632 |
| Large WS responses (>~22 MB) drop the connection | https://github.com/surrealdb/surrealdb/issues/7623 |
| Throughput/latency figures on a 4–8 GB facility PC | **unverified** — must be measured |

## 10. Licensing

| Fact | Source |
|---|---|
| Business Source License 1.1; Licensed Work "SurrealDB 3.0"; Additional Use Grant: any use except as a **"Database Service"** (giving third parties DB functionality to create/manage schemas or tables); Change Date **2030-01-01** → Apache-2.0 | https://github.com/surrealdb/surrealdb/blob/v3.3.4/LICENSE |
| Rust SDK crate carries the same licence file (crates.io shows "non-standard") | `SRC/surrealdb/Cargo.toml` (`license-file.workspace`); crates.io |
| SurrealKV crate: Apache-2.0; SurrealKit: Apache-2.0 | crates.io |
| Multi-node HA / distributed live queries are Enterprise/Cloud, not Community; TiKV in Community is "for local multi-node experimentation"; FoundationDB removed in 3.0 | https://surrealdb.com/docs/surrealdb/cli/start; live docs |

## Implications for the decision

**Recommended baseline:** SurrealDB **3.3.x**, **RocksDB** engine (`kv-rocksdb`), `sync=every` (default), for both the Facility Edge store and the standalone Tauri store. Revisit SurrealKV when 3.4 ships it as recommended and it has a release or two of field history.

**Facility Edge Server**
- Fits: single process owns the data directory (LOCK enforces it); LAN clients talk to our Axum API, not to SurrealDB directly. Either run `surreal start rocksdb://…` as a sibling service or embed via `kv-rocksdb` in the edge Rust process — embedding avoids a second service and network auth surface; running the server allows `surreal export` and ops tooling while live. Pick one; both cannot open the same path.
- Snapshot isolation + optimistic conflicts ⇒ every write path needs **retry on conflict**, and invariants spanning multiple records (stock levels, MRN/sequence allocation, double-booking) need write-skew protection (e.g. write a guard record / unique index both txns touch).
- HA/failover is not available in Community; facility edge is single-node by design, so acceptable.

**Standalone Tauri**
- Embedded RocksDB requires libclang + C++ toolchain in the Windows/Linux build (CI cost, binary size); SurrealKV would avoid that but is beta in 3.3.
- Only one `Surreal` handle per path per process: the Tauri app must own one shared instance; no side tools (SurrealKit, export CLI) while the app runs — use SDK `export()` in-app for backups.

**Sync (blocks issues #16/#18 design choices)**
- Do **not** use changefeed versionstamps as the sync cursor without mitigation (#7606: HLC resets on restart; laptops/facility PCs with bad clocks are exactly our environment). Use our own durable outbox table with a monotonic sequence written in the same transaction as the clinical write, or persist and enforce a high-watermark.
- Live queries are a UI convenience only (per-connection, best-effort order, lost-notification race #6983).

**Backup**
- Only logical export is available; it is snapshot-consistent but slow for large DBs and currently lossy for record-id `set` values (#7621) ⇒ avoid `set<record>` in the schema or verify round-trips in CI. Consider filesystem-level snapshot of a stopped/locked store for edge backups (procedure **unverified**).

**Risks / open questions for a validation spike**
1. Power-cut test (hard power-off during write load) for RocksDB embedded on Windows 10/11 NTFS and Linux ext4 — durability under `sync=every` on Windows is undocumented.
2. Memory footprint on a 4–8 GB facility PC; tune block cache explicitly rather than trusting auto-sizing (#7430).
3. Correctness churn: 3.3.4 still fixing wrong-result query bugs ⇒ pin exact versions, keep a FHIR query regression suite, upgrade deliberately.
4. Windows arm64: no official binary; build support for embedded on WoA has an old open issue (#3738) — **unverified** for 3.3.
5. BSL: confirm with counsel that a ministry/NGO hosting the central platform for many facilities is not a "Database Service" (we expose FHIR APIs, not schema/table control — likely fine, but a legal call).
6. Whether 2.x/3.x on-disk formats are upgrade-compatible in place vs export/import — **unverified**.
