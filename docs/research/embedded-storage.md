# Research: embedded storage

This page consolidates the embedded-storage facts already recorded in [surrealdb.md](surrealdb.md), [facility-edge.md](facility-edge.md) and [tauri.md](tauri.md). Those pages condense the research tickets [What do SurrealDB server and embedded engines guarantee today?](https://github.com/carapai/people-centered-healthcare/issues/2), [How do comparable systems run an offline facility server on a LAN?](https://github.com/carapai/people-centered-healthcare/issues/3) and [What does Tauri v2 offer for embedded local services and secure packaging?](https://github.com/carapai/people-centered-healthcare/issues/11). There was no separate ticket for this page. Decisions:
- [persistence.md](../architecture/persistence.md) (ADRs 0017, 0018);
- [facility-edge.md](../architecture/facility-edge.md) and [backup-and-recovery.md](../architecture/backup-and-recovery.md) (ADR 0007);
- [desktop-client.md](../architecture/desktop-client.md) (ADRs 0039, 0040).

## Where storage is embedded

| Runtime | Process | Engine |
|---|---|---|
| Facility Edge Node | SurrealDB embedded in the `facility-edge` process | RocksDB (`kv-rocksdb`), `sync=every` |
| Tauri desktop | Embedded in the Tauri core process, one shared handle | RocksDB, `sync=every` |
| Central | Not embedded: a separate `surreal start rocksdb://…` server | RocksDB, `sync=every` |

## RocksDB vs SurrealKV

- The SDK exposes `kv-mem`, `kv-rocksdb`, `kv-surrealkv`, `kv-tikv` and `kv-indxdb`.
- **RocksDB** needs a C++ compiler and libclang at build time. It uses `fdatasync` with group commit.
- **SurrealKV** is pure Rust but still beta in 3.3. It caps a transaction at the memtable size, and has an OOM report (#7430: 27.6 GB RSS on a 547 MB dataset).
- No transaction size limit is documented for RocksDB.
- Decision: RocksDB everywhere; SurrealKV is revisited only once upstream recommends it and it has some release history. Durability must be re-validated at every engine change.

## Durability and sync modes

- `sync` (`SURREAL_DATASTORE_SYNC_DATA`) can be `never`, `every` (the default for RocksDB and SurrealKV) or an interval. `every` fsyncs before a commit is acknowledged.
- The edge rejects `never` at startup; `sync=every` is mandatory on every runtime.
- **Windows durability is unknown:** no SurrealDB document covers it. Behaviour under hard power loss on NTFS and ext4 is the subject of a spike (below).
- A UPS plus `sync=every` is the edge baseline.

## Process model and the LOCK file

- Both persistent engines take an exclusive OS `LOCK` file on the data directory: one process, and one handle per process. A stale lock is released by the OS after a crash.
- So the edge and desktop each hold one shared `Surreal` handle, and no external tool may open the directory while the node runs. On the desktop, the single-instance plugin prevents a second app instance and the RocksDB `LOCK` backs it up.
- Embedded and server modes share the same SurrealQL and snapshot isolation; a commit-time conflict is a retryable error.

## Memory: block cache sizing

- RocksDB's block cache auto-sizes to `max(total/2 − 1 GiB, 16 MiB)` and ignores cgroup limits.
- Decision: cap it explicitly; the edge default is about 25% of RAM.
- No official benchmarks exist for small machines.

## Encryption at rest

- SurrealDB has no native encryption at rest. Tauri research also lists it as unverified for the embedded case.
- Decision for the desktop: provisioning checks for OS full-disk encryption (BitLocker or LUKS); country configuration decides whether its absence warns or blocks.

## Backup

- Only **logical export** is available. It runs inside one read transaction, so it is snapshot-consistent and can run online, through the SDK as well as the CLI.
- **No hot physical backup** (open request #7189).
- Import requires `OPTION IMPORT`. Record-bearing `set` values are lost or abort the import (#7621), so schemas avoid `set<record>`.
- Whether on-disk formats upgrade in place or need export/import is unverified, which is why every update is preceded by a backup.

## Other unverified points

- Windows arm64 embedded builds (no Windows arm64 release binary exists).
- Multi-instance locking and database size inside the Tauri process.
- No official Tauri + SurrealDB example exists; third-party use exists (appdb, UMLBoard).

## Open spikes

- [Does embedded SurrealDB RocksDB survive hard power loss on facility hardware?](https://github.com/carapai/people-centered-healthcare/issues/34): must pass on Windows NTFS and Linux ext4 before edge storage implementation starts.
- [Does the per-node commit counter sustain facility write rates on edge hardware?](https://github.com/carapai/people-centered-healthcare/issues/36): the single `seq` counter record written in every change-log transaction ([synchronization.md](../architecture/synchronization.md)).
- [Can SurrealDB blocking indexes serve patient matching at 45M-person scale?](https://github.com/carapai/people-centered-healthcare/issues/37): array-keyed `idx_block` tables at central (server, not embedded); fallback is a tantivy sidecar ([patient-identity.md](../architecture/patient-identity.md)).
- [Does embedded SurrealDB run reliably inside the Tauri process on facility PCs?](https://github.com/carapai/people-centered-healthcare/issues/39): the §75 gate for embedded storage on the desktop; if it fails, those facility types run an edge instead.
