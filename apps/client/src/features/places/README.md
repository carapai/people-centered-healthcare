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
- **`api.ts`** holds the queries and the calls.

## Tests

- `test/places.test.ts`: the API calls, refusals, and the review's helpers.
- `e2e/places.spec.ts`: a national administrator previews and imports the NHFR sample, and assigns a role at a district to a user, who signs in for it.
