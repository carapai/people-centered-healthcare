# Progress

_Update after each task (AGENTS.md process step 4)._

| Phase | Done | Total |
|---|---|---|
| [Phase 1 — Monorepo foundations](https://github.com/carapai/people-centered-healthcare/milestone/1) | 6 | 11 |
| [Phase 2 — Persistence and runtime adapters](https://github.com/carapai/people-centered-healthcare/milestone/2) | 13 | 17 |
| [Phase 3 — Identity, security and Client Registry](https://github.com/carapai/people-centered-healthcare/milestone/3) | 0 | 10 |
| [Phase 4 — Facility Edge Node foundation](https://github.com/carapai/people-centered-healthcare/milestone/4) | 0 | 7 |
| [Phase 5 — Synchronization foundation](https://github.com/carapai/people-centered-healthcare/milestone/5) | 0 | 7 |
| [Phase 6 — Terminology and metadata](https://github.com/carapai/people-centered-healthcare/milestone/6) | 2 | 11 |
| [Phase 7 — Universal workflow engine](https://github.com/carapai/people-centered-healthcare/milestone/7) | 0 | 7 |
| [Phase 8 — OPD clinical vertical slice](https://github.com/carapai/people-centered-healthcare/milestone/8) | 0 | 5 |
| [Phase 9 — Clinical services](https://github.com/carapai/people-centered-healthcare/milestone/9) | 0 | 4 |
| [Phase 10 — Patient engagement](https://github.com/carapai/people-centered-healthcare/milestone/10) | 0 | 5 |
| [Phase 11 — Clinical Program Studio](https://github.com/carapai/people-centered-healthcare/milestone/11) | 0 | 3 |
| [Phase 12 — Finance and insurance](https://github.com/carapai/people-centered-healthcare/milestone/12) | 0 | 8 |
| [Phase 13 — Inventory and procurement](https://github.com/carapai/people-centered-healthcare/milestone/13) | 0 | 5 |
| [Phase 14 — Additional clinical operations](https://github.com/carapai/people-centered-healthcare/milestone/14) | 0 | 0 |
| [Phase 15 — Public health and analytics](https://github.com/carapai/people-centered-healthcare/milestone/15) | 0 | 0 |
| [Phase 16 — Advanced desktop offline mode](https://github.com/carapai/people-centered-healthcare/milestone/16) | 0 | 5 |
| [Phase 17 — Production hardening](https://github.com/carapai/people-centered-healthcare/milestone/17) | 0 | 5 |

Total: 21 of 110 implementation tasks done. Validation spikes open: 3.

## Log

- 2026-10-28: backlog created; no implementation started.
- 2026-10-29: model-crate spike resolved (ADR 0050); follow-ups decided (ADRs 0051–0053); added #145.
- 2026-10-29: frontend-scaffold spike resolved (ADR 0054).
- 2026-10-29: matching-scale spike resolved at reduced scale; raised #146.
- 2026-10-29: #42 Cargo workspace created: six-layer layout with one placeholder crate per layer, rust-toolchain 1.98.0 and `deny.toml`.
- 2026-10-29: #43 tools/architecture-checks built: 13 rules, 24 fixture tests; clean on the real workspace.
- 2026-10-29: #44 Bun workspace and React app shell with the platform boundary, the oxlint and boundary guards, and the bundle scan.
- 2026-10-29: #52 health-identifiers: typed v7/v5 ids, the UUIDv5 derivation with golden tests, and number formats with Luhn mod 36. Concurrent allocation moved to #56.
- 2026-10-29: #54 health-persistence traits, `stamp_meta` and `RetryPolicy`; health-time (jiff, Clock); the in-memory store with snapshot isolation in health-persistence-memory.
- 2026-10-29: #55 health-surrealdb: connections (embedded RocksDB `sync=every`, remote, memory) and the forward-only migrations runner with checksums and a schema snapshot; SurrealDB pinned `=3.3.5`.
- 2026-10-29: #56 SurrealStore on interactive transactions (heads, history, guards, counters, outbox); migration 0001 for 84 owned types; ownership registry with category partitions and its architecture test.
- 2026-10-29: #57 health-fhir-search (catalog over the FHIRPath search subset: 1,211 of 1,244 core parameters; registry; extraction; strict parser; SearchPolicy) and health-surrealdb search: migration 0002 index tables with `index_keys` on heads, range-scan compiler with EXPLAIN tests, keyset paging, resumable reindex.
- 2026-10-29: #49 health-observability (allowlist export layer, OTLP/HTTP, JSON logs, redaction test with a negative control), health-http (`/health`, `/ready`, request tracing by route template) and the central-server binary (env config, password from a file, background connect-and-migrate with backoff, graceful shutdown).
- 2026-10-29: #58 health-fhir-server: read, vread, create, update (If-Match required; update-as-create), delete, instance/type/system history, search GET/POST with keyset links, CapabilityStatement generated from behaviour; 11 R5 REST conformance fixtures. Split out #157 (conditional interactions), #158 (transaction and batch Bundles), #159 (reference integrity).
- 2026-10-29: #157 conditional create, update and delete per the R5 matrices, resolved through the search index in the write transaction; `condition_lock` (migration 0003) serializes creates under the same condition; 3 fixtures and an 8-way race test.
- 2026-10-29: #158 transaction Bundles (one SurrealDB transaction in R5 order, `urn:uuid` rewriting, all or nothing, rerun on conflict) and batch Bundles (independent entries); write steps factored into `ops`; 2 fixtures.
- 2026-10-29: #159 reference integrity: local references must resolve (422) at the end of each write transaction; `idx_ref_in` reverse references (migration 0004) make referenced deletes 409; `ReferenceScope` hook for out-of-scope targets.
- 2026-10-29: #53 health-fhir-models facade over `fhir-r5 =4.2.3`; full R5 corpus round-trips (2,823 exact + 1 known-invalid); `facade-bypass` architecture check; corpus fetch script with a pinned SHA-256.
- 2026-10-29: #59 (tier 1): health-fhir-validation structural check on raw JSON from a generated element table (tools/fhir-element-table); on every REST write path; full corpus passes and agrees with typed parsing (one recorded fhir-r5 gap). Split out #165 (tier 2 and FHIRPath), #166 (Java validator cross-check), #167 (Inbound Dead Letter and quarantine).
- 2026-10-29: #165 tier 2: the vendored, patched octofhir-fhirpath (decimals, plus six invariant-correctness patches) and octofhir-ucum (panic on non-ASCII units); FhirPathEngine trait with our own model provider; R5 core invariants on every REST write; decimal conformance suite; corpus holds except 171 independently confirmed example errors. Tiers 1–2 p95 3.06 ms on clinical resources, over the tiers 1–3 budget.
- 2026-10-29: #145 tier 3: `ProfileSet::compile` over StructureDefinition snapshots (refuses differential-only profiles and unsupported constructs) and `SnapshotValidator` behind `ProfileValidator`: cardinality, fixed/pattern, types, target profiles, extension definitions and contexts, profile invariants, slicing (value, pattern, exists, type, profile; closed, open, openAtEnd, ordered). Official vital-signs, heart-rate and BP profiles; every official example claiming them conforms. Engine patch 0008 (`as` on FHIR primitives). Split out #170 (2 ms budget); the Java differential joins #166; wiring is #91.
- 2026-10-29: #170 validation budget: `HOLDS_BY_SHAPE` skips the hot core invariants (`dom-2`–`dom-5`, `ref-1`, `ref-2`, `qty-3`) where the node's shape proves they hold, each verified against the engine over the corpus; `tests/performance.rs` holds tiers 1–3 to p95 ≤ 2 ms. Tiers 1–3 p95 1.12 ms on 731 clinical examples (laptop), from 2.5–3.1 ms for tiers 1–2 alone; reference edge hardware pending.
- 2026-10-29: #47 CI: rust-ci (fmt, clippy, build, nextest, doc tests, doc, architecture checks, deny, coverage ≥ 80% on domain and application crates, and a FHIR corpus job with checksum-verified downloads), frontend-ci, security, dependency-review and documentation workflows; actions pinned to SHAs by pinact; Renovate config; markdownlint config and doc fixes; ADR index check and Mermaid render scripts. CodeQL and dependency review wait for GitHub Advanced Security. Split out #173 (Vitest and component tests).
- 2026-10-29: #48 merge rules: CODEOWNERS, PR-title Conventional Commit check, ADR two-approvals check, and tools/github/configure-repository.sh (squash only, push protection, the `main` ruleset with every CI job required). Needs the repository public on this account; the merge queue needs an organization.
- 2026-10-29: #166 HL7 Java validator differential (pinned 7.0.2, CI job): 1,373 files; every tier 1 variant and tier 3 case agrees except two where ours is stricter per spec; 151 clinical examples fail only in Java for rules outside our tiers, recorded with reasons. Found and fixed: tier 1 `ele-1` (id-only elements), versioned canonicals in generated snapshots, non-type slicing of choice elements; custom profile fixtures now have HL7-generated snapshots. Split out #176 (Identifier urn:ietf:rfc:3986 rule).
