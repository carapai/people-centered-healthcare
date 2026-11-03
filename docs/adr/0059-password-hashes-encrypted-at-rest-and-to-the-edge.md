# Password hashes are encrypted at rest under a keystore key, and to the edge in its bundle

This amends offline-auth.md § Credentials. That section puts an Argon2id pepper in the edge's keystore, and central-server uses a pepper as Argon2's secret. A hash made with a secret can only be verified by a holder of that secret, so central could not provision its hashes to an edge without giving every edge the same pepper. Instead:

- **Hashing.** Passwords are hashed with plain Argon2id (m = 19456 KiB, t = 2, p = 1, no secret), as offline-auth.md fixes. The PHC string is the same on central and on every edge.
- **At rest.** Each node encrypts every stored hash with its own pepper key: 32 random bytes in its keystore (`health_node_identity::keystore` on an edge, a secret file or KMS at central). The cipher is XChaCha20-Poly1305, as for backups, with a random 24-byte nonce and the account id as associated data, so a hash cannot be moved to another account. The key is the pepper: a copied database yields only ciphertext. Each node verifies by decrypting, then running Argon2.
- **Provisioning.** Central exports a facility's provisioning bundle: its staff's Login Accounts with their hashes, and their signed permission snapshots. Central signs the whole bundle with its snapshot key.
  - The hashes are encrypted to the receiving edge: ECDH between a fresh P-256 key and the edge's facility intermediate public key, HKDF-SHA-256, then XChaCha20-Poly1305. Only that edge, whose intermediate key is sealed in its keystore, can read them.
  - On import, the edge verifies the signature, decrypts the hashes, and re-encrypts each under its own pepper key.
  - Central never learns an edge's pepper, and no node shares one.

We chose this because the pepper's purpose is that a stolen database alone yields nothing crackable, and encrypting the hash keeps that purpose on every node. Hashes still have to move from central to edges, which a secret baked into Argon2 prevents. The intermediate key is already the edge's sealed, central-known identity (ADR 0058), so the bundle needs no new key exchange.

## Considered Options

| Option | Decision |
|---|---|
| Keep the pepper as Argon2's secret, and give every edge central's pepper | Rejected: one compromised edge would reveal the pepper for every hash in the country |
| Plain Argon2id with no pepper at all | Rejected: a copied database, or a leaked backup, would yield crackable hashes |
| Send plain hashes in the bundle, protected only by its signature | Rejected: a leaked bundle file would yield every hash at the facility |

## Consequences

- **Existing hashes.** Hashes made with central's Argon2 secret (`HEALTH_AUTH_PEPPER_FILE`) cannot be converted. Their users set a new password. Only development and test data has them today.
- **`HEALTH_AUTH_PEPPER_FILE`** holds the 32-byte pepper key from now on, not an Argon2 secret.
- **Rotating a pepper key** means decrypting and re-encrypting every hash on that node, with no password reset.
- **Sync** (Phase 5) carries the same encrypted-to-the-edge hashes in its identity lane. The bundle stays as the path for a first install, and for an edge that has never been online.
