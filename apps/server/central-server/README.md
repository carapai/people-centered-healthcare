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
| `HEALTH_AUTH_ISSUER` | none | The authorization server's issuer URL, e.g. `https://central.ug/auth`; without it nothing is mounted at `/auth` |
| `HEALTH_AUTH_KEYS_FILE` | required with an issuer | Signing keys, one hex-encoded 32-byte P-256 secret per line, the active key first. Keep a rotated-out key on a later line until its tokens have expired |
| `HEALTH_AUTH_PEPPER_FILE` | none | The Argon2id pepper |
| `HEALTH_AUTH_CLIENT_ID` | `health-client` | The web client |
| `HEALTH_AUTH_REDIRECT_URIS` | required with an issuer | The web client's redirect URIs, comma-separated |
| `HEALTH_AUTH_LOGIN_PAGE` | none | The web client's sign-in page, e.g. `/sign-in`; without it the authorization server shows its own minimal form |

## Startup

1. Telemetry starts through the allowlist.
2. The listener starts at once: `/health` answers, and `/ready` is 503 until the database is ready.
3. The database is connected and migrated in the background, with capped exponential backoff (0.5 s to 30 s). Only an error kind is logged, never the message.
4. With an issuer configured, the authorization server (`health-auth` over `SurrealAuthStore`) is built once the database is ready. `/auth` answers 503 until then.

## Sign-in

- **Routes:** `/auth` serves discovery, `/jwks`, `/authorize`, `/token` and `/revoke` (offline-auth.md). `/api/session` is the first authorized route: it answers 401 without a valid access token, and the signed-in principal with one.
- **Key rotation:** add a new key as the first line of the keys file and keep the old one below it until its tokens have expired. Both keys are published in `/jwks`, and tokens signed with the old key still verify.
- **Key storage:** keys and the pepper are read from secret files. Holding them in central KMS is part of central's key custody ([#138](https://github.com/carapai/people-centered-healthcare/issues/138)).
- **The FHIR routes** mount behind `AccessVerifier` once the policy decision point is wired per request ([#192](https://github.com/carapai/people-centered-healthcare/issues/192)).

SIGTERM or Ctrl-C shuts the server down gracefully.

## OpenAPI

`central-server openapi` prints central's OpenAPI document and exits, without reading any configuration. `tools/codegen` generates `@health/api-client` from it (`bun run generate`).
