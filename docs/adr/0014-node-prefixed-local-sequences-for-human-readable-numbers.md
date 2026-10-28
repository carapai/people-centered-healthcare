# Human-readable numbers come from node-prefixed local sequences, not reserved pools

MRNs, receipt and invoice numbers, lab accession numbers and program numbers are minted offline as `{facility code}-{node short code}-{sequence}`. The counter is held in the minting node's own store and incremented in the same transaction as the record. Each facility has exactly one Facility Authority Node (ADR 0012), and every desktop that mints offline has its own short code, so these numbers cannot collide and never run out. DHIS2 and OpenSRP use pre-reserved ID pools instead, and both document the failure modes: pools run dry so creation stops offline, and reserved values expire. We keep central reserved blocks only for a number an external authority requires to come from one central sequence. Those blocks have a low-water alert. When one runs out, the record is still created, with the number marked pending until sync.

## Consequences

- Each identifier type's format (prefix, padding, check digit, yearly reset) is a Published Definition in the country configuration package.
- A number reveals which node minted it. This is accepted and useful for tracing.
