# health-edge-runtime

Layer: **runtime**. The Facility Edge Node's runtime ([facility-edge.md](../../../docs/architecture/facility-edge.md), ADR 0007). It is used by [`apps/server/facility-edge`](../../../apps/server/facility-edge/README.md).

## `run`

`run(config, manager, stop)` is the node's whole life:
1. **Listen.** The listener starts at once: `/health` answers, `/ready` is 503 until the store is ready (the shared `health-http` system routes), and the embedded web app is served (`web`).
2. **Update check.** A pending update copies the store to `pre-update-store/` on its first start, or rolls back after three starts that never reached ready (`update::before_open`), before the store is opened.
3. **Open the store.** The embedded store (`Target::RocksDb`, `sync=every`) is opened once. If it fails, `run` returns `EdgeError::Store` and the service manager restarts the process.
4. **Migrate.** Migrations run before ready. The service manager is asked for 30 s more every 10 s while they run. A store that a newer binary migrated is refused (`EdgeError::Migration`), so an old binary never runs against it. During an update, a failed migration marks the update failed and the next start rolls it back.
5. **Ready.** A pending update is committed. The manager is told, and the supervised tasks start: the watchdog keep-alive, the UPS watch and the backup schedule.
6. **Stop.** On `stop`, a UPS low battery, or a task that keeps failing:
   - `/ready` turns 503 (the `serving` check);
   - the manager is told;
   - in-flight requests drain within `HEALTH_DRAIN_SECS`;
   - the store closes.

## Modules

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
- **`supervisor`:** a failed or panicking task restarts with exponential backoff, from 1 s to 60 s. Five failures within 10 minutes stop the node (`EdgeError::Task`). A task that returns `Ok` is finished.
- **`update`:** signed packages, the update journal, and rollback. A pending update copies the store before its first open; a failed migration or three starts that never reach ready roll it back, store first. See the binary's README for the commands.
- **`web`:** the web app, from the edge's own origin (#76).
  - `build.rs` embeds `apps/client/dist` (or `HEALTH_WEB_DIST`), leaving out source maps and the bundle's `runtime.json`. Without a bundle the crate still builds, and the edge serves no app.
  - The edge serves `runtime.json` (`mode: edge`, and the bundle's build from `build.json`).
  - `sw.js` is `src/web/sw.js` with the build and the shell's paths prepended. It caches exactly those paths when it installs, and never an API response.
  - Hashed `assets/` are immutable, and everything else is revalidated.
  - Any other path that is not a file, and not under a reserved prefix (`/api`, `/fhir`, `/auth`, …), gets the page.
- **`ups`:** polls a NUT server for `ups.status`. `OB` is warned about, and `LB` stops the node. An unreachable NUT server is warned about and polled again; it never stops the node.

## Not yet

- **Event Log output on Windows:** logs go to the console and telemetry; the Event Log sink comes with the MSI.
- **Final backup on a UPS stop:** the stop drains requests and closes the store; a last backup within the UPS budget is not taken yet.
- **Blobs:** the edge has no blob store yet, so its backups hold the export only.
- **Alerts for UPS and task restarts:** with the admin page (#77).
