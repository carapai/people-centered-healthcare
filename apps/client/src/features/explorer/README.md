# Explorer

The data explorer (ADR 0075; analytics.md § Dashboards; #599).

- **`ExplorerPage`** (`/explore`, linked from the home page): pickers on the left (indicators, a period type and a range of its periods, places in the session's area) and a live column chart on the right, with the charted figures as a table. The card says where the figures were computed (rolled up at central, or on this facility), when the newest summary was computed, how many facilities reported for the latest period, which facilities are missing with their last sync (never counted as zero), and why a median or distinct count is absent above facility level.
- **On the server:** `GET /api/analytics/explorer` and `POST /api/analytics/explore`, at central (central-server's `analytics`) over the facilities' synced summaries, and on an edge (`health-node-api`'s `analytics`) over its own, offline. Both need `reporting.view`.
- The chart is drawn as SVG here, as the dashboards prototype did; no charting library is used.

## Tests

- `test/explorer.test.ts`: standard period ids, the range pickers, the API and the chart's series.
- `e2e/explorer.spec.ts`: `e2e-district` explores Kampala on the local backend.
