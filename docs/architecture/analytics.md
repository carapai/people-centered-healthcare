# Analytics: indicators

How program indicators are defined, computed, stored and synced. The decision is [ADR 0068](../adr/0068-indicators-are-fhirpath-measures-computed-on-each-edge.md); the research is [indicators.md](https://github.com/carapai/people-centered-healthcare/blob/research/indicators/docs/research/indicators.md) (branch `research/indicators`).

| Step | Where | What |
|---|---|---|
| Define | Configuration package | A FHIR R5 `Measure`: populations with FHIRPath criteria, stratifiers, scoring. Reviewed and activated like any Published Definition |
| Run | A Report | A Report lays out indicators for one period type; the indicators have no period of their own. Running it reads stored Indicator Reports, on the edge for its facility or at central for any place in scope (`reporting.view`) ([ADR 0070](../adr/0070-reports-are-configuration-that-lay-out-indicators-and-carry-the-dhis2-mapping.md)) |
| Compute | Every Facility Edge Node, and central | The same engine. Each node keeps membership per indicator, period and patient, updated from the change log for changed patients, plus a nightly and period-close run for date-dependent indicators |
| Store | The node that computed it | A summary `MeasureReport` per indicator, period, facility and combination of bands, carrying what its calculation needs to roll up (counts, numerator and denominator, sum and count, minimum and maximum), owned by the analytics module |
| Sync | Edge to central | Summary `MeasureReport`s only. Membership and patient lists never leave the node |
| Roll up | Central | District and national figures are recomputed from summed facility parts, never by averaging percentages. Medians and distinct counts stay at facility level. Central computes cross-facility indicators itself |
| Sign off | Facility, offline | The in-charge (`reporting.submit`) reviews a form's preview for the period and signs it off |
| Send | Central | Posts the signed-off Report to DHIS2 through its cells' mappings ([ADR 0069](../adr/0069-hmis-reports-are-signed-off-at-the-facility-and-sent-by-central.md)) |

- **Periods follow when care happened,** not when it was entered. A late entry marks its period's report changed.
- **Exact counts** go to MoH. Small-cell suppression is for public dashboards only.
- **Patient lists** (for example, who is overdue) are worklists on the node, not indicators.

## Reports

Decided in [ADR 0070](../adr/0070-reports-are-configuration-that-lay-out-indicators-and-carry-the-dhis2-mapping.md) and [ADR 0072](../adr/0072-report-sections-carry-their-own-disaggregations-and-line-lists-stay-at-the-facility.md); the designer is in [studio.md § Reports](studio.md#reports).

- A **Report** is configuration in a Reporting package: a title, one period type, the levels it runs for, and sections.
- A **table section** has indicator rows (by `url|version`) and its own Disaggregations, each laid out as rows or columns (several on one side are crossed), with optional column totals, row totals and % change from the previous period.
- A **line list section** has one row per enrollment or per visit, with any attribute or concept as a column. It is patient-level, so facility only.
- **Outputs:** screen, print or PDF, CSV.
- An **HMIS form** is a Report marked *sent to DHIS2*, each cell carrying its DHIS2 mapping.

## Age Band Sets and Population Estimates

Decided in [ADR 0071](../adr/0071-population-estimates-ship-as-yearly-population-packages.md).

- **Age Band Sets** live in the national core package (MoH's) or a program package, edited in a small Studio table (name, unit, the date age is taken at, from–to rows, with gaps and overlaps flagged). Changing the bands is a major version; indicators and Reports pin the version.
- **Population Estimates** ship as a yearly **Population** package: estimate types given per place or derived as a percentage of another (expected pregnancies = 5% of total), figures at the lowest level available and summed upward, no guessed shares. A Report's period takes the annual figure pro rata (month 1/12, quarter 1/4, week 7/365).
- **v1 source:** the HMIS lead imports a year's figures at central from a CSV template (UBOS projections), checked against the facility registry and the NHFR, with district totals shown before approval.

## Reporting to DHIS2

Decided in [ADR 0069](../adr/0069-hmis-reports-are-signed-off-at-the-facility-and-sent-by-central.md).

- **v1 forms:** 105, 108 and 106a, aggregate only. 033b stays on mTRAC; case-based surveillance waits for MoH.
- **Mapping:** each cell of a Report sent to DHIS2 maps to a data set, data element and category option combo, with effective dates, checked against the instance's metadata at Validation. Each facility's org unit ID is set at central and checked against the NHFR.
- **Corrections:** changes after sign-off mark the period changed; the in-charge re-signs and central resends the whole form. Lock or approval rejections become a task for the district HMIS focal person. A corrected zero is sent as a deletion unless the cell's zeros are significant.
- **Manual entry:** a per-facility, per-form *reported through the platform* switch at central; overwrite counts from DHIS2's import summary are shown to the in-charge and district.

## Open

- Which DHIS2 login MoH grants (one integration account or one per facility), and whether eHMIS uses data approval for these forms.
- CQL: not yet. `octofhir-cql` cannot translate WHO's indicator libraries and misreads R5 data ([#532](https://github.com/carapai/people-centered-healthcare/issues/532)); re-run the spike when it closes those gaps.
- National and district dashboards, population denominators, surveillance alerting (the map's unexplored areas).
