# Desktop secrets live only in the OS keystore, and provisioning checks full-disk encryption

**Secrets.** The device and node keys, tokens and the backup key are stored through the `keyring` crate, called from Rust: Windows Credential Manager, or Linux Secret Service. They are never exposed to JavaScript.

**No keystore, no install.** Provisioning is refused on a system without a working keystore. There is no Stronghold or encrypted-file fallback. TPM-backed, non-exportable keys are a later hardening step.

**Data at rest.** The data directory is per machine (e.g. `%ProgramData%`), because facility PCs are shared. SurrealDB has no encryption at rest, so provisioning checks for OS full-disk encryption (BitLocker or LUKS). Country configuration decides whether its absence warns or blocks.

**Backups:**
- **Mode B** reuses the edge backup design: hourly encrypted logical export, a second volume or USB, and the key escrowed at central (ADR 0007).
- **Desktops under an edge** take an hourly encrypted export, to protect their unsent outbox.

**Revocation:**
- **`retire`:** push the outbox, then wipe.
- **`compromised`:** the session is refused, and the desktop wipes on its next contact. Unsynced data is lost.

We chose this for three reasons:
- Stronghold needs a user-supplied password and shows little maintenance activity.
- A fallback key file next to the data would protect nothing.
- Full-disk encryption is the only practical at-rest protection for an embedded engine without native encryption.

## Considered Options

- **`tauri-plugin-stronghold`.** Rejected: it is password-bound and low-activity.
- **App-level field encryption in SurrealDB.** Rejected for v1: it breaks indexing and search, and the key would still sit on the same machine.
- **Per-user data directories.** Rejected: a shared PC would end up with several divergent replicas and outboxes.

## Consequences

- Minimal Linux installs without a Secret Service (e.g. kiosks) are unsupported as desktops.
- A stolen, unencrypted desktop exposes its replica. Configuration can therefore require full-disk encryption.
