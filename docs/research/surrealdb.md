# Research: SurrealDB

A condensed version of the research ticket [What do SurrealDB server and embedded engines guarantee today?](https://github.com/carapai/people-centered-healthcare/issues/2). The full findings, with citations, are on the unmerged branch `research/surrealdb`. Decisions:
- [docs/architecture/persistence.md](../architecture/persistence.md) (ADRs 0017, 0018)
- [facility-edge.md](../architecture/facility-edge.md) and [backup-and-recovery.md](../architecture/backup-and-recovery.md) (ADR 0007)

Edge-specific facts are also summarised in [facility-edge.md](facility-edge.md).

## Releases

- **Current versions:** latest stable is **3.3.4** (crates.io `surrealdb` 3.3.4, 2026-10-13). No LTS policy is published.
- **Recent query bugs:** 3.3.4 fixed indexed-scan filters that returned wrong rows. Pin exact versions and keep a query regression suite.
- **Platforms:** release binaries cover Linux amd64/arm64, macOS and Windows amd64. There is no Windows arm64 binary.
- **Feature flags:** the SDK exposes `kv-mem`, `kv-rocksdb`, `kv-surrealkv`, `kv-tikv` and `kv-indxdb`.
- **Build requirements:** embedded RocksDB needs a C++ compiler and libclang, while SurrealKV is pure Rust.
- **Local CLI:** this workstation has `surreal` 3.2.4, which was used to validate the persistence sketch.

## Durability

- `sync` can be `never`, `every` (the default for RocksDB and SurrealKV) or an interval. `every` fsyncs before a commit is acknowledged.
- RocksDB uses `fdatasync` with group commit. No SurrealDB document covers Windows durability, hence the power-cut spike.

## Transactions

- Every statement runs as its own transaction; `BEGIN … COMMIT/CANCEL` groups statements.
- Isolation is **snapshot**, not serializable. A second committer on the same record or unique-index value gets a retryable **transaction conflict**, so write skew is possible.
- The SDK exposes interactive transactions. No transaction size limit is documented for RocksDB. SurrealKV caps a transaction at the memtable size.

## Process model

- Both engines take an exclusive `LOCK` file, so only one process (and one handle) can open a data directory. The OS releases a stale lock after a crash.
- Live queries are per connection, best-effort in ordering, single-node only in the Community edition, and have a known lost-notification race (#6983).

## Change feeds

- `DEFINE TABLE … CHANGEFEED` and `SHOW CHANGES … SINCE` exist.
- Versionstamps come from a process-local HLC that resets on restart (#7606). If the wall clock steps backwards, `SINCE` cursors silently skip changes, so sync uses its own outbox sequence.

## Backup and migrations

- Only **logical export** is available. It runs inside a single read transaction, so it is snapshot-consistent and can run online, through the CLI or the SDK.
- Import requires `OPTION IMPORT`.
- Record-bearing `set` values are lost on import (#7621), so avoid `set<record>`.
- There is no hot physical backup (#7189).
- There is no built-in migration runner. SurrealKit 0.7 offers phased rollouts, but needs a custom build for embedded engines. Third-party runners are unverified on 3.3.

## Performance

- No official benchmarks exist for small machines.
- RocksDB's block cache auto-sizes to `max(total/2 − 1 GiB, 16 MiB)` and ignores cgroup limits. SurrealKV has an OOM report (#7430). Cap the cache explicitly.
- Known planner issues cause full scans: `LIMIT` without `ORDER BY` (#7631), and `IN [..]` with more than 32 items (#7632). Verify query plans with `EXPLAIN`.
- Large WebSocket responses (over about 22 MB) drop the connection (#7623).
- Record IDs are stored in sorted order, so range scans over array record IDs are the fastest access path.

## Licence and editions

- The licence is **BSL 1.1**. Any use is allowed except offering SurrealDB as a "Database Service". The change date is 2030-01-01, after which it becomes Apache-2.0.
- Multi-node HA and distributed live queries are Enterprise or Cloud features. TiKV in Community is for experimentation only.
- Whether central multi-facility hosting falls under "Database Service" needs legal review (map fog).
