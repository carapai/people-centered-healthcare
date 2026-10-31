# health-audit

Layer: **infrastructure**. The audit module (resource-ownership.md rule 7; security.md; write-authority.md: AuditEvent is append-only at its origin node and synced up).

- **`Auditor`** is the only writer of AuditEvent. `append(tx, event)` creates one in the caller's transaction, so an audited write and its event commit together or not at all. Nothing updates or deletes an AuditEvent.
- **`Auditor::event`** builds an R5 AuditEvent that passes tier 1, with:
  - the `rest` category and the interaction code (`create`, `read`, `vread`, `update`, `delete`, `search-type`, `history`);
  - the `action` (C, R, U, D, E) and the outcome;
  - the agent: who (a reference, or a display), the client, the network address, and the purpose of use as `authorization`;
  - under break-glass, `BTG` in `authorization` and a `BTG` label on the event;
  - the observing node as `source.observer`;
  - each entity as `Type/id/_history/version`, with its security labels copied;
  - the patient.
- **Break-glass grants:** `Auditor::break_glass` is the AuditEvent of a grant: a DICOM `110113` security alert coded `BTG`, labelled, naming the patient, with the justification on the entity. The free function `append` lets a store write it in its own transaction (`health_auth::SurrealAuthStore`).
- **What must be audited:** every write. A read only when a record carries a configured sensitivity label or the access relied on break-glass (`must_audit_read`).
- **`Agent`** comes from the verified token and the request (#192), or is `Agent::unauthenticated()` / `Agent::system(job)`.
- **`provenance(...)`** builds the Provenance for a derived write. Its target's Owner writes it in the same transaction (rule 7).

`health-fhir-server` uses this through `FhirServer::with_audit` (see its README). The domain rules crate the catalogue names (`health-audit-domain`) will be created once audit has domain rules of its own. Today the rules fit in `must_audit_read`.

## Tests

- **Unit:** the events and the Provenance are valid R5 (tier 1), with who, what and why; failures are flagged; only sensitive or break-glass reads are audited.
- **`health-fhir-server/tests/audit.rs`:** the end-to-end behaviour.
