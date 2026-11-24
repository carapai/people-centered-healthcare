# Reports

Running a Report (ADR 0070, ADR 0072; analytics.md § Reports; #557).

- **`ReportsPage`** (`/reports`, linked from the home page): the Reports the session may run on the left, each from its Reporting package version, with a place in the session's area at a level the Report runs for and a closed period of its type; the Report on the right, read from stored Indicator Reports: each table section laid out by the Report designer's `layout.ts`, where and when the figures were computed, the facilities reported out of expected and the missing ones with their last sync (never counted as zero), *Print or save as PDF* and *Download CSV*.
- **One grid for the screen and the CSV** (`api.ts`' `sectionGrid`, `runGrids`, `reportCsv`), so the export holds exactly the cells on screen. A Total is the server's whole, rolled up from summed parts, never an addition of percentages.
- **On the server:** `GET /api/analytics/reports` and `POST /api/analytics/reports/run`, at central (central-server's `analytics`) over the facilities' synced summaries for any place in the session's area, and on a Facility Edge Node (`health-node-api`'s `analytics`) over its own, offline, for its facility only. Both need `reporting.view`.
- **Not yet** (#557, part 2): line lists on the edge (at central they never run: they are patient-level), and cells crossing several Disaggregations.

## Tests

- `test/reports.test.ts`: the grid of a run (band columns, Total, % change, a missing indicator), the CSV holding the screen's cells, the closed periods of each type, and the API.
- `e2e/reports.spec.ts`: `e2e-district` runs the sample ANC report for Kampala (its places only, Kitebi HC III missing, the Total the sum of the bands) and downloads the same cells as CSV.
