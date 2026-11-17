# Report sections carry their own Disaggregations, and line lists stay at the facility

> Amends [ADR 0070](0070-reports-are-configuration-that-lay-out-indicators-and-carry-the-dhis2-mapping.md) (the Report's layout).

Decided in [How does the Report designer look: sections, rows, crossed column disaggregations, DHIS2 cell mappings and preview?](https://github.com/carapai/people-centered-healthcare/issues/544), from a prototype (branch `prototype/report-designer`). ADR 0070 gave a Report one set of columns, the bands of one Disaggregation or two crossed. HMIS forms don't work that way: one section is split by age band, the next by HIV status and age, the next not at all. Facilities also need to know who was counted, not just how many.

**The decision:**
- **A Report is sections, and each section is a table or a line list.**
- **A table section has its own Disaggregations**, shared by every indicator in it, so indicators broken down the same way go in the same section. Each Disaggregation is laid out as **rows or columns**; several on one side are crossed. A section may show **column totals** (a Total row under each indicator, over its row bands), **row totals** (a Total column) and **% change from the previous period**, and is or isn't **sent to DHIS2**. Totals are computed from the summed parts like any other cell (ADR 0070's roll-up rule), never by adding up percentages; there is no total across different indicators.
- **A DHIS2 cell** is an indicator × row band × column band, mapped to a data element and category option combo with effective dates. Totals and *not recorded* bands are sent only if mapped.
- **A line list section** has one row per **enrollment** in a program (started in, or active during, the period) or per **visit** (of chosen types, in the period), optionally only those an indicator counted. Its columns are any **attribute** of the person, enrollment or visit, or any **concept**; on an enrollment row a concept shows the latest value, the first, all values with dates, or how many times it was recorded. Membership is the program, visit types and that indicator only; a line list has no conditions of its own.
- **Line lists are facility only.** They show patient-level records, which never leave the facility (ADR 0068), so they run on the facility's edge and are never sent to DHIS2. Validation refuses a Report that runs above facility level and has a line list.

## Considered options

- **One column layout per Report (ADR 0070):** forces a Report per breakdown, or a single wide table where most cells don't apply.
- **Line lists as separate worklists, outside Reports:** the in-charge then reads the counts in one place and the names behind them in another; worklists stay for day-to-day follow-up (analytics.md).
- **A condition tree on line lists:** the indicator editor already has one; a list that needs one names an indicator.
- **Line lists at district and national level:** needs patient-level data at central, which ADR 0068 rules out.
