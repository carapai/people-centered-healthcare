# LAN TLS uses a private PKI with a name-constrained intermediate per facility

LAN clients reach the Facility Edge Node over HTTPS backed by a private PKI with three tiers:
- **Root:** an offline national root, held as country configuration.
- **Intermediate:** one per facility, issued to its edge at provisioning, valid 5 years, and name-constrained to that facility's DNS names and the private IP ranges.
- **Leaf:** a server certificate the edge issues to itself, valid 397 days and renewed locally without internet.

Browsers need a trusted secure context for service workers and non-extractable WebCrypto keys. No public CA can issue a certificate for an offline private address. Public DNS-01 certificates expire too fast for sites offline for months (≤200 days now, ≤47 days by 2029). Tauri clients pin the facility intermediate and the node ID, so they need no OS trust.

## Considered Options

- **Plain HTTP:** no secure context and no encryption on Wi-Fi. Rejected.
- **Self-signed certificates with click-through:** trains users to ignore TLS warnings. Rejected.
- **Public domain with DNS-01:** renewal needs internet. Deferred as an optional extra for connected sites.
- **A per-facility root:** avoids national governance, but every device must trust a separate root per facility. Rejected in favour of a single national root.

## Consequences

- Browser clients are limited to enrolled PCs where the root is installed (GPO, Firefox policy or the enrollment installer). iOS and unmanaged phone browsers are out of v1.
- Revoking a compromised facility intermediate reaches Tauri clients through sync. Browsers cannot check revocation offline and rely on the leaf lifetime plus re-provisioning, which is a known limitation.
- Who holds the national root (MoH or the platform operator) is a governance question. Until it is settled, the platform operator holds it for Uganda.
