# Adding a resource to sync

> Status: planned layout. No code exists yet; `health-sync-protocol` and the Sync Scope definition are created by the scaffold.

## When to use

A resource type or ledger that is written on more than one node, or replicated down from central. Writes enter the `change_log` and outbox automatically through the unit of work; what you add is the **policy**.

## Steps

1. **Authority row** in [write-authority.md](../architecture/write-authority.md): scope (global or facility), allowed creators, allowed offline changes, Authority Node, direction, conflict strategy, central validation.
2. **Transfer kind:** `version` (a FHIR resource version, `op_id` = `meta.versionId`) or `op` (an append-only ledger operation).
3. **Conflict class** from the per-class table in [synchronization.md](../architecture/synchronization.md#per-class-policy-89): e.g. version-safe correction (clinical), immutable after signing (signed notes), allowed-transition (Encounter, Task, workflow), operation (stock, finance), registry review (Patient), revocation wins (devices). A type that fits no class needs a decision before it syncs.
4. **Commuting transitions:** for workflow-bearing types, list which transitions commute so they auto-apply at the authority.
5. **Sync Scope:** add the type to the server-side scope definition for each node kind (edge from central, desktop from edge, mode-B desktop).
6. **Priority lane:** revocations/snapshots, metadata and packages, clinical data and ledgers, audit, blobs.
7. **Validation on ingest:** the authority re-runs validation (profile, references, authorization at action time, invariants). Failures become `rejected` with a reason code, never a silent drop.
8. **Attachments:** reference blobs by hash; upload follows the resource; 50 MiB cap.
9. **Tombstones and retention:** deletes are versions with `deleted: true`; upstream keeps tombstones, receipts and non-head history ≥ 180 days.

## Expected files

- The Sync Scope definition and per-class policy registration (location to be fixed by the scaffold; shared apply logic lives in `crates/contracts/health-sync-protocol/`)
- `crates/infrastructure/health-sync-server/…` (central) and the edge/desktop sync modules, only if new wire handling is needed

## Contracts

The sync envelope (`kind`, `op_id`, `tx_id`, `parent_version_id`, `hlc`, `content_hash`, `packages`, `snapshot_version`, `provisional`, `deleted`, `blobs`, `body`); item states (`pending`, `sending`, `accepted`, `conflict`, `rejected`, `held`); Conflict Record; Reconciliation Record; `sync_receipt`.

## Tests

- **Offline and sync** (nightly): multi-node simulation with partitions, clock skew, long outages, `reseed_required`, restore and epoch change.
- **Recovery:** crash mid-push and mid-pull resume from receipts and checkpoints.
- **Stock concurrency** or **Payment reconciliation** for ledger classes.
- **Security:** facility isolation of the scope.

## Governing decisions

[ADR 0012](../adr/0012-authority-node-follows-record-scope.md), [ADR 0018](../adr/0018-version-id-is-the-op-id.md), [ADR 0019](../adr/0019-own-sync-protocol-with-commit-ordered-cursors.md), [ADR 0020](../adr/0020-whole-version-conflicts-without-merge.md), [ADR 0021](../adr/0021-content-addressed-blob-store-for-attachments.md), [synchronization.md](../architecture/synchronization.md).
