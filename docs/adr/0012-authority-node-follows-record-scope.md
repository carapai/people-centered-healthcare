# Each record has one Authority Node, derived from its scope and never switched implicitly

Every record instance has exactly one Authority Node: the node whose acceptance makes a version canonical.
- **Global-scope records** have central as their Authority Node.
- **Facility-scope records** have the facility's single Facility Authority Node, recorded in the facility registry. Depending on the facility's deployment mode, this is central (online web), its Facility Edge Node, or one standalone desktop.
- A facility-scope record stays authoritative at the facility that created it permanently. Authority follows the record's facility, not the patient.

Non-authoritative nodes write Candidate Versions, each carrying its parent version. Ledgers (stock, finance) are append-only operations with a single writer per scope.

Authority moves only through explicit hand-overs:
- an **Authority Lease** from an edge to a desktop, for outreach;
- a central-run change of a facility's Authority Node;
- a program transfer, where the old facility closes its record and the new facility opens its own.

Each hand-over requires the outbox to be drained first. We chose this so that every conflict has one place where it is decided, without version vectors, as the sync research recommends for a fixed tree. It also satisfies §90's rule that write authority never switches silently while operations are pending.

## Considered Options

- **Authority by resource type per tier** (for example, "Observations are authoritative at the edge"). Rejected: it cannot express facilities without an edge, or outreach desktops.
- **Authority follows the patient** (records move to the patient's current facility). Rejected: it rewrites history across facilities and needs multi-writer concurrency metadata.
- **Multi-master with version vectors.** Rejected as unjustified for a fixed tree with one writer per record.

## Consequences

- Another facility, or central, never edits a facility's clinical records. They add their own records, or route a request to the authority.
- A Conflict Record is resolved at the record's Authority Node by roles holding the resolution permission. Patient identity conflicts are resolved at central.
- If an Authority Lease expires before it is returned, the leased scope freezes at both ends rather than falling back silently.
