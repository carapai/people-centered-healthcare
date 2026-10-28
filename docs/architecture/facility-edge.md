# Facility Edge Node: process, storage and recovery

Status: **accepted** (ADR 0007). Produced while resolving the Wayfinder ticket [What is the Facility Edge Node's process, storage and recovery model?](https://github.com/carapai/people-centered-healthcare/issues/18). Background: [docs/research/facility-edge.md](../research/facility-edge.md).

LAN discovery and TLS trust are decided in [How do LAN clients find and trust the Facility Edge Node?](https://github.com/carapai/people-centered-healthcare/issues/19). Staff authentication is decided in [How do staff authenticate and get authorized offline?](https://github.com/carapai/people-centered-healthcare/issues/20). The sync protocol is decided in [What is the sync protocol and per-resource conflict policy?](https://github.com/carapai/people-centered-healthcare/issues/22).

## Topology

- Each facility has **exactly one Facility Edge Node** in v1. Large hospitals scale the hardware, not the number of nodes.
- LAN clients (browsers, Tauri) talk only to the edge's Axum API. Nothing on the LAN reaches SurrealDB directly.

## Process model

- There is one OS service running one binary, `facility-edge` (a runtime crate over `health-edge-runtime`; see [crate-architecture.md](crate-architecture.md)). It has a `--service` mode and a foreground mode for development.
- It hosts the Deployable contents from [resource-ownership.md](resource-ownership.md): the operational contexts, plus read-only Registries and Terminology & Metadata.
- **SurrealDB is embedded** (`kv-rocksdb`, version pinned exactly per the SurrealDB research). The process owns the data directory alone. There is one shared `Surreal` handle per process, and no external tool opens the directory while the node runs.
- **Durability:** `sync=every` is mandatory, and `never` is rejected at startup. The RocksDB block cache is capped explicitly (default about 25% of RAM) rather than auto-sized.
- **Background work runs as supervised Tokio tasks in the same process:**
  - outbox dispatcher;
  - sync uploader and downloader;
  - workflow timers;
  - backup scheduler;
  - certificate renewal;
  - health checks.

  A failed task restarts with exponential backoff and raises an alert. If the database handle fails, or a task crashes repeatedly, the process exits so the service manager restarts it cleanly.

## OS service

| Platform | Supported in v1 | Integration |
|---|---|---|
| Windows 10/11 x64 | Yes (reference) | `windows-service`. SCM failure actions: restart, restart, restart, then stay stopped and alert, with the counter reset after 24 h. Logs go to the Event Log and to rotating files. Long migrations and shutdowns report wait hints. |
| Ubuntu LTS x64 | Yes | systemd `Type=notify`, `WatchdogSec`, `Restart=on-failure`, generous `TimeoutStopSec`. `sd-notify` sends `READY`/`WATCHDOG`/`STOPPING`, and `EXTEND_TIMEOUT_USEC` during migrations and shutdown. |
| Linux in Docker | Allowed for centrally managed sites only | Not the reference path, because mDNS is unavailable in containers. |
| Windows arm64, macOS | No | |

## Hardware and power baseline

- x64, at least 8 GB RAM, SSD system storage.
- A **second storage volume or a dedicated external drive** for backups.
- A **UPS is mandatory**. NUT is used on Linux, and NUT or the vendor agent on Windows. A low-battery signal triggers a graceful stop: drain requests, finish the in-flight transaction, take a final backup if time allows, then stop. Where no UPS signal is available, durability rests on `sync=every`.
- A power-cut validation spike must pass on Windows NTFS and Linux ext4 before edge storage implementation starts. See the task ticket [Does embedded SurrealDB RocksDB survive hard power loss on facility hardware?](https://github.com/carapai/people-centered-healthcare/issues/34).

## Backup

Recovery draws on two sources. **Central** holds everything already synced. **Local backups** cover the window that has not been uploaded, and any outage.

- **What:** a snapshot-consistent logical export, taken through the SurrealDB SDK inside the edge process. It includes the outbox, so un-uploaded work is preserved.
- **When:** hourly while the facility is active. Retention is 48 hourly and 14 daily copies. An admin can also run it on demand, including to USB. One is taken automatically before every update.
- **Where:** the second volume, never the data volume.
- **Protection:** every backup is encrypted with a per-node backup key. The key is escrowed at central (custody details stay in the map's Not yet specified under disaster recovery and key escrow).
- **Integrity:** every backup is verified after it is written, using its checksum and manifest. CI runs an export→import round-trip test on representative data, and the schema avoids `set<record>` values because of SurrealDB bug #7621.

## Restore and failover

- **No hot standby and no automatic failover** in v1 (ADR 0007).
- **Same machine, corrupted store:** stop the node, restore the latest verified backup, start, and resume sync.
- **Machine lost:** use a cold spare, either pre-enrolled or any baseline PC. First re-provision it from central (node identity re-issued, facility data pulled down). Then import the un-uploaded outbox operations from the last local backup. Sync deduplicates them by `op_id`.
- **Targets:** data loss of at most one backup interval for work not yet synced, and under one hour to recover when a spare is available. Formal RPO/RTO and DR drills stay in Not yet specified.

## Updates and migrations

- Updates are **signed packages**. They are pulled from central when online, or applied from USB when offline, and the signature is verified before staging.
- They install only in a configured maintenance window, or when an admin approves with no active sessions.
- Install sequence:
  1. Drain sessions.
  2. Take a mandatory pre-update backup.
  3. Swap the binary.
  4. Start, and run migrations before reporting ready.
- **Migrations are forward-only and versioned.** They run at startup inside the edge process, extending the service-manager timeout while they run.
- **On a failed migration or failed readiness:** reinstall the previous binary automatically and restore the pre-update backup. Old binaries are never run against a migrated store.
- **Compatibility:** central supports edge versions N and N-1 on the sync protocol, so a facility may lag one release.
- Release signing and the CI pipeline are decided in [What are the CI/CD, release and production-operations rules?](https://github.com/carapai/people-centered-healthcare/issues/31).

## Health monitoring

- **Endpoints:** `/health/live` (process up) and `/health/ready` (store open, migrations done, not draining).
- **Admin status page** in the shared React app, showing node identity, version, store size, backup status, sync status, certificate expiry, UPS status and active sessions.
- **Telemetry:** OpenTelemetry metrics, traces and logs go into a bounded local buffer and are shipped to central when online. The oldest data is dropped first when the buffer is full.
- **Alerts** go to both the local status page and central:

| Condition | Threshold |
|---|---|
| Disk usage (data or backup volume) | > 80% |
| Last good backup | older than 2× its interval, or failed |
| Sync | oldest un-uploaded op older than a configured threshold, or upload failing |
| TLS certificate expiry | < 30 days |
| UPS | on battery, or low battery |
| Clock skew against central | above a configured threshold (HLC ordering tolerates skew; alerts catch broken clocks) |
| Permission snapshot expiry | approaching hard expiry |
| Task restarts | repeated restarts of any supervised task |
