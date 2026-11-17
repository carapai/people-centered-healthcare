# Research spike: does `octofhir-cql` evaluate WHO SMART indicator libraries?

Findings for the research ticket [Does octofhir-cql evaluate WHO SMART indicator libraries correctly?](https://github.com/carapai/people-centered-healthcare/issues/532). This feeds ADR 0068 (indicators are FHIR R5 Measures with hand-written FHIRPath criteria, and CQL is left open; branch `docs/indicators`) and follows [indicators.md](https://github.com/carapai/people-centered-healthcare/blob/research/indicators/docs/research/indicators.md) (branch `research/indicators`). Run on 2026-11-17. Anything marked **(analysis)** is reasoning from these results, not a measured fact.

## Answer

**No, not as shipped.** `octofhir-cql` 0.2.5 does not translate any WHO SMART indicator library as published: 0 of 90 HIV, 0 of 13 ANC and 0 of 41 immunization indicator libraries pass `cql validate`. The reference engine (cqframework clinical-reasoning 4.15.0) translates and evaluates the same files.

The failures are translator gaps in valid CQL 1.5 that the WHO libraries use everywhere, not runtime defects. When we patched four of those gaps in a scratch copy and worked around two more:

- **ANC:** 12 of 13 `ANCIND` libraries evaluated, and every definition matched the reference engine for the one WHO test patient.
- **HIV and immunization:** still 0 translated. They hit further gaps (`sort by` over an implicit property with a fluent call, typed list selectors, anonymous tuple selectors, a parameter named `Today`).

The evaluator itself looks sound where the translator lets it through. The engine is fast and small: 0.3 s and 30 MB per library and patient, against 7.9 s and 1.1 GB for the reference CLI including JVM start-up.

**Verdict for ADR 0068 (analysis):** keep hand-written FHIRPath criteria. CQL cannot replace them today. The engine is close enough that upstream fixes could change this, so it is worth re-checking later rather than ruling out. See [Verdict](#verdict).

## What was run

| Item | Version |
|---|---|
| `octofhir/cql-rs` | commit `848156b` (2026-11-06), crate `octofhir-cql` 0.2.5, built with `--features cli` |
| WHO `smart-hiv` | commit `b1e7735` (2026-03-04), `input/cql/` |
| WHO `smart-anc` | commit `301a6b8` (2026-10-16), `input/cql/` and `bundles/measure/ANCIND01/` (test patient, value sets, expected MeasureReport) |
| WHO `smart-immunizations` | commit `bd7fa72` (2026-10-01), `input/cql/` |
| Reference engine | `org.opencds.cqf.fhir:cqf-fhir-cr-cli:4.15.0` (cqframework clinical-reasoning, HAPI FHIR 8.12.0) on OpenJDK 17.0.14, run through jbang. Setup took about 10 minutes. |

Steps:

1. Ran `cql validate` (parse, translate to ELM, resolve names) on every library.
2. Isolated each distinct error in a minimal CQL file ([`cql-spike/minimal.sh`](cql-spike/minimal.sh)).
3. Patched four parser gaps in a **copy** of the engine ([`cql-spike/patch_parser.py`](cql-spike/patch_parser.py)). The patched copy still passes 1693/1693 of the conformance suite.
4. Ran `cql run` (stock and patched) and the reference `cql` command on the same libraries, data and value sets, then diffed every definition per patient.

## Results per library

"Stock" is `octofhir-cql` 0.2.5 unmodified. "Patched" is the scratch copy with patches 1–4. "DT period" means the Measurement Period was passed as `Interval<DateTime>` to get past gap L below.

| Library | Includes (direct / transitive) | Stock: translates | Patched: translates | Patched: evaluates | Reference evaluates | Matches reference |
|---|---|---|---|---|---|---|
| **HIVIND2Logic** (uses `Combine`, fluent functions, `toInterval()`) | 5 / 10 | No: `WHOCommon.cql:67:35 error[CQL0014]: the operand type, found List` (gap A, reported against the wrong file) | No: 21× `` `effective` is not an included library alias `` (gap K), 7× `` could not resolve identifier `List` ``/`` `String` `` (gap J), 3× `onset` (K) | — | Yes (synthetic patient: IP true, Measure Population false) | — |
| **HIVIND13Logic** | 5 / 10 | No: gap A | No: same as HIVIND2 | — | not run | — |
| **HIVIND28Logic** | 7 / 10 | No: `100:1 error[CQL0012]: an expression, found HIE` (misleading location, cleared by patches 1–3) | No: gaps K and J | — | Yes (IP true, numerator false, denominator false) | — |
| **HIVIND61Logic** (`duration in days of`, nested `exists`) | 6 / 10 | No: `84:3 error[CQL0012]: an expression, found exists` (gap C, misleading location) | No: gaps K and J | — | Yes (IP true, Numerator false, Denominator false) | — |
| All 90 `HIVIND*Logic` | | **0 / 90** | **0 / 90** | | | |
| **ANCIND01** | 7 / 12 | No: gap A (and gap E when FHIRHelpers.cql is present) | Yes | Only with DT period. Otherwise: `Unsupported operator: Compare for types DateTime, Date` (gap L) | Yes | **Yes**, 9/9 definitions |
| **ANCIND02** | 7 / 12 | No | Yes | Yes | Yes | **Yes**, 5/5 |
| **ANCIND03** | 7 / 12 | No | No: `103:3 error[CQL0012]: an expression, found First` (gap I, anonymous tuple in `return`) | — | Yes | — |
| ANCIND04–06, 08–13 | 7 / 12 | No | Yes | Yes | Yes | **Yes**, 5/5 each |
| **ANCIND07** | 7 / 12 | No | Yes | Yes | Yes | **Yes**, 6/6 (one string differs only in output quoting) |
| All 13 `ANCIND` | | **0 / 13** | **12 / 13** | 12 / 12 | 13 / 13 | 12 / 12 |
| **IMMZIND01Logic**–**IMMZIND45Logic** (41 libraries) | via `IMMZIndicatorElements` | **0 / 41**: gap A | **0 / 41**: anonymous tuple in a multi-source `from` query (`IMMZIndicatorElements.cql:267`, gap I). Gap H (`less than 12 months before`) was cleared by patch 4. | — | not run | — |

### What the ANC comparison covers

The ANC test patient (`charity-otala-1`) is the only WHO test case that is executable FHIR JSON. For every ANC indicator she is in the Initial Population, and every Numerator is false. So the match covers:

- retrieves filtered by value set;
- seven-library include chains;
- `Coalesce` and extension helpers;
- age and education stratifiers;
- `exists` over aliased queries.

It does **not** cover a true Numerator. "Matches the reference" therefore means "agrees on a shallow case", not "conformant on DAK logic" **(analysis)**.

### WHO's own expected output is stale

The checked-in `measurereport-numerator-charity-otala-1.json` for ANCIND01 expects Numerator = 1. Both engines say Numerator = false.

The reason is that `ContactData."Gestational age"` filters on a library parameter `encounter String`. Nobody sets it, so the result is null. The same expected report gives `measureScore` 0.0 alongside numerator 1 and denominator 1, so it contradicts itself. We treated the reference engine, not the IG fixture, as the oracle.

Immunization tests are YAML in a WHO-specific format (`input/tests/measures/IMMZIND02Logic/examples.yaml`: relative birth dates and dose schedules) and need WHO's generator to become FHIR. HIV has no test data in the repository (`input/testing/` holds only a `.gitignore`). HIV comparisons above therefore use a one-resource synthetic Patient.

## Gaps found

All are valid CQL 1.5 that the reference translator accepts. Reproductions are in [`cql-spike/minimal.sh`](cql-spike/minimal.sh).

| # | Construct | Example from WHO source | Stock error | Patched in spike |
|---|---|---|---|---|
| A | Type names with three or more parts | `List<FHIR.Dosage.DoseAndRate>` (`WHOCommon.cql:67`, included by nearly every HIV and immunization library; also in ANC's `WHOCommon`) | `CQL0014 the operand type, found List` | yes (1) |
| B | `exists` over an aliased query without parentheses | `exists Elements."Currently pregnant B.DE29" C where …` | `CQL0001 expected the end of the definition, found C` | yes (2) |
| C | `duration in <unit> of <interval>` | `duration in days of (t intersect P.performed.toInterval())` | `CQL0001 … found of` | yes (3) |
| D | A parameter named `Today` (`today` is on the engine's reserved list) | `parameter Today Date default Today()` (`HIVEncounterElements`, DT libraries) | `CQL0013 a parameter name, found Today` | no |
| E | Compiling the real `FHIRHelpers.cql` 4.0.1 source, which uses `concept` as an identifier | `if concept is null then` (ANC ships `FHIRHelpers.cql` beside its libraries, and the engine prefers the file over its built-in) | `CQL0013 a concept name, found is`, which fails all 60 dependent ANC libraries | worked around by deleting the file so the built-in FHIRHelpers is used |
| F | Escaped quotes in a definition name | `define "Should \"Anaemia counselling conducted\" IS OPTIONAL"` (ANCDT25, a decision table, not an indicator) | `CQL0001 ':' after the definition name` | no |
| H | `less than`/`more than` offsets after a non-literal operand | `Patient.birthDate less than 12 months before end of "Measurement Period"` (`IMMZIndicatorElements:244`) | `CQL0001 … found than` (`less` is read as a query alias) | yes (4) |
| I | Tuple selector without the `Tuple` keyword | `return { immunization: I, adverseEvent: O }` | `CQL0001`/`CQL0012` | no |
| J | Typed list selector | `List<String>{ … }` (`HIVIndicatorElements:295`) | `CQL0100 could not resolve identifier List` | no |
| K | `sort by` with an implicit property and a fluent call | `sort by start of effective.toInterval()` (`WHOCommon` `mostRecent`, `HIVElements`) | `` CQL0103 `effective` is not an included library alias `` | no |
| L | Implicit `Date` to `DateTime` conversion | `C.period starts during "Measurement Period"` where the period is `Interval<Date>` | at run time: `Unsupported operator: Compare for types DateTime, Date` | worked around by passing a DateTime period |
| M | Parameters for included libraries | `-p encounter=…` has no effect on `ANCContactDataElements.encounter` | silently ignored | no |
| N | Error locations | Errors inside an included library are printed with the main file's name and a source line from the wrong file. Backtracking also reports the outer token (`found exists`) rather than the real cause. | — | no |

Gap L is the documented limit "ELM carries no result types and no implicit conversions" [1]. Gaps E and K–M follow from the same design: no type checking, and `FHIRHelpers` built into the evaluator rather than compiled. The other gaps are parser coverage.

## Conformance claim

**Reproduced, with a caveat.** `cargo test -p octofhir-cql --test cqframework_spec_tests` passes 1693 of 1693 across the 14 vendored cqframework/cql-tests suites, on both the stock and the patched copy. That suite tests operator semantics through small expressions. It does not exercise the library-level translator features that fail above: includes of real FHIRHelpers, multi-part model types, typed selectors, implicit conversions or sort-by scoping. So "100% conformant" is true of the suite and says little about real measure libraries **(analysis)**. The README's other figure, 807 of 1168 of the google/cql engine tests, was not re-run.

## Footprint

ANCIND02 for one patient, on an Apple-silicon laptop, measured with `/usr/bin/time -l`:

| Engine | Wall time | Peak RSS |
|---|---|---|
| `octofhir-cql` (compile 7 libraries + 598 value sets + evaluate) | 0.38 s | 30 MB |
| Reference `cqf-fhir-cr-cli` 4.15.0 (JVM start, translate, evaluate) | 7.85 s | 1.1 GB |

The reference figure includes JVM start-up, so it is not a throughput comparison. It does show why ADR 0068 rules out a JVM on the edge.

## R5 bridge

Every WHO library declares `using FHIR version '4.0.1'` and includes `FHIRHelpers version '4.0.1'`.

- **The engine does not use a FHIR model to type data.** `octofhir-cql-model` embeds R4 and R5 ModelInfo XML, but neither the translator nor the evaluator references it. FHIR JSON is mapped by element name and value shape [1][2]. R5 JSON therefore loads without error, and any element R5 renamed or retyped silently evaluates to null.
- **Measured:** we reshaped only the test Encounter to R5 (`period` to `actualPeriod`, `reasonCode` to `reason[].value[].concept`, `class` to a CodeableConcept list). ANCIND01's Denominator flipped from **true to false** with no error or warning. A silent wrong count is worse than a failure **(analysis)**.
- **R4 paths the indicators read that R5 changed** (from the libraries; R5 names from the R5 resource pages [5]):
  - `Encounter.period` to `actualPeriod`;
  - `Encounter.reasonCode` to `reason` (CodeableReference);
  - `Encounter.class` (Coding) to `class` (list of CodeableConcept);
  - `MedicationRequest.medication[x]` to `medication` (CodeableReference);
  - `Immunization.reaction.detail` to `reaction.manifestation` (CodeableReference);
  - `MedicationStatement` to `MedicationStatement.medication` (CodeableReference);
  - `MedicationStatement.status` codes changed.
- **Bridge options (analysis):**
  - **(a) Down-convert R5 to R4 JSON** at the `DataProvider` boundary for the resource types the DAK reads. This is a per-type mapping, and it is the only option that keeps WHO's CQL unchanged.
  - **(b) Port the libraries to `using FHIR version '5.0.0'`.** This forks WHO content, and the engine would still not type-check it.
  - **(c) Teach the engine an R5 FHIRHelpers or ModelInfo.** This is upstream work.
  - Every option also needs a version-aware FHIRHelpers. Today it is hard-wired into `json.rs`.

## Verdict

**CQL via `octofhir-cql` cannot replace hand-written FHIRPath for indicators today (analysis):**

1. Nothing WHO publishes translates. Even after four parser patches, HIV (the programme the platform targets first) still fails on gaps J and K in the shared `WHOCommon` and `HIVIndicatorElements`. Fixing those is upstream work in a one-contributor project (1 contributor, 2 stars, created 2025-12-26, 316 crates.io downloads) [3][4].
2. Where it does run, it needed a hand-made workaround for implicit conversions. Without type checking, more such cases will only surface at run time, per patient.
3. On our R5 data it gives silent wrong answers without a down-conversion layer.
4. WHO's own fixtures are thin (one ANC patient, no HIV data) or stale (ANCIND01 expected numerator), so "reuse the DAK as-is" still needs our own test patients and a reference oracle.

**It is worth keeping open:**

- The evaluator matched the reference engine on everything it could run.
- It is fast and embeddable.
- The gaps are concrete and listed above with reproductions.

A reasonable follow-up is to file A–N upstream and re-run this spike (scripts below) when a release closes them. ADR 0068's "CQL left open" stands. The spike adds no reason to move the decision towards CQL now, and one reason to keep the JVM reference engine at central as an oracle (option D in indicators.md): it was easy to run (about 10 minutes) and it caught a stale WHO fixture.

## Reproducing

In this branch:

- [`cql-spike/minimal.sh`](cql-spike/minimal.sh): one-line reproductions of gaps A–D and H–K. Run it with `CQL=/path/to/cql`.
- [`cql-spike/patch_parser.py`](cql-spike/patch_parser.py): the four spike patches. Apply to `crates/octofhir-cql-parser/src/expression.rs` at commit `848156b`.

The remaining harness is in the session scratchpad (not committed): `cql-spike-harness/` holds `fetch*.sh`, `run.sh`, `ref.sh` (reference CLI through jbang), `compare.py` (per-definition diff), `split_bundle.py` and `r5ify.py`. `cql-spike/out/` holds the raw outputs, including `anc-results.json`.

To run the reference engine:

```
jbang --java 17 org.opencds.cqf.fhir:cqf-fhir-cr-cli:4.15.0 cql -fv R4 -source <cql dir> -name ANCIND01 -data <dir>/input/<Type>-<id>.json… -terminology <dir>/input/ValueSet-….json -c Patient -cv charity-otala-1
```

## Sources

1. octofhir/cql-rs README (Conformance, FHIR data model, Limitations), commit 848156b — https://github.com/octofhir/cql-rs
2. `crates/octofhir-cql-model/src/fhir/{r4,r5,json}.rs`, same commit — https://github.com/octofhir/cql-rs/tree/main/crates/octofhir-cql-model/src/fhir
3. crates.io `octofhir-cql` (max 0.2.5, 316 downloads) — https://crates.io/crates/octofhir-cql
4. GitHub API `repos/octofhir/cql-rs` and `/contributors`, read 2026-11-17
5. HL7 FHIR R5 Encounter, MedicationRequest, Immunization, MedicationStatement — https://hl7.org/fhir/R5/encounter.html , https://hl7.org/fhir/R5/medicationrequest.html , https://hl7.org/fhir/R5/immunization.html , https://hl7.org/fhir/R5/medicationstatement.html
6. WHO smart-hiv `input/cql/` (`WHOCommon.cql`, `HIVIndicatorElements.cql`, `HIVIND*Logic.cql`), commit b1e7735 — https://github.com/WorldHealthOrganization/smart-hiv/tree/main/input/cql
7. WHO smart-anc `input/cql/`, `bundles/measure/ANCIND01/`, `input/tests/measure/ANCIND01/`, commit 301a6b8 — https://github.com/WorldHealthOrganization/smart-anc
8. WHO smart-immunizations `input/cql/`, `input/tests/measures/`, commit bd7fa72 — https://github.com/WorldHealthOrganization/smart-immunizations
9. cqframework clinical-reasoning 4.15.0 (`cqf-fhir-cr-cli`), Maven Central and release notes — https://github.com/cqframework/clinical-reasoning/releases/tag/v4.15.0 , https://repo1.maven.org/maven2/org/opencds/cqf/fhir/cqf-fhir-cr-cli/4.15.0/
10. cqframework/cql-tests (vendored in cql-rs `tests/`) — https://github.com/cqframework/cql-tests
11. CQL 1.5 grammar (`cql.g4`: `existenceExpression`, `durationExpressionTerm`, `exclusiveRelativeQualifier`, `tupleSelector`, `listSelector`) — https://cql.hl7.org/19-l-cqlsyntaxdiagrams.html
