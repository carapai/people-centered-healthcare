# Service documentation

This folder will hold one page per **Deployable**: the runnable processes the platform ships. No service exists yet.

## Deployables

Under ADR 0001 (modular monolith first) there are three Deployables. The spec's `apps/server/*` services are modules of `central-server` until a real need justifies extracting one.

| Deployable | Planned page | Decided in |
|---|---|---|
| `central-server` | `central-server.md` | [resource-ownership.md § Deployables](../architecture/resource-ownership.md#deployables), [deployment.md](../architecture/deployment.md) |
| `facility-edge` | `facility-edge.md` | [facility-edge.md](../architecture/facility-edge.md), [backup-and-recovery.md](../architecture/backup-and-recovery.md) |
| Tauri desktop host | `desktop.md` | [desktop-client.md](../architecture/desktop-client.md) |

How the spec's service names map onto these is in [service-catalogue.md](../architecture/service-catalogue.md).

## What a service page covers

- Which Bounded Contexts and crates it links, and which runtime crate composes them ([crate-catalogue.md](../architecture/crate-catalogue.md)).
- Its process model, storage engine and durability settings ([persistence.md](../architecture/persistence.md)).
- Configuration keys and secrets, and where secrets are held.
- Health endpoints, alerts and telemetry ([deployment.md § Observability](../architecture/deployment.md#observability-adr-0047)).
- Install, update and rollback procedure, and its build artifacts ([deployment.md § Build matrix](../architecture/deployment.md#build-matrix-114)).
- Backup and recovery runbook links.
