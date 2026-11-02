# health-pki

Layer: **infrastructure**. The private PKI for LAN TLS and package signing ([lan-security.md](../../../docs/architecture/lan-security.md); ADRs 0008, 0058). The ceremony and issuance tooling over it is [`tools/pki`](../../../tools/pki).

| Tier | Issued by | Lifetime | Constraints |
|---|---|---|---|
| National root | itself (`Authority::root`) | `ROOT_YEARS` (25) | CA, path length 2; key usage keyCertSign and cRLSign |
| Facility issuing CA | the root (`issue_issuing_ca`) | `ISSUING_CA_YEARS` (10) | CA, path length 1; serverAuth; names `.local`, the facility DNS suffixes, the private IP ranges |
| Facility intermediate | the issuing CA (`issue_facility`) | `FACILITY_YEARS` (5) | CA, path length 0; serverAuth; names `<node-id>.local`, the facility's DNS names, the private IP ranges |
| Edge server leaf | the facility intermediate (`issue_server`) | `LEAF_DAYS` (397) | not a CA; serverAuth; the node's DNS names and private addresses as SANs |
| Publication signing | the root (`issue_publication`) | `PUBLICATION_DAYS` (396) | not a CA; code signing only |

The private IP ranges are 10/8, 172.16/12, 192.168/16 and fc00::/7.

- **`Authority`** is a CA whose key is at hand. It reads its tier from its path length and refuses to issue anything outside that tier. `from_pem` refuses a key that does not match the certificate.
- **Issued from a request.** Every tier below the root is issued from a PKCS#10 request (`request(key, name)` makes one). Only its public key is used, and it must be ECDSA P-256. The issuer sets the subject, names, lifetime and extensions, and ignores what the request asks for. The holder's key never leaves its keystore or HSM.
- **Refused, not trimmed.** Issuance is refused (`Error::Refused`) for:
  - a name outside the issuer's DNS constraints;
  - a public IP address;
  - a node ID or name that is not plain lowercase DNS;
  - a certificate that would outlive its issuer. This is also what stops the issuing CA after its first 5 years.
- **`Revocations`** is one issuer's list: `revoke(serial, reason, at)`, and `crl(issuer, at)` for the next numbered CRL, current for `CRL_DAYS`. The root's list covers issuing CAs and publication signing. The issuing CA's list covers facility intermediates. Facility intermediates publish none. It serializes to JSON for the tooling.
- **`Trust`** is the root plus the CRLs a node has received.
  - `verify_server(chain, name, at)` checks a served chain (leaf first) for a DNS name or IP address.
  - `verify_publisher(certificate, at)` checks a publication-signing certificate.
  - Every CA in the chain is checked against its issuer's CRL. A CA with no CRL yet is accepted, and an out-of-date CRL still counts, because nodes can be offline for months.
  - A forged CRL makes the chain fail.
- **`inspect(pem)`** reads a certificate's validity, its DNS and IP names, and its SPKI fingerprint (`sha256/<base64>`, what Tauri clients pin) back, so an edge knows when its leaf is due and what to put in its pairing code.
- **Algorithms:** ECDSA P-256 with SHA-256 throughout, on ring (rcgen to issue, rustls-webpki to verify, x509-parser to read certificates back). Certificates are backdated by `BACKDATE_SECONDS` for verifiers whose clocks run behind.
- **Not here:**
  - The keystore and HSM integration. `Authority` signs with a key in memory, which suits the root ceremony and tests. An edge keystore (#73) or central's HSM or KMS needs an rcgen `SigningKey` adapter.
  - The edge's leaf renewal and pairing pins: #74.
  - CRL delivery in the sync revocation lane: the sync work.
  - The real ceremony and root custody: #265.

## Tests

`tests/pki.rs` (security, testing.md):
- **The chain:** the full chain verifies for the node's names and address, and not for another name or after the leaf expires.
- **The extensions:** issuing CAs and facility intermediates carry exactly their path lengths, name constraints, EKUs and key usages.
- **Requests:** a request asking for a CA, other names or more usages gets only what its issuer gives.
- **A stolen facility key** cannot mint another node's name, a public name, a public IP or a sub-CA. Verifiers reject each one with the exact constraint error.
- **A stolen issuing-CA key** cannot mint a public name.
- **Tiers:** each tier issues only its own, and the issuing CA stops once an intermediate would outlive it.
- **Publication signing** verifies as a publisher only, and is never a server or a CA.
- **Revocation:** revoking a facility intermediate, or the issuing CA above it, fails the chain with `CertRevoked`. A forged CRL fails it with `InvalidCrlSignatureForPublicKey`.
