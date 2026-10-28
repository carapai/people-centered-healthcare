# Adding a feature

> Status: planned layout. No code exists yet; paths are confirmed when the scaffold lands.

## When to use

A user-visible capability that crosses the stack: a business rule, a use case, an API and a screen. Use the narrower guides for each piece; this one gives the order.

## Steps

1. **Find the Owner.** Look up every record the feature writes in [resource-ownership.md](../architecture/resource-ownership.md) and its Authority Node in [write-authority.md](../architecture/write-authority.md). If a write spans Owners, it is one unit of work inside one Deployable (ADR 0004).
2. **Domain rule** in the Owner's domain crate: pure function returning new state and events ([adding-a-domain.md](adding-a-domain.md)).
3. **Contract**: events, DTOs or typed Workflow actions in `health-<context>-contract` ([adding-an-event.md](adding-an-event.md)).
4. **Use case** in `health-<context>-app`: open the unit of work, call the Owner's repositories, append events and sync ops to the outbox.
5. **Storage**: new tables, fields or indexes through a migration ([adding-a-migration.md](adding-a-migration.md)).
6. **API**: a route in `health-http` (command) or `health-fhir-server` (FHIR REST) with OpenAPI ([adding-an-api.md](adding-an-api.md)). The same route serves central, the edge and the desktop over IPC.
7. **Client**: a method on the context's command port, then a feature module that uses it ([adding-a-ui-component.md](adding-a-ui-component.md)).
8. **Gating**: show the feature by permission-snapshot claims, `Platform.capabilities` and configuration (active services and programs), never by runtime checks ([client-application.md](../architecture/client-application.md)).
9. **Offline**: if the feature writes on edge or desktop, check its sync class ([adding-a-sync-resource.md](adding-a-sync-resource.md)) and what the desktop may do offline ([desktop-client.md](../architecture/desktop-client.md)).
10. **Configuration**: anything that varies by country, district or facility goes through declared override points, never code ([ADR 0026](../adr/0026-overrides-only-at-declared-configuration-override-points.md)).

## Expected files

- `crates/domains/health-<module>/src/…`
- `crates/contracts/health-<context>-contract/src/…`
- `crates/application/health-<context>-app/src/…`
- `crates/infrastructure/health-surrealdb/…` (repository, migration)
- `crates/infrastructure/health-http/…` or `crates/infrastructure/health-fhir-server/…`
- Generated OpenAPI document and TypeScript/Zod types (committed; location to be fixed by the scaffold)
- `apps/client/src/…` feature module (directory convention to be fixed by the scaffold)

## Contracts

Domain ports, the `health-persistence` unit of work, context events and DTOs, the OpenAPI document, the TypeScript command port.

## Tests

Unit and domain; Repository; API contract (HTTP and IPC envelopes identical); Frontend (Vitest, component, axe); Security (authorization matrix); and, for offline-capable writes, Offline and sync. End-to-end coverage comes from the §117–119 acceptance scenarios. See [testing.md](../architecture/testing.md).

## Governing decisions

[ADR 0002](../adr/0002-workflow-owns-tasks.md), [ADR 0004](../adr/0004-transactions-within-one-deployable.md), [ADR 0036](../adr/0036-ports-over-swappable-transport-with-tauri-ipc-into-the-shared-router.md), [dependency-rules.md](../architecture/dependency-rules.md).
