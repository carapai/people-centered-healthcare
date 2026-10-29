# health-persistence

Layer: **contracts**. Pure persistence traits (ADR 0005; persistence.md § The write transaction). It has no runtime and no SurrealDB.

- **`Store::begin` → `Transaction`.** The transaction reads, writes with a `Precondition` (`MustNotExist`, `IfMatch`, `None`), deletes as a tombstone, claims and releases `GuardKey`s, advances `CounterKey` sequences, appends `OutboxEntry` rows, and commits. Dropping it rolls back.
- **Stored form.** The store keeps the validated input JSON, stamped by `stamp_meta` with `id`, `meta.versionId` (the write's `op_id`) and `meta.lastUpdated` (ADR 0051). Key order and number text are kept.
- **`WriteError`** maps to HTTP: `PreconditionFailed` is 412, `Gone` is 410, and `Conflict` is retryable.
- **`RetryPolicy`** gives up to 3 retries, on conflicts only. The adapter supplies the backoff.

Port methods return `impl Future + Send` (dependency-rules.md § Port style). The in-memory fake is `health-persistence-memory`.
