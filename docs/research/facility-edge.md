# Research: Facility Edge Node

This page condenses two research tickets:
- [How do comparable systems run an offline facility server on a LAN?](https://github.com/carapai/people-centered-healthcare/issues/3), full findings on branch `research/facility-edge`;
- [What do SurrealDB server and embedded engines guarantee today?](https://github.com/carapai/people-centered-healthcare/issues/2), branch `research/surrealdb`.

It covers process, storage, power and recovery. LAN TLS and discovery findings are condensed when [How do LAN clients find and trust the Facility Edge Node?](https://github.com/carapai/people-centered-healthcare/issues/19) is resolved. Decisions: [docs/architecture/facility-edge.md](../architecture/facility-edge.md), ADR 0007.

## Prior art

- **Open mSupply** is the closest comparable system.
  - It is a Rust server (actix-web + rustls) that runs as a Windows service via `windows-service` 0.8, with Event Log output.
  - It announces itself over mDNS as `_omsupply._tcp`, and identifies servers by hardware ID, never by address.
  - Its Docker deployment turns mDNS off.
- **UgandaEMR/OpenMRS** ships a Windows installer that bundles Tomcat and MySQL on a facility PC. It documents no LAN TLS or recovery model.
- **DHIS2** prefers one central server.
- **OpenSRP** is an Android client syncing to central.

## OS service layer

- **Windows:** `windows-service` 0.8.1 provides a control handler, wait hints for long pending states and a service manager. SCM failure actions restart a service that "terminates without reporting a status of SERVICE_STOPPED". The action array is indexed by failure count, and the count resets after `dwResetPeriod`.
- **Linux:** systemd `Type=notify` units send `READY=1` when started. `WatchdogSec` requires periodic `WATCHDOG=1`, or systemd kills the service with SIGABRT. `STOPPING=1` and `EXTEND_TIMEOUT_USEC` let long shutdowns or migrations avoid SIGKILL. The `sd-notify` 0.5 crate is pure Rust.

## SurrealDB facts relevant to the edge

- Both persistent engines take an exclusive OS `LOCK` on the data directory. Only one process may open it, and only one handle per process. A stale lock is released by the OS after a crash.
- Embedded and server modes share the same SurrealQL and the same snapshot isolation. A commit-time conflict is a retryable error.
- `SURREAL_DATASTORE_SYNC_DATA` defaults to `every` (fsync before a commit is acknowledged). Durability under `every` on Windows is undocumented, hence the power-cut spike.
- RocksDB embedded needs libclang and a C++ toolchain at build time. SurrealKV is pure Rust but still beta in 3.3.
- Block-cache auto-sizing ignores cgroup limits. One report (#7430) saw a 27.6 GB RSS on a 547 MB dataset with SurrealKV, so the cache must be capped explicitly.
- Backup:
  - Only a **logical export** is available, as SurrealQL inside one read transaction. It is snapshot-consistent and works online, through the SDK as well as the CLI.
  - Engine-level hot backup is not exposed (open request #7189).
  - Bug #7621 makes record-bearing `set` values re-import lossy or abort the import.
- There is no built-in migration runner. SurrealKit 0.7 offers phased rollouts. Third-party runners are unverified against 3.3.
- Change-feed versionstamps reset on restart (#7606), so sync uses its own outbox sequence.
- It is unverified whether on-disk formats upgrade in place or need export/import, which is why updates are always preceded by a backup.
- Windows arm64 embedded build support is unverified.

## Power

- A UPS plus `sync=every` is the baseline.
- Network UPS Tools covers Linux. Its Windows branch exists, but MSI packaging and some drivers depend on community work.
- The upsmon `SHUTDOWNCMD` details were not confirmed.
- Graceful-stop hooks: the SCM stop control and systemd SIGTERM / `STOPPING=1`.

## Open items carried forward

- Power-cut validation: [Does embedded SurrealDB RocksDB survive hard power loss on facility hardware?](https://github.com/carapai/people-centered-healthcare/issues/34).
- The RocksDB → SurrealKV transition means durability must be re-validated at every engine upgrade.
