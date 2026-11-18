# Dashboards read Indicator Reports only, over standard periods

Decided in [What is a dashboard, and how do district and national users explore indicators?](https://github.com/carapai/people-centered-healthcare/issues/595). ADR 0070 left free exploration over any period to dashboards. Only summary MeasureReports leave a facility (ADR 0068), and they carry the parts needed to roll them up. District HMIS staff already use DHIS2's Data Visualizer and dashboards.

**The decision:**
- **Two terms.** A *Visualization* is one saved view of indicators: a chart, table, map or single number over chosen periods, places and Disaggregations. A *Dashboard* is a named arrangement of Visualizations. Neither is a Report: they span any periods and are never sent to DHIS2.
- **Summaries only.**
  - Dashboards read Indicator Reports, never patient-level data: at central for district, regional and national users, and on the Facility Edge Node for the facility's own users, offline.
  - Medians and distinct counts show at facility level only, with the reason given.
- **Standard periods only.** Users pick any range of the period types indicators are computed for (week, month, quarter, financial year), rolled up from the finest stored summaries. Arbitrary date ranges are not offered, because summaries cannot be split.
- **A data explorer.** Users pick indicators, places, periods and Disaggregations, switch between table, column, line, single number and map, and save the result as a Visualization on a Dashboard.
- **Two kinds of Dashboard.**
  - *Standard Dashboards* ship in the Reporting package, versioned and reviewed like Reports.
  - *Saved Dashboards* are made by users holding `analytics.author` and shared with a person, a role or a place level. They need no review, since they only read indicators.
- **Scope.**
  - A user sees the places in their assigned area (a district officer sees their district and its facilities).
  - Population Estimates give the denominators (ADR 0071).
- **Freshness.** Every Visualization shows when its latest summary was computed and its reporting completeness (facilities reported out of expected). A facility that has not synced shows as missing, never as zero.
- **Maps.** Facility points come from the National Health Facility Registry's coordinates. District and region boundaries come from UBOS/MoH and ship in the national core package.
- **Thresholds.** Values can be coloured by threshold bands (a legend set). Notifying someone when a threshold is crossed belongs to surveillance alerting, which is not decided.
- **Not in v1:** public, unauthenticated dashboards. Small-cell suppression is defined when they come.

## Considered options

- **Querying patient-level data at central** (arbitrary dates, medians anywhere): contradicts ADR 0068's rule that only summaries leave the facility.
- **Daily summaries for arbitrary date ranges:** multiplies stored summaries about thirty-fold for a need nobody has stated.
- **Dashboards in DHIS2 only:** covers only the indicators sent to DHIS2, and leaves facilities without dashboards offline.
