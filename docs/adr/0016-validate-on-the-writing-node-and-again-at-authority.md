# Resources are validated on the writing node and again at the Authority Node and central

Each of our own writes is validated fully on the node that writes it, whichever node that is:
- structure;
- core FHIRPath invariants;
- active profiles from the node's configuration snapshot;
- required bindings.

A write that fails is rejected. The Authority Node and central re-run the same validation on ingest, and an item that fails there gets a per-item `rejected` status rather than being dropped silently. Sources outside our control (integrations and R4 adapters) are rejected only on structural failure. A profile or terminology failure from them goes to a quarantine review queue instead. A code missing from the node's local terminology snapshot is a warning offline, re-checked at central.

**Refined by ADR 0051:** structural checks run on raw JSON before typed parsing, and an external structural failure keeps its raw payload in the Inbound Dead Letter.

**Refined by ADR 0053:** tier 3 is our own snapshot-driven validator. The Java validator stays a CI oracle only.

We want invalid clinical data stopped at the point of entry, including on an offline edge or desktop. Because the same Rust validation code runs everywhere (ADR 0001), the result does not depend on where the data was written. Strict rejection would discard legitimate data from systems we don't control, so external data is quarantined instead.

## Consequences

- Profile validation needs the profile snapshots and terminology to be in each node's configuration package.
- The HL7 Java validator is used only as a CI oracle and is never shipped.
