# facility-edge

Layer: **runtime**. The Facility Edge Node's composition root (ADR 0007, [facility-edge.md](../../../docs/architecture/facility-edge.md)): one OS service, one process, embedding SurrealDB. [`health-edge-runtime`](../../../crates/edge/health-edge-runtime/README.md) runs the node.

Deploying an edge to a facility waits for the power-loss spike, [Does embedded SurrealDB RocksDB survive hard power loss on facility hardware?](https://github.com/carapai/people-centered-healthcare/issues/34) (#34). The binary is built on the embedded adapter now. If the spike fails, only the engine settings change.

## Modes

- `facility-edge`: foreground, for development in a terminal. Ctrl-C stops it.
- `facility-edge --service`: under the service manager. On Linux this is systemd, `Type=notify`, with [`packaging/facility-edge.service`](packaging/facility-edge.service). The Windows service is part 2 of [#72](https://github.com/carapai/people-centered-healthcare/issues/72).

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
| `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` | none | Without it only logs are written |
| `HEALTH_LOG` | `info` | `EnvFilter` directive |
| `SURREAL_ROCKSDB_BLOCK_CACHE_SIZE` | set by the installer | The RocksDB block cache cap, in bytes (about 25% of RAM) |

## Exit codes

| Code | Meaning | systemd |
|---|---|---|
| 0 | Stopped on request or on UPS low battery | not restarted |
| 1 | The listener, telemetry or serving failed | restarted |
| 2 | The configuration is missing or malformed | not restarted (`RestartPreventExitStatus=2`) |
| 3 | The store cannot be opened: another process holds it, or the disk failed | restarted |
| 4 | A migration failed, or a newer binary migrated the store | restarted; the update rollback (part 2) acts on it |
| 5 | A supervised task kept failing | restarted |

The first start and three restarts are allowed within 24 hours. After that the unit stays stopped.

## Tests

`tests/service.rs` runs the built binary (Unix):
- it reports ready after migrating, and stops on SIGTERM with exit 0;
- it recovers after SIGKILL, and a second process cannot open its store;
- under systemd it sends `EXTEND_TIMEOUT_USEC`, `READY=1`, `WATCHDOG=1` and `STOPPING=1`;
- a UPS low-battery signal stops it gracefully;
- an old binary refuses a store that a newer one migrated (exit 4).
