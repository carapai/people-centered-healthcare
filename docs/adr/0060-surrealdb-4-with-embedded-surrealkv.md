# SurrealDB 4, with SurrealKV as the embedded engine

This amends ADR 0007 and persistence.md § Engines. Every node now runs **SurrealDB 4.0.0**. **Edges and standalone desktops embed SurrealKV**, opened with `sync=every`, in place of RocksDB. Central stays a separate `surreal start` server, now version 4.0.0, reached over WebSocket.

- **Why now.** RUSTSEC-2026-0454 (2026-11-06) found that SurrealDB 3.3.5's executor runs every statement through `reblessive`, whose `Send` and `Sync` implementations are unsound: a data race and double free are reachable from safe code. SurrealDB 4.0.0 removes `reblessive` and parses and executes by plain recursion. There is no 3.x fix.
- **Why SurrealKV.** The SurrealDB 4 SDK has no embedded RocksDB engine; its embedded engines are memory and SurrealKV.
- **Durability is unchanged in kind.** SurrealKV's default on native targets is `sync=every`: a commit's write-ahead log is synced to disk before the commit is acknowledged. `Target::SurrealKv` always asks for it explicitly. An interval mode (`sync=200ms`) exists and is never used.
- **Stack size.** Because SurrealDB 4 recurses, every runtime that runs it uses 10 MiB thread stacks, as SurrealDB recommends (`health_surrealdb::THREAD_STACK`). Test threads get the same through `RUST_MIN_STACK` in `.cargo/config.toml`. On the default 2 MiB stack, a deep statement overflows.

We chose this because an unsound executor under every clinical write is not acceptable, and 4.0.0 is the only fixed line. Its embedded engine is SurrealKV, so the engine change comes with the version.

## Considered Options

| Option | Decision |
|---|---|
| Stay on 3.3.5 and ignore RUSTSEC-2026-0454 in `deny.toml` | Rejected by the product owner: the executor's unsoundness touches every write |
| Upgrade central only, keeping embedded RocksDB on 3.3.5 | Rejected: two SDK major versions in one workspace, and edges keep the unsound executor |
| SurrealDB 4 with embedded SurrealKV | **Chosen** |

## Consequences

- **Power loss.** Spike #34 asked whether embedded *RocksDB* survives hard power loss on facility hardware. It now applies to **SurrealKV**, and is to be run on SurrealKV before the first facility installation. Until then, durability rests on SurrealKV's `sync=every` contract and the nightly encrypted backups.
- **Existing data.** No facility runs yet, so no RocksDB data has to be migrated. If a development edge directory holds RocksDB data, restore it from a logical backup (`export_sql` and `import_sql`) into a fresh SurrealKV directory.
- **Tuning.** The RocksDB block cache setting (`SURREAL_ROCKSDB_BLOCK_CACHE_SIZE`) no longer applies. SurrealKV's cache and memtable sizes are datastore options, to be set from the edge hardware spikes (#36).
- **The 3.3.5 measurements** in persistence.md (counter contention and others) are to be measured again on 4.0.0, with the spikes.
- **Measured on 4.0.0 (debug build, macOS, 2026-11-06).**
  - A single-row commit takes about 39 ms with `sync=every`, against 15 ms in memory. That is fine at clinic write rates, and Linux's sync is usually cheaper than macOS's `F_FULLFSYNC`.
  - A fresh store's migrations take about 78 s. Most of it is `0001_resource_tables`, whose 1,103 statements take 49 s with `sync=every` and 6 s with `sync=never`: **SurrealDB 4 syncs on every schema statement, even inside one transaction**, and macOS's sync (`F_FULLFSYNC`) is slow. Linux syncs are usually far cheaper. This is paid once, at a store's first start, so the edge tests allow 180 s to become ready, and the edge e2e suite, which starts a fresh edge per test, allows 300 s. `sync=every` is kept for migrations too. Applying a fresh store's migrations with a cheaper sync is a possible follow-up, to be decided with spike #36.
  - First-start migration time and commit latency in a release build are to be measured on edge hardware with spikes #34 and #36.
- **Release age.** 4.0.0 was published on 2026-11-03, three days before this decision. Patch releases are to be taken as they come, each with the full test suite and e2e.
- **CI** pins the 4.0.0 server binary by its SHA-256, for `surreal validate` and for e2e.
- **Licences** (approved 2026-11-06, each for one crate only in `deny.toml`): **BSL-1.0** (Boost, OSI-approved) for `xxhash-rust`, used by SurrealKV, and **CDLA-Permissive-2.0** for `webpki-root-certs`, the root-certificate data SurrealDB 4 brings in. Three new SurrealDB crates ship the same BSL 1.1 text as the rest, and join ADR 0045's documented exception.
