# FHIR R5 model, validation and search

Status: **accepted** (ADRs 0015, 0016, 0050, 0051, 0052, 0053). Produced while resolving the Wayfinder ticket [How are FHIR R5 resources modelled, validated and searched in Rust?](https://github.com/carapai/people-centered-healthcare/issues/15). Background: [docs/research/fhir-r5.md](../research/fhir-r5.md).

The physical storage of resources, history and index entries in SurrealDB is in [persistence.md](persistence.md). Layer placement is in [dependency-rules.md](dependency-rules.md) and [crate-catalogue.md](crate-catalogue.md).

## Crates involved

| Crate | Layer | Role |
|---|---|---|
| `health-fhir-models` | contracts | Facade over the adopted R5 model crate. Re-exports the types domain code uses, plus our helpers (typed extension accessors, reference helpers, typed ids). |
| `health-fhir-validation` | contracts | Validation tiers 1–4, the `FhirPathEngine` trait and its default implementation, and validation results as OperationOutcome. |
| `health-fhir-search` | contracts | SearchParameter registry, search request parser and AST, index-value extraction and normalization. Pure. |
| `health-fhir-mapping` | contracts | R4 ↔ R5 conversion for boundary adapters. |
| `health-fhir-server` | infrastructure | FHIR REST handlers, CapabilityStatement, Bundle assembly. |
| `health-surrealdb` | infrastructure | Translates search ASTs into SurrealQL, and stores resources, history and index entries. |

## Resource model (ADR 0015)

- **Adopted crate.** One generated R5 (5.0.0) model crate, pinned to an exact version with only R5 enabled.
  - It is **`fhir-r5 =4.2.3`** (ADR 0050), the choice of the spike [Which R5 model crate passes the round-trip spike: helios-fhir or fhir-r5?](https://github.com/carapai/people-centered-healthcare/issues/35). R4 conversion uses its sibling `fhir-r4 =4.2.2`.
  - CI round-trips the whole official R5 example corpus on every bump, with a short list of examples the model crate cannot parse excluded.
  - Only rarely changing crates name the whole `Resource` enum, because its serde code is rebuilt in each crate that uses it.
  - Parsing is strict: a missing required element is a parse error, and unknown elements are dropped. So typed parsing is never the validator or the stored form (ADR 0051).
- **Raw JSON first (ADR 0051).** Tier 1 runs on raw JSON before any typed parse, using an element table generated from `definitions.json.zip`. The stored resource is the validated input JSON in canonical form, with the server-set `id` and `meta` fields. Reads, search, `_history` and sync serve the stored bytes and never re-parse them.
  - It lives in its own workspace member and is cached in CI, because generated models are expensive to compile.
- **Facade only.** Domain crates import only `health-fhir-models`. A crate swap must not touch domain code.
- **Typed in code, JSON at rest.** Domain logic works on typed models, and canonical R5 JSON is the stored resource of record.
- **Tolerant codes.** Required-binding codes keep unknown values on the wire (a `Known | Unknown(String)` pattern). Stored data may come from older or newer configurations, and parsing must never lose it.
- **R4 only at the boundaries.** R4 resources never enter storage.
  - `health-fhir-mapping` converts them in integration adapters (UgandaEMR/OpenMRS, insurers, labs) and in WHO SMART content import.
  - Every converted resource gets a Provenance pointing to its source.
- **JSON first.** XML is out of v1.

### In code

`health-fhir-models` (contracts) depends on `fhir-r5 =4.2.3`. It re-exports `resources`, `types`, `codes`, `coded` and `validate` under its own paths, and provides `parse_resource`, `to_json` and `round_trip`.
- **Facade enforcement.** No other crate may depend on `fhir-r5`, which the `facade-bypass` architecture check enforces. ADR 0050's "no `fhir-r5` type in the public API" is read this way: consumers name model types only through `health_fhir_models::…`, so a swap changes the facade, not its users. Wrapping every generated type would defeat ADR 0015's "domain code works on typed models".
- **Round-trip.** The corpus test round-trips all 2,824 official examples: 2,823 exact, plus `questionnaireresponse-example-f201-lifelines.json`. ADR 0050 called it invalid, but it is valid R5: it carries `_questionnaire`, an extension-only required primitive, which tier 1 accepts and `fhir-r5` refuses. It is listed as `KNOWN_UNPARSEABLE_EXAMPLES`, a crate gap worth reporting upstream. Committed fixtures cover the edge cases on every PR. `tools/fhir/fetch-r5-examples.sh` fetches the full corpus with a pinned SHA-256 for CI.
- **Findings.**
  - The model writes `1.0e3` as `1.0e+3`: the same value and precision, but different text. Stored resources are the client's bytes and never re-serialized (ADR 0051), so only JSON *produced* from typed models is affected.
  - An element the type does not allow is dropped, e.g. `Observation.valueInteger64`, since `integer64` is not an Observation value type. Tier 1 runs first for that reason.

## FHIRPath

- FHIRPath runs behind a `FhirPathEngine` trait in `health-fhir-validation`.
- The default implementation is `octofhir-fhirpath` over JSON, which passes 1176/1176 official tests and is independent of the model crate.
- **Decimal-exact (ADR 0052).** 0.4.57 converts JSON numbers through f64. We carry a patched fork, offered upstream, that builds decimals from the number's text.
  - Results keep their source scale, so 1.50 stays 1.50.
  - An expression over a decimal of more than 28 significant digits fails closed.
  - A platform decimal conformance suite runs on every engine bump.
- The official FHIRPath test suite runs in CI against the configured engine.
- Terminology functions (`memberOf`, `subsumes`) delegate to the local terminology service.
- Whether CQL is needed stays in the map's Not yet specified.

## Validation (ADR 0016)

| Tier | Checks | Error / warning |
|---|---|---|
| 1. Structural | On raw JSON before typed parsing: syntax, known `resourceType`, **unknown elements**, cardinality, primitive formats, choice exclusivity, no empty objects or strings | Error |
| 2. Core invariants | The 314 R5 core invariants via FHIRPath (`ele-1`, `dom-2`, …) | Error |
| 3. Profiles | Core profiles, plus country and program profiles active in the node's configuration snapshot, including slicing | Error |
| 4. Terminology | Required bindings are errors; extensible and preferred bindings are warnings | Per binding strength |

| Source of the write | Where it is validated | On failure |
|---|---|---|
| Our UI or API on any node (central, edge, desktop) | Tiers 1–4 on the **writing node** | Reject with an OperationOutcome |
| Sync ingest at the Authority Node, and at central | Tiers 1–4 again | Per-item `rejected` status plus a review record, never a silent drop |
| External or legacy inbound (integrations, R4 adapters) | Tier 1 strict; tiers 2–4 | Tier 1 failure: reject, and keep the raw payload in the **Inbound Dead Letter** (ADR 0051). Tier 2–4 failure: **quarantine** review queue |

- If a code is missing from the node's **local** terminology snapshot, an offline write gets a warning, and central re-checks the code on ingest.
- **Profile validator (ADR 0053).** Tier 3 is our own validator behind a `ProfileValidator` trait. It extends the tier 1 raw-JSON walker and is driven by compiled snapshots.
  - **Coverage:** cardinality, `fixed[x]` and `pattern[x]`, type and target-profile restrictions, extension contexts, profile invariants, and slicing with all five discriminator types and the `closed`, `open` and `openAtEnd` rules.
  - **Snapshots and compilation.** Packages must ship snapshots, and a differential-only profile fails publication. Profiles are compiled at activation.
  - **Unsupported constructs** fail package Validation.
  - **Budget:** p95 ≤ 2 ms per resource for tiers 1–3 on reference edge hardware.
  - **Rejected crates.** `helios-fhir-validator` cannot share a build with `fhir-r5`, and `octofhir-fhirschema` 0.3.30 drops slicing on base elements and never enforces `fixed` or `pattern`.
- **HL7 Java validator.** It is a **CI oracle only**: test fixtures and generated examples are cross-checked against it. It is never shipped.

### Tier 1 in code

`health-fhir-validation::validate_structure` walks the raw JSON against `data/r5-elements.json`. That table is generated from the core package's base StructureDefinitions by `tools/fhir-element-table`, and `tools/fhir/check-element-table.sh` fails on drift.

- **Write paths.** Every REST write (create, update, conditional writes and each Bundle entry) runs tier 1 before anything else reads the body. A failure is 400 with one OperationOutcome issue per problem, each with its FHIRPath `expression`; inside a Bundle the expression is rewritten to `Bundle.entry[n].resource…`.
- **Two deliberate departures from the snapshots:**
  - `Element.id` is checked as `string`, not `id`. ElementDefinition ids hold `:` and exceed 64 characters in the official examples.
  - Decimals are any JSON number, not the core regex's 18 digits (ADR 0052: tier 1 accepts longer values and stores them unchanged).
- **Corpus.** All 2,824 official examples pass tier 1. Every one that passes also parses into `fhir-r5`, except `questionnaireresponse-example-f201-lifelines.json`. That example is valid R5, with an extension-only required primitive, but `fhir-r5` refuses it, so it is listed as `KNOWN_UNPARSEABLE_EXAMPLES`. ADR 0050 called it invalid, which was a misreading.
- **`skeleton(type)`** builds a minimal valid instance of any resource type, for tests and tooling.
- **Follow-ups:**
  - tier 2 invariants and the decimal-exact FHIRPath engine ([#165](https://github.com/carapai/people-centered-healthcare/issues/165));
  - the HL7 Java validator cross-check ([#166](https://github.com/carapai/people-centered-healthcare/issues/166));
  - the Inbound Dead Letter and quarantine ([#167](https://github.com/carapai/people-centered-healthcare/issues/167)).

## Search parameters and indexed projections

- **Registry.** It is built from the `SearchParameter` resources in the R5 core package (`definitions.json.zip` / `hl7.fhir.r5.search`) plus custom SearchParameters in active configuration packages.
  - Only parameters declared **supported** in our CapabilityStatement are indexed.
  - Activating a configuration package that adds or changes a SearchParameter starts a background **reindex**. The parameter becomes searchable only when the reindex completes.
- **Extraction at write time.**
  1. Each supported parameter's FHIRPath `expression` is evaluated against the resource. Extraction uses the **search subset** of FHIRPath, compiled once per parameter: unions of paths with `ofType`/`as`, `[n]`, `first()`, `.(a | b)`, `extension('url')`, `where(resolve() is T)` and `where(field = 'literal')`. It never computes values, so it needs none of the decimal arithmetic of ADR 0052. It compiles 1,211 of the 1,244 R5 core SearchParameters. The rest, such as `Patient.deceased.exists() and Patient.deceased != false`, compute a value; they cannot be supported until extraction runs them on the full engine.
  2. The results are normalized per search type into index entries:

     | Type | Normalized form |
     |---|---|
     | token | system + code (plus display text for `:text`) |
     | string | case- and accent-folded |
     | date | a [start, end) period from the stated precision |
     | reference | target type + id, or canonical URL + version |
     | quantity | value + UCUM-canonical unit (`octofhir-ucum`), plus the original system/code |
     | number | exact decimal |
     | uri | as stored |
     | composite | tuple of component entries |

  3. The entries are written in **the same transaction** as the resource version and its history.
- **Strict by default.** An unknown or unsupported parameter, modifier or prefix returns **400 with an OperationOutcome**. A client must send `Prefer: handling=lenient` to have unknown parameters ignored. Unsupported modifiers on an advertised parameter are always rejected, as R5 requires.
- **GET and POST.** Both `GET` and `POST [type]/_search` are supported.

## v1 search feature set

| Area | Supported in v1 | Later (when a use case needs it) |
|---|---|---|
| Parameter types | number, date, string, token, reference, composite, quantity, uri | special |
| Prefixes | `eq ne gt lt ge le sa eb ap` | — |
| Modifiers | `:exact :contains :missing :not :identifier :of-type :text`, and `:in :not-in :above :below` against the local terminology snapshot | `:code-text`, `:text-advanced`, `:[type]` |
| Chaining | one level of `.` chaining; one level of `_has` | multi-level chains |
| Includes | `_include`, `_revinclude` | `:iterate` |
| Result control | `_sort`, `_count`, `_summary`, `_elements`; `_total=accurate` where cheap, otherwise `estimate` or `none` | `_contained`, `_score`, `_filter`, `_text`, `_content` |
| Paging | keyset (cursor) links | offset paging (never) |

Authorization filters from the policy decision point ([offline-auth.md](offline-auth.md)) are added to every search query.

## Versioning and REST obligations

These are fixed here for [How are FHIR resources, versions and history persisted in SurrealDB?](https://github.com/carapai/people-centered-healthcare/issues/16) to implement:
- `versioned-update` capability;
- weak ETag `W/"<versionId>"`;
- `If-Match` → 412 on mismatch;
- 410 for deleted resources and 404 for unknown ones;
- the R5 conditional create, update and delete matrices;
- `_history` at instance, type and system level;
- transactions that are all-or-nothing.

Version-id format and client-supplied ids versus R5's "server SHALL ignore client id on create" are also resolved there. Our sync protocol relies on client-generated ids (ADR 0013).
