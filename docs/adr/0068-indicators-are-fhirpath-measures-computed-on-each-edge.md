# Indicators are FHIRPath Measures, computed on each Facility Edge Node and summed at central

Decided in [Where are indicators defined, computed and stored: facility, district or central?](https://github.com/carapai/people-centered-healthcare/issues/528), from the research in [indicators.md](https://github.com/carapai/people-centered-healthcare/blob/research/indicators/docs/research/indicators.md) (branch `research/indicators`). A facility must see its own figures and draft its HMIS report while offline, and a district must see the same numbers the facility in-charge signed off. This amends the Deployables table in [resource-ownership.md](../architecture/resource-ownership.md), which kept Analytics at central only.

**The decision:**
- **An indicator is a published FHIR R5 `Measure`** in a configuration package: populations (numerator, denominator, exclusions), stratifiers for disaggregation, and scoring. It is versioned, reviewed and activated like any other Published Definition.
- **Criteria are FHIRPath** (`text/fhirpath`), evaluated by the vendored FHIRPath engine. WHO SMART indicators, published as CQL on FHIR R4, are translated into FHIRPath by package authors. CQL stays open: a spike runs `octofhir-cql` on WHO's real indicator libraries before anything depends on it.
- **The same engine runs on every Facility Edge Node and at central.**
  - The edge computes its own facility's indicators, offline.
  - Central's figure for a facility is the summary report that facility sent. District and national figures are sums of facility reports.
  - Central computes only what no single facility can see, such as patients who move between facilities.
- **Incremental membership.** Each node keeps, per indicator and period, which patients count toward which population. A change in the change log (ADR 0061) re-evaluates only that patient. A nightly run, and one at period close, re-evaluates indicators that depend on the date rather than on new records (for example *currently on ART*).
- **Only summaries leave the facility.** The stored result is a summary `MeasureReport` per indicator, period and facility (counts and strata), and only that syncs up. Membership rows and patient lists stay on the node. MoH reporting gets exact counts. Small-cell suppression applies only to public dashboards.
- **Care counts in the period it happened in.** A visit on 30 March entered on 3 April counts toward March, and March's report is marked changed. Resubmitting a report already sent to DHIS2 is decided in [How does the platform report to DHIS2: who submits, when, and how are corrections handled?](https://github.com/carapai/people-centered-healthcare/issues/529).

## Considered options

- **CQL now**, reusing WHO's libraries as-is: the only Rust engine is young, R4-only and unverified on real libraries, and the mature engines need a JVM, Node or .NET at the edge.
- **SQL on FHIR views as the definition**: views flatten rows but carry no populations or scoring, and the specification is still at ballot. They remain useful for exports.
- **Central only, or DHIS2 only**: no offline facility figures, and the in-charge could not review a report before it leaves.
- **Central recomputes every facility**: two answers for the same facility, and they differ whenever sync lags.
- **Scheduled batch recompute** (the DHIS2 and UgandaEMR way): simpler, but figures go stale between runs.

## Consequences

- The Analytics context gains a module on `facility-edge`; external integrations still run at central only.
- Every indicator is hand-authored FHIRPath until the CQL spike says otherwise, so the Studio's indicator screen and Simulation must make FHIRPath criteria testable over sample patients.
