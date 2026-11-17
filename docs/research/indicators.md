# Research: program indicators

Findings for the research ticket [How do FHIR R5 Measure, WHO SMART indicators and a Rust platform compute program indicators?](https://github.com/carapai/people-centered-healthcare/issues/526). Checked against primary sources on 2026-11-17. Anything marked **(unverified)** was not confirmed at its source. Anything marked **(analysis)** is reasoning from the facts here, not a claim taken from a source.

## Decision-relevant facts

1. **Measure can carry FHIRPath.** A Measure population's `criteria` is an `Expression`. Its `language` binding is extensible over `text/cql`, `text/fhirpath`, `text/x-fhir-query`, `text/cql-identifier` and `text/cql-expression` [1][3]. So a FHIRPath-only Measure is valid R5, but no WHO DAK ships one.
2. **The WHO DAK indicators are CQL, on FHIR R4.** The HIV, immunization and ANC repositories each have one CQL Library per indicator: `HIVIND*Logic`, `IMMZIND*Logic`, `ANCIND01`–`13`. They declare `using FHIR version '4.0.1'`, include `FHIRHelpers 4.0.1`, and Measures point to them with `text/cql-identifier` [4][5][6][7]. The published HIV (1.0.0, 2025-07-08) and immunization (1.0.0, 2026-10-02) IGs are both FHIR 4.0.1 [8].
3. **A Rust CQL engine now exists, which overturns the map's fog.** `octofhir/cql-rs` (crate `octofhir-cql` 0.2.5, MIT or Apache-2.0) comes from the same organisation as the vendored FHIRPath engine. Its README claims 1693/1693 on cqframework/cql-tests. It has no JVM and plugs in through `DataProvider` and `TerminologyProvider` traits [9]. Its own stated limits are that it does no Measure scoring and produces no MeasureReport, reads FHIR R4 JSON only, does not support related-context retrieves, and has no static type checking. It has one contributor and about 316 crates.io downloads [9][10]. The conformance claim is **(unverified)**: we did not run the suite.
4. **SQL on FHIR has no published STU at HL7.** HL7's package list shows only `3.0.0-ballot` (2026-08-14, "STU 3", FHIR 6.0.0) [11]. Version 2.0.0 (2024-10-09) was published on sql-on-fhir.org [12]. The 3.0.0 ballot makes ViewDefinition an R6 "additional resource" and adds `SQLQuery`/`SQLView` Libraries and the `$sql-run`/`$sql-export` operations [12]. View semantics (`select`, `column`, `forEach`, `forEachOrNull`, `where`, `constant`, `unionAll`, plus `repeat` in 3.0) are unchanged from 2.0.0. Aggregation is out of scope for views and left to SQL [12][13].
5. **A Rust ViewDefinition runner exists.** `helios-sof` 0.2.9 (HeliosSoftware/hfs, MIT, released 2026-11-16) is built on Helios's own FHIRPath and models, not octofhir. Its default is R4; R5 is a feature flag. It pulls in arrow, object_store, axum and an in-memory SQLite for `$sqlquery-run` [14]. Nothing found runs over SurrealDB. A ViewDefinition's output rows would have to be loaded into SurrealDB, or into an embedded SQL engine, before aggregation **(analysis)**.
6. **Every mature Measure engine needs a non-Rust runtime.**
   - HAPI/cqframework `clinical-reasoning`: Java 17 [15]. `cqf-ruler` is being folded into the HAPI JPA starter [16].
   - `fqm-execution`: Node/TypeScript. It takes precompiled ELM (no CQL translation) and R4 bundles [17].
   - Firely/NCQA `firely-cql-sdk`: .NET. It compiles ELM to assemblies [18].
7. **Prior art computes at a fixed time, in SQL.**
   - DHIS2 program indicators are a filter, an expression and an aggregation type over event or enrollment rows, in the `d2:` expression language [19]. The server compiles them to SQL with `ProgramIndicatorService.getAnalyticsSql` against analytics tables that a scheduled job generates [20].
   - The DHIS2 Android SDK evaluates the same expressions offline through `ProgramIndicatorSQLEvaluator` over its local SQLite [21]. This is the closest precedent for computing indicators on the edge.
   - OpenMRS's reporting module has `CohortIndicator` (`COUNT`/`FRACTION`/`LOGIC`, with a cohort, a denominator and a location filter) [22]. UgandaEMR writes HMIS 105/106a indicators as Java cohort libraries (`Moh105IndicatorLibrary`) [23]. A scheduled `MambaTask` runs a flattening ETL whose JSON configs map concept UUIDs to columns of `mamba_flat_encounter_*` tables [24]. These flat tables play the same role as ViewDefinitions.
8. **SurrealDB table views do not cover FHIR indicators.** `DEFINE TABLE … AS SELECT … GROUP BY` updates incrementally on writes to its source table only. Changes to joined records do not propagate, and `OPTION IMPORT` writes bypass it [25]. A stratifier that reads the Patient from an Observation-based population would go stale **(analysis)**. The docs do not say which aggregate functions update incrementally **(unverified)**.

## The shape of an indicator

| Concept | FHIR R5 Measure / MeasureReport [1][2] | WHO DAK CQL [4][6] | DHIS2 [19] | OpenMRS [22] |
|---|---|---|---|---|
| Unit counted | `basis` (default boolean per subject; can be Encounter etc.), `subject[x]` (default Patient) | `context Patient` | analytics type: event or enrollment | Cohort (patients) |
| Populations | initial-population, denominator(-exclusion/-exception), numerator(-exclusion), measure-population(-exclusion), measure-observation | `define "Numerator"` etc. | filter + expression | cohort + denominator cohort |
| Scoring | proportion, ratio, continuous-variable, cohort; `improvementNotation` | HIV IND2 is continuous-variable; IMMZ IND01 is proportion [7][8] | aggregation type (count, sum, avg, ...) | COUNT / FRACTION / LOGIC |
| Disaggregation | `stratifier` (one criteria, or `component`s) | `define "... Stratifier"`; HIV joins several into one `Stratification` string | category combos / dimensions | dimensions |
| Period | `$evaluate-measure` periodStart/periodEnd | `parameter "Measurement Period"` | analytics period boundaries (configurable offsets) | `${startDate}`/`${endDate}` params |
| Output | MeasureReport `individual` / `subject-list` / `summary` / `data-exchange`; `group.population.count`, `stratum`, `measureScore[x]` | — | aggregate data values | dataset rows |

Measure and MeasureReport are FMM 4, Trial Use, in R5 [1][2]. `evaluatedResource` is allowed only on individual reports [2].

## How hard the WHO CQL is to translate

- **A simple case.** `ANCIND01`'s numerator: some first ANC contact (Encounter `reasonCode` in a ValueSet) starts in the period, with gestational age ≤ 12 weeks [6]. `HIVIND2`: some completed MedicationStatement for PrEP falls in the period; the age bands are a `case` on age in years [4][5]. Both rest on retrieves, value-set membership, interval `during`, `exists` and age arithmetic. Per subject, that maps to FHIRPath over a bundle of the patient's resources, plus `memberOf`, and to ViewDefinition rows plus SQL **(analysis)**.
- **Where it gets hard.** The shared element libraries (`HIVIndicatorElements`, `ANCDataElements`, `IMMZIndicatorElements`) are thousands of lines. `HIVIndicatorElements` reaches line 2300+ [5]. A hand translation is a fork that must track every WHO release **(analysis)**.
- **The FHIR version gap.** All the DAK CQL targets FHIR 4.0.1 and the cqfm/CRMI Measure profiles [7][8]. Our storage is R5, so running it unchanged needs either R5 model info and FHIRHelpers, or per-subject down-conversion of R5 to R4 (`health-fhir-mapping`) before evaluation **(analysis)**.

## Keeping projections fresh offline (analysis)

- **Event indicators** count things that happened in the period, such as ANC1 before 12 weeks or PrEP dispensed. Per-subject membership rows of the form *(indicator, period, subject, stratum, population)* can be recomputed for each **dirty subject** whenever its resources change. Counting those rows gives the aggregate.
  - Counting subjects rather than incrementing counters keeps "count once" deduplication correct.
  - It also handles late offline entries, backdating and `entered-in-error` retractions.
  - DHIS2 and Mamba do this as batch jobs [20][24]. The edge can do it incrementally by tailing the existing `change_log` (ADR 0061) with a cursor, outside the write transaction.
- **State indicators** depend on the clock as well as on writes: currently on ART, missed an appointment by 28 days, active in care. Nothing is written when a patient becomes lost to follow-up, so these need a scheduled re-evaluation, at period close or nightly, over the open cohort.
- **Stratifier values** such as age at the period boundary are fixed once the period is fixed.
- **Scope of an edge projection.** Under the care-journeys rules, an edge projection covers only its own Sync Scope, is rebuildable, and is never synced as authoritative. Central recomputes the cross-facility figures, so edge numbers are provisional, facility-scoped views.

## Candidates

| # | Representation / engine | Cost | What it rules out |
|---|---|---|---|
| A | **Measure with `text/fhirpath` criteria**, evaluated per subject by the vendored `octofhir-fhirpath` over a subject bundle, plus our own scorer that writes MeasureReports | Our own scorer, period/parameter plumbing and the subject-bundle loader. Every WHO indicator is hand-written in FHIRPath | Importing DAK CQL as-is. Portability to other Measure engines, since most expect CQL. Cross-subject logic, e.g. mother–infant |
| B | **ViewDefinitions plus aggregation**: our own runner on `octofhir-fhirpath` (conformance suite in `FHIR/sql-on-fhir.js`), or `helios-sof`; rows go into SurrealDB tables and are aggregated in SurrealQL | Our own runner is small: about 8 view constructs plus `getResourceKey`/`getReferenceKey` [13]. Helios brings a second FHIRPath engine and model stack with an R5 feature flag. The spec is only at ballot and moving to R6 | Portable `SQLQuery` Libraries, since SurrealQL is not one of the SQL dialects. Using views as the whole definition: the populations, scoring and stratifiers still need a Measure-like layer |
| C | **CQL via `octofhir-cql`**, plus our own Measure scorer and an R4 bridge | A young dependency with one contributor. R5 data has to be converted, or the engine's model extended. We still write the scorer. The "no CQL" assumption (workflow-engine.md) is limited to guards | Little. It keeps DAK reuse open. The risks are the dependency's bus factor and its unverified conformance on real DAK libraries |
| D | **JVM `clinical-reasoning` at central only**, as the `$evaluate-measure` reference and as the oracle in Studio's Simulation | Java 17 service, ops and memory at central | Running it on the edge or desktop. Offline facility indicators would need A, B or C on the edge anyway |
| E | **Hand-coded Rust per indicator** (the UgandaEMR way) | A code change for every indicator change | Configuration-as-data (AGENTS.md: country behaviour lives in packages) |
| F | **Aggregate only at central or DHIS2 after sync** (no edge analytics) | The cheapest | Offline facility dashboards and worklists driven by indicators |

These are not exclusive. One combination is: Measure as the definition artefact; C or A on the edge with membership rows driven by `change_log`; D as the central oracle; B for flat exports to DHIS2 and research **(analysis)**.

## Open, not verified

- Whether `octofhir-cql` evaluates the real `HIVIND*` and `ANCIND*` libraries, which have deep includes, `Combine`, fluent functions and `toInterval()`.
- `helios-sof` pass rate on the SQL on FHIR suite.
- SurrealDB incremental aggregate support beyond `count`, `math::sum` and `math::mean`.
- Whether WHO publishes DHIS2 metadata mappings for these DAK indicators.
- The ANC IG's published URL. `smart.who.int/anc/` returned 404, so it is checked here from GitHub only.

## Sources

1. HL7 FHIR R5 Measure — https://hl7.org/fhir/R5/measure.html
2. HL7 FHIR R5 MeasureReport — https://hl7.org/fhir/R5/measurereport.html
3. ValueSet expression-language — https://hl7.org/fhir/R5/valueset-expression-language.html
4. WHO smart-hiv `HIVIND2Logic.cql` — https://github.com/WorldHealthOrganization/smart-hiv/blob/main/input/cql/HIVIND2Logic.cql
5. WHO smart-hiv `HIVIndicatorElements.cql` — https://github.com/WorldHealthOrganization/smart-hiv/blob/main/input/cql/HIVIndicatorElements.cql
6. WHO smart-anc `input/cql/` (`ANCIND01.cql`, `ANCStratifiers.cql`) — https://github.com/WorldHealthOrganization/smart-anc/tree/master/input/cql
7. WHO smart-immunizations `input/cql/IMMZIND*Logic.cql`, `input/fsh/measures/IMMZIND01.fsh` — https://github.com/WorldHealthOrganization/smart-immunizations
8. Published IGs and Measures: https://smart.who.int/hiv/package-list.json, https://smart.who.int/hiv/Measure-HIVIND2.json, https://smart.who.int/immunizations/package-list.json, https://smart.who.int/immunizations/Measure-IMMZIND01.json
9. octofhir/cql-rs README — https://github.com/octofhir/cql-rs
10. crates.io `octofhir-cql` — https://crates.io/crates/octofhir-cql
11. HL7 SQL on FHIR package list — https://hl7.org/fhir/uv/sql-on-fhir/package-list.json
12. SQL on FHIR change log (`input/pagecontent/changes.md`) — https://github.com/HL7/sql-on-fhir/blob/main/input/pagecontent/changes.md ; CI build https://build.fhir.org/ig/FHIR/sql-on-fhir-v2/
13. ViewDefinition notes (required functions, unnesting, unions) — https://github.com/HL7/sql-on-fhir/blob/main/input/pagecontent/StructureDefinition-ViewDefinition-notes.md
14. `helios-sof` — https://github.com/HeliosSoftware/hfs/tree/main/crates/sof , https://crates.io/crates/helios-sof
15. cqframework/clinical-reasoning README — https://github.com/cqframework/clinical-reasoning
16. cqframework/cqf-ruler README — https://github.com/cqframework/cqf-ruler
17. projecttacoma/fqm-execution README — https://github.com/projecttacoma/fqm-execution
18. FirelyTeam/firely-cql-sdk — https://github.com/FirelyTeam/firely-cql-sdk
19. DHIS2 docs, Programs: program indicators — https://docs.dhis2.org/en/use/user-guides/dhis-core-version-master/configuring-the-system/programs.html
20. dhis2-core `ProgramIndicatorService.java`, `dhis-service-analytics/readme.md` — https://github.com/dhis2/dhis2-core
21. dhis2-android-sdk `ProgramIndicatorEngine.kt`, `ProgramIndicatorSQLEvaluator.kt` — https://github.com/dhis2/dhis2-android-sdk
22. openmrs-module-reporting `CohortIndicator.java` — https://github.com/openmrs/openmrs-module-reporting
23. UgandaEMR reports `Moh105IndicatorLibrary.java`, `Setup106A1BReport.java` — https://github.com/METS-Programme/openmrs-module-ugandaemr-reports
24. UgandaEMR reports `tasks/MambaTask.java`, `omod/src/main/resources/_etl/config/anc.json` — same repository
25. SurrealDB `DEFINE TABLE` (table views, changefeeds) — https://surrealdb.com/docs/surrealql/statements/define/table
