# An edge gives its desktops their staff for offline sign-in, encrypted to the device key

This extends ADR 0059, which covers credentials from central to an edge, to a desktop under an edge. Such a desktop signs staff in at the edge while it can reach it (ADR 0010). After 30 s without the edge it works offline (desktop-client.md), and then it signs staff in itself, with the same `health-auth` rules: snapshot expiry, lockout, the clock floor and the idle lock. For that it needs the staff's hashes and snapshots. desktop-client.md says which staff ("a desktop under an edge gets them for staff assigned to its service points") but not how they get there. The maintainer chose this mechanism (2026-11-12, #133).

- **The request.** The desktop calls `POST /api/desktop/accounts` on the edge.
  - It is behind the device gate: the request carries a DPoP proof by the desktop's enrolled device key.
  - The body names the device's public key as a JWK. The edge refuses any key whose thumbprint is not the enrolled device's, so the hashes can only be encrypted to the key that proved the request.
- **The bundle.** The edge answers with ADR 0059's provisioning bundle format.
  - **For the device:** `node_id` is the device id.
  - **The staff:** those whose snapshots at the facility hold a facility-wide role, or a role at the device's location. The location is the one the admin assigned at enrollment, named by its Location id. Expired snapshots are left out.
  - **The hashes:** each is decrypted from the edge's pepper, then encrypted to the device key (ECDH P-256, HKDF-SHA-256, XChaCha20-Poly1305, the account id as associated data), as central encrypts them to an edge.
  - **The snapshots:** passed on exactly as central signed them.
  - **The signature:** the edge's facility intermediate key, the key its TLS leaves and grants already chain to.
- **What else the answer carries:**
  - the facility;
  - central's snapshot public keys;
  - the signer's public key.
- **The desktop's import.**
  - It opens the bundle with its device key and checks the bundle's signature against the signer key. It checks every snapshot against central's keys, never the edge's.
  - It re-encrypts each hash under its own pepper, kept in the OS keystore.
  - It keeps central's keys for sign-in.
  - It takes the facility the first time. After that, an answer naming another facility is refused.
- **When it imports:** after enrollment, and then hourly, after a sync round that reached the edge.
- **Trust rests on the pinned edge.** The answer comes over TLS to the edge, verified against the CA named at enrollment (`health-desktop enroll --ca`, now required). The edge's certificates chain to the national root, not the OS trust store. The signer key and central's snapshot keys are taken from the same answer, so a desktop trusts them because it trusts its edge, which is its authorization server already.

## Considered Options

| Option | Decision |
|---|---|
| **The edge re-encrypts its staff's hashes to the enrolled device key, filtered by location** | **Chosen** |
| Central exports a bundle per desktop, which the edge relays | Rejected for now: central does not know desktops (enrollment is the edge's, ADR 0009), and would have to before it could encrypt to them |
| No offline sign-in: a session signed in at the edge stays usable until it expires | Rejected: new sign-ins wait for the edge, which fails #133's acceptance (a desktop keeps working through a LAN outage) |

## Consequences

- **A desktop now holds Argon2id hashes** for the staff at its location, as an edge does for its facility. ADR 0010's mitigations apply:
  - encrypted at rest under a pepper in the OS keystore;
  - only the staff it needs;
  - expiring snapshots.

  Revoking the device stops the next export, and snapshots bound how long an old one lasts.
- **The import adds and updates; it does not remove.** A member of staff who leaves the location keeps a stored hash until their snapshot expires, after which sign-in refuses them. A purge of accounts no longer exported is later work.
- **`health-auth`:** `Provisioning::export` builds the bundle, and `Provisioning::with_bundle_verifier` lets an import check the bundle with one key and the snapshots with another.
