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
- **`Issuer`** is the issuing CA's port, since its key belongs in the HSM or KMS. `health_pki::Authority` implements it with its key at hand, until the KMS adapter.

## Tests

`tests/provisioning.rs`, over SurrealDB in memory with a real root and issuing CA:
- **Redemption:** a hand-typed code provisions `E1`, and the chain's leaf for `<node-id>.local` verifies under the root. The code then refuses a second use.
- **Refusals:** unknown and malformed codes, an unencrypted edge, and an expired code. A refused request leaves the code redeemable.
- **Exemptions:** an exempt facility provisions unencrypted and stands in `exempt()`.
- **Short codes** count up per facility.
- **The route:** each answer.
