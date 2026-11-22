# Area

The session's place and its area (ADR 0094; #869).

- **`AreaPage`** (`/area`, linked from the home page at central): the place the session acts for, chosen at sign-in, and its level. At a district, region or the national place it lists the facilities in the area, grouped by district, with their NHFR level of care and whether they are open or closed. At a facility it names the facility.
- **On the server:** `GET /api/places/area` (central-server's `places`), from the facility registry's Organizations (`health_registries_app::places`). The facilities listed are what the area's figures roll up over.
- An area session reads summaries, Dashboards and Reports, never patient-level records: the API refuses its patient reads and searches.

## Tests

- `test/area.test.ts`: the API and the grouping by district.
