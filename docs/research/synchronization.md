# Research: synchronization

A condensed version of the research ticket [What sync designs suit central / edge / desktop hierarchy for clinical data?](https://github.com/carapai/people-centered-healthcare/issues/4). The full findings, with citations, are on the unmerged branch `research/synchronization`. Decisions: [docs/architecture/synchronization.md](../architecture/synchronization.md) (ADRs 0019, 0020, 0021) and [write-authority.md](../architecture/write-authority.md) (ADR 0012).

## Prior art

| System | What we took | What we left |
|---|---|---|
| CouchDB replication | Checkpoints on both sides; per-document bulk results; keeping losing revisions; the document goes before its attachments | Deterministic silent winner; `_revs_diff` negotiation |
| DHIS2 Android SDK | Per-record sync state machine; pulled data never overwrites unsynced records; capture scope; reserved values | SMS sync |
| OpenSRP / Android FHIR SDK | Location-based sync strategy | `_lastUpdated` cursors; AcceptLocal/AcceptRemote resolution that cannot defer to a human |
| OpenMRS Sync 2.0 | Multi-level parent/child validated in practice | Global newest-wins merge |
| FHIR Bulk Data | Snapshot seeding with a transaction-time cursor and a deleted list | As an incremental protocol |
| FHIR R5 Subscriptions | Wake-up hints only | As a source of truth (delivery is best-effort) |
| FHIR R5 REST | `If-Match` / versionId semantics; batch vs transaction | Clock-based `_since` pulling |
| ElectricSQL | Confirms that write-path sync is application-specific | — |
| CRDTs | — | Convergence is not clinical correctness |

## Key facts

- **Cursors.** CouchDB sequence ids are opaque and increasing. Clock-based cursors (`_lastUpdated`, `_since`) are inclusive at the boundary and break under clock skew.
- **HLC.** It orders events and stays close to NTP time, but `hlc.e < hlc.f` does not imply happened-before, so it cannot detect concurrent edits. Only version vectors can.
- **Version vectors.** Dotted version vectors grow with the number of servers that write a given item. They are not justified when each record has a single Authority Node.
- **Outbox.** The transactional outbox gives at-least-once delivery, so consumers must be idempotent. The `Idempotency-Key` header draft (draft-08, 2026-10-22) suggests 422 for key reuse with a different payload.
- **Bulk Data `_since`.** It populates a `deleted` array, and deleted resources do not appear in the output.
- **Android FHIR SDK.** Its bundle upload mode requires squashing local changes into one patch per resource.

## Risks carried into the design

- **Clock skew:** never use timestamps as cursors.
- **Conflict backlog after long outages:** needs review queues and ageing alerts.
- **Tombstone horizon vs. multi-month outages:** re-seeding over poor links is expensive, hence 90-day supported windows and a 180-day horizon.
- **A desktop switching upstream with pending operations forks history:** desktops are pinned and drain first.
- **Atomic resource, change-log and outbox write:** confirmed possible in one SurrealDB transaction (docs/research/surrealdb.md).

## Open items

- Throughput of the per-node commit counter on edge hardware.
- An SMS fallback is out of scope.
