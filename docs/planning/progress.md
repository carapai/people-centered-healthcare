# Progress

_Update after each task (AGENTS.md process step 4)._

| Phase | Done | Total |
|---|---|---|
| [Phase 1 — Monorepo foundations](https://github.com/carapai/people-centered-healthcare/milestone/1) | 3 | 10 |
| [Phase 2 — Persistence and runtime adapters](https://github.com/carapai/people-centered-healthcare/milestone/2) | 5 | 11 |
| [Phase 3 — Identity, security and Client Registry](https://github.com/carapai/people-centered-healthcare/milestone/3) | 0 | 10 |
| [Phase 4 — Facility Edge Node foundation](https://github.com/carapai/people-centered-healthcare/milestone/4) | 0 | 7 |
| [Phase 5 — Synchronization foundation](https://github.com/carapai/people-centered-healthcare/milestone/5) | 0 | 7 |
| [Phase 6 — Terminology and metadata](https://github.com/carapai/people-centered-healthcare/milestone/6) | 0 | 9 |
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

Total: 8 of 101 implementation tasks done. Validation spikes open: 3.

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
