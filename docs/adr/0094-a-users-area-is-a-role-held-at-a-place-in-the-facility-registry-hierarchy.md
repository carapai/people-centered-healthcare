# A user's area is a role held at a place in the facility registry's hierarchy

ADR 0075 says district, regional and national users see "their assigned area", but nothing said what a place is or how a user gets an area. ADR 0056 lets a central session act for one facility, and puts national roles at a reserved facility code. The facility registry is named in facility-configuration.md but has no source of places yet.

We decided:

**1. The facility registry holds the place hierarchy.**
- Every place is an Organization: facilities, districts, regions, and one national Organization for the Ministry of Health.
- `Organization.partOf` links each place to the one above it: facility → district → region → national.
- A facility's Organization carries its NHFR identifier, and its Location carries the NHFR coordinates.
- The registry is global and read-only on edges (write-authority.md), so every node knows every place and its parents.

**2. The NHFR is the source of places.**
- An administrator imports the NHFR's facility list at central. The import shows what is new, changed or closed before it is applied, and is re-run when the NHFR changes.
- Districts and regions, with their boundaries, ship in the national core package (ADR 0075). The import links each facility to its district by code and refuses a district the core package does not hold.
- A facility closed in the NHFR becomes `active: false`. It is never deleted, because its records and summaries still refer to it.

**3. A role is held at a place.**
- A PractitionerRole's `organization` may be a facility, a district, a region or the national Organization.
- A role held at a place gives its permissions over that place and every place below it. That place and everything below it is the user's **area**.
- The reserved facility code `central` of ADR 0056 becomes the national Organization.

**4. A central session acts for one place.**
- ADR 0056's "one facility per session" becomes "one place per session". A user with roles at several places chooses one at sign-in. The access token's `facility` claim carries the place's code, so a facility session is unchanged, and a new `place-level` claim says whether it is a facility, district, region or national.
- A session at a district, region or national place reads only what the area grants: Indicator Reports and Dashboards (ADR 0075), Reports for sign-off and sending (ADR 0069), and the registries.
- It never reads patient-level records by virtue of the area. Patient-level access still needs a role at the facility, as ADR 0056 says.

We chose this because it reuses the role model the platform already has (PractitionerRole, roles at a facility). The same hierarchy then serves dashboards, the rollup of summaries, maps, DHIS2 org unit checks (ADR 0069) and population checks (ADR 0071). Taking places from the NHFR keeps the platform's facility list the same as the country's.

## Considered Options

- **An area code stored on the user's account, with the facility list in the national core package.** Rejected. It is a second, parallel way of granting access, outside the roles that every policy decision already reads. A facility list in a package would also need a new package version for every NHFR change.
- **A session holding every place the user has a role at.** Rejected for the reason ADR 0056 gives: the subject's attributes would be undefined, and the policy would decide differently at central and on an edge.
- **Areas that also open patient-level records to district staff.** Rejected. ADR 0075 limits dashboards to summaries, and ADR 0056 confines shared reads to one patient.

## Consequences

- The facility registry needs an NHFR import, the Organization hierarchy and an area query ("this place and everything below it").
- Access tokens gain a `place-level` claim. A facility session behaves exactly as before.
- Role assignment in the administration screens offers districts, regions and the national place as well as facilities.
- Dashboards, the rollup of summaries and maps read the area from the session's place.
