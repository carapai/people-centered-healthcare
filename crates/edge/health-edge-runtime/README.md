# health-edge-runtime

Layer: **runtime**. The Facility Edge Node's runtime ([facility-edge.md](../../../docs/architecture/facility-edge.md), ADR 0007). It is used by [`apps/server/facility-edge`](../../../apps/server/facility-edge/README.md).

## `run`

`run(config, manager, stop)` is the node's whole life:
1. **Listen.** The listener starts at once: `/health` answers, and `/ready` is 503 until the store is ready (the shared `health-http` system routes).
2. **Open the store.** The embedded store (`Target::RocksDb`, `sync=every`) is opened once. If it fails, `run` returns `EdgeError::Store` and the service manager restarts the process.
3. **Migrate.** Migrations run before ready. The service manager is asked for 30 s more every 10 s while they run. A store that a newer binary migrated is refused (`EdgeError::Migration`), so an old binary never runs against it.
4. **Ready.** The manager is told, and the supervised tasks start: the watchdog keep-alive and the UPS watch.
5. **Stop.** On `stop`, a UPS low battery, or a task that keeps failing:
   - `/ready` turns 503 (the `serving` check);
   - the manager is told;
   - in-flight requests drain within `HEALTH_DRAIN_SECS`;
   - the store closes.

## Modules

- **`config`:** `EdgeConfig::from_lookup` reads the service environment. Errors name the variable, never its value.
- **`notify`:** the `ServiceManager` port.
  - `Foreground` is the no-op for development.
  - `Systemd` speaks sd_notify(3) on `$NOTIFY_SOCKET`, including Linux abstract sockets. It sends `WATCHDOG=1` at half of `WATCHDOG_USEC`, when `WATCHDOG_PID` is unset or names this process.
- **`supervisor`:** a failed or panicking task restarts with exponential backoff, from 1 s to 60 s. Five failures within 10 minutes stop the node (`EdgeError::Task`). A task that returns `Ok` is finished.
- **`ups`:** polls a NUT server for `ups.status`. `OB` is warned about, and `LB` stops the node. An unreachable NUT server is warned about and polled again; it never stops the node.

## Not yet

- **Windows service, and update staging with rollback:** part 2 of [#72](https://github.com/carapai/people-centered-healthcare/issues/72).
- **Final backup on a UPS stop:** with the backup scheduler (#78).
- **Alerts for UPS and task restarts:** with the admin page (#77).
