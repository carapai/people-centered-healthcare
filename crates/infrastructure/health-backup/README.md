# health-backup

Layer: **infrastructure**. Encrypted logical backups (backup-and-recovery.md; ADR 0046).

- **`create(db, blobs, root, node, key, now)`** takes a backup:
  - **The export:** SurrealDB's snapshot-consistent export (`Connection::export_sql`), which includes the outbox. It is encrypted to `{root}/{created}/export.surql.enc`.
  - **New blobs:** each blob the node's `health-files` store holds is encrypted to `{root}/blobs/{sha256}.enc`, only if that hash is not there yet.
  - **The manifest:** `manifest.json` records the node, the time, the export's size and its plaintext and file SHA-256s, and the blob hashes.
  - **Verification:** the backup is verified before `create` returns.
- **Encryption:** XChaCha20-Poly1305 with the node's 256-bit `BackupKey` (wrapped and escrowed in central KMS, ADR 0046), with a fresh nonce per file.
  - The associated data binds each ciphertext to its role, so a blob file cannot stand in for another.
  - Nothing is ever written in the clear.
- **`verify(dir, key)`** checks the manifest, the export's file checksum, its decryption and plaintext checksum, and every blob's decryption and hash. A corrupt backup, a wrong key, a swapped blob or a tampered manifest is `BackupError::Corrupt`.
- **`restore(dir, key, db, blobs)`** verifies first, then imports the export into an empty database (`Connection::import_sql`, which runs in memory, so the decrypted export never touches disk). It then puts the blobs back with `put_expecting`. A backup that fails verification restores nothing.
- **Limits:**
  - The export is held in memory while it is encrypted; streaming it in chunks can come when node databases outgrow that.
  - The SDK's export works on the embedded engines (edges and desktops). Central's remote engine exports over HTTP, which is part of central disaster recovery ([#138](https://github.com/carapai/people-centered-healthcare/issues/138)).
  - The schedule (hourly, with 48 hourly and 14 daily copies), the second volume, USB and the weekly test restore are [#78](https://github.com/carapai/people-centered-healthcare/issues/78).

## Tests

`tests/round_trip.rs` is the export → import round-trip fixture CI runs (backup-and-recovery.md):
- **Representative data:** versions and history, a tombstone, search, reference and blocking entries, block counts, the migration ledger and the inbound queue.
- **The round trip:** every table matches row for row after a restore, and the decimal `12.30` keeps its text.
- **The restored node:** it needs no migration, and its blobs read back.
- **A second backup** copies no blob again.
- **The schema** has no `set<record>` field (SurrealDB #7621).
- **Corruption:** a wrong key, a flipped byte in the export, a damaged or swapped blob and a tampered manifest are all detected, and a corrupt backup restores nothing.
