# Research: FHIR R5 resource models, validation and search in Rust

Ticket: [#5](https://github.com/carapai/people-centered-healthcare/issues/5) · Parent map: #1 · Unblocks: #15
Researched: 2026-10-27 · Spec context: `MASTER_PROMPT.md` §3.1 (R5 canonical throughout), §16 (`health-fhir-models`, `-validation`, `-search`, `-server`), Part 5 §34–39 (validated resources + indexed projections, history, optimistic concurrency).

## Summary

- **Types/serde: adopt, don't hand-roll.** Three actively maintained Rust crates generate complete R5 (5.0.0) models from the official StructureDefinitions and round-trip canonical JSON, including primitive extensions (`_field`): **`helios-fhir`** (HeliosSoftware/hfs, MIT, released 2026-10-25), **`fhir-model`/`fhir-sdk`** (FlixCoder, MIT, last release 2026-05-15) and **`fhir`/`fhir-r5`** (fhir-rust, multi-licence, released 2026-10-16). `fhirbolt` supports R5 but has not been released since 2023-05 and should be treated as unmaintained.
- **FHIRPath: two serious engines exist.** `octofhir-fhirpath` reports 1176/1176 official tests passing (2026-10-21 report) and works over JSON with a pluggable model provider. `helios-fhirpath` implements FHIRPath 3.0.0-ballot over Helios' typed models, with a documented list of unimplemented functions (`conformsTo`, `%factory`, `slice`, ...).
- **Validation: only Helios ships a full profile validator in Rust** (`helios-fhir-validator` 0.2.6: FHIR-Schema-based structural and profile validation, FHIRPath invariants, required bindings, embedded R4–R6 core packages). The `fhir` crate validates structure, cardinality, primitive regexes and required bindings, and 3 of 314 R5 invariants. `fhir-sdk` has no validation. Everything else, including `octofhir-fhirschema`, is pre-1.0 and lightly used.
- **Search: no crate gives search we could drop onto SurrealDB.** Search-parameter extraction, the R5 parameter types, modifiers and prefixes, chaining, `_include` and `_history` exist as working code only inside complete servers (Helios `helios-persistence` + `helios-rest` over SQLite, Postgres, Mongo, S3 and Elasticsearch, and `fhir-rust` over SQL engines). None of them has a SurrealDB backend. We will build `health-fhir-search` ourselves. It should be driven by the official `SearchParameter` definitions, with FHIRPath used to extract indexed values.
- **Codegen from the official package is well understood.** `hl7.fhir.r5.core` 5.0.0 is the only R5 core package version, and HL7 calls `definitions.json.zip` "the first choice whenever generating any implementation artifacts". All three model crates already do this. Writing our own generator only makes sense if we want SurrealDB-specific projections generated from the same source.
- **Recommendation for the decision (#15): hybrid.** Adopt a third-party R5 model crate behind a thin `health-fhir-models` facade. Use or wrap a FHIRPath engine. Own search-parameter indexing, history and versioning in SurrealDB, and generate the indexing tables from `SearchParameter` resources. Evaluate `helios-fhir-validator` for profile validation, and keep the HL7 Java validator as a fallback or CI oracle. All candidate crates are pre-1.0 (or 4.x with a weeks-old R5 crate), so pin exact versions.

## Crate comparison

All rows are from crates.io API (`https://crates.io/api/v1/crates/<name>`) and GitHub API (`https://api.github.com/repos/<repo>`), fetched 2026-10-27.

| Crate (repo) | Scope | R5 | Latest / released | Created | Downloads | Licence | Serde & primitive `_field` handling | Validation | FHIRPath | Notes |
|---|---|---|---|---|---|---|---|---|---|---|
| **helios-fhir** ([HeliosSoftware/hfs](https://github.com/HeliosSoftware/hfs)) | Models R4/R4B/R5/R6 (feature flags, default R4) | Yes (5.0.0) | 0.2.6 / 2026-10-25 | 2025-08 | 80.8k | MIT | Primitives are `Element<V, Extension>` type aliases (`DecimalElement` for exact decimals). A derive macro merges and splits `field`/`_field` pairs. **Verified in source** (`crates/fhir-gen/src/lib.rs` L2676–2808, `crates/fhir-macro/src/lib.rs` L28, L831). | Separate crate `helios-fhir-validator` 0.2.6 (profiles, invariants, bindings) | `helios-fhirpath` (typed, version-aware) | 57★, pushed daily, 90 open issues. Part of a full server (`helios-hfs`, `-rest`, `-persistence`, `hts` terminology). docs.rs build of 0.2.6 failed (0.2.5 OK). |
| **fhir-model** / **fhir-sdk** ([FlixCoder/fhir-sdk](https://github.com/FlixCoder/fhir-sdk)) | Models STU3/R4B/R5 + async REST client | Yes | 0.13.0 / 0.16.0, 2026-05-15 | 2023-03 | 134k / 108k | MIT | Each primitive gets a parallel `*_ext: Option<FieldExtension>` field, or `Vec<Option<FieldExtension>>` for arrays. Verified on [docs.rs `HumanNameInner`](https://docs.rs/fhir-model/latest/fhir_model/r5/types/struct.HumanNameInner.html). README: "`Vec<Option<T>>` ... required to allow `[null, {...}, null]`" and "not supported to replace required fields by an extension". | None ("Resource validation using FHIRpath" unchecked in README) | None | 73★. README warns that "compile time and its memory usage are really high". MSRV 1.95. No R4 (R4B only). Last commit 2026-05-28. |
| **fhir** / **fhir-r5** ([fhir-rust/fhir-rust](https://github.com/fhir-rust/fhir-rust)) | Models R2–R6 (R5 default), builders, XML, client, cross-release conversion | Yes (158 resources, 50 datatypes, 442 code enums) | 4.2.4 / 2026-10-16 (`fhir-r5` 4.2.3) | `fhir` name 2022-04. `fhir-r5` created 2026-08-10 | 9.9k / 764 | MIT OR Apache-2.0 OR BSD-3 OR GPL-2/3 | Transparent primitive newtypes. `Coded<E>` for required bindings. `Vec1<T>` for `1..*`. README lists a `primitive_extensions` example ("`_field` primitive extensions"). **Mechanism not inspected: unverified.** | `Validate` derive: structure, cardinality, regex, choice exclusivity, required bindings, and 3 of 314 R5 invariants. No profiles or terminology. | None | 6★, pre-release monorepo (relational SQL stores). 24 releases in about 3 months, so its stability record is short. MSRV 1.96. |
| fhirbolt ([lschmierer/fhirbolt](https://github.com/lschmierer/fhirbolt)) | JSON + XML ser/de, element model + typed R4/R4B/R5 | Yes | 0.4.0 / **2023-05-17** | 2023-03 | 13k | MIT OR Apache-2.0 | Custom serde via generic element model | None ("might be added eventually") | None | Self-described "(Experimental)". No release in 3.5 years. |
| fhir-types ([FerroHEALTH/FerroBRIDGE](https://github.com/FerroHEALTH/FerroBRIDGE)) | Generated per-version types for an openEHR bridge | unverified | 0.1.128 / 2026-10-25 | 2026-09 | 16k | Apache-2.0 | unverified | unverified | No | 1★. Internal to an openEHR↔FHIR bridge. Not assessed further. |
| haste-fhir-model (no repo listed) | Haste Health platform model | unverified | 0.127.4 / 2026-10-27 | 2025-11 | 8.3k | Apache-2.0 | unverified | unverified | unverified | No repository URL on crates.io. Not assessed further. |
| fhir-rs ([yeyanbo/fhir-rs](https://github.com/yeyanbo/fhir-rs)) | Models | unverified | 0.1.0 / 2024-12-08 | 2024-01 | 13.5k | non-standard (repo CC0) | — | — | — | Single release. Inactive. |
| **octofhir-fhirpath** ([octofhir/fhirpath-rs](https://github.com/octofhir/fhirpath-rs)) | FHIRPath engine (JSON input, pluggable `ModelProvider`, terminology provider, LSP) | Model-agnostic (R4/R5 via provider) | 0.4.57 / 2026-10-19 | 2025-07 | 14.8k | MIT OR Apache-2.0 | n/a | n/a | 1176/1176 official tests (report dated 2026-10-21) | Async API. Companion crates `octofhir-fhirschema` 0.3.29 (FHIR Schema validation) and `octofhir-canonical-manager` (package loading). |
| **helios-fhirpath** (hfs) | FHIRPath 3.0.0-ballot over Helios models | Yes | 0.2.6 / 2026-10-25 | 2025-08 | 10.5k | MIT | n/a | n/a | README lists missing `conformsTo`, `slice`, `checkModifiers`, `%factory.*`, function-form `subsumes`/`subsumedBy`, `htmlChecks` | `memberOf` via `%terminologies` and an external terminology server |
| rh-fhirpath ([reason-healthcare/rh](https://github.com/reason-healthcare/rh)) | FHIRPath parser/evaluator in an AI-agent FHIR toolkit | unverified | 0.3.0 / 2026-10-08 | 2026-04 | 2.8k | MIT OR Apache-2.0 | n/a | n/a | Compliance level unverified | Young (6 months). |
| fhirpath (no repo) | FHIRPath | — | 0.1.0 / 2024-06-15 | 2024-06 | 1.4k | MIT | — | — | unverified | Single release. Ignore. |

Sources: crates.io API JSON for each crate listed above; GitHub API repo metadata; READMEs at
[hfs README](https://github.com/HeliosSoftware/hfs/blob/main/README.md),
[hfs crates/fhir/README](https://github.com/HeliosSoftware/hfs/blob/main/crates/fhir/README.md),
[hfs crates/fhirpath/README](https://github.com/HeliosSoftware/hfs/blob/main/crates/fhirpath/README.md),
[hfs ROADMAP](https://github.com/HeliosSoftware/hfs/blob/main/ROADMAP.md),
[hfs crates/persistence/README](https://github.com/HeliosSoftware/hfs/blob/main/crates/persistence/README.md),
[fhir-sdk README](https://github.com/FlixCoder/fhir-sdk/blob/main/README.md),
[fhir-rust README](https://github.com/fhir-rust/fhir-rust/blob/main/README.md),
[fhir-rust fhir/README](https://github.com/fhir-rust/fhir-rust/blob/main/fhir/README.md),
[fhirbolt README](https://github.com/lschmierer/fhirbolt/blob/main/README.md),
[octofhir fhirpath-rs README](https://github.com/octofhir/fhirpath-rs/blob/main/README.md),
[octofhir TEST_COVERAGE.md](https://github.com/octofhir/fhirpath-rs/blob/main/TEST_COVERAGE.md).

## Findings by area

### 1. Resource models and (de)serialization

- **The hard part is the JSON primitive representation.** An `id` or extensions on a primitive go in a sibling property "with `_` prepended to the name of the element". Repeating primitives use parallel arrays with `null` placeholders, and if the arrays differ in length "implementations SHALL infer null values in the suffix of the shorter array". Decimal "precision, including trailing zeros, is preserved". Objects are never empty and strings are never empty. ([R5 JSON §primitive](https://hl7.org/fhir/R5/json.html#primitive))
  - `helios-fhir` models this as one value, `Element<V, E> { id, extension, value }`, with derive-generated serde. This is the cleanest fit for FHIRPath, which treats the value and its extensions as one node.
  - `fhir-sdk` uses parallel `x` / `x_ext` fields. This is faithful to the JSON shape, but clumsy in domain code and in FHIRPath adaptors.
  - Decimal: Helios has `DecimalElement`/`PreciseDecimal`, which preserves the original string. `fhir` has a `Decimal` in `fhir-core` (precision claims in its README, mechanism unverified).
- **Required bindings as enums.** `fhir` maps required-binding codes to `Coded<E>` (`Known(E) | Unknown(String)`), which keeps unknown codes on the wire. Helios and fhir-sdk also generate code enums (fhir-sdk: "Generated FHIR codes"). A tolerant fallback matters because SurrealDB will hold data written by older or newer configurations.
- **Build cost is real for every option.** fhir-sdk: "compile time and its memory usage are really high". Helios suggests `CARGO_BUILD_JOBS=4` and an 8 MiB stack link-arg. fhirbolt: "only include the release that you really need". Enable only R5. Isolate the model crate in its own workspace member so incremental builds skip it.
- **XML.** Only `fhir` (feature `xml`) and fhirbolt support XML. Helios lists an `xml` feature on crates.io (scope unverified). fhir-sdk does not plan XML. The spec's R5 format support is JSON-first for us, so this is low priority.

### 2. Validation (profiles, invariants/FHIRPath, terminology)

- Full validation needs four layers: (a) structure and cardinality from the base StructureDefinitions; (b) FHIRPath invariants such as `ele-1` and `dom-2` (R5 has 314 in core, per the fhir-rust README); (c) profile snapshots and slicing (`meta.profile`, IG packages); (d) terminology bindings (ValueSet membership, needing `$validate-code`/`$expand`).
- `helios-fhir-validator` covers (a)–(d). Its ROADMAP says: "validates structure, `meta.profile` claims, FHIRPath invariants, and required bindings against embedded R4–R6 core packages", exposed as `$validate`, with write-path mode `HFS_VALIDATION_MODE=off|log|enforce`. Its crates.io download count is low (1.4k), so it has seen little production use. Its conformance against the HL7 validator test suite is **unverified**.
- The `fhir` crate covers (a), plus required-binding enums for (d) and 3 invariants. It is explicitly *not* profile or terminology validation.
- `octofhir-fhirschema` 0.3.29 offers FHIR Schema conversion and validation (0★ repo, no licence file on GitHub, crates.io says MIT OR Apache-2.0). Treat it as experimental.
- The HL7 reference validator (Java) is not a Rust option. It can still serve as the CI oracle for whatever we adopt (inference, not checked against a source in this ticket).

### 3. Search parameter handling

- **R5 search surface** ([R5 search](https://hl7.org/fhir/R5/search.html)):
  - Parameter types: number, date, string, token, reference, composite, quantity, uri, special (`#ptypes`).
  - Prefixes: `eq ne gt lt ge le sa eb ap`, with implicit ranges from precision. For example, `2015-08-12` covers the whole day (`#prefix`).
  - Modifiers: `above, below, code-text (TU), contains, exact, identifier, in, iterate, missing, not, not-in, of-type, text, text-advanced (TU), [type]`. "A server SHALL reject a search with an unsupported modifier on a parameter it advertises" (`#modifiers`).
  - Repeated parameters are ANDed and commas are ORed (`#multivalue`). Chaining is `.`, reverse chaining is `_has`, and inclusion is `_include`/`_revinclude`/`:iterate`. Includes are "invoked only on the initial results" unless `:iterate`.
  - Result parameters: `_sort` (`-` for descending), `_count` ("SHALL NOT return more resources in a single page than requested"), `_total` = `none|estimate|accurate` (TU), `_summary`, `_elements`, `_contained`, `_maxresults` (SHALL appear in the self link), `_score`.
  - Searchset Bundle: `entry.search.mode` = `match|include|outcome`. The self link SHALL be a GET search carrying "the parameters that were actually used". Unknown parameters SHOULD be ignored, and `Prefer: handling=strict|lenient` SHOULD be honoured (`#errors`). `Bundle.total` counts matches only.
  - "Servers supporting Search via HTTP SHALL support both modes of operation": `GET` and `POST [type]/_search` with `application/x-www-form-urlencoded` ([R5 http §search](https://hl7.org/fhir/R5/http.html#search)).
- **The definitions are machine-readable.** `hl7.fhir.r5.search.tgz` contains "search parameters, uncombined per resource", and `definitions.json.zip` contains the combined `SearchParameter` bundle ([R5 downloads](https://hl7.org/fhir/R5/downloads.html)). Each SearchParameter carries a FHIRPath `expression`. The extractor therefore reduces to: evaluate the expression, then normalise to typed index rows (token system|code, date ranges, quantity with UCUM, reference targets, normalised strings).
- **Existing Rust search code is bound to its server.** Helios has a `SearchProvider` trait plus `ResourceStorage`, `VersionedStorage` (vread, `update_with_match`), `ConditionalStorage` and `TransactionProvider`, with backends for SQLite, PostgreSQL, MongoDB, S3 and Elasticsearch. **No SurrealDB backend** exists. In principle we could implement its traits for SurrealDB, but `helios-persistence` is pre-1.0 and carries its own schema and tenancy model ([persistence README](https://github.com/HeliosSoftware/hfs/blob/main/crates/persistence/README.md)). fhir-rust compiles search to SQL over its relational map, which does not apply to SurrealDB. fhir-sdk only has the *client* side of search (`SearchParameters`, `TokenSearch`).
- String search default: case-insensitive, accent-insensitive, and may match the start of the string (`#string`). UCUM normalisation for quantity search can come from `octofhir-ucum` 0.8.0 (crates.io, 23.7k downloads).

### 4. Versioning, `_history`, concurrency and conditional operations (server obligations)

All rules are from [R5 RESTful API](https://hl7.org/fhir/R5/http.html). Conditional create, update, patch and delete are marked trial use on that page.

- **Version ids and meta.**
  - On create, the server "SHALL populate the `id`, `meta.versionId` and `meta.lastUpdated`". If the client provides an `id`, the server "SHALL ignore it" (`#create`).
  - On update, the server "SHALL ignore the provided `versionId` and `lastUpdated`" (`#update`).
  - Version ids may be generated "in whatever fashion" the server wishes (`#concurrency`).
  - Capability is declared via `CapabilityStatement.rest.resource.versioning` = `no-version | versioned | versioned-update`. Non-versioning servers "SHALL ensure that `Resource.meta.versionId` is not present" (`#versions`). Part 5 §39 requires history and optimistic concurrency, so our target is **`versioned-update`**.
- **ETag and If-Match.**
  - The version id is a weak ETag: `ETag: W/"<versionId>"`, and the value "SHALL match the value of the version id". `Last-Modified` comes from `meta.lastUpdated` (`#versioning`, `#concurrency`).
  - A version-aware update sends `If-Match`. On mismatch the server returns **412 Precondition Failed**, and it may return **409 Conflict** when it detects a conflict. Servers MAY require `If-Match` (`#concurrency`).
  - PATCH: "servers that support PATCH SHALL support Resource Contention" (`#patch`). Patch bodies may be JSON Patch, XML Patch or FHIRPath Patch.
- **Conditional read.** `If-None-Match` or `If-Modified-Since` returns 304, or the full content if unsupported (`#cread`).
- **Read of deleted resources.** A read returns **410 Gone** for a deleted resource and **404** for an unknown one. A vread of a deleted version should return 410 (`#read`, `#vread`).
- **Update-as-create.** Servers MAY allow PUT to a new id. They "SHALL not return a 201" unless they created the resource (`#upsert`).
- **Conditional create** (`If-None-Exist`, `#ccreate`):

  | Matches | Result |
  |---|---|
  | 0 | Create |
  | 1 | 200, nothing created |
  | >1 | 412 |

- **Conditional update** (`#cond-update`):

  | Case | Result |
  |---|---|
  | 0 matches, no id | Create |
  | 0 matches, id given and new | Update-as-create (or reject) |
  | 0 matches, id given and existing | 409 |
  | 1 match | Update it (400 if the body id mismatches) |
  | >1 matches | 412 |

- **Conditional delete** (`#cdelete`): delete the one match. For multiple matches the server may delete all or return 412.
- **History** (`#history`):
  - Endpoints: `[type]/[id]/_history`, `[type]/_history` and `_history`.
  - The result is a Bundle with `type` = `history`, sorted with oldest versions last and including deletes. Entries with `request.method` PUT/POST SHALL carry the resource, and delete entries carry none.
  - Parameters `_since`, `_at`, `_count`, `_list` and `_sort`, each "SHALL NOT appear more than once".
  - A patch appears in history as an update.
- **Prefer.** `return=minimal|representation|OperationOutcome` SHOULD be honoured for successful interactions. Failures SHOULD return an OperationOutcome (`#ops`).
- **Transactions** (`#trules`):
  - Processing order is deletes, then creates, then updates/patches, then reads, then conditional-reference resolution.
  - The result "SHALL NOT depend on the order of the resources", a resource may appear only once, and the transaction is all-or-nothing.
  - A batch SHALL have no interdependencies.

  This puts a requirement on persistence: one SurrealDB transaction must span resource, history and index-row writes.

### 5. Generating types from the official R5 definitions

- **Source.** The only R5 core package version is `hl7.fhir.r5.core` **5.0.0** (`latest` = 5.0.0 per the [packages.fhir.org registry](https://packages.fhir.org/hl7.fhir.r5.core)). HL7 calls `definitions.json.zip` "the first choice whenever generating any implementation artifacts". Codegen aids also exist: `expansions.json`, `choice-elements.json`, `backbone-elements.json` and `fhir.schema.json.zip` (draft-06). HL7 also warns that names "may collide with reserved words", which matters in Rust for `type`, `use`, `ref` and so on ([R5 downloads](https://hl7.org/fhir/R5/downloads.html)).
- **Prior art.**
  - `helios-fhir-gen` (MIT, published) generates the Helios models from StructureDefinitions.
  - fhir-sdk's generator runs via `cargo make generate`.
  - fhir-rust's generator also emits a relational map and DDL from StructureDefinitions *and* SearchParameters. This is the closest analogue to generating SurrealDB projections.
- **Cost of writing our own.** The generator itself is roughly a 2–4k-line effort (estimate, unverified). The ongoing cost is the edge cases: primitive extensions, choice types, contentReference recursion (`Questionnaire.item.item`), decimals, reserved words, compile time. All three crates have already paid this cost.

## Implications for the decision

### Option A: Adopt a crate wholesale (e.g. the Helios stack: models + FHIRPath + validator + persistence traits)

- Pros:
  - Fastest route to a full set of R5 types, invariants, profile validation and a reference REST/search implementation.
  - MIT licence.
  - Very active development (releases 2026-10-02, -19 and -26).
- Cons and risks:
  - The crates are pre-1.0 (0.2.x) and APIs churn.
  - The project is analytics-first and has no SurrealDB backend.
  - Its persistence and tenancy model would compete with ours (§3.4 ownership).
  - Single-vendor bus factor (57★).
  - The docs.rs build of 0.2.6 failed.
  - Validator maturity is unproven (1.4k downloads).
  - The default feature is R4, so R5 must be enabled explicitly everywhere.

### Option B: Own codegen from `hl7.fhir.r5.core` 5.0.0

- Pros:
  - Full control over the Rust shape, SurrealDB projections, search-index schema and compile time.
  - One generator emits types, indexing tables and capability metadata from the same definitions. This matches §3.1 "FHIR R5 is canonical" and §39 "indexed projections".
- Cons and risks:
  - We would re-solve problems the existing crates already handle (primitive `_field`, decimals, choice types, recursion).
  - We would still need a FHIRPath engine and a validator.
  - A large upfront cost before any clinical feature.

### Option C (recommended): Hybrid

1. **`health-fhir-models`** wraps one adopted model crate, R5 only, with exact version pins. Two strong candidates:
   - **helios-fhir**: single-value `Element<V,E>` primitives, typed FHIRPath integration, MIT.
   - **fhir/fhir-r5**: idiomatic `Vec1`/`Coded<E>`, strongest semver claim, but the R5 crate is 2.5 months old with 6★.

   fhir-sdk is a viable fallback. Its `_ext` parallel fields and its 5-month release gap weigh against it. A short follow-up spike should round-trip the R5 `examples-json.zip` through the two front-runners, measure compile time, and check `_field` fidelity (unverified for `fhir`).
2. **FHIRPath:** use `octofhir-fhirpath` (100% official suite, JSON-native, pluggable model and terminology) or `helios-fhirpath` (if the models are Helios). Hide the choice behind a trait in `health-fhir-validation`.
3. **`health-fhir-validation`:** start with structural, cardinality and required-binding checks plus core invariants via FHIRPath. Evaluate `helios-fhir-validator` (or `octofhir-fhirschema`) for profiles and slicing. Use the HL7 Java validator as a CI oracle.
4. **`health-fhir-search` and persistence: build in-house on SurrealDB.**
   - Generate index projections from the R5 `SearchParameter` definitions (FHIRPath `expression`).
   - Implement the R5 types, prefixes and modifiers listed above. Reject unsupported modifiers.
   - Store versioned history rows with weak-ETag `versionId`.
   - Enforce `If-Match` → 412 and the conditional create/update/delete matrices.
   - Write resource, history and index rows in a single SurrealDB transaction.
   - Declare `versioned-update` in the CapabilityStatement.

   Helios' `ResourceStorage`/`VersionedStorage`/`SearchProvider`/`ConditionalStorage` trait split is a good template to copy, not a dependency.

### Cross-cutting risks

- **Ecosystem youth.** Every candidate is under 2 years old in its current form, or pre-1.0. Pin versions, vendor-review upgrades, and keep the model crate behind our facade so a swap stays local.
- **Compile time and memory** of generated models. Enable R5 only, keep the models in their own crate, and consider CI caching.
- **Trial-use features** (conditional ops, `_total`, `code-text`, `text-advanced`) may change in R6. Keep them behind capability flags.
- **Terminology-dependent search and validation** (`:in`, `:not-in`, `:above`, `:below`, required and extensible bindings) depends on the terminology decision for `health-terminology` (Part 6 §40).
- **R6 drift.** Helios and fhir-rust already track R6 ballots. Choosing a multi-version crate eases the future "legacy version conversion in explicit adapters" required by §34.

## Unverified items

- How `fhir`/`fhir-r5` implements `_field` primitive extensions (the README lists an example, but the code was not inspected).
- Whether `helios-fhir-validator` passes the HL7 validator test cases, and whether it handles slicing in full.
- Whether `helios-fhir`'s `xml` feature is complete.
- Details of `fhir-types` (FerroBRIDGE), `haste-fhir-model` and `rh-fhirpath`.
- The effort estimate for an in-house generator.
- That the HL7 Java validator suits use as a CI oracle (common practice, not sourced here).
