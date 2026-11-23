# Dashboards

Saved Visualizations and Dashboards (ADR 0075; analytics.md § Dashboards; #602).

- **`DashboardsPage`** (`/dashboards`, linked from the home page at central): standard Dashboards (from the Reporting package, #603; none yet) and saved ones on the left, marked *Yours* or *Shared with you*, and the picked one's Visualizations on the right, each drawn for the viewer's own area (`forViewer`) with *Open in explorer*. A user holding `analytics.author` makes Dashboards here, and shares their own with place levels (*Every district*), roles and people by username; sharing never widens what a viewer reads.
- **On the server:** `GET /api/analytics/dashboards`, `POST /api/analytics/dashboards`, `PUT /api/analytics/dashboards/{id}` and `POST /api/analytics/visualizations`, at central only (central-server's `analytics`). Saving needs `analytics.author`.
- **Saving** is in the explorer (*Save Visualization*, `features/explorer`).

## Tests

- `test/dashboards.test.ts`: places that follow the viewer, *Open in explorer*'s search, and the API.
- `e2e/dashboards.spec.ts`: `e2e-district` saves a Visualization of each area under Kampala onto a new Dashboard and shares it with every district; `e2e-wakiso` opens it, sees Wakiso's facilities, cannot save, and opens it in the explorer.
