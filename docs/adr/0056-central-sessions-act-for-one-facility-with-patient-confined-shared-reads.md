# Central sessions act for one facility, other facilities' records open only to patient-confined reads, and purpose of use is a header defaulting to TREAT

**The decisions:**

**1. One facility per session.**
- A user signed in at central acts for **one facility**, chosen at sign-in when they hold role assignments at more than one.
- The access token carries `facility`, as edge tokens already do.
- The subject's permissions and programs come from the user's role assignments at that facility.
- National roles (privacy officer, national admins) are assigned at a reserved facility code, `central`.

**2. Shared reads are confined to one patient** (our reading of MASTER_PROMPT §97, "Do not expose unrelated patient records merely because multiple facilities share the central platform"). At central:

| Access to another facility's facility-scope record | Allowed? |
|---|---|
| Reads, vreads and instance history | Yes: central is the shared record (write-authority.md). Labels and purpose of use still apply. |
| Searches confined to one patient (`patient=`, or a `subject=` naming a Patient) | Yes |
| Searches across patients, and type or system history | No: only the session's own facility's records |
| Writes | Never: authority stays with the record's facility (ADR 0012) |

- On an edge, facility isolation stays strict, because an edge holds only its own facility's scope.
- Break-glass still opens one patient's sensitive records, everywhere.

**3. Purpose of use is a per-request header.**
- Requests declare it in `X-Purpose-Of-Use`, as a v3 PurposeOfUse code.
- Without the header, the purpose is `TREAT`.
- An unknown code gets 400.
- A declared purpose never opens a sensitive label by itself: the program assignment is still required.

We chose this because:
- One facility per session keeps the subject's attributes well defined, so the access policy decides the same way at central and on an edge.
- Confining shared reads to one patient lets clinicians see a patient's whole record across facilities without letting anyone browse other facilities' patient lists.
- A header lets the client change purpose (for example to payment) without signing in again, and the default keeps ordinary clinical use free of friction.

## Considered Options

| Option | Decision |
|---|---|
| No facility at central, with the union of all assignments | Rejected: the subject's facility becomes undefined, and the policy would decide differently at central and on an edge |
| Opening every other facility's record to reads at central | Rejected: it allows browsing other facilities' patients, against §97 |
| Requiring a care relationship (an Encounter or EpisodeOfCare at the session's facility) | The strongest reading of §97, but the relationship is not modelled yet. Revisit when it is |
| Strict isolation at central, with break-glass for any other facility's record | Rejected: it makes routine continuity of care an emergency, and floods break-glass review |
| Purpose of use chosen once at sign-in | Rejected: changing purpose would mean signing in again |
| Purpose of use with no default | Rejected: every clinical request would need the header to see ordinary records' labels |

## Consequences

- **Sign-in:** central's sign-in asks for the facility when the user holds assignments at more than one, and tokens carry it.
- **Access policy:** `Pdp` takes a scope (central or edge). At central, facility isolation on reads applies only when the access is not confined to one patient.
- **Searches:** a search across patients at central needs a facility clause. Until records name their facility in searchable form (#211), such searches return only records that are not facility-scope.
- **Opening a record by id:** a user who knows a record's id can read it at central. The audit of every sensitive read and the break-glass review are the controls on this.
