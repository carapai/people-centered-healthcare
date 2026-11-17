# Analytics: indicators

How program indicators are defined, computed, stored and synced. The decision is [ADR 0068](../adr/0068-indicators-are-fhirpath-measures-computed-on-each-edge.md); the research is [indicators.md](https://github.com/carapai/people-centered-healthcare/blob/research/indicators/docs/research/indicators.md) (branch `research/indicators`).

| Step | Where | What |
|---|---|---|
| Define | Configuration package | A FHIR R5 `Measure`: populations with FHIRPath criteria, stratifiers, scoring. Reviewed and activated like any Published Definition |
| Compute | Every Facility Edge Node, and central | The same engine. Each node keeps membership per indicator, period and patient, updated from the change log for changed patients, plus a nightly and period-close run for date-dependent indicators |
| Store | The node that computed it | A summary `MeasureReport` per indicator, period and facility, owned by the analytics module |
| Sync | Edge to central | Summary `MeasureReport`s only. Membership and patient lists never leave the node |
| Roll up | Central | District and national figures are sums of facility reports. Central computes cross-facility indicators itself |
| Report | Central | DHIS2 submission: [How does the platform report to DHIS2?](https://github.com/carapai/people-centered-healthcare/issues/529) |

- **Periods follow when care happened,** not when it was entered. A late entry marks its period's report changed.
- **Exact counts** go to MoH. Small-cell suppression is for public dashboards only.
- **Patient lists** (for example, who is overdue) are worklists on the node, not indicators.

## Open

- Whether CQL replaces hand-written FHIRPath (the `octofhir-cql` spike).
- The Studio's indicator authoring screen and its Simulation over sample patients.
- National and district dashboards, population denominators, surveillance alerting (the map's unexplored areas).
