# Analytics: indicators

How program indicators are defined, computed, stored and synced. The decision is [ADR 0068](../adr/0068-indicators-are-fhirpath-measures-computed-on-each-edge.md); the research is [indicators.md](https://github.com/carapai/people-centered-healthcare/blob/research/indicators/docs/research/indicators.md) (branch `research/indicators`).

| Step | Where | What |
|---|---|---|
| Define | Configuration package | A FHIR R5 `Measure`: populations with FHIRPath criteria, stratifiers, scoring. Reviewed and activated like any Published Definition |
| Compute | Every Facility Edge Node, and central | The same engine. Each node keeps membership per indicator, period and patient, updated from the change log for changed patients, plus a nightly and period-close run for date-dependent indicators |
| Store | The node that computed it | A summary `MeasureReport` per indicator, period and facility, owned by the analytics module |
| Sync | Edge to central | Summary `MeasureReport`s only. Membership and patient lists never leave the node |
| Roll up | Central | District and national figures are sums of facility reports. Central computes cross-facility indicators itself |
| Sign off | Facility, offline | The in-charge (`reporting.submit`) reviews a form's preview for the period and signs it off |
| Send | Central | Posts the signed-off form to DHIS2 through a Reporting package's mappings ([ADR 0069](../adr/0069-hmis-reports-are-signed-off-at-the-facility-and-sent-by-central.md)) |

- **Periods follow when care happened,** not when it was entered. A late entry marks its period's report changed.
- **Exact counts** go to MoH. Small-cell suppression is for public dashboards only.
- **Patient lists** (for example, who is overdue) are worklists on the node, not indicators.

## Reporting to DHIS2

Decided in [ADR 0069](../adr/0069-hmis-reports-are-signed-off-at-the-facility-and-sent-by-central.md).

- **v1 forms:** 105, 108 and 106a, aggregate only. 033b stays on mTRAC; case-based surveillance waits for MoH.
- **Mapping:** a Reporting package maps indicators and disaggregations to data set, data element and category option combo, with effective dates, checked against the instance's metadata at Validation. Each facility's org unit ID is set at central and checked against the NHFR.
- **Corrections:** changes after sign-off mark the period changed; the in-charge re-signs and central resends the whole form. Lock or approval rejections become a task for the district HMIS focal person. A corrected zero is sent as a deletion unless the cell's zeros are significant.
- **Manual entry:** a per-facility, per-form *reported through the platform* switch at central; overwrite counts from DHIS2's import summary are shown to the in-charge and district.

## Open

- Which DHIS2 login MoH grants (one integration account or one per facility), and whether eHMIS uses data approval for these forms.
- Whether CQL replaces hand-written FHIRPath (the `octofhir-cql` spike).
- The Studio's indicator authoring screen and its Simulation over sample patients.
- National and district dashboards, population denominators, surveillance alerting (the map's unexplored areas).
