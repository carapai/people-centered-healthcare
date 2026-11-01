# facility-edge

Layer: **runtime**. The Facility Edge Node's composition root (ADR 0007, [facility-edge.md](../../../docs/architecture/facility-edge.md)): one OS service, one process, embedding SurrealDB. [`health-edge-runtime`](../../../crates/edge/health-edge-runtime/README.md) runs the node.

Deploying an edge to a facility waits for the power-loss spike, [Does embedded SurrealDB RocksDB survive hard power loss on facility hardware?](https://github.com/carapai/people-centered-healthcare/issues/34) (#34). The binary is built on the embedded adapter now. If the spike fails, only the engine settings change.

## Modes

- `facility-edge`: foreground, for development in a terminal. Ctrl-C stops it.
- `facility-edge --service`: under the service manager. On Linux this is systemd, `Type=notify`, with [`packaging/facility-edge.service`](packaging/facility-edge.service). On Windows it is the Service Control Manager: [`packaging/install-service.ps1`](packaging/install-service.ps1) registers the service, its environment and the failure actions (restart three times, then stay stopped; the count resets after 24 hours).
- `facility-edge update …`: signed updates (below).
- `facility-edge backup …`: backups on demand, to USB, verification and restore (below).

## Configuration

| Variable | Default | |
|---|---|---|
| `HEALTH_DATA_DIR` | required | The embedded store's directory. This process owns it alone |
| `HEALTH_BIND` | `0.0.0.0:8080` | Listen address |
| `HEALTH_TENANT` | `ug` | SurrealDB namespace |
| `HEALTH_DATABASE` | `main` | |
| `HEALTH_UPS` | none | The NUT UPS to watch, `name@host[:port]` (port 3493 by default). Without it, durability rests on `sync=every` |
| `HEALTH_UPS_POLL_SECS` | `5` | |
| `HEALTH_DRAIN_SECS` | `20` | How long in-flight requests may take to finish on a stop |
| `HEALTH_UPDATE_DIR` | `updates` beside `HEALTH_DATA_DIR` | Staged packages, the previous binary and the pre-update store copy. Keep it on the store's volume |
| `HEALTH_UPDATE_KEY_FILE` | required to stage | The pinned release key: a P-256 public key, hex SEC1 |
| `HEALTH_NODE_ID` | `facility-edge` | The node name recorded in its backups |
| `HEALTH_BACKUP_DIR` | none | Backups, on the second volume. Without it the node takes none and warns at every start |
| `HEALTH_BACKUP_KEY_FILE` | required with `HEALTH_BACKUP_DIR` | The node's backup key, 64 hex characters. Escrow at central is #138 |
| `HEALTH_BACKUP_INTERVAL_SECS` | `3600` | |
| `HEALTH_BACKUP_TEST_SECS` | `604800` | How often the newest backup is test-restored |
| `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` | none | Without it only logs are written |
| `HEALTH_LOG` | `info` | `EnvFilter` directive |
| `SURREAL_ROCKSDB_BLOCK_CACHE_SIZE` | set by the installer | The RocksDB block cache cap, in bytes (about 25% of RAM) |

## Web app

The binary embeds the client and serves it, with its `runtime.json` and app-shell service worker, from its own origin (#76). Build the client first: `bun run build` in `apps/client`, then `cargo build -p facility-edge`. A binary built without `apps/client/dist` serves only its API. `bun run test:e2e:edge` in `apps/client` runs the browser suite against the built binary.

## Exit codes

| Code | Meaning | systemd |
|---|---|---|
| 0 | Stopped on request or on UPS low battery | not restarted |
| 1 | The listener, telemetry or serving failed | restarted |
| 2 | The configuration is missing or malformed | not restarted (`RestartPreventExitStatus=2`) |
| 3 | The store cannot be opened: another process holds it, or the disk failed | restarted |
| 4 | A migration failed, or a newer binary migrated the store | restarted; the update rollback (part 2) acts on it |
| 5 | A supervised task kept failing | restarted |
| 6 | A failed update was rolled back | restarted, which runs the previous binary |
| 7 | An update step failed, or an `update` command was refused | not restarted |
| 8 | A failed update's store is restored, and the binary is left for the service manager | not restarted; `OnFailure=` runs `packaging/facility-edge-rollback.service` |
| 9 | A `backup` command failed | (commands only) |

The first start and three restarts are allowed within 24 hours. After that the unit stays stopped.

On Windows a non-zero code is reported to the SCM as a service-specific error, and the failure actions restart it whatever the code. Exits 2, 7 and 8 therefore retry three times before the service stays stopped. The service runs as LocalSystem and can replace its own binary, so a rollback finishes in the process (exit 6), and the rollback unit's step is not needed. Long migrations and drains advance the SCM checkpoint with a wait hint, so the SCM keeps waiting.

## Updates

A package is a gzipped tar holding `manifest.json` (`version`, `binary`, `sha256`) and the binary. It comes with a detached ECDSA P-256 signature over the whole package, as hex `r || s`. `health_edge_runtime::update::pack` builds one; the release job signs it.

1. `facility-edge update stage <package> <signature>`: verifies the signature against the pinned key and the binary against its digest, then unpacks into `staged/`. The node may keep running.
2. `facility-edge update apply`, with the service stopped (`systemctl stop`): keeps this binary in `previous/`, swaps in the staged one, and journals the update as pending. Then start the service.
3. **The new binary's first start** copies the store to `pre-update-store/` before opening it, then migrates. Ready commits the update.
4. **A failed update rolls back** when a migration fails, or after three starts that never reached ready:
   - the store comes back first (the failed one is kept in `failed-store/`), then the previous binary, so an old binary never runs against a migrated store;
   - the sandboxed service may not write `/opt/health/bin`, so it restores the store and exits 8;
   - `facility-edge-rollback.service` then runs `previous/facility-edge update rollback /opt/health/bin/facility-edge` as root and starts the service again.
5. `facility-edge update status` prints the journal and anything staged.

Updates install in a maintenance window, or when an admin approves with no active sessions. That decision belongs to whoever runs `apply`; the session check comes with the local authorization server (#75), and pulling packages from central comes with sync.

## Backups

The running node takes a backup every hour and test-restores the newest one every week (`health_edge_runtime::backup`). Both are recorded in `status.json` under `HEALTH_BACKUP_DIR`. A failure is logged as an alert and never stops the node. The pre-update backup is the store copy an update takes.

- `facility-edge backup now`: takes a backup. The node owns its store, so stop the service first.
- `facility-edge backup copy <dir>`: copies the newest backup and its blobs to `dir` (a USB drive), then verifies the copy. The node may keep running.
- `facility-edge backup verify`: checks every backup's checksums, decryption and blobs.
- `facility-edge backup test`: the weekly test restore, now.
- `facility-edge backup restore <backup>`: with the service stopped and the old data directory moved aside, restores `backup` into a new store at `HEALTH_DATA_DIR`. It refuses a directory that holds anything.
- `facility-edge backup status`: prints `status.json` and the backups kept.

A backup on demand while the node runs comes with the admin page (#77).

## Tests

`tests/service.rs` runs the built binary (Unix):
- it reports ready after migrating, and stops on SIGTERM with exit 0;
- it recovers after SIGKILL, and a second process cannot open its store;
- under systemd it sends `EXTEND_TIMEOUT_USEC`, `READY=1`, `WATCHDOG=1` and `STOPPING=1`;
- a UPS low-battery signal stops it gracefully;
- an old binary refuses a store that a newer one migrated (exit 4).

`tests/update.rs` installs the binary in a temporary directory and updates it:
- a package signed by another key is refused, and a signed one is staged, applied and committed once ready;
- a failed migration exits 4, and the next start rolls back and exits 6;
- a release that crashes before ready is rolled back by the previous binary's `update rollback`.

`tests/backup.rs` runs the built binary with a backup directory:
- the node takes its first backup and test restore once ready;
- `backup now` is refused while the node runs, and works once it stops;
- `backup verify`, `backup copy` to a USB directory, and `backup restore` into a fresh store that then starts and reports ready;
- a corrupt backup fails `backup test`, which records the failure for the alert, and `backup verify`.
