# Research: sync designs for the Central / Edge / Desktop hierarchy

Issue: #4. Spec context: `MASTER_PROMPT.md` Part 14 (§86–91). Researched 2026-10-27.

## Summary (answer to the question)

No single prior-art system fits as-is, but their common parts combine cleanly for a fixed tree topology (Central ↔ Edge ↔ Desktop):

1. **Use state transfer for FHIR resources and operation transfer for ledgers.** Each sync unit is a full resource *version* that carries its parent version, as in the CouchDB revision tree and FHIR `meta.versionId`/`If-Match`. Stock and payments sync as append-only, idempotent *operations* (ledger entries), not as mutable state.
2. **Use a server-assigned, monotonic per-node change sequence as the cursor, not wall-clock timestamps.** CouchDB checkpoints on `seq`. FHIR `_since`/`_lastUpdated` pulling (Android FHIR SDK, Bulk Data) works, but it is clock-based and can return duplicates at the boundary.
3. **Write a durable transactional outbox** in the same database transaction as the local write. On top of it: client-generated UUIDs, an operation ID per change, and a receiver-side dedup table. Delivery is then at-least-once, and the receiver's idempotent apply makes the effect exactly-once.
4. **Detect conflicts with a parent-version check.** A write conflicts when its base version is not the receiver's current head. Keep the losing branch, as CouchDB does, and resolve it by a policy per resource class. Never resolve by universal LWW. In a tree with few writers per resource, full per-resource version vectors are not justified. A **hybrid logical clock (HLC)** is worth using for ordering, audit and display, but not for deciding conflicts, because HLC cannot detect concurrency.
5. **Keep tombstones** at least as long as the maximum allowed offline window. Process batches per item, as CouchDB `_bulk_docs` and FHIR `batch` do. Use FHIR `transaction` only for groups that are referentially dependent.
6. **Scope data per facility**, like DHIS2 "capture scope" and OpenSRP `syncStrategy: Location`. Seed an empty node with a snapshot (Bulk Data–style), then follow incremental cursors. R5 topic-based Subscriptions should only serve as low-latency *wake-up hints*, never as the source of truth, because their delivery is best-effort.

CRDT-based systems guarantee convergence but not clinical correctness. They are a poor fit for clinical resources. ElectricSQL's current docs explicitly leave write-path sync to the application.

## Comparison table

| System | Transfer unit | Cursor / change tracking | Conflict detection | Conflict resolution | Deletes | Partial batch | Scoping | Fit for us |
|---|---|---|---|---|---|---|---|---|
| CouchDB replication | Document revisions (state) with `_revisions` history | `_changes?since=<seq>`; checkpoints stored on both sides | Revision tree; multiple leaves = conflict | Deterministic winner on all peers; losers kept; app merges | `_deleted` tombstone revisions | `_bulk_docs` per-doc results, batch not atomic | Filtered replication (not researched here) | **High** for the protocol shape (checkpoints, revs-diff, keep-losers) |
| DHIS2 Android SDK | Records (state) uploaded via tracker importer | "Downloads skip unmodified data after the last successful run" | Server import conflicts | Stored per record; record → ERROR/WARNING; user edits to retry | Not covered in source read | Per-record state machine | User capture scope, limits per org unit/program | **High** for the record sync-state machine, scope and reserved IDs |
| OpenSRP FHIR Core (on Android FHIR SDK) | FHIR resources | `_lastUpdated` per resource type | Remote version vs local change | `ConflictResolver`: AcceptLocal / AcceptRemote, in-flight only | — | Bundle size limit | `syncStrategy` (Location, …) | **Medium**: good scoping; resolution too coarse |
| OpenMRS Sync 2.0 | FHIR (DSTU3) / REST resources, triggered by Atom feed events | Atom feed position | Feed event + fetch | `sync2.mergeBehaviour`: auto-newer or manual | — | — | Feed filtering per child | **Medium**: multi-level parent/child validated; LWW default is a warning |
| FHIR Bulk Data `$export` | NDJSON snapshots | `_since` + `transactionTime` | None | None | `deleted` array in manifest | Async job | Group-level export | **Seeding/backfill only** |
| FHIR R5 Subscriptions | Notification bundles | `eventsSinceSubscriptionStart` counter; `$events` replay | Gap detection | n/a | n/a | n/a | Topic filters | **Wake-up hint only** (best-effort) |
| FHIR REST (R5) | Resource versions | `_history?_since` | `If-Match` ETag = versionId → 412 | Client retries/merges | History includes deletions | `batch` vs `transaction` | Search | **Native wire semantics** for apply |
| ElectricSQL (current) | Read "shapes" from Postgres | Shape log | n/a (writes go through your API) | App-defined | — | — | Shapes | **Low**: confirms write-path is app-specific |
| CRDTs (generic) | Ops or merged state | Causal delivery / state merge | Not needed (commutative) | Automatic convergence | Tombstones/metadata | — | — | **Low** for clinical; maybe free-text drafts |
| HLC | Timestamp, not a protocol | Orders events close to NTP time | **Cannot** detect concurrency | — | — | — | — | Use for ordering/audit |
| (Dotted) version vectors | Causality metadata | — | Detects concurrency exactly | — | — | — | — | Justified only for multi-writer resources |

## Cited findings

### CouchDB replication protocol
Source: https://docs.couchdb.org/en/stable/replication/protocol.html

- Checkpoints ("Intermediate Recorded Sequence ID") are stored as replication logs on **both** source and target. A failed run restarts "from last point of success". If no common checkpoint exists, a full replication runs.
- Changes are read with `GET /{db}/_changes?style=all_docs&since=<checkpoint>`. Sequence IDs increase but "MAY NOT always be an integer", so treat them as opaque cursors.
- `POST /_revs_diff` returns only revisions the target lacks. This is a cheap "what do you need?" step before sending bodies.
- `POST /_bulk_docs` with `new_edits=false` stores revisions as-is, together with their `_revisions` history. Results come back per document, so one failed document does not fail the batch.
- Attachments are sent as `multipart/related` after their document ("the Document ALWAYS goes before its attachments").

Source: https://docs.couchdb.org/en/stable/replication/conflicts.html

- The winner among conflicting leaves is chosen by "a deterministic algorithm so that the same choice will be made on all peers". Losing revisions are **kept** (`_conflicts`, `open_revs=all`), not discarded. An application resolves by writing a merged revision and marking the other leaves `_deleted:true`. Compaction keeps revision history so that old replicas can still reconcile.

### DHIS2 Android SDK
Source: https://developers.dhis2.org/docs/mobile/android-sdk/workflow

- Only a subset of metadata syncs to the device.
- Data download is scoped: "By default, the SDK only downloads TrackedEntityInstances and Events that are located in user capture scope". `limitByOrgunit` and `limitByProgram` cap the volume.
- Per-record states: `TO_POST`, `TO_UPDATE`, `SYNCED`, `UPLOADING`, `ERROR`, `WARNING`. Editing during upload reverts the record to `TO_UPDATE`. `ERROR`/`WARNING` records are not re-uploaded until edited.
- "By default, the SDK does not overwrite data in the device in a status other than SYNCED". Pulled data never silently clobbers unsynced local edits.
- Server import conflicts are stored in the database and auto-removed after a successful upload.
- "Reserved values": server-generated unique attributes are reserved in advance so they can be issued offline.
- File resources are always uploaded but only optionally downloaded.

Source code: https://github.com/dhis2/dhis2-android-sdk/blob/master/core/src/main/java/org/hisp/dhis/android/core/common/State.kt. The enum also contains `RELATIONSHIP`, `SENT_VIA_SMS` and `SYNCED_VIA_SMS`, so an SMS fallback transport exists.

### OpenSRP FHIR Core and the Android FHIR SDK
- OpenSRP configures data filtering via `"syncStrategy": ["Location"]` (other values include related-entity location). Non-patient config resources are excluded from data sync and distributed via a Composition. Source: https://github.com/opensrp/fhircore/blob/main/docs/engineering/backend/readme.mdx
- Android FHIR SDK `ConflictResolver.resolve(local, remote)`. The shipped resolvers are `AcceptLocalConflictResolver` and `AcceptRemoteConflictResolver`. The code says "There is no way for the client application to abort or defer the conflict resolution to a later time." Source: https://github.com/google/android-fhir/blob/master/engine/src/main/java/com/google/android/fhir/sync/ConflictResolver.kt
- Upload strategies offer bundle or individual requests, PUT/POST for creates, PUT/PATCH for updates, and optional squashing of multiple local changes into one patch per resource. Bundle mode currently requires `squash = true`. Source: https://github.com/google/android-fhir/blob/master/engine/src/main/java/com/google/android/fhir/sync/upload/UploadStrategy.kt
- Download adds a `_lastUpdated` parameter per resource type and saves the max `meta.lastUpdated` seen as the next cursor. Source: https://github.com/google/android-fhir/blob/master/engine/src/main/java/com/google/android/fhir/sync/download/ResourceParamsBasedDownloadWorkManager.kt

### OpenMRS Sync 2.0
Sources: https://openmrs.atlassian.net/wiki/spaces/projects/pages/26936063/Sync+2.0, https://openmrs.atlassian.net/wiki/spaces/projects/pages/26936273/Sync+2.0+user+manual, https://github.com/openmrs/openmrs-module-sync2

- One parent with many children, and each child can be a parent, giving multi-level sync. Atom feeds publish change events. The child pulls the parent's feed, and pushes by reading its own feed and fetching the resource from its local FHIR/REST API. FHIR DSTU3 is the transfer format.
- Conflict handling is a global setting, `sync2.mergeBehaviour`: newer wins automatically, or a user decides manually. Manual sync of selected records exists since 1.2.0.
- Caveat: the Confluence pages are largely design-stage, so current behaviour was not verified against source (**unverified**).

### FHIR Bulk Data (STU2)
Source: https://hl7.org/fhir/uv/bulkdata/STU2/export.html. The current published IG is v3.0.0 per https://hl7.org/fhir/uv/bulkdata/export.html; the STU2 text was read for the details.

- `_since`: "Resources will be included in the response if their state has changed after the supplied time". The client keeps the `transactionTime` and uses it as the next `_since`.
- The response "SHOULD NOT include any resources modified after this instant" (transactionTime).
- When `_since` is supplied, a `deleted` array SHOULD be populated, and those resources "SHALL NOT appear in the 'output' section".
- The kick-off is async: `Prefer: respond-async` → `202` + `Content-Location`. Group-level export exists. `_typeFilter` is optional and may be ignored by servers.

### FHIR R5 REST semantics
Source: https://hl7.org/fhir/R5/http.html

- Optimistic locking: `If-Match` with ETag = versionId. A stale version returns `412 Precondition Failed`.
- Conditional create `If-None-Exist` is meant "to avoid the risk of two clients creating duplicate resources".
- History is "sorted with oldest versions last, and including deleted resources". `_since` is inclusive, so clients may see boundary versions twice, and servers need not support sub-second precision.

### FHIR R5 topic-based Subscriptions
Source: https://hl7.org/fhir/R5/subscriptions.html (v5.0.0)

- "channels are assumed to be best-effort unless otherwise specified". Subscribers track `eventsSinceSubscriptionStart` to detect gaps, and use `heartbeatPeriod` to detect silence.
- `$events` lets a client request past events, though its availability is server-limited.
- `empty` and `id-only` payloads reduce exposure because the subscriber fetches from a trusted location.

### ElectricSQL
Source: https://electric.ax/docs/guides/writes (redirected from electric-sql.com)

- "Electric does not do write-path sync". Writes go through your own API, with optimistic or shared persistent optimistic state. The fourth pattern, "through-the-database sync", uses a local PGlite and a change log pushed by your own utility. The docs warn that rollback handling gets harder.
- (**Unverified**: that the earlier ElectricSQL versions were CRDT-based and were replaced. This is from memory only.)

### CRDTs
Source: Shapiro, Preguiça, Baquero, Zawirski, "Conflict-free Replicated Data Types", SSS 2011, https://www.cs.tufts.edu/~nr/cs257/archive/marc-shapiro/CRDTs_SSS-2011.pdf

- The paper defines Strong Eventual Consistency and sufficient conditions for state-based and op-based replication. The exact causal-delivery wording was not re-read (**unverified**).
- Inference (ours): convergence ≠ correctness. Merging two conflicting allergy statuses or patient identities automatically is a clinical-safety problem, not a data-structure problem.

### Hybrid logical clocks
Source: Kulkarni, Demirbas et al., "Logical Physical Clocks and Consistent Snapshots in Globally Distributed Databases", https://cse.buffalo.edu/tech-reports/2014-04.pdf

- HLC guarantees `e hb f ⇒ l.e < l.f`, keeps `|l.e − pt.e|` bounded, and "fits in to 64 bits NTP timestamp format", tolerating NTP kinks.
- `l.e = l.f ⇒ e||f`. However, `hlc.e < hlc.f` does **not** imply happened-before; only vector clocks give `e hb f ⇔ vc.e < vc.f`. So HLC orders events but cannot flag a concurrent edit.

### Version vectors
Source: Preguiça et al., "Dotted Version Vectors: Logical Clocks for Optimistic Replication", https://arxiv.org/abs/1011.5808

- Existing causality-tracking methods "either lose information about causality or do not scale". Dotted version vectors grow "linearly only with the number of servers that register updates for a given data element".

### Outbox and idempotency
- Transactional outbox: store the message in the DB as part of the business update. Because "The Message relay might publish a message more than once", consumers must be idempotent. Source: https://microservices.io/patterns/data/transactional-outbox.html
- `Idempotency-Key` HTTP header, draft-ietf-httpapi-idempotency-key-header-08 (22 Oct 2026, Internet-Draft, not an RFC). Reuse with a different payload → 422; a concurrent retry → 409. Source: https://datatracker.ietf.org/doc/draft-ietf-httpapi-idempotency-key-header/

## Implications for the decision

### Recommended building blocks
1. **Identity.** Each `node_id` (Central, Edge, Desktop) and `device_id` gets a certificate-bound identity. `resource_id` is a client-generated UUID, so creates are idempotent by construction. Each change gets an `op_id` (UUIDv7) that also acts as the `Idempotency-Key`.
2. **Version metadata per resource version:** `versionId`, `parent_versionId` (base), `origin_node`, `hlc` (for ordering and audit), and `op_id`. This is the CouchDB revision-tree idea mapped onto FHIR `meta.versionId`, with `If-Match` semantics on apply.
3. **Change feed per node.** Every committed version appends to a local, strictly increasing `seq` log. Each hop (Desktop→Edge, Edge→Central and back) keeps an **opaque cursor per peer and direction**, checkpointed on both sides as in CouchDB. Do not use timestamps as cursors.
4. **Durable outbox.** The resource write, the change-log append and the outbox row go in one SurrealDB transaction. The relay sends at least once. The receiver has a dedup table keyed by `op_id` that stores the original result, so retries return the same answer.
5. **Push protocol.** Negotiate first ("which of these op_ids/versions do you lack?", like `_revs_diff`). Then send a FHIR `batch` of independent units, or a `transaction` per referentially dependent group such as Encounter + its Observations. Results come back per item. Each item moves through a DHIS2-like state machine: `pending → uploading → synced | rejected | conflict`. Pulled data never overwrites a record that is not synced.
6. **Pull protocol.** Use the incremental cursor feed within the facility scope. Seed a new or long-offline node with a snapshot (Bulk Data–style NDJSON with a `transactionTime`-equivalent cursor), then follow incremental pulls. R5 Subscriptions (`id-only`) may wake a peer, but the cursor pull stays authoritative.
7. **Tombstones.** Deletes are versions with `deleted=true`. The tombstone retention window must exceed the maximum allowed offline period plus a margin. A node whose cursor is older than the retention horizon must do a full re-seed and rebase its outbox.
8. **Attachments.** Content-addressed (hash) blobs, transferred after their referencing resource, resumable, and deduplicated by hash.
9. **Offline identifiers.** Pre-reserved identifier ranges per node (DHIS2 "reserved values") for MRNs and receipt numbers.
10. **Scope.** Each node subscribes to a scope (facility, plus catchment patients, plus approved metadata snapshot). Central enforces the scope on both push and pull. Remote revocation shrinks the scope and triggers purge.

### Per-resource-class conflict strategies (§89)

| Class | Transfer | Detection | Strategy |
|---|---|---|---|
| Patient demographics / identity | State | Parent-version mismatch; plus duplicate-candidate matching on create | Never auto-merge. Raise a conflict record; mark the record provisional until central identity reconciliation (merge/link via `Patient.link`). |
| Observation, Condition, AllergyIntolerance | State, mostly append | Parent-version mismatch (rare: edits are corrections) | Version-safe correction: keep both versions, apply the later one only if its parent matches, else flag `entered-in-error`/amend review. Do not LWW. |
| Signed notes (Composition/DocumentReference) | State, immutable after signature | Any edit after signature | Amendment only: a new resource that `relatesTo` the original. Reject in-place edits at the receiver. |
| Encounter / Task / workflow states | State + transition | Concurrent transition from same base | Validate against an allowed-transition table at the authority. Apply a commuting pair deterministically. Raise a conflict otherwise. |
| QuestionnaireResponse | State, mostly create-once | Duplicate `op_id` / identifier | Idempotent create; treat later edits like Observation. |
| Stock | **Operations** (ledger entries) | None needed: entries commute | Append-only ledger. Balance is derived. Negative balance or over-dispense becomes a reconciliation record, not a conflict. |
| Payments / invoices | **Operations** with external refs | Duplicate `op_id` / external transaction id | Idempotent financial ledger. Provisional until externally confirmed. Reversal entries, never edits. |
| Metadata (ValueSets, Questionnaires, config) | Immutable versioned snapshots, downward only | Version compatibility check | Central publishes; nodes activate approved versions. Refuse sync if the peer's metadata version is incompatible. |

### Risks
- **Clock skew.** If HLC or `_lastUpdated` is ever used as a cursor, records will be lost or duplicated. Keep cursors as sequence numbers.
- **Unbounded conflict backlog** after long offline periods needs a review UI and ownership (who resolves a conflict at the Edge vs at Central?).
- **Tombstone horizon vs. multi-month outages.** A full re-seed of a large facility over poor links is expensive.
- **§90 "do not silently switch write authorities".** A Desktop that falls back to direct-to-Central sync while it still has unsynced ops for its Edge could fork history. Pin each Desktop to one upstream until its outbox is drained.
- **Scope changes** (patient transfers between facilities) need explicit hand-off semantics, or data leaks and goes stale.
- **SurrealDB** transactional guarantees for the combined resource + changelog + outbox write must be verified (cross-ref the database research ticket).

### Open questions
- Are full version vectors needed anywhere? They would be if any resource is legitimately co-edited at two Edges at once (e.g. a shared referral). Default answer: no, use parent-version checks plus origin node.
- Granularity of conflicts: whole resource vs. element-level (FHIR Patch)? The Android FHIR SDK squashes changes into one patch per resource.
- Transport between Desktop and Edge on LAN: the same HTTP protocol, or a lighter one?
- Should an SMS or low-bandwidth fallback exist, as DHIS2 provides (`SYNCED_VIA_SMS`)?
- How long is the maximum supported offline period? This sets tombstone retention and reserved-ID range sizes.
