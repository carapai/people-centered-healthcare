# The platform is the client registry; source Patients link to a central golden record, and nothing is merged by default

`client-registry` at central is the master patient index for v1. It exposes IHE PDQm, PIXm and PMIR interfaces, and has a federation port so that it can become a client of an external national registry if MoH later mandates one.

**Source Patients.** Every registration is a **source Patient** and keeps its own id: facility, provisional offline or self-registered. Clinical records reference the source Patient, never the golden one.

**Golden record.** Central creates and owns a separate **golden Patient**, which carries the ECID.
- Each source links to it with `refer`, and the golden record links back with `seealso`.
- Search, the Care Journey and the shared record go through the golden record.
- Golden field values follow survivorship by trust: NIRA-verified, then staff-verified in person, then staff-entered, then self-reported; most recent within a level. A lower-trust value never overwrites a higher-trust one.

**Linking and unlinking.** Linking is reversible. Unlinking is an audited reviewer action that splits the golden record and leaves an adjudication marker, so automatic matching does not re-link the pair.

**Merging.** `$merge` (`replaced-by`, `active=false`) is used only for two source records from the **same facility** that are the same person, after human approval and a preview. References are never rewritten.

We chose this for three reasons:
- Offline-created references must stay valid forever.
- False links are a patient-safety risk, so every link must be undoable.
- Uganda has no confirmed national health client registry in production. Owning the registry behind standard interfaces keeps either future open.

## Considered Options

- **Merge on match** (rewrite or deactivate the duplicates). Rejected: it is irreversible in practice, and it invalidates offline references.
- **First registration becomes the golden record.** Rejected: a facility's record would carry national identity, and correcting it would mean editing another facility's data.
- **Federate with an external registry from day one.** Deferred: no target registry is confirmed. The federation port keeps it possible.

## Consequences

- One person has at least two Patient resources (a source and the golden record).
- A reconciled provisional Patient gains `refer` to the golden record. ADR 0020's note is amended accordingly.
