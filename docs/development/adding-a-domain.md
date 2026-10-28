# Adding a domain (Module)

> Status: planned layout. No code exists yet; paths are confirmed when the scaffold lands.

## When to use

A new **Module** inside a Bounded Context, i.e. a new Owner of resources or records. Every Module already planned is listed in [resource-ownership.md](../architecture/resource-ownership.md) and [crate-catalogue.md](../architecture/crate-catalogue.md). A Module that is not there needs an ownership decision first: the matrix change is reviewed like an ADR (CODEOWNERS covers `docs/architecture/`).

## Steps

1. Record ownership: add the Module and the resources it owns to [resource-ownership.md](../architecture/resource-ownership.md), and its authority, sync direction and conflict strategy to [write-authority.md](../architecture/write-authority.md).
2. Decide whether it starts as a crate or as a module inside a host crate (crate-catalogue "Starts as"). Split it out only when a second consumer or a different dependency set appears.
3. Create the domain crate (see [adding-a-crate.md](adding-a-crate.md)) with `layer = "domain"` and the context tag.
4. Write pure domain functions: input state + command → new state + `Vec<Event>`. No I/O.
5. Declare driven ports in the domain crate: its repository port (transaction-agnostic) and a port for anything it needs from another Module.
6. Add or extend the context's application crate `health-<context>-app` with use cases that open a unit of work, bind repositories and append events to the outbox. Implement cross-Module ports here (anti-corruption seam).
7. If the Module publishes events, typed Workflow actions or cross-context DTOs, add them to `health-<context>-contract` ([adding-an-event.md](adding-an-event.md)).
8. Implement the repository in `health-surrealdb` with the tables it owns ([adding-a-migration.md](adding-a-migration.md)), declaring those tables for the table-ownership test.
9. Register the Module's records in the in-code ownership registry, which is doc-synced against resource-ownership.md.
10. Wire it into each Deployable that hosts the context (central always; edge and desktop per the Deployables table).

## Expected files

- `crates/domains/health-<module>/Cargo.toml`, `src/lib.rs`, `src/ports.rs` (port file layout to be fixed by the scaffold)
- `crates/application/health-<context>-app/src/…` (use cases)
- `crates/contracts/health-<context>-contract/src/…` (only when it publishes something)
- `crates/infrastructure/health-surrealdb/…` (repository, migrations)
- `crates/testing/health-test-support/…` (in-memory fakes of the new ports)
- `apps/server/central-server`, `crates/edge/health-edge-runtime`, `crates/desktop/health-desktop-runtime` (wiring only)

## Contracts

- Repository port and cross-Module ports (native `impl Future + Send`, no `async-trait`).
- `UnitOfWork` / `Transaction` and outbox traits from `health-persistence`.
- Events and DTOs in `health-<context>-contract`.

## Tests

- **Unit and domain** suite for every rule; at least 80% line coverage on the domain and application crates.
- **Repository** suite for the SurrealDB adapter, including guard-record conflicts and `EXPLAIN` plans.
- **Security** authorization-matrix cases for any new permission.
- `tools/architecture-checks` passes (no domain→domain edge, allowlisted externals only).

See [testing.md](../architecture/testing.md).

## Governing decisions

[ADR 0001](../adr/0001-modular-monolith-first.md), [ADR 0004](../adr/0004-transactions-within-one-deployable.md), [ADR 0005](../adr/0005-six-layer-crate-stack.md), [ADR 0006](../adr/0006-pure-isolated-domain-crates.md), [dependency-rules.md](../architecture/dependency-rules.md).
