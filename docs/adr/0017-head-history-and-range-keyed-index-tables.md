# FHIR resources are stored as head and history tables per type, with array-keyed search index tables

Each FHIR resource type has two tables:
- a head table, holding the full canonical R5 JSON plus common meta and a few typed "hot" projections;
- a history table keyed `[resource_uuid, version_seq]`, holding every accepted version in full.

Every supported SearchParameter writes entries into per-search-type index tables, such as `idx_token:[type, param, system, code, resource_uuid]`. Because their record IDs are arrays in sorted order, a search compiles to record range scans. The resource, history, index entries, change log and outbox are written in one transaction, inside one namespace per tenant and one database per node, so that the cross-Module transactions of ADR 0004 hold.

We chose this design for three reasons:
- SurrealDB stores record IDs in sorted order, and range scans over well-ordered keys are its fastest access path.
- A generic per-type index layout supports every SearchParameter, including custom parameters added later by configuration packages, without schema changes.
- Full versions match the full-version transfer of sync.

## Considered Options

- **One projection column per SearchParameter on each resource table.** This needs a schema migration for every new SearchParameter, and repeating or composite values fit poorly. Rejected. Hot projections are kept only for the most frequent query paths.
- **History stored as diffs, or through change feeds.** Change-feed versionstamps reset on restart (SurrealDB #7606), and diffs complicate the full-version sync. Rejected.
- **One database per Bounded Context.** This breaks atomic transactions within a Deployable. Rejected. Ownership is enforced by repositories and the architecture check instead.

## Consequences

- Each head row keeps the record ids of its own index entries, so a rewrite deletes them directly instead of scanning.
- Activating a new SearchParameter runs a resumable reindex job.
- Projection references are `record` links held as single values or arrays, never `set<record>`, because of export bug #7621.
