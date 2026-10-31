# Facility-scope records carry a facility tag that the server stamps, and searches filter by it

**The decision:**
- **The tag.** A facility-scope record names its facility in `meta.tag`, with system `urn:health:facility` and the facility code as the code. The facility code is the session's facility (ADR 0056), which is also the facility Organization's identifier.
- **Which types are facility-scope.** A fixed table in code (`health_access_policy::FACILITY_SCOPE_TYPES`), taken from the data authority matrix in write-authority.md:
  - clinical records, documents and program records;
  - workflow instances;
  - charges and invoices.

  Changing the table is a code change and an ADR. Facility-internal Locations, devices and stock join the table with their own contexts.
- **Stamping.** The server stamps the tag on create, from the session's facility, under access control.
  - A tag naming another facility is refused with 403, because authority stays with the creating facility (ADR 0012).
  - An update keeps the facility of the version it replaces, and can never change it.
- **Searches.** A search of a facility-scope type adds a `_tag` clause for the session's facility. The exceptions: a search confined to one patient at central (the shared record, ADR 0056), and break-glass for that patient.
  - The standard `Resource-tag` parameter must be indexed for every facility-scope type; central registers it itself.
  - Without the index the clause matches nothing, so a misconfigured node fails closed.

We chose this because:
- **One convention for every type.** It is read and indexed the same way on all of them, through a standard search parameter, with no profile changes.
- **Server-side stamping.** Clients cannot place a record under another facility's authority, and facility isolation then sits inside the database query.

## Considered Options

| Option | Decision |
|---|---|
| The resource's own Organization or Location reference (`Encounter.serviceProvider`, `Observation.performer`, …) | Rejected: it is a different field per type, absent on some, and not uniformly indexable |
| `meta.source` with the node URI | Rejected: a node is not a facility. Central writes for every web-only facility (mode A), and a desktop may serve one facility through several nodes |
| A country-configurable list of facility-scope types | Rejected: authority and sync would depend on configuration that could differ between nodes |
| The server silently overwriting a client's tag | Rejected: it hides a client bug or an attempt to write under another facility |

## Consequences

- **Search parameters:** every facility-scope type needs `_tag` in the search registry, and central registers it at startup.
- **Writes outside access control:** system jobs and migrations write facility-scope records without the guard, so they must stamp the tag themselves.
- **Sync:** synchronization carries the tag unchanged, and a record arriving at central keeps its facility.
- **Facility codes:** the facility registry gives each facility Organization its facility code as an identifier.
