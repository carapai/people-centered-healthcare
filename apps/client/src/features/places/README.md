# Places

Administering the facility registry's places at central (ADR 0094; #873). Both pages are linked from the home page at central; each answers someone whose role does not include it with a quiet explanation, not an error.

- **`RolesPage`** (`/places/roles`), for an administrator whose role gives `access.administer`:
  - Look a user up by username and see the places they hold roles at: each place's roles and when its permission snapshot expires.
  - Assign roles at a place: a facility, a district, a region or the national place, within the administrator's own area (the session's place and everything below it). The roles chosen replace those held there. The user signs in for the place from their next sign-in.
  - Withdraw every role at a place in the administrator's area, after a confirming click.
  - On the server: `GET /api/assignments/options`, `GET /api/assignments/users/{username}`, and `PUT` / `DELETE /api/assignments/users/{username}/{place}` (central-server's `assignments`). Each assignment is a newly signed permission snapshot, audited. A role carrying a permission the administrator's own session lacks is refused (#888); the page names the role and the permission.
- **`ImportPage`** (`/places/import`), for someone whose role gives `places.manage`:
  - Choose the NHFR facility export (CSV) and review what importing it would do: the regions and districts of the national core package to add, and the facilities new, changed (with the fields that change), closed and not in the file. Nothing is written until *Import*.
  - A file with a district the national core package does not hold is refused, with its problems listed.
  - On the server: `POST /api/places/nhfr/preview` and `/apply`, sent as `{"csv": "…"}`.
- **`ReportingPage`** (`/places/reporting`; #560, ADR 0069), for someone whose role gives `places.manage` (editable) or `reporting.view` (read only, so the district sees which forms it no longer keys):
  - Each facility in the session's area with the DHIS2 org unit ID the NHFR lists for it, its HMIS org unit ID, and the forms (DHIS2 data sets of central's recorded copy of the instance's metadata) reported through the platform.
  - An ID the NHFR does not list, lists for another facility, or that two facilities share, or a facility the NHFR gives another ID, is flagged on the facility; it is kept.
  - A bad ID, an unknown data set or a form without an org unit ID is refused with central's problems.
  - On the server: `GET /api/places/reporting` and `PUT /api/places/reporting/{facility}` (`{"orgUnit": "…" | null, "forms": ["…"]}`).
- **`api.ts`** holds the queries and the calls.

## Tests

- `test/places.test.ts`: the API calls, refusals, and the review's helpers.
- `e2e/places.spec.ts`: a national administrator previews and imports the NHFR sample, assigns a role at a district to a user, who signs in for it, and sets a facility's HMIS org unit ID (flagged when the NHFR does not list it) and HMIS 105 reported through the platform, which Kampala's district user then reads.
