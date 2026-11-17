# Reports are configuration that lays out indicators for one period type, and carries the DHIS2 mapping

> Amends [ADR 0068](0068-indicators-are-fhirpath-measures-computed-on-each-edge.md) (what a summary carries, and roll-up) and [ADR 0069](0069-hmis-reports-are-signed-off-at-the-facility-and-sent-by-central.md) (what the Reporting package maps).
>
> Amended by [ADR 0072](0072-report-sections-carry-their-own-disaggregations-and-line-lists-stay-at-the-facility.md): each section has its own Disaggregations as rows or columns, and a section can be a line list.

Decided in [How is a Report defined: its indicators, period type, places and layout?](https://github.com/carapai/people-centered-healthcare/issues/540). Indicators have no period of their own ([studio.md § Indicators](../architecture/studio.md#indicators)): something has to choose the period, the places and the layout, and an HMIS form is one such thing. FHIR has no resource for a report's layout.

**The decision:**
- **A Report is a configuration artefact in a Reporting package**: a title, exactly **one period type** (weekly, monthly, quarterly, semi-annual, annual, financial year from July), the **levels** it runs for (facility, district, region, national), and **sections of rows**. Each row is an indicator by `url|version` with a label. A quarterly version of a monthly Report is a second Report reusing the same indicators. Free exploration over any period belongs to dashboards, which are not decided.
- **Layout in v1 is a table.** Rows are indicators in sections; columns are the bands of one Disaggregation or of two crossed (e.g. age band × sex), so indicators are computed per combination of bands. Free-form layouts with text and charts come later.
- **Roll-up** (amends ADR 0068). Each summary `MeasureReport` carries what its calculation needs to roll up: counts; numerator and denominator for percentages, rates and ratios; sum and count for averages; minimum and maximum. Higher levels recompute from summed parts, never by averaging facility percentages. **Medians and distinct counts** cannot be built from facility summaries, since patient-level data never leaves the facility, so a Report may use them only at facility level, and the Report designer says why.
- **Running a Report reads stored Indicator Reports**, so it is instant. A facility runs its Reports on its edge, offline; district and national users run them at central for any place in their scope, with a new permission, `reporting.view`.
- **HMIS forms are Reports** (amends ADR 0069). A Report marked *sent to DHIS2* carries, on each cell, a DHIS2 data element and category option combo with effective dates. The Reporting package maps Report cells, not bare indicators. Sign-off and sending (ADR 0069) apply to any Report so marked.
- **Outputs in v1:** on screen, print (or save as PDF), and CSV export. Scheduled sending comes later.

## Considered options

- **A FHIR `Library` listing `Measure`s:** still needs a custom layout section, and says nothing of period type or levels.
- **Several period types per Report:** one form would then mean different things at different times; HMIS forms each have one.
- **Averaging facility percentages at district level:** gives small facilities the same weight as large ones, so the district figure is wrong.
- **Sending raw patient data to central for medians:** contradicts ADR 0068's rule that only summaries leave the facility.
