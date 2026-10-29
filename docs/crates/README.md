# Crate documentation

This folder holds one page per workspace crate, written when the crate gets real content. Crates are created lazily, when the first implementation task needs them ([crate-catalogue.md](../architecture/crate-catalogue.md)).

## Current members

The workspace scaffold created one placeholder crate per layer, so that the layer rules can be checked from the start. Each has only a crate README until its task fills it:

| Crate | Layer | Filled by |
|---|---|---|
| `health-identifiers` | foundation | Built (#52): typed ids, UUIDv5 derivation and number formats; see its [README](../../crates/foundation/health-identifiers/README.md) |
| `health-errors` | foundation | Implement typed identifiers and node-prefixed sequences (#52) |
| `health-persistence` | contracts | Built (#54); see its [README](../../crates/contracts/health-persistence/README.md) |
| `health-time` | foundation | Built (#54): `jiff` timestamps and the `Clock` port |
| `health-persistence-memory` | testing | Built (#54): the in-memory store for tests |
| `health-client-registry` | domain (registries) | Implement the matching engine and Patient/$match (#69) |
| `health-registries-app` | application (registries) | The first Registries use case |
| `health-surrealdb` | infrastructure | Connections and migrations built (#55); the unit of work comes with #56. See its [README](../../crates/infrastructure/health-surrealdb/README.md) |
| `central-server` | runtime | Create central-server with the shared Axum router and telemetry (#49) |
| `architecture-checks` | tooling | Built (#43); see [tools/architecture-checks](../../tools/architecture-checks/README.md) |

## What a crate page covers

- **Layer and context**, as declared in `[package.metadata.health]` ([dependency-rules.md](../architecture/dependency-rules.md)).
- **Purpose** and the Module or Bounded Context it serves ([resource-ownership.md](../architecture/resource-ownership.md)).
- **Public API**: what `lib.rs` re-exports, and the ports it defines or implements.
- **Dependencies** it is allowed to take, and any allowlist entries (domain crates only).
- **Tables** it owns, for repository crates ([persistence.md](../architecture/persistence.md)).
- **Tests**: the suites from [testing.md](../architecture/testing.md) that cover it.
- **Modules that may split out** later (the catalogue's "Starts as" column).

## Naming

Pages are named after the crate, e.g. `health-fhir-models.md`, `health-clinical-records-app.md`. A crate that starts as a module inside a host crate is documented in the host's page until it is split out.

## Where to start

- The full list of planned crates and their layers: [crate-catalogue.md](../architecture/crate-catalogue.md).
- The layering rules and their enforcement: [dependency-rules.md](../architecture/dependency-rules.md).
- How to add one: [docs/development/adding-a-crate.md](../development/adding-a-crate.md).
