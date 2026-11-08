# Nodes sign their sync requests with the facility intermediate's key

This settles what ADR 0058 left open ("Sync mTLS node certificates are not decided here") and replaces mTLS on the node-to-central sync hop (synchronization.md, deployment-topology.md). Sync still runs over TLS: central's certificate is verified as for any other request. What changes is how a node proves who it is: **every `/sync/v1` request carries a JWS signed with the node's facility-intermediate key**, the key it already holds and central already records.

- **The token.** A compact ES256 JWS with `typ` `health-sync+jwt`, sent as `Authorization: Health-Node <jws>`. Its claims are:
  - `node`, the node id;
  - `iat`, the signing time;
  - `htm` and `htu`, the request's method and path;
  - `bh`, the sha-256 of the request body.

  A token is good for that one request only, as a DPoP proof is (ADR 0009).
- **Central's check, on every request.**
  1. The node is in the registry and `active`. A decommissioned node is refused at once, with no certificate or CRL to wait for.
  2. The signature verifies against the node's `intermediate_key`.
  3. `iat` is within 5 minutes of central's clock.
  4. `htm`, `htu` and `bh` match the request.

  A copied token replays only the same request within those 5 minutes. Sync answers a repeated push from its receipts and a repeated pull with the same data (ADR 0019), so a replay changes nothing.
- **Scope follows identity.** The registry gives the node's facility, and the facility gives its Sync Scope. Nothing the node sends can widen it.
- **Escrow and sync share a verifier.** Recovery-key escrow already sends a JWS signed with the same key (`health-recovery+jwt`). Central checks both through one path, differing only in `typ` and claims.

We chose this because central's TLS is terminated in front of `central-server`, which serves plain HTTP. Client certificates would need that terminator to verify them and forward the identity. They would also need a certificate issued to every node and renewed, and a revocation check. The node already holds a provisioned key whose public half central records, and the registry already answers "is this node in service" at once.

## Considered Options

| Option | Decision |
|---|---|
| mTLS with a node client certificate issued at provisioning | Rejected: it needs client-certificate verification at central's TLS terminator, identity forwarding, issuance and renewal for every node, and a revocation path; a decommissioned node stays trusted until the CRL reaches the terminator |
| A bearer token issued to the node at provisioning | Rejected: a long-lived secret on the wire that, once copied, works from anywhere until rotated |
| **Each request signed with the facility-intermediate key, checked against the registry** | **Chosen** |

## Consequences

- **The facility-intermediate key** now signs sync requests as well as edge TLS leaves and recovery escrows. A compromised key is handled as before: decommission the node or re-provision it, which replaces the key on record.
- **Clocks.** A node whose clock is more than 5 minutes off central's cannot sync, and central's refusal names the skew, so it is visible and fixable. The edge's clock floor (offline-auth.md) only guards against its clock going backwards, so NTP stays a requirement for edges.
- **Standalone desktops (mode B)** will sign with the key they hold from provisioning, in the same way. Desktops under an edge are unchanged: they use device enrollment and DPoP on the LAN (ADR 0009).
- **Documents.** synchronization.md, deployment-topology.md, overview.md and security.md say "node-signed requests" where they said mTLS.
