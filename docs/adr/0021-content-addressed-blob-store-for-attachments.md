# Attachments live in a content-addressed blob store outside SurrealDB

Binary content (scans, photos, PDFs, signatures) is stored on each node's filesystem, keyed by sha-256, and included in backups.
- **References.** FHIR resources reference a blob by hash, size and content type in `Attachment.url` / `Attachment.hash`.
- **Upload** happens after the resource that references it, in resumable 1 MiB chunks deduplicated by hash. The resource may be accepted before its blob arrives, and shows the attachment as pending until then.
- **Download** is on demand by default. A per-type metadata flag can pre-fetch, e.g. signed consent scans.
- **Size cap:** 50 MiB per attachment in v1.

We chose this because large values would bloat SurrealDB transactions, logical exports, sync batches and the RocksDB block cache. Hash addressing also makes deduplication, integrity checks and resumable transfer free.

## Considered Options

- **FHIR `Binary` resources stored inline in SurrealDB.** Rejected: base64 inflation, 4 MiB batch limits, and the WebSocket response-size issue (SurrealDB #7623).
- **SurrealDB `DEFINE BUCKET` file storage.** Rejected for now: it is new, and its backup and embedded-engine behaviour is unverified.

## Consequences

- Backup and restore must cover the blob directory as well as the logical export (backup-and-recovery.md).
- Garbage collection removes a blob only when no live or historic version references it, and only after the retention horizon.
