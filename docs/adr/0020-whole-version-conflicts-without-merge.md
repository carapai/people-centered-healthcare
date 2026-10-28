# Conflicts are detected per whole resource version, and nothing is merged automatically

A Candidate Version conflicts when its parent version is not the Authority Node's current head. Only three cases resolve automatically:
1. the candidate's content hash equals the head's, so it is accepted as a duplicate;
2. the candidate is a workflow transition that the class's allowed-transition table permits from the current state and that commutes with the head;
3. the item is a ledger operation, which never has version conflicts. A broken invariant raises a Reconciliation Record instead.

Every other case opens a Conflict Record holding both branches. A person with conflict-resolution permission resolves it at the Authority Node by writing a new version on top of the head. Pulled data never overwrites a local record that still has an unaccepted candidate.

We chose this because automatic merging of clinical or identity data produces records that converge but are clinically wrong. Examples are two allergy statuses, two sets of demographics, or an edited signed note. Universal last-write-wins is explicitly forbidden (§89).

## Considered Options

- **Element-level merge** (FHIR Patch, or three-way merge of non-overlapping elements). Deferred: it is cheap to add for specific classes later, but it is hard to make safe in general. It also makes "what did the clinician actually see?" ambiguous.
- **Deterministic winner kept on every node, with losers stored** (CouchDB). Rejected as the default, because a silently chosen winner is still effectively last-write-wins for the clinician reading it. We keep the loser-retention part.
- **CRDTs.** Rejected: they guarantee convergence, not clinical correctness.

## Consequences

- Concurrent edits produce more Conflict Records, which need a review queue and an ageing alert (7 days) at each Authority Node.
- Identity merges never rewrite references at facilities. A merged provisional Patient carries `link: replaced-by` the golden record.
