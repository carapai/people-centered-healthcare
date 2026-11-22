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
- **Exact counts** go to MoH. Small-cell suppression is for public dashboards only, which are not in v1.
- **Patient lists** (for example, who is overdue) are worklists on the node, not indicators.
- **In code (#548):** `health-analytics` (definition, periods, counting) and `health-analytics-app` (evaluation on the FHIRPath engine). A Measure carries its period date in the `indicator-period-date` extension and what it counts in `indicator-counts`; `basis` names the record type its criteria read, and a count's criteria are its `initial-population`. Each record is evaluated with `%patient`, `%periodStart` and `%periodEnd`. Package test patients are `tests/` files of type `test-patient`; the reference is `packages/reference-packages/anc`.
- **In code (#549):** the calculation is read from the scoring: `cohort` a count, `proportion` a percentage, `ratio` a ratio or, with `indicator-rate-per`, a rate, and `continuous-variable` the `measure-observation`'s `aggregateMethod` (`sum`, `average`, `median`, `minimum`, `maximum`, or `distinct-count`). Its value is its `criteria` or `indicator-value-between` (two dates and a unit), and `indicator-value-pick` takes each person's or enrollment's `first` or `last`. `indicator-cumulative` counts up to the period's end. An enrollment is a record's `episodeOfCare` or `workflow-episodeOfCare`. Criteria read the patient's other resources through `%records`, a `collection` Bundle. Figures are rounded to two decimal places; the summary keeps the exact parts.
- **In code (#550):** date conditions compare a declared date (`<date>.toDate()`) with `%periodStart.toDate()`, `%periodEnd.toDate()` or a fixed `@YYYY-MM-DD`, optionally plus or minus N days, weeks or months; *in the previous N periods* reads `%previousPeriods.start[N-1]`, the start of the Nth period of the Report's type before. *Is in* another indicator is `'<url>|<version>' in %indicators.numerator` (or `.denominator`): evaluation takes the referred indicators from a library (in *Try*, the package's own Measures) and evaluates them over the same period and patients first. Validation refuses an unpinned reference, one to itself, and indicators that refer to each other in a circle. The condition tree that writes and reads both is `apps/client/src/features/studio/indicators/conditions.ts`.
- **In code (#551):** an Age Band Set is a `Library` of type `age-band-set` with inline `application/vnd.health.age-band-set+json` content (`unit`: days, weeks, months or years; `ageAt`: `period-start`, `period-end` or `period-date`; `bands`, each `from` and `to` included, the last optionally open). Validation refuses a gap or an overlap between bands, naming the ages. The semver check compares the decoded content, so a changed band, unit or age-at date is major and a rename a patch. A Measure pins the sets it disaggregates by on its first group's stratifiers (`indicator-age-band-set`, `url|version`), and `Publications::age_band_sets` lists every published version, so a pinned indicator keeps its bands. MoH's sets are in `packages/reference-packages/core` (`ug.moh.core`); the Studio edits one in its *Age band sets* section.
- **In code (#537):** the Studio's indicator editor (`apps/client/src/features/studio/indicators/`) reads and writes a Measure as above (`indicator.ts`), with the condition trees of #550 and *Try* (`POST …/indicators/try`) beside it. *Save as test* writes a `tests/` file of type `indicator-test` (the indicator's `url|version`, the period type and period, and the count, denominator and value as exact decimal text), which submission's simulation evaluates again (`health-publication`'s `indicators::run_tests`), failing the draft when they changed. Disaggregations are not compared there.
- **In code (#552):** a Disaggregation is a stratifier of the Measure's first group: with `indicator-age-band-set` it is by age (the patient's `birthDate`, in the set's unit, at the period's start or end or the record's period date), with `criteria` (FHIRPath) by the code it gives, named by the stratifier's `code`. No birth date, an age in none of the bands or no code is the band `not-recorded`. A counted thing falls in the band of its first record (by period date) in the denominator, or counted when there is none, so the bands add up to the total and a band's numerator is within the band's denominator. The `Summary` carries, per disaggregation, each band's own summary; `Summary::roll_up` rolls them up band by band. Evaluation takes the pinned sets from the `Library` (`with_age_band_sets`), and a set it does not hold is an error. *ANC 1st visits* in `ug.moh.anc` is disaggregated by MoH's ANC age bands.

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

- **v1 forms:** 105, 108 and 106a, aggregate only. 033b is computed and signed in the platform and keyed into mTRAC, or sent to DHIS2 where enabled (ADR 0091); case forms print for the district (ADR 0087); electronic case-based surveillance waits for MoH.
- **Mapping:** each cell of a Report sent to DHIS2 maps to a data set, data element and category option combo, with effective dates, checked against the instance's metadata at Validation. Each facility's org unit ID is set at central and checked against the NHFR.
- **Corrections:** changes after sign-off mark the period changed; the in-charge re-signs and central resends the whole form. Lock or approval rejections become a task for the district HMIS focal person. A corrected zero is sent as a deletion unless the cell's zeros are significant.
- **Manual entry:** a per-facility, per-form *reported through the platform* switch at central; overwrite counts from DHIS2's import summary are shown to the in-charge and district.

## Dashboards

Decided in [ADR 0075](../adr/0075-dashboards-read-indicator-reports-only-over-standard-periods.md).

- **A Visualization** is one saved view (chart, table, map or single number) over chosen indicators, periods, places and Disaggregations. **A Dashboard** arranges Visualizations.
- **Summaries only:** at central for district and above, and on the edge for the facility, offline. Medians and distinct counts show at facility level only.
- **Standard periods:** any range of weeks, months, quarters or financial years. There are no arbitrary date ranges.
- **The data explorer** pivots indicators by place, period and Disaggregation, and saves Visualizations.
- **Standard Dashboards** ship in the Reporting package. **Saved Dashboards** are user content (`analytics.author`), shared with a person, role or place level.
- **Scope, freshness and completeness:**
  - Users see their assigned area.
  - Each Visualization shows when its data was computed and how many facilities have reported.
  - A facility that has not synced shows as missing, never as zero.
- **Maps:** facility points from the NHFR, and boundaries from the national core package.
- **Thresholds:** values can be coloured by threshold bands. Outbreak threshold alerts are in [surveillance.md](surveillance.md) (ADR 0091).
- **Not in v1:** public dashboards.
- **Layout: explorer first** (decided from the prototype in [How do the data explorer and a Dashboard look?](https://github.com/carapai/people-centered-healthcare/issues/597), branch `prototype/dashboards`, variant A):
  - **Data explorer:**
    - Pickers on the left: data, periods (standard periods only), places in the user's area (or *each area under my place*, which follows the viewer), disaggregation, and series.
    - Chart-type buttons sit above a live Visualization.
    - *Save Visualization* takes a title and optionally a Dashboard.
  - **Dashboards tab:**
    - Dashboards are listed on the left, standard or saved.
    - Sharing is with a person, role or place level, and each viewer still sees only their own area.
    - Each item has *Open in explorer*.
  - **Every Visualization card** shows where it was computed, when, how many facilities reported, the missing facilities with their last sync, and why a median is absent above facility level.

## Open

- Which DHIS2 login MoH grants (one integration account or one per facility), and whether eHMIS uses data approval for these forms.
- CQL: not yet. `octofhir-cql` cannot translate WHO's indicator libraries and misreads R5 data ([#532](https://github.com/carapai/people-centered-healthcare/issues/532)); re-run the spike when it closes those gaps.
- Surveillance alerting, and public dashboards with small-cell suppression.
