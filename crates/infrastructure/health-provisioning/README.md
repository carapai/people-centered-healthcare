# health-provisioning

Layer: **infrastructure**. Central's node registry and edge provisioning (#268; identifiers.md; [lan-security.md](../../../docs/architecture/lan-security.md); ADRs 0048, 0058). The rules are `health_identity`'s `node` module; the PKI is [`health-pki`](../health-pki); the tables are `health-surrealdb`'s migration `0024_nodes`.

- **`NodeRegistry::issue_code(facility, dns_names, exemption, issued_by)`** issues a one-time provisioning code, audited.
  - **The code:** 8 Crockford base32 characters from the OS's random source, shown once as `XXXX-XXXX`. Only its sha-256 is stored. It redeems once, within 72 hours.
  - **`dns_names`:** the facility's names, for the intermediate's name constraints.
  - **`exemption`:** a reason, given only when the operator exempts the facility's edge from full-disk encryption (ADR 0048). The exemption is audited as a warning.
- **`NodeRegistry::redeem(code, request, encrypted, issuer)`** is the edge's half, in one transaction:
  1. It reads the code forgivingly: lower case, spaces, and `I`, `L` and `O` as Crockford reads them.
  2. It refuses a code that is unknown, expired or used, or an unencrypted edge without an exemption.
  3. It mints the node's UUIDv7 and its next short code at the facility (`E1`, `E2`, …, never reused).
  4. The `Issuer` issues the facility intermediate for `<node-id>.local` and the code's DNS names.
  5. It writes the node, the redeemed code and an AuditEvent.
  - **If issuance fails,** nothing commits and the code stays redeemable.
  - **A concurrent second redemption** fails at commit, on the unique indexes.
  - **An exempt node** raises an `encryption-exempt` alert.
- **`NodeRegistry::exempt()`** lists the active exempt nodes. central-server logs an `encryption-exempt` alert for each, hourly: the standing alert ADR 0048 asks for.
- **`router(registry, issuer)`** serves `POST /provisioning/nodes`. The code is the only credential, since the edge has no account at central yet.
  - **The body (JSON):** `code`, `request` (the PKCS#10 request, PEM) and `encrypted`.
  - **201:** `node_id`, `short_code`, `facility` and `chain` (the intermediate, then the issuing CA).
  - **Refusals:** 403 `code_refused`, 422 `not_encrypted`, 400 `invalid_request`, and 503 if the store failed.
- **Recovery keys** (ADR 0048):
  - **`recovery_jws(node, keys, key, at)`** is the escrow an edge sends: a compact ES256 JWS, `typ` `health-recovery+jwt`, carrying `node`, `iat` and each volume's key. It is signed with the node's facility intermediate key.
  - **`NodeRegistry::escrow(jws, kms)`:**
    1. It verifies the signature against the intermediate key recorded on the node at provisioning, so only an active node escrows its own keys.
    2. It refuses an `iat` more than 5 minutes from central's clock, or no later than the current key's, so a replay cannot bring back a key that was read out.
    3. It wraps each key through the `KeyWrap` port and stores it, audited without the key.
  - **`NodeRegistry::read_recovery(node, volume, by, verified, kms)`** is the phone read-out.
    - It needs who reads it and how the caller was verified.
    - It records the retrieval and audits it as a warning.
    - From then on the key is used: it is listed by `used_recovery_keys()`, an hourly `recovery-key-used` alert at central, until the edge escrows its replacement.
  - **`KeyWrap`** is the KMS port. `LocalKeyWrap` is XChaCha20-Poly1305 under a 32-byte master key, with a random nonce per key. Its associated data is `health-recovery/<node>/<volume>`, so a wrapped key unwraps only for its own node and volume.
  - **`recovery_router(registry, kms)`** serves `POST /provisioning/recovery`, with the JWS as the body.
    - **Success:** 204.
    - **Refusals:** 401 `unauthorized`, 409 `escrow_refused`, 400 `invalid_request`, and 503 if the store or the KMS failed.
- **`Issuer`** is the issuing CA's port, since its key belongs in the HSM or KMS. `health_pki::Authority` implements it with its key at hand, until the KMS adapter.

## Tests

`tests/provisioning.rs`, over SurrealDB in memory with a real root and issuing CA:
- **Redemption:** a hand-typed code provisions `E1`, and the chain's leaf for `<node-id>.local` verifies under the root. The code then refuses a second use.
- **Refusals:** unknown and malformed codes, an unencrypted edge, and an expired code. A refused request leaves the code redeemable.
- **Exemptions:** an exempt facility provisions unencrypted and stands in `exempt()`.
- **Short codes** count up per facility.
- **The route:** each answer.

`tests/recovery.rs`:
- **Escrow and read-out:**
  - a node escrows its system and data keys, and the stored row holds neither key, wrapped or not;
  - a read-out needs who and how, returns the key, and leaves it used;
  - the replayed escrow is refused as stale;
  - a newer escrow rotates the system key and clears the alert, while the data key stays untouched.
- **Refusals:** an impostor's signature, an unknown node, a skewed clock, malformed JWSs, and no keys. Reading a key never escrowed is not found, and another master key cannot unwrap.
- **The route:** each answer.
