# Every LAN device is enrolled, and its key is the DPoP key for every token

A LAN client device can do nothing beyond the app shell and enrollment until a facility admin approves it at the edge. The device generates a key pair that cannot be exported: in the OS keyring for Tauri, or as a non-extractable WebCrypto key for browsers. The edge's device registry records that public key, and every token is DPoP-bound to it. Revocation is enforced by the edge removing the key from its registry, not by CRL or OCSP, and access ends within one short token lifetime. Comparable systems document no per-device revocation on a LAN, and browsers will not check private-CA revocation offline. Binding devices to the DPoP key that the offline-auth decision already requires gives device control without client certificates, whose browser UX is unworkable.

## Considered Options

- **mTLS client certificates:** browser selection and installation UX is poor, and revocation still needs edge enforcement. Rejected.
- **Network-level control only** (Wi-Fi password, firewall): any device on the network could then try staff credentials. Rejected as the sole control; it is kept as an extra layer.

## Consequences

- Stolen staff credentials are useless from an unenrolled device.
- Device registrations and revocations sync between the edge and central, like other security records.
