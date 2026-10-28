# FHIR R5 model, validation and search

Status: **accepted** (ADRs 0015, 0016). Produced while resolving the Wayfinder ticket [How are FHIR R5 resources modelled, validated and searched in Rust?](https://github.com/carapai/people-centered-healthcare/issues/15). Background: [docs/research/fhir-r5.md](../research/fhir-r5.md).

The physical storage of resources, history and index entries in SurrealDB is decided in [How are FHIR resources, versions and history persisted in SurrealDB?](https://github.com/carapai/people-centered-healthcare/issues/16). Layer placement is in [dependency-rules.md](dependency-rules.md) and [crate-catalogue.md](crate-catalogue.md).

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
  - It is chosen by the spike [Which R5 model crate passes the round-trip spike: helios-fhir or fhir-r5?](https://github.com/carapai/people-centered-healthcare/issues/35); `helios-fhir` is the default if the spike is inconclusive.
  - It lives in its own workspace member and is cached in CI, because generated models are expensive to compile.
- **Facade only.** Domain crates import only `health-fhir-models`. A crate swap must not touch domain code.
- **Typed in code, JSON at rest.** Domain logic works on typed models, and canonical R5 JSON is the stored resource of record.
- **Tolerant codes.** Required-binding codes keep unknown values on the wire (a `Known | Unknown(String)` pattern). Stored data may come from older or newer configurations, and parsing must never lose it.
- **R4 only at the boundaries.** R4 resources never enter storage.
  - `health-fhir-mapping` converts them in integration adapters (UgandaEMR/OpenMRS, insurers, labs) and in WHO SMART content import.
  - Every converted resource gets a Provenance pointing to its source.
- **JSON first.** XML is out of v1.

## FHIRPath

- FHIRPath runs behind a `FhirPathEngine` trait in `health-fhir-validation`.
- The default implementation is `octofhir-fhirpath` over JSON, which passes 1176/1176 official tests and is independent of the model crate. `helios-fhirpath` is the alternative if Helios models are chosen.
- The official FHIRPath test suite runs in CI against the configured engine.
- Terminology functions (`memberOf`, `subsumes`) delegate to the local terminology service.
- Whether CQL is needed stays in the map's Not yet specified.

## Validation (ADR 0016)

| Tier | Checks | Error / warning |
|---|---|---|
| 1. Structural | Parse, cardinality, primitive formats, choice exclusivity, no empty objects or strings | Error |
| 2. Core invariants | The 314 R5 core invariants via FHIRPath (`ele-1`, `dom-2`, …) | Error |
| 3. Profiles | Core profiles, plus country and program profiles active in the node's configuration snapshot, including slicing | Error |
| 4. Terminology | Required bindings are errors; extensible and preferred bindings are warnings | Per binding strength |

| Source of the write | Where it is validated | On failure |
|---|---|---|
| Our UI or API on any node (central, edge, desktop) | Tiers 1–4 on the **writing node** | Reject with an OperationOutcome |
| Sync ingest at the Authority Node, and at central | Tiers 1–4 again | Per-item `rejected` status plus a review record, never a silent drop |
| External or legacy inbound (integrations, R4 adapters) | Tier 1 strict; tiers 2–4 | Tier 1 failure: reject. Tier 2–4 failure: **quarantine** review queue |

- If a code is missing from the node's **local** terminology snapshot, an offline write gets a warning, and central re-checks the code on ingest.
- **Profile validator.** v1 starts with our structural and invariant checks plus profile snapshot evaluation. The spike evaluates `helios-fhir-validator` for profiles and slicing.
- **HL7 Java validator.** It is a **CI oracle only**: test fixtures and generated examples are cross-checked against it. It is never shipped.

## Search parameters and indexed projections

- **Registry.** It is built from the `SearchParameter` resources in the R5 core package (`definitions.json.zip` / `hl7.fhir.r5.search`) plus custom SearchParameters in active configuration packages.
  - Only parameters declared **supported** in our CapabilityStatement are indexed.
  - Activating a configuration package that adds or changes a SearchParameter starts a background **reindex**. The parameter becomes searchable only when the reindex completes.
- **Extraction at write time.**
  1. Each supported parameter's FHIRPath `expression` is evaluated against the resource.
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
