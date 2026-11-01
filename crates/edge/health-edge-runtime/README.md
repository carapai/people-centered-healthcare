# health-edge-runtime

Layer: **runtime**. The Facility Edge Node's runtime ([facility-edge.md](../../../docs/architecture/facility-edge.md), ADR 0007). It is used by [`apps/server/facility-edge`](../../../apps/server/facility-edge/README.md).

## `run`

`run(config, manager, telemetry, stop)` is the node's whole life (`telemetry` is the buffer the process's log lines go to):
1. **Listen.** The listener starts at once: `/health` answers, `/ready` is 503 until the store is ready (the shared `health-http` system routes), the embedded web app is served (`web`), and so is `/api/admin/status` (`admin`).
2. **Update check.** A pending update copies the store to `pre-update-store/` on its first start, or rolls back after three starts that never reached ready (`update::before_open`), before the store is opened.
3. **Open the store.** The embedded store (`Target::RocksDb`, `sync=every`) is opened once. If it fails, `run` returns `EdgeError::Store` and the service manager restarts the process.
4. **Migrate.** Migrations run before ready. The service manager is asked for 30 s more every 10 s while they run. A store that a newer binary migrated is refused (`EdgeError::Migration`), so an old binary never runs against it. During an update, a failed migration marks the update failed and the next start rolls it back.
5. **Ready.** A pending update is committed. The manager is told, and the supervised tasks start: the watchdog keep-alive, the backup schedule, the UPS watch, the indicators and the telemetry shipper.
6. **Stop.** On `stop`, a UPS low battery, or a task that keeps failing:
   - `/ready` turns 503 (the `serving` check);
   - the manager is told;
   - in-flight requests drain within `HEALTH_DRAIN_SECS`;
   - the store closes.

## Modules

- **`admin`:** health monitoring (#77, facility-edge.md § Health monitoring).
  - **Indicators:** node ID and version, store size, free space on the data and backup volumes, the backup schedule (`status.json`), the UPS's last poll, supervised-task failures, and the telemetry buffer.
  - **Alerts,** against the thresholds in facility-edge.md: a volume over 80% used (`disk-full`), backups off, failed (`backup-failed`), older than twice the interval (`backup-stale`, counted from the node's start before a first backup), a failed test restore, the UPS on battery, low or unreachable, and a task with two or more failures in the supervisor's window (`task-restarts`).
  - **`GET /api/admin/status`:** the `Report` as JSON, `Cache-Control: no-store`, for the client's edge status page. It answers loopback peers only (403 otherwise) until the local authorization server can require an admin (#75). It holds no personal health data.
  - **Telemetry:** every `HEALTH_INDICATORS_SECS` an `indicators` line carries the measures as allowlisted fields (`store_bytes`, `data_free_bytes`, `backup_age_seconds`, `state`, …), and each alert is logged with `alert` when raised and with `outcome = "cleared"` when it clears.
  - **`Buffer`** is the process's `LogSink`: each line goes to standard output and to 256 KiB segment files under `HEALTH_TELEMETRY_DIR`, bounded at 64 MiB with the oldest segments dropped first. It survives restarts.
  - **Shipping:** every `HEALTH_TELEMETRY_SHIP_SECS`, the sealed segments are posted oldest first to `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` as OTLP/HTTP JSON log records, and each is removed only once the collector accepts it. A failing collector raises `telemetry-unshipped` once, and its recovery clears it.
- **`backup`:** scheduled backups on `health-backup` (#78, [backup-and-recovery.md](../../../docs/architecture/backup-and-recovery.md)).
  - **Hourly** (`HEALTH_BACKUP_INTERVAL_SECS`), each verified after writing. The next is due one interval after the last attempt, so a restart neither skips nor repeats one.
  - **Retention:** the 48 newest, and the newest of each of the last 14 days (UTC). Pruning also removes unfinished backups and blobs that no kept backup refers to.
  - **Weekly test restore** (`HEALTH_BACKUP_TEST_SECS`) of the newest backup into a scratch store beside it. The restored store must hold its migration ledger.
  - **`status.json`** under the backup root records the last attempt, backup, failure and test restore, for the status page.
  - **Alerts:** a failed backup or test restore is logged at error with `alert = "backup-failed"` or `"backup-test-failed"`. Neither stops the node. A node without `HEALTH_BACKUP_DIR` warns `backups-off` at every start, and one whose backups share the data volume warns `backup-on-data-volume`.
  - `copy_newest` (to USB, verified) and `restore_into` (a new store only) back the `backup` commands.
- **`config`:** `EdgeConfig::from_lookup` reads the service environment. Errors name the variable, never its value.
- **`notify`:** the `ServiceManager` port.
  - `Foreground` is the no-op for development.
  - `WindowsScm` reports to the Service Control Manager: `StartPending` and `StopPending` with wait hints and an advancing checkpoint, `Running`, then `Stopped` with the exit code as a service-specific error.
  - `Systemd` speaks sd_notify(3) on `$NOTIFY_SOCKET`, including Linux abstract sockets. It sends `WATCHDOG=1` at half of `WATCHDOG_USEC`, when `WATCHDOG_PID` is unset or names this process.
- **`supervisor`:** a failed or panicking task restarts with exponential backoff, from 1 s to 60 s. Five failures within 10 minutes stop the node (`EdgeError::Task`). A task that returns `Ok` is finished. `Restarts` gives each task's recent failures to the status page.
- **`update`:** signed packages, the update journal, and rollback. A pending update copies the store before its first open; a failed migration or three starts that never reach ready roll it back, store first. See the binary's README for the commands.
- **`web`:** the web app, from the edge's own origin (#76).
  - `build.rs` embeds `apps/client/dist` (or `HEALTH_WEB_DIST`), leaving out source maps and the bundle's `runtime.json`. Without a bundle the crate still builds, and the edge serves no app.
  - The edge serves `runtime.json` (`mode: edge`, and the bundle's build from `build.json`).
  - `sw.js` is `src/web/sw.js` with the build and the shell's paths prepended. It caches exactly those paths when it installs, and never an API response.
  - Hashed `assets/` are immutable, and everything else is revalidated.
  - Any other path that is not a file, and not under a reserved prefix (`/api`, `/fhir`, `/auth`, …), gets the page.
- **`ups`:** polls a NUT server for `ups.status`. `OB` is warned about, and `LB` stops the node. An unreachable NUT server is warned about and polled again; it never stops the node. Each poll's `Power` is left in a `PowerState` for the status page.

## Not yet

- **Event Log output on Windows:** logs go to the console and telemetry; the Event Log sink comes with the MSI.
- **Final backup on a UPS stop:** the stop drains requests and closes the store; a last backup within the UPS budget is not taken yet.
- **Blobs:** the edge has no blob store yet, so its backups hold the export only.
- **Indicators from later work:** sync lag and conflicts (Phase 5), clock skew against central, certificate and permission-snapshot expiry (#73, #74, #75), the disk-encryption check (#73) and active sessions (#75). The report lists them as `pending`.
- **Spans offline:** spans still go straight to `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` and are lost while it is unreachable; only log lines are buffered.
- **HTTPS to central:** the shipper speaks plain HTTP to the in-country collector, like the span exporter; mutual TLS with the node certificate comes with provisioning (#73).
- **A backup on demand while the node runs**, from the admin page: `facility-edge backup now` still needs the service stopped.
