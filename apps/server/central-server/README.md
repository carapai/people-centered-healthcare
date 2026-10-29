# central-server

Layer: **runtime**. Central server composition root (ADR 0001; service-catalogue.md). It runs one process over the shared `health-http` router, with a separate SurrealDB server.

## Configuration

| Variable | Default | |
|---|---|---|
| `HEALTH_BIND` | `0.0.0.0:8080` | Listen address |
| `HEALTH_SURREALDB_ENDPOINT` | required | e.g. `ws://surrealdb:8000` |
| `HEALTH_SURREALDB_USER` | `root` | |
| `HEALTH_SURREALDB_PASSWORD_FILE` | required | A file holding the password; the password is never read from the environment |
| `HEALTH_TENANT` | `ug` | SurrealDB namespace |
| `HEALTH_DATABASE` | `main` | |
| `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` | none | The in-country collector; without it only logs are written |
| `HEALTH_LOG` | `info` | `EnvFilter` directive |

## Startup

1. Telemetry starts through the allowlist.
2. The listener starts at once: `/health` answers, and `/ready` is 503 until the database is ready.
3. The database is connected and migrated in the background, with capped exponential backoff (0.5 s to 30 s). Only an error kind is logged, never the message.

SIGTERM or Ctrl-C shuts the server down gracefully.
