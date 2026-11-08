# Facility intermediates come from an online issuing CA under the offline national root

This amends ADR 0008, and ADR 0024's publication-signing certificate. ADR 0008 has a three-tier PKI whose root is offline, yet central issues a facility intermediate at every edge provisioning, and an offline key cannot sign on demand. The PKI therefore has four tiers:

| Tier | Holder | Issued by | Valid | Constraints |
|---|---|---|---|---|
| National root | Offline, air-gapped; custody in the PKI governance fog | Itself, at the root ceremony | 25 years | CA, path length 2 |
| Facility issuing CA | Central's HSM or KMS, non-exportable | The root, at a ceremony | 10 years; it issues only in its first 5, then a successor takes over | CA, path length 1; serverAuth EKU; names limited to `.local`, the country's configured facility DNS suffixes, and the RFC 1918 and ULA IP ranges |
| Facility intermediate | The edge's OS keystore | The issuing CA, at provisioning | 5 years | CA, path length 0; serverAuth EKU; names limited to that facility (ADR 0008) |
| Edge server leaf | The edge | The facility intermediate | 397 days | As ADR 0008 |

- **Provisioning.** The edge generates the intermediate's key in its keystore and sends central a certificate signing request. Central takes only the public key from it: the issuing CA sets the subject, names, lifetime and extensions from the facility registry and ignores anything the request asks for. The key never leaves the edge.
- **The publication-signing certificate** stays directly under the root, issued at a ceremony. It is not a CA (path length does not apply) and carries only the code-signing EKU, so it can sign packages but never a certificate or a TLS session.
- **Revocation.**
  - The root signs a CRL that covers issuing CAs and publication-signing certificates. It is reissued at each ceremony, and at least every 13 months.
  - The issuing CA signs a CRL that covers facility intermediates. Central reissues it on every revocation.
  - Both travel in the sync revocation lane. Tauri clients and nodes refuse a chain that either CRL revokes. Browsers still cannot check revocation offline (ADR 0008).

We chose this because provisioning has to be immediate, without waiting for a ceremony, and the root has to stay air-gapped, because every enrolled browser trusts it for any name. The issuing CA's name constraints bound what a compromise of central's key could mint: certificates for `.local`, the facility suffixes and private addresses, never for a public name.

## Considered Options

| Option | Decision |
|---|---|
| Sign facility intermediates in batches at scheduled offline root ceremonies | Rejected: a new or replacement edge would wait weeks for TLS, and the lost-machine RTO of under 1 h (ADR 0007) would be out of reach |
| Keep the root in central's HSM and sign intermediates with it directly | Rejected: the root would no longer be offline, and a compromise of central would expose the anchor that every browser trusts |
| An issuing CA without name constraints | Rejected: anyone holding its key could mint a certificate for any public name that enrolled browsers would accept |

## Consequences

- **Ceremonies.** The root ceremony issues the issuing CA, the publication-signing certificate and the root CRL. A ceremony is due every 5 years for a successor issuing CA, at least every 13 months for the root CRL, and yearly for publication signing (configuration.md).
- **Chains.** An edge serves the leaf, its facility intermediate and the issuing CA. Tauri pins stay on the facility intermediate (ADR 0008).
- **Facility DNS names** must fall under one of the issuing CA's configured suffixes. Adding a suffix means issuing a new issuing CA.
- **Sync mTLS node certificates** are not decided here.

  > Settled by ADR 0062: nodes sign their sync requests with the facility-intermediate key; there are no sync client certificates.
