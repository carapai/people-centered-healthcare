# Dashboards

Saved Visualizations and Dashboards (ADR 0075; analytics.md § Dashboards; #602).

- **`DashboardsPage`** (`/dashboards`, linked from the home page at central): standard Dashboards (from the Reporting package, #603, marked with the package version) and saved ones on the left, marked *Yours* or *Shared with you*, and the picked one's Visualizations on the right, each drawn for the viewer's own area (`forViewer`) with *Open in explorer*. A user holding `analytics.author` makes Dashboards here, and shares their own with place levels (*Every district*), roles and people by username; sharing never widens what a viewer reads.
- **Standard Dashboards** (#603) come from the newest published version of each Reporting package (`configuration/dashboards/<id>.json`), shown to everyone with `reporting.view`, their places following the viewer and their periods the last few up to today. They cannot be changed here: a user holding `analytics.author` uses *Save as* to make their own saved copy (`POST /api/analytics/dashboards` with `copyOf`).
- **On the server:** `GET /api/analytics/dashboards`, `POST /api/analytics/dashboards`, `PUT /api/analytics/dashboards/{id}` and `POST /api/analytics/visualizations` at central (central-server's `analytics`). Saving needs `analytics.author`.
- **On a Facility Edge Node** (#879): the tab lists the active Reporting packages' standard Dashboards, offline (`GET /api/analytics/dashboards` from health-node-api's `analytics`), each item drawn over the node's own summaries; *each area under* the facility is the facility itself. Saved Dashboards are central's, so the page says so and nothing is authored there.
- **Saving** is in the explorer (*Save Visualization*, `features/explorer`).

## Tests

- `test/dashboards.test.ts`: places that follow the viewer, *Open in explorer*'s search, and the API.
- `e2e/standard-dashboards.spec.ts`: the published sample Reporting package's *ANC overview* shows for `e2e-district` and `e2e-wakiso`, each over their own district; `e2e-district` saves it as their own copy.
- `e2e/edge/dashboards.spec.ts`: on an edge with the sample Reporting package active and first ANC visits recorded today, the staff member opens the ANC overview offline and sees the facility's figures.
- `e2e/dashboards.spec.ts`: `e2e-district` saves a Visualization of each area under Kampala onto a new Dashboard and shares it with every district; `e2e-wakiso` opens it, sees Wakiso's facilities, cannot save, and opens it in the explorer.
