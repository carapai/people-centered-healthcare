# Explorer

The data explorer (ADR 0075; analytics.md § Dashboards; #599).

- **`ExplorerPage`** (`/explore`, linked from the home page): pickers on the left (indicators, a period type and a range of its periods, places in the session's area) and a live column chart on the right, with the charted figures as a table. The card says where the figures were computed (rolled up at central, or on this facility), when the newest summary was computed, how many facilities reported for the latest period, which facilities are missing with their last sync (never counted as zero), and why a median or distinct count is absent above facility level.
- **On the server:** `GET /api/analytics/explorer` and `POST /api/analytics/explore`, at central (central-server's `analytics`) over the facilities' synced summaries, and on an edge (`health-node-api`'s `analytics`) over its own, offline. Both need `reporting.view`.
- **Visualization types** (#600): chart-type buttons above the card switch between a column chart, a line chart, a table (places × indicators, or indicators × periods) and single numbers (the range's last period, with its change on the period of the same type before, asked for separately when the range does not hold it). The type and table layout are kept apart from the question (`Visualization` in `api.ts`), so switching keeps the selection; #602 saves the same shape.
- **Threshold colours** (#600): an indicator's bands (`thresholds`, from its Measure's `indicator-threshold` extensions) colour bars, line points, table cells and single numbers, with a legend per banded indicator (`toneOf`).
- **Disaggregation and series** (#601): the Disaggregation picker offers what every picked indicator is broken down by (`sharedDisaggregations`); the answer then carries each figure's `bands`. *Series* is by indicator, place or band (`chartSeries`, also the rows of the table under a chart), and *Each area under* the user's place picks the places directly below it (`childAreas`). Picking an indicator without the disaggregation drops it (`fitted`).
- **Saving** (#602): *Save Visualization* (shown at central to those holding `analytics.author`) saves the setup with a title, optionally onto one of the user's Dashboards or a new one (`features/dashboards`). The default places and *Each area under* follow the viewer (`follow`; `forViewer`), so a shared Visualization shows each viewer their own area; picking places by hand names them. `/explore?v=` opens a saved setup (*Open in explorer*). `LiveVisualization` asks for and draws a setup, here and on each Dashboard item.
- The chart is drawn as SVG here, as the dashboards prototype did; no charting library is used.

## Tests

- `test/explorer.test.ts`: standard period ids, the range pickers, the API, the chart's series, threshold tones, the period before, single numbers, table layouts, and band and place series.
- `test/dashboards.test.ts`: places that follow the viewer, *Open in explorer*'s search, and the dashboards API.
- `e2e/explorer.spec.ts`: `e2e-district` explores Kampala on the local backend, and switches chart types over the sample malaria positivity (45%, bad), and splits the sample ANC 1st visits by age band and compares Kampala's facilities.
