# Adding a domain event

> Status: planned layout. No code exists yet; the event naming and versioning convention is to be fixed by the scaffold.

## When to use

A Module needs to tell other contexts that something happened (e.g. MedicationDispense completed → stock movement and charge). Events are the only way domains reach Workflow, Finance, Supply Chain and Analytics Read Models.

## Steps

1. **Define the event** in the publishing context's `health-<context>-contract` crate (create it if this is the context's first published item). Carry ids and the facts subscribers need, not copies of whole clinical records.
2. **Emit it** from the pure domain function, which returns `(new state, Vec<Event>)`. Domains never publish directly.
3. **Append it** in the application use case to the outbox, in the same unit of work as the state change.
4. **Subscribe** in the consuming context's application crate. `health-messaging` dispatches in-process after commit. Workflow consumes through `workflow_inbox`, one transaction per event, deduplicated by event id.
5. **Derived records** created by a subscriber use UUIDv5 ids `{source record id or op_id}|{derivation rule canonical URL}`, so replays converge ([identifiers.md](../architecture/identifiers.md)).
6. **Cross-Deployable** effects (e.g. edge → central) go through sync and sagas, never a distributed transaction (ADR 0004).
7. **UI hints:** if the client should refresh, publish an invalidation hint (resource type and id, or worklist key) on `EventSource`; hints never carry data.
8. Keep personal health data out of any telemetry the event produces (ADR 0047).

## Expected files

- `crates/contracts/health-<context>-contract/src/…` (event type)
- `crates/domains/health-<module>/src/…` (emission)
- `crates/application/health-<context>-app/src/…` (outbox append; subscriber in the consuming context)
- `crates/infrastructure/health-messaging/…` (only if dispatch registration needs it)

## Contracts

The event type; the `health-persistence` outbox append; the `health-messaging` subscription; for Workflow, the inbox.

## Tests

- **Unit and domain:** the domain function returns the event.
- **Repository / integration:** event is in the outbox iff the transaction committed; replays are idempotent.
- **Workflow** scenarios if a definition reacts to it.
- **Offline and sync** if the effect crosses nodes.

## Governing decisions

[ADR 0002](../adr/0002-workflow-owns-tasks.md), [ADR 0004](../adr/0004-transactions-within-one-deployable.md), [ADR 0044](../adr/0044-workflow-instances-history-and-timers-at-the-authority.md), [dependency-rules.md § Ports, adapters, transactions and events](../architecture/dependency-rules.md#ports-adapters-transactions-and-events).
