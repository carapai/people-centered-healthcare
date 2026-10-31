# health-integration

Layer: **infrastructure**. Inbound integrations: integrations, pulled feeds, and R4 adapters after conversion (ADR 0051; fhir-r5-model.md § Validation). No external payload that fails validation is lost.

## The pipeline

`Inbound::receive(integration, payload)` sends each payload one of three ways:

| Result | When | What happens |
|---|---|---|
| `Accepted` | tiers 1–3 pass and the `Sink` writes it | it is in the record |
| `DeadLettered { id, outcome }` | tier 1 fails: not JSON, or not structurally R5 | the caller gets an OperationOutcome, and the payload is kept exactly as received in the **Inbound Dead Letter** |
| `Quarantined { id, outcome }` | tier 2 (core invariants) or tier 3 (the profiles the resource claims, with `with_profiles`) fails, or the sink refuses the write (4xx: an unresolved reference, a conflict) | the resource goes to the **quarantine** review queue instead of the record |

If the sink is unavailable (5xx), nothing is kept: the caller still holds the payload and retries.

## Review

- **`pending(kind, reviewer, limit)`** lists pending entries, oldest first.
- **`replay(id, corrected, reviewer)`** validates the payload, or the reviewer's correction, from tier 1 again, then writes it. If that works, the entry is resolved. If it still fails, the entry stays pending and the reviewer gets the problems.
- **`discard(id, reason, reviewer)`** resolves an entry without writing it.

Resolved entries stay until their retention ends.

## Retention, audit and protection

- **Retention:** each `Integration` has its own retention, 90 days by default. `purge()` deletes expired entries; run it as a background job.
- **Audit:** receiving a failure, listing, replaying, discarding and purging each write an AuditEvent (`health-audit`) in the same transaction. The entry appears as `urn:health:inbound:{id}`.
- **Protection:** entries are protected at rest like the rest of the node's data, by full-disk encryption and encrypted backups (security.md). Issues record locations, codes and diagnostics, not payload values.

## Storage

The `inbound` table (`health-surrealdb` migration `0005_inbound`) is owned by the integration module, through `SurrealTx::put_inbound`, `inbound`, `inbound_pending`, `resolve_inbound` and `purge_inbound`.

## Not here

- **Sync ingest:** a sync item that fails validation at the Authority Node gets the per-item `rejected` status plus a review record (ADR 0016). That belongs with sync ingest ([#81](https://github.com/carapai/people-centered-healthcare/issues/81)).
- **Tier 4:** terminology validation joins with the terminology tasks.
- **Replay idempotency:** a replay writes through the sink and then resolves the entry. If the node dies in between, the entry stays pending, so replaying it again could write a second copy. Sinks for create-only feeds should make writes conditional on a business identifier.

## Tests

`tests/inbound.rs` uses the real `FhirServer` over SurrealDB as the sink:
- non-JSON payloads and unknown elements go to the dead letter, verbatim, with an OperationOutcome for the caller;
- a `pat-1` failure and an unresolved reference go to quarantine;
- none of the failures reach the record, and all of them are listed for review and audited;
- replaying before the fix still fails, and replaying after it writes;
- a dead letter replays with a correction, and a resolved entry cannot be replayed twice;
- discard;
- per-integration retention, with the purge audited.
