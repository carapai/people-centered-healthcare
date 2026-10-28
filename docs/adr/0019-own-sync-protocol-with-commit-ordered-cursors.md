# Sync uses its own protocol, with commit-ordered sequence cursors and a transactional outbox

Every hop (desktop ↔ edge, edge ↔ central, standalone desktop ↔ central) uses one versioned sync API, `/sync/v1/…`, implemented once: the wire types and the shared client and apply logic live in `health-sync-protocol`, used by `health-sync-server` at central and by the edge and desktop sync modules.
- **Items.** Each item is a full FHIR resource version, which carries its parent version, or a ledger operation (`kind: op`). Every item travels in a sync envelope.
- **Change log.** Each node appends every accepted write to a `change_log` with a strictly increasing `seq`. The `seq` comes from a single counter record written inside the write transaction. Concurrent writers therefore conflict and serialize, so `seq` order equals commit order.
- **Cursors** are opaque `(node_id, epoch, seq)`, checkpointed on both sides of each peer and direction. `epoch` changes when a node is restored from backup.
- **Outbox.** Outgoing items are written to an outbox in the same transaction as the change. They are delivered at least once, and the receiver's `sync_receipt` table, keyed by `op_id`, makes the effect exactly-once.
- **Push** sends groups (one group = one local transaction) that apply all-or-nothing. Results come back per group.
- **Pull** follows the cursor within a server-computed Sync Scope. A new or too-stale node starts from a snapshot seed.

We chose this for three reasons:
- Clock-based cursors (`_lastUpdated`, `_since`, SurrealDB change-feed versionstamps) skip or duplicate changes under clock skew or restarts.
- A sequence taken from a non-transactional counter can expose a later number before an earlier one commits.
- Plain FHIR REST has no per-item sync status, no cursors and no handshake.

## Considered Options

- **FHIR REST with `_history?_since` pulls and `batch` pushes** (the Android FHIR SDK and OpenSRP pattern). Rejected: its cursors are clock-based and inclusive at the boundary, and it has nowhere to carry item state, provenance or compatibility checks. FHIR REST remains the API for apps and external clients.
- **SurrealDB change feeds as the change log.** Rejected: versionstamps reset on restart (#7606).
- **R5 topic-based Subscriptions as the transport.** Rejected: delivery is best-effort. A server-sent-events channel is used only as a wake-up hint.
- **Version vectors per resource.** Rejected: the tree topology with one Authority Node per record (ADR 0012) needs only a parent-version check. A hybrid logical clock is recorded for ordering and audit, never for deciding conflicts.

## Consequences

- All writes on a node serialize on the counter record. This is acceptable at facility write rates but must be measured on edge hardware.
- Receipts, tombstones and history are kept on the upstream for at least 180 days. Supported offline windows are 90 days (edge → central) and 30 days (desktop → edge). A node beyond the horizon re-seeds, keeping its unsent outbox and re-pushing it as Candidate Versions.
