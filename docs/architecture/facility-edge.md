# Facility Edge Node: process, storage and recovery

Status: **accepted** (ADR 0007). Produced while resolving the Wayfinder ticket [What is the Facility Edge Node's process, storage and recovery model?](https://github.com/carapai/people-centered-healthcare/issues/18). Background: [docs/research/facility-edge.md](../research/facility-edge.md).

LAN discovery and TLS trust are in [lan-security.md](lan-security.md). Staff authentication is in [offline-auth.md](offline-auth.md). The sync protocol is in [synchronization.md](synchronization.md).

## Topology

- Each facility has **exactly one Facility Edge Node** in v1. Large hospitals scale the hardware, not the number of nodes.
- LAN clients (browsers, Tauri) talk only to the edge's Axum API. Nothing on the LAN reaches SurrealDB directly.

## Process model

- There is one OS service running one binary, `facility-edge` (a runtime crate over `health-edge-runtime`; see [crate-catalogue.md](crate-catalogue.md)). It has a `--service` mode and a foreground mode for development.
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

In code (#72): `apps/server/facility-edge` over `health-edge-runtime`. The systemd unit is `apps/server/facility-edge/packaging/facility-edge.service`. On Windows, `install-service.ps1` registers the service and its failure actions, and `WindowsScm` reports wait hints. `edge-build.yml` runs the binary under both service managers. The Event Log sink is still to come. The exit codes are in the binary's README; a configuration error is not restarted.

## Web app

The edge serves the client from its own origin (lan-security.md § App delivery to browsers). In code (#76):
- `health-edge-runtime`'s build script embeds `apps/client/dist` in the binary, without its source maps, so a LAN browser needs nothing from the internet. Build the client before the binary; a binary built without it serves only its API.
- The edge answers:
  - `runtime.json` (the edge platform and the embedded build);
  - `sw.js`, the app-shell service worker ([client-application.md](client-application.md) § Serving and versions);
  - the bundle's files, with hashed `assets/` immutable and everything else revalidated;
  - the page for any other path that is not a file.

  Unknown paths under `/api`, `/fhir` and `/auth` are 404s, never the page.
- The app is served from the listener's first moment, so browsers load the shell while the store opens.

## Hardware and power baseline

- x64, at least 8 GB RAM, SSD system storage.
- **TPM 2.0 and UEFI Secure Boot enabled.** Full-disk encryption with TPM-only unlock (BitLocker or LUKS2 + TPM2) is mandatory before provisioning, so the edge still restarts unattended after a power cut (ADR 0048).
- A **second storage volume or a dedicated external drive** for backups.
- A **UPS is mandatory**. NUT is used on Linux, and NUT or the vendor agent on Windows. A low-battery signal triggers a graceful stop: drain requests, finish the in-flight transaction, take a final backup if time allows, then stop. Where no UPS signal is available, durability rests on `sync=every`.
- A power-cut validation spike must pass on Windows NTFS and Linux ext4 before an edge is deployed to a facility. See the task ticket [Does embedded SurrealDB RocksDB survive hard power loss on facility hardware?](https://github.com/carapai/people-centered-healthcare/issues/34). The edge is built on the embedded adapter ahead of it (#72); if the spike fails, only the engine settings change.

## Backup and recovery

Local backups, restore, failover and recovery targets are in [backup-and-recovery.md](backup-and-recovery.md). In code (#78): `health_edge_runtime::backup` takes the hourly backups and the weekly test restore, and `facility-edge backup` covers backups on demand, USB copies and restore.

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
- **In code (#72):** `health_edge_runtime::update` and `facility-edge update stage | apply | rollback | status`.
  - **Package:** a gzipped tar of `manifest.json` (version, binary name, SHA-256) and the binary, with a detached ECDSA P-256 signature. The installer pins the release key.
  - **Pre-update backup:** a cold copy of the store, taken by the new binary before its first open.
  - **Rollback:** after a failed migration, or three starts that never reach ready. The store is restored first, then the binary. Under systemd, the sandboxed service restores the store, and `facility-edge-rollback.service` (`OnFailure=`) restores the binary as root.
- **Compatibility:** central supports edge versions N and N-1 on the sync protocol, so a facility may lag one release.
- Release signing and the CI pipeline are in [deployment.md](deployment.md).

## Health monitoring

- **Endpoints:** the shared system routes. `/health` means the process is up. `/ready` means the store is open, migrations are done, and the node is not draining.
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

- **In code (#77):** `health_edge_runtime::admin`, `GET /api/admin/status`, and the client's `/edge/status` page.
  - **Reported:** identity, version, store size, free disk, backups, UPS, task restarts (two or more within the supervisor's window) and the telemetry buffer.
  - **Pending:** sync, clock skew, certificates, the permission snapshot, disk encryption and sessions arrive with their features (Phase 5, #73–#75). The report lists them as pending.
  - **Access:** until the local authorization server can require an admin (#75), the report is served only to the node itself.
  - **Telemetry:** the buffer is under `HEALTH_TELEMETRY_DIR`, shipped to `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT`. Spans are not buffered yet.
