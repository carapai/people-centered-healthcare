# The edge is always the authorization server for LAN clients, built by the same Rust code as central

LAN clients always authenticate to the Facility Edge Node, whether or not central is reachable. Central stays the identity provider of record: it provisions facility staff credential hashes and signed permission snapshots to the edge. Central's OIDC provider and the edge's local authorization server are one Rust implementation (`health-identity` plus `health-auth`), not Keycloak or Ory. We chose a single login path because a "pass through to central, fall back locally when offline" design leaves two code paths, and the fallback would be rarely exercised and under-tested exactly when it matters. We chose one implementation because no off-the-shelf IdP can run as the edge, and two implementations would drift in semantics (ADR 0001: the same code path everywhere).

## Considered Options

- **Pass-through to central when online, with a local fallback.** This still needs everything the local server needs, plus mode-switch logic. Rejected.
- **Long-lived central tokens cached on devices.** This violates the ban on indefinitely cached permissions and breaks shared workstations. Rejected.
- **Keycloak or Ory at central plus a Rust edge.** Two implementations to keep consistent. Rejected. An upstream OIDC IdP (for example, a national health-worker registry) can still be federated into central later.
- **Capability tokens (Biscuit).** Kept as a possible encoding for the signed snapshot, not as a separate architecture.

## Consequences

- The edge holds Argon2id hashes for every staff member of its facility, which makes it a high-value target. Mitigations: encryption at rest, a pepper in the OS keystore, provisioning only the facility's staff, and remote revocation on reconnect.
- We build and maintain an OIDC/SMART v2 authorization server ourselves, including DPoP, PKCE and SMART Backend Services.
