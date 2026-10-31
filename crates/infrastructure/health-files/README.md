# health-files

Layer: **infrastructure**. The content-addressed blob store for attachments: SHA-256 keyed files on the node filesystem, outside SurrealDB (ADR 0021; synchronization.md § Attachments).

- **`FsBlobStore::open(root)`** creates `blobs/`, `uploads/`, `tmp/` and `quarantine/` under `root`. Stored blobs live at `blobs/ab/cd/{64 hex}`.
- **`put` / `put_expecting`** stream and hash the content into `tmp/`, sync it, and rename it into place, so a stored blob is always complete.
  - An existing hash is never rewritten, so duplicate uploads deduplicate (`Stored::created` is false).
  - `put_expecting` refuses content whose hash or size differs from the declared `BlobRef`.
  - The cap is `MAX_BLOB_SIZE` (50 MiB), enforced while streaming.
- **Resumable uploads:** `received(blob)` is the offset of the next chunk; for a stored blob it is the full size, so senders skip it. `write_chunk(blob, offset, bytes)` appends 1 MiB chunks to `uploads/{hash}.part`.
  - A chunk at any other offset is `OutOfOrder`.
  - The completing chunk verifies the whole blob and stores it. A mismatch deletes the partial file, and the upload restarts from zero.
- **`read(hash)`** recomputes the hash. A mismatch moves the file to `quarantine/` and returns `Corrupt`, so a rotten blob is never served and can be fetched again.
- **Backups:** `hashes()` lists the stored blobs, and the backup copies `blobs/` next to the logical export, adding only new hashes (backup-and-recovery.md).
- **`collect(references, horizon, now)`** removes a blob only when `BlobReferences::is_referenced` is false and the blob is older than the horizon. It also removes abandoned uploads and temporary files older than the horizon.
  - A newer unreferenced blob is kept, because its resource may still be arriving.
  - The persistence adapter will implement `BlobReferences` over the live and historic `Attachment` references. Until then, nothing calls `collect` in a runtime.
- **I/O is synchronous.** Async callers run it on a blocking thread.

## Tests

`tests/store.rs` runs against a temporary directory and covers:
- hex hashes;
- deduplication of `put`, `put_expecting` and a stored blob's chunks;
- declared-content mismatches;
- bit rot detected on read and quarantined;
- the 50 MiB cap, while streaming and when declared;
- a 2.5 MiB upload resumed across a reopen, an out-of-order chunk, and a corrupted upload restarting;
- garbage collection keeping referenced blobs and anything within the horizon.
