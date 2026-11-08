# Progress

_Update after each task (AGENTS.md process step 4)._

| Phase | Done | Total |
|---|---|---|
| [Phase 1 — Monorepo foundations](https://github.com/carapai/people-centered-healthcare/milestone/1) | 11 | 12 |
| [Phase 2 — Persistence and runtime adapters](https://github.com/carapai/people-centered-healthcare/milestone/2) | 17 | 17 |
| [Phase 3 — Identity, security and Client Registry](https://github.com/carapai/people-centered-healthcare/milestone/3) | 24 | 26 |
| [Phase 4 — Facility Edge Node foundation](https://github.com/carapai/people-centered-healthcare/milestone/4) | 6 | 8 |
| [Phase 5 — Synchronization foundation](https://github.com/carapai/people-centered-healthcare/milestone/5) | 0 | 8 |
| [Phase 6 — Terminology and metadata](https://github.com/carapai/people-centered-healthcare/milestone/6) | 3 | 12 |
| [Phase 7 — Universal workflow engine](https://github.com/carapai/people-centered-healthcare/milestone/7) | 1 | 7 |
| [Phase 8 — OPD clinical vertical slice](https://github.com/carapai/people-centered-healthcare/milestone/8) | 0 | 5 |
| [Phase 9 — Clinical services](https://github.com/carapai/people-centered-healthcare/milestone/9) | 0 | 4 |
| [Phase 10 — Patient engagement](https://github.com/carapai/people-centered-healthcare/milestone/10) | 2 | 8 |
| [Phase 11 — Clinical Program Studio](https://github.com/carapai/people-centered-healthcare/milestone/11) | 0 | 3 |
| [Phase 12 — Finance and insurance](https://github.com/carapai/people-centered-healthcare/milestone/12) | 3 | 9 |
| [Phase 13 — Inventory and procurement](https://github.com/carapai/people-centered-healthcare/milestone/13) | 5 | 7 |
| [Phase 14 — Additional clinical operations](https://github.com/carapai/people-centered-healthcare/milestone/14) | 0 | 0 |
| [Phase 15 — Public health and analytics](https://github.com/carapai/people-centered-healthcare/milestone/15) | 0 | 0 |
| [Phase 16 — Advanced desktop offline mode](https://github.com/carapai/people-centered-healthcare/milestone/16) | 0 | 5 |
| [Phase 17 — Production hardening](https://github.com/carapai/people-centered-healthcare/milestone/17) | 0 | 5 |

Total: 72 of 136 implementation tasks done (counted from the GitHub milestones). Validation spikes open: 3.

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
- 2026-10-30: #176 tier 1 enforces the Identifier rule: with system `urn:ietf:rfc:3986` the value is an absolute URI. `json-edge-cases.json` is recorded as an invalid example (a bare OID); the Java differential's recorded reason for the rule is gone (152 disagreements, all explained).
- 2026-10-30: #45 `@health/design-system` (`packages/design-system`): Panda 2 preset via `panda lib`; semantic tokens for light, dark and high-contrast (`data-theme`) and comfortable/compact density (`data-density`; 44 and 32 px controls); slot recipes and Ark UI 5 parts for Field, Select, Combobox, Dialog, Menu, Toast and Date Picker; ICU messages with react-intl (`ds.*`, typed values) and `I18nProvider`. The example page renders every recipe in all six theme × density combinations; 12 Vitest browser tests in Chromium, including axe (WCAG 2.2 AA tags, colour contrast checked) with no violations. The client loads it as its Panda `designSystem` and externalizes its strings (`app.*`). A `bun patch` fixes Ark 5.39's menu group declarations under `exactOptionalPropertyTypes`. New CI job in frontend-ci.
- 2026-10-30: #46 DataTable in `@health/design-system`: TanStack Table 9 (sorting, column visibility) and Virtual 3, an ARIA grid with the APG keyboard model (arrows, Home/End, Ctrl+Home/End, PageUp/PageDown, Enter/Space to sort, one tab stop; the focused row stays rendered), a Columns menu, a status message on sort, and a view (`sort`, `hide`) that round-trips through URL search parameters with unknown ids dropped. Keyboard-only navigation of 10,000 rows is tested in Chromium with fewer than 80 rows in the DOM; axe reports no violations in any theme. Found and fixed: Panda 2 presets have no z-index tokens (the Dialog and popups emitted `z-index: modal`); the bundle scan now rejects unresolved z-index tokens. Split out #180 (NVDA manual check, needs Windows).
- 2026-10-30: #50 generated client types: `@health/fhir-types` (158 resources, 43 data types, 630 backbone interfaces) from the R5 element table, with type-level tests; health-http's `/health` and `/ready` bodies typed and documented with utoipa 6 and utoipa-axum, `central-server openapi` prints central's document, and `@health/api-client` holds the generated Zod schemas, types, `operations` and `parseResponse`. The generators in `tools/codegen` print text because TypeScript 7 has no compiler API. New frontend-ci job regenerates everything and fails on drift.
- 2026-10-30: #62 the client's FHIR access layer (`apps/client/src/platform/`): `FhirClient` (read, vread, search over GET and POST, history, `$match`, dry-run `$extract`, `$populate`; `FhirError` with the OperationOutcome), `operationClient` over `@health/api-client` (Zod loaded on first call), HttpTransport auth strategies (central bearer; edge RFC 9449 DPoP with nonce retry), a fetch-based server-sent-events `EventSource`, in-memory sessions, and the Query keys, options and `connectQueryCache` (hints invalidate, logout and lock clear). One contract suite passes against a central and an edge `FhirServer` (`health-fhir-server`'s `contract_server` example), including exact decimals; 31 unit tests. Also closes #173: the client's Vitest runs in frontend-ci, next to the design system's component tests. New frontend-ci job: the client FHIR contract. Per-context command ports wait for their server use cases.
- 2026-10-31: #51 `tools/synthetic-data`. `generate` writes Ugandan synthetic data as NDJSON with a SHA-256 manifest, from its own PRNG so that a seed is byte-reproducible: 275 facilities in 45 districts, with their buildings, 740 stores, 20 medicines, about 12k stock balances, a price book, 4 payers with plans and contracts, and patients household by household. The patients have regional Zipf surname pools with Buganda clan surnames, Luo O-/A- gendering, Ganda twin names, shared and health-worker phones, NINs at about 75% with 0.1% duplicates, ADR 0014 MRNs and estimated ages. `load` sends batch Bundles of PUTs through the FHIR API. Every resource is tagged `urn:health:synthetic-data`. 1M patients generate in about 35 s, byte-identical across runs, and all 1,013,507 resources loaded through `FhirServer` in 27.4 min. The loader is HTTP only, because reqwest's TLS brings a CDLA-Permissive-2.0 crate.
- 2026-10-31: #60 `health-files`: the content-addressed blob store (ADR 0021). Blobs are SHA-256 addressed files under `blobs/ab/cd/`, written atomically (temp file, fsync, rename) and deduplicated by hash. It has a 50 MiB cap enforced while streaming, resumable 1 MiB chunk uploads verified on completion, and a hash check on every read that quarantines a corrupt file. Garbage collection removes only unreferenced blobs past the horizon, behind a `BlobReferences` port. `hashes()` lists blobs for backups. The sync upload endpoint and the Attachment reference index come with sync and persistence.
- 2026-10-31: #63 the authorization server, in two crates. `health-identity` (domain) has the lockout (10 failures lock for 15 min, hard cap 100), password-length, session (15-min tokens, 12-h cap) and SMART v2 scope rules. `health-auth` (infrastructure) is the server: authorization code with PKCE S256, ES256 access tokens DPoP-bound by `cnf.jkt` (required on edges), rotating refresh tokens with replay detection, `/revoke` logout, discovery and JWKS, Argon2id with a pepper, and `AccessVerifier` checking the token, the proof and session liveness. Its state sits behind an `AuthStore` port, served by `MemoryStore`. 7 end-to-end tests run through the routes. Split out #187 (SurrealDB store), #188 (mounting, signing keys, client sign-in) and #189 (breached-password list). #167 is now blocked by #66, because its raw payloads need audited access.
- 2026-10-31: #64 permission snapshots (ADR 0011). `health_identity::snapshot` has the snapshot model, the lifetime policy (14 days by default, 7–30 configurable, a warning in the last 3 days, and snapshots dated in the future refused), the monotonic clock floor, and role → permission mappings that refuse unknown roles. In `health-auth`, central's `SnapshotSigner` produces an ES256 `snapshot+jwt` that `SnapshotVerifier` checks, so tampering is detected. An edge's `SnapshotGate` requires a valid snapshot for every login, redemption and refresh, behind the clock floor, and an expired snapshot ends the session. Tokens carry `facility` and `snapshot_version`, never outlive the snapshot, and the token response carries `snapshot_not_after`. 5 end-to-end tests. Offline extensions, local grants and revocation ordering stay with #75 (listed in health-auth's README).
- 2026-10-31: #65 the policy decision point. `health-access-policy` (domain) decides from permissions (`{Type|*}.{cruds}`, with use-case codes kept apart), facility isolation, sensitivity labels mapped to programs, purpose of use, and break-glass: one patient, 1 h, `BTG` plus a justification, no extension, the reported event, and an audit obligation on each use. `health-identity-app`'s `SubjectSearchPolicy` turns the search filter into `_security:not=…`, so restricted records are excluded inside the SurrealDB query. Tests: an authorization-matrix table, and a real FhirServer search proving labelled records never appear. Found and fixed during the work: a use-case code such as `payment.record` would have granted reads, because “record” contains `r`. Split out #192 (per-request PDP on every FHIR interaction).
- 2026-10-31: #66 audit and provenance. `health-audit` is the only AuditEvent writer. It builds R5 AuditEvents (who, client, network, purpose, `BTG` under break-glass, versioned entities with their labels, the patient, the observer node) and appends them in the caller's transaction. It also has the Provenance builder. `FhirServer::with_audit` records one per write, including each Bundle entry, and one per read, vread, search, history or Bundle GET returning a sensitive record or relying on break-glass. A failed write leaves no event. The agent comes from a request extension, recorded as unauthenticated until #192. AuditEvent is read-only over REST (CapabilityStatement and 405s). Found and fixed: AuditEvents referencing a record made it undeletable under reference integrity, so `referrers` now skips them.
- 2026-10-31: #167 the Inbound Dead Letter and quarantine. `health-integration`'s `Inbound` pipeline refuses tier 1 failures with an OperationOutcome and keeps them verbatim as dead letters. It quarantines tier 2–3 failures and writes the store refuses, and writes the rest through a `Sink`. Reviewers list, replay (re-validated from tier 1, then written) and discard entries. Each integration sets its retention (90 days by default), and `purge` enforces it. Every access is audited in the same transaction. The `inbound` table is migration `0005_inbound`, owned by the integration module. Tests use the real FhirServer as the sink. The sync-ingest `rejected` status stays with #81.
- 2026-10-31: #68 the client registry's golden records. `health-client-registry` (domain) has trust survivorship (a lower trust never overwrites; a disagreement becomes a review item), the golden record with link, unlink and adjudication markers, same-facility merge planning that needs an approval, and the ECID (11 Crockford base32 characters plus a mod-37 check). `health-registries-app` maps FHIR Patients and runs link (creating a golden Patient with an ECID when needed, `refer`/`seealso`, dropping `provisional`), unlink (values recomputed, never re-linked) and `$merge` (`replaced-by`/`replaces`, `active: false`, references never rewritten). Each runs in one transaction with If-Match and an `AuditTrail` port. Repository tests on SurrealDB. Split out #196 (Provenance storage, which needs the discriminator decision) and #197 (the HTTP operations).
- 2026-10-31: #69 the matching engine. `health_client_registry::matching` has the Matching Rules with platform defaults, the deterministic layer (conflicting verified ids are a hard non-match), the five ADR 0055 blocking keys with year spreading, the stop-list plan with the insufficient-data outcome, and Fellegi–Sunter with Jaro–Winkler and Damerau–Levenshtein, term frequency that only lowers weights, null handling and match grades. Double Metaphone comes from `rphonetic` behind a port; `deny.toml` now allows 0BSD (enum-iterator), as decided with the user, within ADR 0045's BSD family. The CI corruption benchmark on 60,000 synthetic people: blocking recall 99.85%, candidates at most 97, auto-link precision 100% (863 true, 0 false), 91.1% found at auto-link or review. The first run found household siblings falsely auto-linked, so phones now weigh less, different NINs count against a match, and estimated-age agreement counts half. Split out #199 (blocking storage and `$match`) and #200 (the 45M load test, needs hardware).
- 2026-10-31: #199 blocking storage and `$match`. `health-persistence` gains the `DerivedKeys` and `BlockingIndex` ports. `health-surrealdb` adds migration `0006_blocking_index` (`idx_block`, `block_count`); `with_blocking` writes blocking rows in each version's transaction, replaced and removed like search entries; `members` and `counts` read them, and `recount_blocks` is the background count. `health-registries-app` adds `RegistryBlocking` (keys of active source Patients only), `Matcher::find` (stop-list by stored counts, members, scoring) and `bundle` (searchset with probability score and `match-grade`, or the `incomplete` OperationOutcome). Over SurrealDB with 2,000 synthetic patients, 50 queries return exactly the in-memory pipeline's candidates. The route waits for #188 and #197, and the rules for configuration packages.
- 2026-10-31: #70 the registry review queue. The domain has `ReviewEntry` (potential match or survivorship), decisions that must fit their item and are made once, the 7-day ageing rule, and the `ReviewQueue` port, which SurrealDB implements (migration `0007_registry_review`, owned by the client registry). `RegistryReview::register` auto-links certain and probable matches, queues possible ones and creates new identities, skipping golden records that carry an adjudication marker for the source, so a separated pair is never re-linked. `decide` carries out same person, different people (an adjudication marker, then a new identity), keep current and take incoming in one audited transaction. `ClientRegistry` gained in-transaction `link_in`, `unlink_in`, `separate_in` and `override_in`. Split out #203 (the review screen, blocked by #197).
- 2026-10-31: #61 encrypted logical backup. `health-surrealdb` gains `Connection::export_sql` (the SDK's snapshot-consistent export, on embedded engines) and `import_sql` (run in memory). `health-backup` has `create`: the export plus only-new blobs, each encrypted with XChaCha20-Poly1305 under the node key with role-binding associated data, a checksum manifest, and verification before it returns. It also has `verify`, and `restore`, which verifies first, then imports in memory and returns the blobs. The round-trip fixture restores every table row for row (decimals keep their text) and needs no migration afterwards; the schema has no `set<record>`; corruption, a wrong key, a swapped blob and a tampered manifest are detected and nothing is restored. Phase 2 is complete. Scheduling and retention stay with #78, and central's HTTP export and key escrow with #138.
- 2026-10-31: #187 the authorization server's state in SurrealDB. Migration `0008_auth` (owned by identity) holds accounts with lockout, hashed codes, sessions, retired refresh hashes, jtis, snapshots and the clock floor. `SurrealTx` has methods for them, and `health_auth::SurrealAuthStore` uses one transaction per operation. A code is taken with `DELETE … RETURN`, so a race redeems it once; refresh rotation is a compare-and-set; `purge` removes expired rows. Tests: the flow survives a restart mid-session; four concurrent redemptions issue tokens once in each of five rounds; concurrent refreshes rotate at most once.
- 2026-10-31: #188 sign-in at central, in two PRs. In #207, central-server mounts `health-auth` at `/auth`; it answers 503 until the database is migrated and runs over `SurrealAuthStore`, with signing keys from `HEALTH_AUTH_KEYS_FILE` (active key first; rotated-out keys still verify their tokens and stay in `/jwks`). The second PR adds `login_page`, which sends `/authorize` to the client's localized `/sign-in` page with refusals back as `error=refused`; `/api/session` as the first authorized route; and the client's PKCE sign-in (`platform/sign-in.ts`): state and issuer checks, in-memory tokens, refresh before expiry, and sign-out through `/revoke`. The OAuth transaction is the only new storage, allowed by `check-boundaries`, with no clinical data.
- 2026-10-31: #188 merged (#208). #192 adds the PDP on every FHIR interaction: `FhirServer::with_access_control` requires the request's `Subject` (401 without one, except `/metadata`). Searches run under a per-request `SubjectSearchPolicy` (403 when the type may not be searched); reads, vreads and history are decided per record (a denial is 404); writes are decided on the new resource and the version replaced (403). `tests/authorization.rs` runs the matrix through HTTP. Split out: #209 (resolving the subject at central and mounting `/fhir`), #210 (break-glass over the API) and #211 (a record's facility, and facility isolation in searches).
- 2026-10-31: #192 merged (#212). #209 part 1: ADR 0056. A central session acts for one facility, chosen at sign-in. Another facility's records open at central only to patient-confined reads, never to searches across patients or to writes. Purpose of use is an `X-Purpose-Of-Use` header that defaults to `TREAT`. Code: `NodeScope::Central` in the PDP, `purpose_of_use`, and `health_identity_app::subject_of`. Still to come: the facility at sign-in, the request layer and mounting `/fhir`.
- 2026-10-31: #209 part 1 merged (#213; ADR 0056 merged with the maintainer's explicit approval over the two-review ADR gate). Part 2: the facility is chosen at sign-in. Central's `SnapshotGate` has `facility: None`, and the user names one of their snapshots' facilities, or none if they have only one; several with none named comes back as `error=facility`. The code, session and token carry it, and `Principal` exposes `facility` and `snapshot_version`. The client's sign-in page has an optional facility code field.
- 2026-10-31: #209 done in three PRs (#213, #214, and this one). `health_auth::Subjects` and `subject_layer` build each request's subject and audit agent. Central-server reads `HEALTH_AUTH_SNAPSHOT_KEYS_FILE` and `HEALTH_ACCESS_POLICY_FILE` and, with both, mounts `/fhir` (central scope, audited). A request without a token is 401, except `/fhir/metadata`. The OpenAPI document gains `GET /api/session`, and `@health/api-client` is regenerated.
- 2026-10-31: #209 merged (#215). #197: `FhirServer::with_client_registry` serves `Patient/$merge` (preview, then the reviewer's approval; the reviewer's own facility only; different facilities are 422), `$link` (with survivorship review queued in the same transaction) and `$unlink`. They are for subjects whose roles map to `registry.review`. Each version is audited naming the reviewer. Central serves them when `HEALTH_REGISTRY_NIN_SYSTEM` is set. Split out: #216 (`Patient/$match` over HTTP).
- 2026-10-31: #197 merged (#217). #210: break-glass over the API. `POST /api/break-glass` grants one patient for one hour (403 without the permission, 400 without a reason or justification, 409 while one is active). It is stored for the session in `break_glass` (migration `0009`) with its `BTG` AuditEvent, in one transaction. The session's subject carries the grant, and its agent is flagged, so every access is audited with `BTG`. The in-charge (`break-glass.alerts`) lists and acknowledges alerts, and the review is due in 72 h. Split out: #218 (pushing alerts, and the privacy officer's review).
- 2026-10-31: #210 merged (#219). #216: `POST /Patient/$match` (`resource`, `onlyCertainMatches`, `count`) through `FhirServer::with_client_registry(config, rules)`, for anyone who may search Patients. Candidates the subject may not see are left out before `count` applies, and results are audited like a search. With the registry on, central's store writes blocking keys and recounts blocks hourly.
- 2026-10-31: #216 merged (#220). #211, ADR 0057: facility-scope records carry `meta.tag` `urn:health:facility|{code}`, on a fixed table of types taken from the authority matrix. The server stamps it on create, refuses another facility's tag (403), and keeps it through updates. Searches of facility-scope types add the `_tag` clause, except central's patient-confined searches and break-glass. Central registers `_tag` for every facility-scope type and decides with `FacilityTag`.
- 2026-10-31: #211 merged (#221; ADR 0057 merged by the maintainer over the two-review gate). #189: SecLists' `xato-net-10-million-passwords-1000000` (MIT, pinned commit), chosen with the maintainer, ships in `health-auth` as a 1.8 MB Bloom filter with 999,996 entries and 0.1% false positives. `Credentials::set_password` applies `PasswordPolicy` with it, so a listed password is refused when set, offline as well as online.
- 2026-10-31: #189 merged (#222). #218 part 1:
  - `health_http::Events`, an invalidation-hint hub with audiences, and `/api/events` at central, filtered to the caller's facility and permissions.
  - Grants push a hint to the in-charge.
  - The privacy officer's review queue (`/api/break-glass/reviews`, migration `0010`): decisions are audited in the same transaction and made once, and overdue reviews escalate hourly.
  - Still to do: the screens.
- 2026-10-31: #218 part 1 merged (#223). Part 2: the break-glass screens. `/break-glass/alerts` is the in-charge's list with Acknowledge, and `/break-glass/reviews` is the privacy officer's queue with the decision form. They are DataTables keyed by the worklists `/api/events` hints, so they refresh without reloading. The edge review clock needs sync and is #224.
- 2026-10-31: #218 merged (#223, #225). #226: reception's `POST /Patient/$register`. Until the clerk confirms a new patient (`confirmedNew`), it answers with the candidates the subject may see and writes nothing. Then it mints the facility MRN `{facility}-{node}-{sequence}` with `next_sequence` and a guard key in the write's transaction (ADR 0014), creates the provisional source Patient, and runs registration matching. Central enables it with the registry; the node code is `HEALTH_NODE_CODE`.
- 2026-10-31: #226 merged (#227). #71: the reception registration screen (`/registration`).
  - **Flow:** the form is checked through `Patient/$register`, and candidates are chosen by keyboard in a DataTable. A new patient is registered only after "none of these". When there is too little to match on, the clerk searches by hand. Registration shows the MRN.
  - **Port:** the `registries` command port.
  - **Fix:** `runtime.json` `fhirBase`. Central serves FHIR at `/fhir`, but the client was calling `/api/fhir`.
  - **Follow-up:** the e2e harness and scenario are #228.
- 2026-10-31: #71 merged (#229). #203: the registry review queue.
  - **Server:** `/api/registry/reviews` (`registry_review_router`, for `registry.review`) lists pending items with both Patients or both claims, and takes audited decisions.
  - **Client:** `/registry/reviews`, a keyboard DataTable of the queue. Possible duplicates are compared side by side, field by field; disagreeing values are shown with trust and source.
  - **Deferred:** unlinking from the golden record view comes with the patient record screens, and axe and e2e come with #228.
- 2026-10-31: #203 merged (#230). #228: the Playwright e2e harness (`@playwright/test` 1.64.1, Apache-2.0; `@axe-core/playwright` 4.14.0, MPL-2.0) and nightly `e2e.yml`.
  - **Backend:** `e2e/backend.ts` runs SurrealDB in memory and central-server, with users from the new `central-server provision`.
  - **Client:** `vite preview` proxies to it (`HEALTH_DEV_BACKEND`).
  - **Scenarios:** the clerk's keyboard registration (candidates chosen with arrows and Enter, a duplicate only after "none of these"), and axe on the registration and registry review screens.
  - **Fix:** axe found the status area's `header` carrying `role=status`, which a `header` may not take.
- 2026-10-31: #228 merged (#231). #95: `health-workflow`, the pure statechart interpreter. It follows SCXML Appendix D without history:
  - selection by document order, with conflict removal;
  - exit, transition content, then entry order;
  - eventless transitions before internal events;
  - `done.state` for compound and parallel states;
  - invocations after the macrostep;
  - timers and typed actions returned as effects, with idempotency keys.

  `tests/scxml.rs` covers 30 W3C SCXML §3 assertions. Writing them found that initial targets inside a parallel must have their descendants added before any ancestors.
- 2026-10-31: #95 merged (#232). #119: `health-financial-ledger`, the facility's double-entry ledger.
  - **Money:** `rust_decimal` at the ISO 4217 scale, rounded half-even, with largest-remainder splits that always sum exactly.
  - **Chart of accounts:** built from the country template.
  - **Postings:** balanced and append-only, in the functional currency, with dimensions on every line and foreign amounts kept with their rate. Corrections are reversals, made once.
  - **Balances:** derived from postings, including what each funding source owes per encounter (`Account.balance[]`).
- 2026-10-31: #119 merged (#233). #127: the stock ledger and balance guards.
  - **Domain (`health-inventory`):** Stock Units, movements, reversals, the negative-stock policy (blocked at the authority, emergency dispense with permission and reason, late operations flagged, store tightening and relaxing) and the stock card ordered by occurrence.
  - **Use cases (`health-inventory-app`):** each movement reads and writes its unit's balance record in one transaction; a write conflict reruns it. Six concurrent issues of the last pack: exactly one succeeds. Status changes move stock between units in one transaction; the nightly check rebuilds balances and reports drift.
  - **SurrealDB:** migration `0011_stock`.
- 2026-11-01: #127 merged (#234). #111 part 1, the domain of patients' phone accounts and self-registration. Patients sign in with a one-time code every time, with no password; the SMS provider is #235, which needs a person.
  - **`health-identity`:** `PhoneAccount`, E.164 `PhoneNumber`, and `OtpPolicy`/`OtpChallenge` (6 digits, 5 minutes, 5 tries, 1 a minute and 5 an hour per phone).
  - **`health-client-registry::person`:** `Person` links at `level1` or `level3`. Linking an existing Patient needs `level3`; below it, only self-submitted data is visible.
  - **`health-patient-engagement`** (new): the §59 Registration Request. A certain match below `level3` goes to review, never to a link.
- 2026-11-01: #111 part 1 merged (#236). Part 2a: patients sign in by phone in `health-auth`.
  - **Endpoints:** `POST /otp`, plus phone and code on `/authorize`, for patient clients. The first accepted code creates the phone's account. Sessions need no snapshot, and tokens carry `amr: ["otp"]`.
  - **Rules:** one account per phone, even under races. Each try is a compare-and-set, so 8 concurrent tries count once. Codes are sent through `OtpSender`, to `DevOutbox` until #235 picks a provider.
  - **SurrealDB:** migration `0012_phone_sign_in`.
- 2026-11-01: #111 part 2a merged (#237). Part 2b: self-registration.
  - **Use case:** `health-patient-engagement-app` routes a patient's QuestionnaireResponse after `$match`. No candidate gives a new self-registered Patient (provisional, patient-reported, no MRN, never auto-linked), with the Person linked at `level1`. Any candidate goes to review with nothing linked.
  - **Storage:** the Task, QuestionnaireResponse and Person, with ids derived from the account.
  - **Server:** `ClientRegistry::create_self_registered`. `health-fhir-server::portal_router` serves `/registration` behind the new `health_auth::patient_layer` (`PatientSubject`).
- 2026-11-01: #111 part 2b merged (#238). Part 2c: staff review of portal registrations.
  - **Queue:** a request that resembles existing Patients is a `registration-request` item in the registry review queue (migration `0013_registration_review`). It is opened and closed in the self-registration use case's own transaction.
  - **Decisions:** a reviewer links the account to a candidate only with the ID document checked in person, which gives `level3`; otherwise they create a new Patient. The review screen shows the patient's answers against each candidate.
  - **Fix:** a single certain candidate no longer hides the other candidates from review.
- 2026-11-01: #111 part 2c merged (#239). Part 3 completes #111.
  - **Central:** the portal's patient client, phone sign-in through `HEALTH_DEV_OTP_OUTBOX` (development only, until #235), and `/api/portal` behind `patient_layer`.
  - **Client:** the portal's own sign-in, session and transport. `/sign-in` takes the phone and code steps, and `/portal` has the registration form and status.
  - **e2e:** a new patient registers; a lookalike is linked by a reviewer after an in-person ID check.
- 2026-11-01: #111 merged (#240). #113 part 1: `health-consent`, the caregiver delegation rules.
  - **Grants:** scoped, expiring, verified grants; mother–newborn grants at delivery.
  - **Ages:** guardian access ends at the age of majority, and a guardian never sees an adolescent's sensitive records without their consent. Uncertain ages are reckoned protectively.
  - **Revocation:** who may revoke a grant.
  - **Tests:** an authorization matrix.
- 2026-11-01: #113 part 1 merged (#241). Part 2: storing and managing caregiver grants.
  - **Staff:** they record a caregiver by their portal phone, verified against the document checked, and can revoke a grant or record an adolescent's consent (`/api/caregivers`, `caregiver.manage`).
  - **Storage:** the grant and its RelatedPerson and Consent are written in one transaction and audited (migration `0014_caregiver_grant`).
  - **Portal:** caregivers see their dependents at `/api/portal/dependents`.
- 2026-11-01: #113 part 2 merged (#242). Part 3 completes #113.
  - **Staff:** a minimal patient view (`/patients/{id}`) with its Caregivers panel, linked from reception: record by phone, revoke, and the young person's consent.
  - **Portal:** caregivers see the people they care for.
  - **e2e:** a guardian's view of an adolescent, with and without her consent, and revoked.
  - **Follow-ups:** #243 (the mother–newborn link at delivery) and #244 (`decide` on portal record reads).
- 2026-11-01: #113 merged (#245). #129 part 1: two-step transfers and Reconciliation Records.
  - **Transfers:** dispatch, receipt with counted good and damaged stock (damaged into quarantine), and the direct transfer within a facility. Each step is one transaction, and a dispatch beyond what is available moves nothing.
  - **Reconciliation Records:** short, damaged, excess and negative balances each open one, which closes only with a root cause (GSDP §13.2). A transfer closes when its last record is resolved.
  - **SurrealDB:** migration `0015_stock_transfers`.
- 2026-11-01: #129 part 1 merged (#246). Part 2 completes #129.
  - **Counts:** full, cycle and blind, without freeze; variances are counted minus (the balance at the start plus the movements since).
  - **Approval:** a reason for every variance, and a supervisor above the threshold. Each posted variance raises a Reconciliation Record, and a settled negative balance needs its root cause.
  - **Export:** the InventoryReport.
  - **Follow-up:** #247, central matching of inter-facility transfers, which needs sync.
- 2026-11-01: #129 merged (#248). #130 part 1: receiving, requisitions and reorder.
  - **Receiving:** a goods received note writes a receipt of each line with its unit cost, in one transaction, once.
  - **GS1:** DataMatrix parsing.
  - **Requisitions:** ward requisitions (SupplyRequest) are approved, then fulfilled by a dispatched transfer.
  - **Reorder:** min/max suggestions from average monthly consumption.
  - **SurrealDB:** migration `0017_receiving`.
- 2026-11-01: #130 part 1 merged (#249). Part 2 completes #130.
  - **API:** `/api/stock` for staff with `stock.manage`: receiving, stock on hand with costs, reorder, GS1 and requisitions.
  - **Screens:** Stock (`/stock`) and Receive goods (`/stock/receive`), with the barcode filling the batch and expiry.
  - **e2e:** a receipt on hand at its cost.
  - **Fix:** the full e2e run exposed a race. Concurrent requests rewrote the shared clock floor, and the loser of the write conflict was refused with a spurious 401. Now the floor is written only when it advances, and a lost raise refuses nothing.
  - **Follow-up:** #250, the ward requisition screens.
- 2026-11-01: #130 merged (#251). #131: stock valuation and inventory-accounting postings.
  - **Valuation:** weighted average or FIFO, rebuilt from the movement ledger; consignment carries no value, and donor stock is valued at its declared price.
  - **Postings:** one balanced posting per costed movement when inventory accounting is enabled; the inventory account equals the valuation. The financial ledger has no storage yet, so the postings are not persisted.
- 2026-11-01: #131 merged (#252). #250: the ward requisition screens.
  - **API:** listing a store's requisitions, and fulfilment.
  - **Screen:** Requisitions (`/stock/requisitions`): ask, approve or refuse, pick a batch, and send.
  - **e2e:** the stock leaves the pharmacy as a transfer to the ward.
  - **Fix:** the lossless JSON numbers crashed a React render; they are now shown as strings.
- 2026-11-01: #250 merged (#253). #123: tills and two-phase payments.
  - **Rules (`health-payments`):** cash within a shift with supervised settlement; non-cash payments pending until provider evidence (receipts provisional, never confirmed before it); customer-reported payments confirmed only by a statement line; refunds behind the permission; statement reconciliation with an exceptions queue.
  - **Postings (`health-payments-app`):** the ledger postings, with pending money in payments in transit.
  - **Follow-up:** #254, storage, the API and the till screens.
- 2026-11-01: #123 merged (#255). #254 part 1: payments, shifts and the ledger's postings stored.
  - **One transaction per step:** the payment, its posting checked against the chart, and its shift.
  - **Integrity:** a provider reference is claimed once, a till has one open shift, and receipts are numbered.
  - **SurrealDB:** migration `0018_payments`.
- 2026-11-01: #254 part 1 merged (#256). Part 2 completes #254.
  - **API:** `/api/payments`, with cashier, supervisor and refund permissions.
  - **Screens:** the Till (`/till`), where a non-cash receipt is provisional until confirmed, and the supervisor's page (`/till/supervise`), which takes statement lines and approves variances.
  - **e2e:** the provisional receipt is confirmed only by the statement.
- 2026-11-01: #196 Provenance with its target's Owner. One `provenance` table with an `owner` column, set by the store from the target's head in the same transaction (migration `0019_provenance`); a missing or unknown target is refused. The client registry writes one Provenance per changed surviving value with each golden write, naming the field, the source Patient and its trust.
- 2026-11-01: #72 part 1, the facility-edge service (Phase 4 starts ahead of #34, which now gates deployment).
  - **Runtime:** `health-edge-runtime`. The listener starts first. The embedded store opens once, and migrations run before ready, extending the service manager's timeout. A store a newer binary migrated is refused. Supervised tasks restart with backoff, and a crash loop stops the node. A NUT UPS low battery stops it gracefully.
  - **Binary:** `facility-edge`, in foreground and `--service` (systemd `Type=notify`, with a unit file). Distinct exit codes tell the service manager and the update rollback what failed.
  - **Tests:** the built binary: ready and SIGTERM, recovery after SIGKILL, the store's single owner, the systemd notifications, the UPS stop, and an old binary refusing a newer store.
- 2026-11-01: #72 part 1 merged (#259). Part 2a, signed updates with rollback.
  - **Packages:** a gzipped tar of a manifest and the binary, with a detached ECDSA P-256 signature checked against the pinned release key before anything is unpacked.
  - **Sequence:** stage, then apply with the service stopped. The new binary's first start copies the store, and ready commits the update.
  - **Rollback:** after a failed migration, or three starts that never reach ready, the store is restored first, then the binary. Under systemd a root `OnFailure=` unit restores the binary, since the sandboxed service cannot.
  - **Tests:** the installed binary updated, rolled back after a failed migration, and rolled back by the previous binary after a crashing release.
- 2026-11-01: #72 part 2a merged (#260). Part 2b, the Windows service, completes #72.
  - **Runtime:** `WindowsScm` reports `StartPending` and `StopPending` with wait hints and checkpoints, then `Running`, then `Stopped` with the exit code as a service-specific error. `facility-edge --service` on Windows runs under the SCM dispatcher.
  - **Packaging:** `install-service.ps1` registers the service, its environment and the failure actions.
  - **CI:** `edge-build.yml` runs the release binary as a real Windows service and under systemd with both units, with the edge crates' clippy and tests on Windows.
- 2026-11-01: #72 merged (#261). #76, the web app served from the edge.
  - **Edge:** `health-edge-runtime`'s build script embeds `apps/client/dist`, without source maps. The edge serves the page for client routes, the hashed assets (immutable), `runtime.json` (edge mode and the embedded build) and `sw.js`. Unknown API paths are 404s.
  - **Client:** a build ID in the bundle and `build.json`. A shell of another build than its edge's reloads once. In edge mode the client registers the service worker, which caches only the shell's fixed paths when it installs, and goes to the edge first for pages and `runtime.json`.
  - **Tests:** Rust unit tests of the routes, Vitest for the version check, and a Playwright edge suite (nightly in `e2e.yml`). The app loads with every non-edge host refused, and loads from the cached shell after the edge stops, with no API response cached. `edge-build.yml`'s Ubuntu job embeds the app and checks it is served.
- 2026-11-01: #76 merged (#262). #78, scheduled edge backups and self-test restores.
  - **Edge:** `health_edge_runtime::backup` runs as a supervised task once the node is ready. It takes an hourly `health-backup` backup into `HEALTH_BACKUP_DIR`, verified after writing. It keeps the 48 newest and the newest of each of the last 14 days, prunes unreferenced blobs, and test-restores the newest backup weekly into a scratch store. Each outcome goes to `status.json`, and failures are logged as alerts without stopping the node. A node without backups, or with backups on its data volume, warns at start.
  - **Binary:** `facility-edge backup now | copy <dir> | verify | test | restore <backup> | status`, with exit code 9 for a failed command. The pre-update backup stays the update's cold store copy.
  - **Tests:** unit tests for retention, scheduling, a failed test restore recorded for the alert, the USB copy and restore. `tests/backup.rs` runs the binary end to end, through to a restored store that starts ready.
- 2026-11-02: #78 merged (#263). #77, edge health monitoring and the admin page.
  - **Edge:** `health_edge_runtime::admin` measures the node: identity, version, store size, free space on the data and backup volumes, the backup schedule, the UPS's last poll, task restarts and the telemetry buffer. It raises alerts against the facility-edge.md thresholds. `GET /api/admin/status` serves the report to the node itself until the local authorization server can require an admin (#75).
  - **Telemetry:** the process's log lines go to a bounded on-disk buffer (64 MiB, oldest dropped first) as well as stdout. Every interval an `indicators` line and each raised or cleared alert are logged. The buffer is shipped as OTLP/HTTP log records when the collector answers. `alert` and `state` join the allowlist; the redaction test covers them.
  - **Client:** `/edge/status`, linked from the home page on an edge (the `edgeStatus` capability), with the alerts first and the indicators not reported yet listed.
  - **Tests:** unit tests for each threshold, the buffer's bound and restarts, the OTLP encoding, loopback-only access, and shipping after an outage. `tests/health.rs` runs the binary against a collector that is down, then up. A Playwright edge spec opens the page from the real binary with axe.
- 2026-11-02: #77 merged (#264). #67 split: ADR 0058 (#266) adds the online facility issuing CA under the offline root. `health-pki` issues the root, the issuing CA, facility intermediates, edge leaves and publication signing, each from a request, with exact name constraints, EKUs and path lengths, refusing anything a verifier would reject. It also publishes numbered CRLs and verifies chains with rustls-webpki. `tools/pki` runs the ceremonies, with a draft RUNBOOK.md. 10 security tests show a stolen facility or issuing-CA key cannot mint other names, public names, public IPs or sub-CAs, and that revocation reaches verifiers. The ceremony and custody are #265, which needs people.
- 2026-11-02: #67 merged (#267). #73 split: part 1, the full-disk encryption check. `health-node-identity` (runtime, composed in `facility-edge`) probes Linux (findmnt, lsblk, the LUKS2 `systemd-tpm2` token sealed to PCR 7, sysfs) and Windows (BitLocker, Win32_Tpm, Secure Boot through PowerShell), failing closed. `facility-edge encryption check` is the provisioning gate, exit 10 unless compliant or exempted. The systemd unit runs the root-only probe as `ExecStartPre=+`, and the status page shows the state with its alerts. Part 2 is node keys and the facility intermediate; the central side is #268.
- 2026-11-02: #73 part 1 merged (#269). Part 2: `health_node_identity::keystore` seals secrets with systemd-creds (`--with-key=auto`, unsealed for the service by `LoadCredentialEncrypted=`) or DPAPI in machine scope through PowerShell (SYSTEM and Administrators only), with no `unsafe` code. `facility-edge provision request | install | status` gates on encryption, seals the facility intermediate's key, and installs a chain only if a leaf for `<node-id>.local` verifies against the root. The edge-build systemd job provisions end to end. #73 is complete; the central side is #268 (Phase 4 now has 8 tasks).
- 2026-11-02: #73 part 2 merged (#270), completing #73. #74 split in three. Part 1: `health-identity` enrollment rules (6-digit pairing codes, 10-minute requests, versioned final revocation); migration `0020_devices`; `health_auth::DeviceGate` with `POST`/`GET /api/enrollment`, the registry's admin routes (served to the edge's console only until #75) and a gate that requires a fresh, unreplayed DPoP proof by an enrolled, active device on every other edge API route. Enrollment and revocation commit with their AuditEvents. Part 2 is TLS, the pairing QR and mDNS; part 3 the client.
- 2026-11-02: #74 part 1 merged (#271). Part 2 split into 2a (TLS) and 2b (pairing and mDNS). Part 2a: the edge issues its own ECDSA leaf under the installed facility intermediate (unsealed from the keystore only to issue) for `<node-id>.local`, `HEALTH_DNS_NAME` and its private addresses, keeps it with a fresh key in `HEALTH_TLS_DIR`, and serves HTTPS only with rustls. A supervised task renews it at two-thirds of its life or on an address change, swapping it with no restart. Unprovisioned or unservable, the node listens with plain HTTP on the loopback address only. The status page shows the certificate and its alerts; `HEALTH_BIND` now defaults to `0.0.0.0:443`, and the unit grants `CAP_NET_BIND_SERVICE`. The edge-build systemd job checks HTTPS against the root, and that plain HTTP no longer answers.
- 2026-11-02: #74 part 2a merged (#272), with a plain-HTTP console on the loopback address for the edge's own screen. Part 2b: `GET /api/admin/pairing` (console only) gives the node's https origin, node ID and facility intermediate's SPKI fingerprint (`health_pki::inspect`), as a `health-edge://pair` URI that the status page draws as a QR code (`uqr`, SVG path). While serving HTTPS the node announces `_health-edge._tcp` over mDNS (`mdns-sd`; TXT `node`, `v=1`) at its leaf's addresses, re-announcing on change; `HEALTH_MDNS=off` stops it. The edge-build systemd job checks the payload's pin against openssl's.
- 2026-11-02: CI speed. Rust caches never survived: each PR saved its whole `target` (up to 8 GB), evicting `main`'s from the repository's 10 GB, so jobs compiled from cold (the Windows edge job took 70 minutes). Now every run restores and only `main` saves, after `tools/github/prune-target.sh` leaves just the dependencies; Cargo jobs use line-table debug info; and `edge-build` runs the debug binary under systemd on PRs, with the release build on `main` and in the merge queue. Windows keeps the release binary: the debug one does not start as a service.
- 2026-11-02: #74 part 2b merged (#273). Part 3, the browser client: `src/platform/device.ts` keeps a non-extractable ECDSA P-256 key pair in IndexedDB (allowlisted in `check-boundaries.ts`), and `dpopAuth` proves every edge request with it, before sign-in too. `/enroll` makes the key, asks, shows the pairing code and polls until approved, then confirms through the gate; a revoked device starts over. The edge status page's Devices section lists pending requests without their codes, approves by the typed code with a name and location, and revokes. The edge e2e harness now uses the console listener, and a new scenario shows an unknown device refused, an approved one let in and a revoked one shut out. This completes #74; Tauri keyring keys and SPKI pinning come with the desktop and Android builds.
- 2026-11-03: #75 split in three; part 1 in three PRs. ADR 0059 (#276): hashes are plain Argon2id, encrypted at rest under each node's pepper key, and encrypted to the edge in a signed provisioning bundle. Part 1a (#277): `Credentials` seals and opens hashes under the pepper key, bound to the account. Part 1b: `health_auth::Bundle`, signed with the snapshot key, its hashes encrypted to the edge's facility intermediate (ECDH P-256, HKDF-SHA-256, XChaCha20-Poly1305), and `central-server provision bundle`. Part 1c imports it on the edge and mounts the authorization server.
- 2026-11-03: #75 parts 1a (#277), 1b (#279) and 1c-i (#280) merged with ADR 0059 (#276). Part 1c-ii: the edge runs the authorization server (`health-edge-runtime`'s `auth`): `/auth` with DPoP tokens only for enrolled devices' keys, snapshots for `HEALTH_FACILITY` verified with central's keys (`HEALTH_SNAPSHOT_KEYS_FILE`, from `central-server provision snapshot-keys`), `/api/session` behind the device gate, the bundle import on the console, and `runtime.json`'s `auth`. `provision request` seals the pepper and token-signing keys, which the unit unseals. A binary test signs a provisioned member of staff in offline on an enrolled device, and refuses one whose snapshot expired.
- 2026-11-03: #75 part 1c-ii merged (#281). Part 1c-iii, the browser on the edge: `signIn` is composed on an edge too, and its `/token` and `/revoke` requests carry a DPoP proof by the device's key (`dpopProof`, shared with `dpopAuth`). The edge's issuer is the first origin in `HEALTH_ORIGINS`. An `edge-fixture` example plays central for the e2e suite, whose new scenario imports a bundle on the edge's screen, refuses the token to an unenrolled device with the right password, and signs staff in on an enrolled one. This completes #75 part 1.
- 2026-11-03: #75 part 1c-iii merged (#282), completing part 1. Part 2 split in two; 2a, the server: sessions record their last activity, and on an edge (`AuthConfig::idle_lock`) a refresh after 15 minutes idle gets `session_locked`; `POST /auth/pin` sets a 6-digit PIN per user per device (sealed under the pepper key); `POST /auth/unlock` resumes or switches to a user's live session on the device by username and PIN with the device's DPoP proof, and five wrong PINs end it. One session per user per device. Migration `0021_pins`. Part 2b is the client.
- 2026-11-03: #75 part 2a merged (#283). Part 2b, the client: on an edge `signIn` gains `lock`, `unlock` and `setPin`, and a refresh refused with `session_locked` locks instead of signing out. `LockGuard` wraps every page, locks after 15 minutes without use (clearing the tokens and the query cache) and shows the lock screen, whose username and PIN resume or switch; the home page sets the device PIN. An e2e scenario sets a PIN, fast-forwards the page clock 16 minutes, sees the lock, is refused a wrong PIN and unlocks with the right one. This completes #75 part 2.
- 2026-11-03: #75 part 2b merged (#284), completing part 2. Part 3 split: 3a offline administration on the server (this), 3b provisional accounts, 3c the client screens. 3a: `health-identity` local grants (1 to 7 days, only grantable roles, never a role with a never-offline permission) and revocations; the bundle carries the role mappings and grantable list (`LocalConfig`, from central's access policy); `current_snapshot` refuses a locally revoked user and adds unexpired grants' roles; `LocalAdmin` serves `/api/access/grants` and `/api/access/revocations` to a signed-in `access.administer-local` admin, audited, with grants signed by the facility intermediate's key. Migration `0022_local_admin`.
- 2026-11-03: #75 part 3a merged (#285). Part 3b, provisional accounts: `POST /api/access/accounts` creates new staff's Login Account, `ProvisionalAccount` record and role grant together, audited; their first sign-in is answered `change_password` until `POST /auth/password` (open to every user, policy- and breach-checked) sets their own; without a central snapshot they sign in under the grant's roles alone, lapsing with it. Migration `0023_provisional`.
- 2026-11-03: #75 part 3b merged (#286). Part 3c, the client screens:
  - **`/access` page:** for a facility admin on an edge. It lists the local grants and revocations by username, grants a role chosen from the new `GET /api/access/roles` (grantable, with no never-offline permission) for 1 to 7 days, creates a provisional account, and revokes access after a confirming click.
  - **Sign-in page:** on `error=change_password`, it asks for the temporary password and a new one twice, posts them through `SignIn.changePassword` to `/auth/password`, then signs in with the new password.
  - **`edge-fixture`:** its staff member is now also a `facility-admin`, and `clinician` is grantable.
  - **e2e:** an admin creates an account; its user is refused a mismatched and a breached password, then sets their own; the admin revokes them, and they can no longer sign in.

  This completes #75.
- 2026-11-03: #75 part 3c merged (#287), completing #75. Decisions for what comes next:
  - **#86:** a package's `.sig` is a detached JWS (ES256, `x5c`, `iat`) over the tarball's sha-256.
  - **#268:** central's KMS starts as a key-wrapping port with a local adapter, and the HSM or KMS product comes later.

  #86 part 1, the package format, pinning and signing:
  - **`health-configuration`** (domain) holds the package model, exact semver versions, and the publication rules: R5 only, pinned dependencies, a manifest that pins every canonical, versioned artefacts, no duplicates, and the folder layout.
  - **`health-packages`** (infrastructure) does the rest:
    - deterministic tarballs that refuse hostile archives;
    - the generated `.index.json`;
    - `publish` with a `PublicationSigner` port and a local signer;
    - `verify`, which checks the expected sha-256, then the signature against the national root at the signing time with CRLs, then the archive, the index and the rules.

  Part 2 is JSON Schema validation of `configuration/`.
- 2026-11-03: #86 part 1 merged (#288). Part 2, configuration schemas:
  - **Schemas belong to the release:** `health-packages::Schemas` holds the schemas the software release reads, keyed by `$schema` URI, using `jsonschema` 0.60 with no remote resolution.
  - **Each file names its schema:** every `configuration/` file is JSON and names its schema.
  - **Checks:** `publish` and `verify` refuse a file with no schema, an unknown schema, or failing validation, reported by JSON pointer.
  - **The built-in set is empty for now.** Form Layouts, override points and settings bring their schemas with their features.

  This completes #86.
- 2026-11-03: #86 part 2 merged (#289), completing #86. #268, with its provisioning exchanged online over HTTPS by one-time code, is split in four:
  1. central's registry and endpoint;
  2. the edge's online `provision --code`;
  3. recovery-key escrow behind a KMS port with a local adapter;
  4. decommissioning.

  Part 1:
  - **`health-identity` node rules:** Crockford one-time codes, single use within 72 hours; encryption required unless the code carries an exemption; short codes `E1`, `E2`, … never reused.
  - **Migration `0024_nodes`.**
  - **`health-provisioning`:**
    - `NodeRegistry` issues codes, stored hashed and audited;
    - `redeem` mints the node and has the `Issuer` (the issuing CA from files for now) issue its intermediate, in one audited transaction;
    - `POST /api/provisioning/nodes`;
    - the exempt nodes as an hourly `encryption-exempt` alert.
  - **central-server:** `HEALTH_ISSUING_CA_CERT_FILE` and `HEALTH_ISSUING_CA_KEY_FILE`, and `central-server provision code`.
- 2026-11-03: #268 part 1 merged (#290). Part 2, the edge's online `facility-edge provision --code CODE --central URL --root FILE [--central-ca FILE]`:
  - **Steps:** the encryption gate, with central deciding an exemption; the key sealed, or reused from an unfinished attempt; the HTTPS call; the chain installed for the minted node ID; and `node.json`, adopted at start unless `HEALTH_NODE_ID` or `HEALTH_FACILITY` is set.
  - **The HTTPS client:** `hyper-rustls` with the ring provider and the OS trust store (`rustls-native-certs`, Apache-2.0/ISC/MIT).
    - reqwest's TLS features bring `webpki-root-certs` (CDLA-Permissive-2.0), which `cargo deny` refuses.
  - **Test:** the binary against `health-provisioning`'s router over TLS. It covers:
    - an untrusted central;
    - an unencrypted edge without an exemption;
    - a wrong code;
    - an exempt code provisioning `E1`;
    - a second provisioning, refused.
- 2026-11-04: #268 part 2 merged (#291). Decisions for part 3:
  - after provisioning, an edge authenticates its escrows with a JWS signed by its facility intermediate's key;
  - a recovery key is read out with a `central-server` command only, never a web endpoint.

  Part 3 is split, and 3a is central's half:
  - **Domain:** `RecoveryKey` with its retrievals and the escrow freshness rule, and the node's intermediate key, recorded at provisioning.
  - **Migration `0025_recovery_keys`.**
  - **`health-provisioning`:**
    - the `KeyWrap` port with `LocalKeyWrap` (XChaCha20-Poly1305, the node and volume as associated data);
    - `recovery_jws`, `escrow`, `read_recovery` and `used_recovery_keys`;
    - `POST /api/provisioning/recovery`.
  - **central-server:** `HEALTH_KMS_KEY_FILE`, `central-server recovery read | used`, and the hourly `recovery-key-used` alert.

  3b is the edge: creating recovery protectors, escrowing them, and `rotate-recovery`.
- 2026-11-04: #268 part 3a merged (#292). Decision for 3b: on Linux, cryptsetup prompts for a secret the volume already has (systemd 255 cannot unlock with the TPM), and the install passphrase is kept.

  Part 3b:
  - **`health-node-identity::encryption::recovery`:** BitLocker-format recovery passwords, and the `RecoveryProtectors` port over `Add`/`Remove-BitLockerKeyProtector` and `cryptsetup luksAddKey`/`luksKillSlot`. The new slot is found by diffing `luksDump`.
  - **facility-edge `recovery`:** add new keys, escrow them in one signed JWS (rolling back if central refuses), record their ids in `recovery.json`, then remove the keys escrowed before.
    - It runs as `encryption rotate-recovery` and at the end of `provision --code`.
    - `node.json` now records where central is.
  - **Tests:** unit tests of the formats and parsers, and of the flow with a fake.
  - **Not yet checked:** the platform commands, on facility hardware.
- 2026-11-04: #268 part 3b merged (#293). Part 4, decommissioning:
  - **Domain:** `Node::decommission`, recording the erasure (`tpm-cleared` or `key-slots-destroyed`), who and why, once.
  - **Migration `0026_node_revocations`:** the issuing CA's revocations and CRL number.
  - **`NodeRegistry::decommission`:** one audited transaction that revokes the intermediate and destroys the escrowed recovery keys. Afterwards the node is never active again.
  - **`NodeRegistry::crl`.**
  - **central-server:** `node decommission | crl`.

  This completes #268. The HSM or KMS adapters for the issuing CA and the escrow master key, and CRL delivery to edges over sync, remain.
- 2026-11-04: #268 part 4 merged (#294), completing #268. Phase 5 waits on the commit-counter spike #36, so Phase 6 starts with #88 before #87, whose package must publish through the lifecycle. #88 is split in three:
  1. the lifecycle and the package-level semver check, in the domain (this);
  2. the artefact-level bump rules: what a Questionnaire, CodeSystem, ValueSet or profile change requires;
  3. central's storage of lifecycles and their records, audited, with publication through `health-packages` and `central-server` commands.

  Part 1, `health-configuration::lifecycle`:
  - **`Lifecycle`:** Draft → Validation → Simulation → Review → Approval → Published, each transition returning a `Record`. Failures and requested changes return it to Draft, which clears the review and the approvals.
  - **Separation of duties:** revising makes you an author; no author reviews, approves or requests changes; clinical kinds need two distinct approvers and configuration kinds one.
  - **Content pinned:** only a draft changes, and publication must sign the approved fingerprint.
  - **`semver::check(latest, next)`:** the package bump follows its artefacts' own bumps, dependencies and files; a rewritten `url|version`, a downgrade, an inexact version or a version no newer is refused. Major zero and pre-releases promise nothing.
- 2026-11-04: #88 part 1 merged (#295). Part 2, the artefact-level bump rules:
  - **`health-packages::bump::changes(before, after)`:** every change between two versions of an artefact, with the bump it needs. Questionnaire items by `linkId`, CodeSystem concepts by `code`, ValueSet compose rules, and StructureDefinition elements by `id` follow metadata.md's table. Every other change is major.
  - **`semver::check(latest, next, content)`:** it refuses an artefact whose own new version understates those changes (`ArtefactUnderstated`).
  - **Decision:** the JSON diff sits in `health-packages`, not the domain, because `serde_json` is not on the domain allowlist. The domain keeps `ContentChange` and the decision.
- 2026-11-04: #88 part 2 merged (#296). Part 3 is split: 3a is central's storage and use cases (this), and 3b the `central-server` commands. Part 3a:
  - **`health-packages`:** publication in steps: `pack` (unsigned, no rules), `check`, `sign` and `unpack`. `publish` is now the three in one.
  - **Migration `0027_package_lifecycle`** (owned by configuration) has three tables:
    - `package_lifecycle`, changed only by compare-and-set on `revision`;
    - `package_lifecycle_record`, appended;
    - `published_package`, created once.
  - **New crate `health-publication`:** `Publications` with `draft`, `submit` (validation and simulation run at once), `review`, `approve`, `request_changes` and `publish`. Every transition is audited in its transaction.
    - Tarballs are blobs in `health-files`, and the draft's sha-256 is what is approved and signed.
    - Validation applies the rules, the schemas, and the package and artefact semver checks against the latest published version.
    - **Decision:** simulation runs through a `Sandbox` port; `NoSandbox` runs no case until #101.
- 2026-11-04: #88 part 3a merged (#297). Part 3b, `central-server package draft | submit | review | approve | request-changes | publish | status | list | export`:
  - **Settings:** `HEALTH_BLOB_DIR`, and for publishing `HEALTH_PUBLICATION_KEY_FILE` and `HEALTH_PUBLICATION_CHAIN_FILE`.
  - **Output:** refusals print their message.
  - **Run by hand** against SurrealDB with a test root and publication certificate from `tools/pki`:
    - an author is refused as reviewer;
    - publishing without the key is refused;
    - with one approval, a configuration package publishes and exports its tarball and signature;
    - re-drafting it is refused;
    - a 1.0.1 that removes a code under a patch bump of its CodeSystem returns to Draft, naming `concept[clerk] (removed, major)`.

  This completes #88.
- 2026-11-04: #88 part 3b merged (#298), completing #88. #87, the platform core package:
  - **Decisions (by the user):**
    - the platform's canonical base is `https://carapai.github.io/people-centered-healthcare/fhir`;
    - profiles are written in FSH, compiled by SUSHI, with snapshots from the HL7 validator in tooling.
  - **Split:** part 1 is the ADR 0049 profiles, their extensions, the platform CodeSystem and the toolchain (this); part 2 is the locally pinned financial and inventory profiles.
  - **Part 1:**
    - **`packages/fhir-core-package`** (`health.platform.core@0.1.0`, national core): `ExtractionMappingLibrary`, `MatchingRulesLibrary`, `WorkflowDefinitionLibrary`, `ProgramPlanDefinition`, five extensions, `CodeSystem/platform` and `ValueSet/template-modes`, with examples.
    - **`tools/fhir/build-core-package.sh`** builds it. CI rebuilds it in the Java job and fails on drift.
    - **Tier 3 fixes** found by the package:
      - an extension slice may name its extension by its type's profile, as HL7 snapshots do;
      - a contained resource is typed by its own `resourceType`, so its extensions are checked.
    - **Tests:** the package compiles and enforces its constraints, and publishes through the lifecycle with two approvers.
- 2026-11-04: #87 part 1 merged (#299). The licence exception for `hl7.fhir.r5.core` (CC0-1.0, that package only) was approved by the user. Part 2, the locally pinned profiles:
  - **15 `Platform<Resource>` profiles** in the core package, each pinning R5 5.0.0 with no constraint of its own:
    - 11 financial (ADR 0030);
    - 4 inventory façades (ADR 0034).
  - **Tests:** they compile with HL7's `SimpleQuantity` from the dependency. A valid resource of each type passes, and another type does not.
  - **Not in this package:** JSON Schemas ship with the software release (decided in #86).

  This completes #87.
- 2026-11-04: #87 part 2 merged (#300), completing #87. #90, the terminology service, is split in two: part 1 is the domain operations (this), and part 2 is reading terminology from packages plus the FHIR operations. Part 1, the new domain crate `health-terminology`:
  - **`Terminology`:** CodeSystems, ValueSets and ConceptMaps by URL and version, the latest by semver.
  - **The five operations:** `lookup`, `subsumes`, `expand` (pinned expansion first, otherwise the compose with the hierarchy and property filters, imports, exclusions, inactive codes, text filter and paging), `validate_code` (with displays and designations) and `translate`.
  - **Fails closed:** whatever would need another terminology server is an error, never an empty answer.
- 2026-11-04: #90 part 1 merged (#301). Part 2 is split: 2a reads terminology from packages (this), and 2b is the FHIR operation endpoints.
  - **`health-packages::terminology::load`** reads CodeSystems, ValueSets (their `expansion` as the pinned expansion) and R5 ConceptMaps into `health-terminology`'s model.
  - **Refused:** a malformed resource or an unknown filter operator, rather than a skipped one.
  - **Tested** with the core package's `template-modes`.
- 2026-11-04: #90 part 2a merged (#302). Part 2b, the FHIR operations:
  - **`FhirServer::with_terminology(ActiveTerminology)`** serves `ValueSet/$expand`, `ValueSet/$validate-code`, `CodeSystem/$validate-code`, `CodeSystem/$lookup`, `CodeSystem/$subsumes` and `ConceptMap/$translate`, by GET or by a Parameters POST.
  - **Errors:** 404 for what the node does not hold, and 422 for what cannot be answered offline.
  - **Swapping:** `ActiveTerminology::replace` swaps the terminology served.
  - **Decision:** what a node serves is set by activation (#89), which loads the active packages' terminology and calls `replace`. Until then no node wires it in.

  This completes #90.
- 2026-11-04: #90 part 2b merged (#303), completing #90. #91, tiers 3–4, is split: part 1 is tier 4 in the validator (this), and part 2 wires tiers 3–4 into the write path.
  - **The `Bindings` port** (`member(value_set, system, code)`), which the node's terminology answers.
  - **Bindings from snapshots:** `ProfileSet` compiles each element's binding (required, extensible or preferred).
  - **`SnapshotValidator::with_bindings`** checks `code`, `Coding` and `CodeableConcept` values:
    - a required binding not met is a `code-invalid` error;
    - a required binding the node cannot check offline is a warning;
    - an extensible or preferred binding not met is a warning.
  - **`Issue::severity`:** issues now carry it, and OperationOutcomes report it.
- 2026-11-04: #91 part 1 merged (#304). Part 2, tiers 3–4 on the write path:
  - **`FhirServer::with_profiles(ActiveProfiles)`** validates every write after tiers 1–2 against its claimed `meta.profile`s and its type's required profiles. Bindings are answered by `ActiveTerminology`. Errors refuse the write, and warnings let it through.
  - **Decision:** a claimed profile the node does not have is refused.
  - **Activation (#89)** compiles and sets the active profiles; until then tiers 3–4 do not run.
  - **The Java oracle** covers tier 3's profile cases. Tier 4 is not in it, because the Java run has no terminology server (`-tx n/a`).

  This completes #91.
- 2026-11-04: #91 part 2 merged (#305), completing #91. #96, Workflow Definitions, is split: part 1 is the action contract and the definition with its publication checks (this), and part 2 runs those checks in package publication.
  - **New shared contract crate `health-workflow-actions`:**
    - `WorkflowAction` (id, major, permission, compensation, parameters);
    - `ActionDescriptor` and its JSON Schema for the Studio.
  - **`health-workflow::definition`:** `WorkflowDefinition` (statechart, typed data model, data requirements, decision tables, principal). Its `problems(catalog)` refuses:
    - unknown actions and bad references;
    - invalid parameters;
    - untyped assignments;
    - unreachable states and dead ends;
    - malformed decision tables;
    - unneeded principal permissions.
  - **Decision:** actions are referenced as `<id>@<major>`.
- 2026-11-04: #96 part 1 merged (#306). Part 2, the check at publication:
  - **What is checked:** `Publications` validation reads every `workflow-definition` Library's inline `application/vnd.health.workflow+json` content. It must have its Library's url and version, and it is checked with `problems` against the actions given to `with_actions`.
  - **Decision:** central registers no actions until contexts declare theirs, so definitions that execute actions fail closed.

  This completes #96.
- 2026-11-04: #96 part 2 merged (#307), completing #96. #97, persisting workflow instances, is split: part 1 is the records and their store (this), and part 2 is `health-workflow-app`, which runs commands and domain events through them.
  - **`health-workflow::WorkflowStore`** holds `InstanceRecord` (definition `url|version`, context, authority, the interpreter's `Instance`, version), `HistoryEntry`, `TimerRecord` and the event inbox.
  - **Migration `0028_workflow`** (owned by workflow): `workflow_instance`, changed only by compare-and-set on `version`; `workflow_history`, keyed `instance|seq` with READONLY fields, so it is append-only; `workflow_timer`; and `workflow_inbox`, by event id.
  - **Decision:** each transition's history `seq` is the instance version it produced, so the history has no gaps and a concurrent step fails its compare-and-set.
- 2026-11-04: #97 part 1 merged (#308). Part 2, the workflow application:
  - **New crate `health-workflow-app`:** `Workflows` with `start`, `command`, `deliver` and `cancel`. Each runs its transition, typed actions (through the `Actions` port), timers and history entry in one transaction, and a failed action keeps nothing.
  - **Replay is a no-op:** domain events are claimed in the inbox by event id, and commands by `op_id`.
  - **Compensation:** `cancel` compensates completed actions newest first. Compensating actions go through the port; human-task and none compensations are noted on the cancellation's history entry.
  - **Decisions:**
    - a command that enables nothing is refused (`NotEnabled`), while such a domain event is consumed and ignored;
    - sub-workflow invocations are refused until supported;
    - permission checks on actions stay with each target context's use case.

  This completes #97.
- 2026-11-04: #97 part 2 merged (#309), completing #97. #98, durable timers and escalation:
  - **`WorkflowStore::due_timers`:** the pending timers that are due, earliest first.
  - **`Workflows::fire_due`** fires each due timer in its own transaction. The transaction re-reads the timer, marks it fired and runs its event as the workflow principal. A failure keeps the timer pending, so it fires exactly once across restarts.
  - **Authority:** only the instance's authority fires its timers; any other node skips them.
  - **Clock floor:** with a `TimeFloor` port, nothing fires while the node clock reads before the floor.
  - **Escalation** is a timer transition with its typed actions.
  - **Decisions:**
    - on a broken clock, timers wait rather than fire by the floor's time;
    - a timer whose state was re-entered is armed again with a new `fire_at`.
  - **Not wired yet:** no runtime calls `fire_due` every 30 s yet. That waits for a node to run workflows, which needs activation (#89).

  This completes #98.
- 2026-11-04: #98 merged (#310), completing #98. #99, Tasks and worklists, is split: part 1 is the domain (this), part 2 the claim and escalation commands with Task, RequestOrchestration and CarePlan projections, and part 3 the client's worklist screens.
  - **`HumanTask` on statechart states:** role, form, worklist, priority and claim timeout, checked at publication.
  - **`health_workflow::worklists`:** `project` rebuilds Task projections from instance state alone (12 R5 status codes, `businessStatus` = state id), and `worklist` orders a worklist's open tasks by priority, then arrival.
  - **Decisions:**
    - `health-worklists` starts as a module of `health-workflow`, as the crate catalogue says, not as its own crate;
    - claims and escalated priorities live in reserved `task.<state>.*` data-model fields, so projections need nothing but instance state;
    - an idle claim lapses when read, rather than through a timer.
- 2026-11-04: #99 part 1 merged (#311). Part 2, the task commands and the worklists' read model:
  - **`health-workflow-app`:**
    - `claim`, `release`, `reassign` and `escalate` are workflow commands with history entries;
    - every step rebuilds the instance's Tasks in its transaction, and states left behind lose their claims;
    - `tasks(instance)` and `worklist(key)` read them back.
  - **Definitions** are now `Compiled` (statechart plus machine).
  - **Migration `0029_workflow_tasks`:** `workflow_task`, the projected Tasks by worklist, priority rank and arrival.
  - **Decisions:**
    - claiming refreshes your own claim and is refused while another's is live;
    - only the holder releases;
    - reassignment overrides any claim;
    - permission checks stay with the caller (the policy decision point).
  - **Next:** part 3 writes Tasks and RequestOrchestrations as FHIR resources, and part 4 adds the client's worklist screens.
- 2026-11-04: #99 part 2 merged (#312). Part 3, the FHIR projections:
  - **What is written:** `health-workflow-app` writes each projected Task as a FHIR Task, and each instance as a RequestOrchestration whose actions are its Tasks. They go through the unit of work, in the transaction that changed the instance.
  - **Versions:** a new version is written only when a projection changed. A test checks both resources pass tier 1.
  - **Decisions:**
    - CarePlan projections are left to Care Coordination's typed actions, because CarePlan belongs to care-plans (resource-ownership.md);
    - a claim that lapses with no further step stays `in-progress` in the stored Task until the next step, and worklists apply the lapse as they read.
  - **Next:** part 4 adds the client's worklist screens.
- 2026-11-04: #99 part 3 merged (#313). Part 4, workflows over HTTP and the client's worklists:
  - **HTTP:** `health-fhir-server`'s `workflow_router` covers starting instances, events, cancelling, task commands, worklists and the definitions list. It needs `workflow.work` and `workflow.supervise`. Central mounts it at `/api/workflow`.
  - **`HEALTH_DEV_WORKFLOW_DEFINITIONS`:** central runs Workflow Definitions from a development file until activation (#89). Each is checked as at publication. Definitions with guards or typed actions are refused, because neither is wired yet.
  - **Client `/worklists`:**
    - start a workflow;
    - pick a worklist and select a task;
    - take it, put it back, or complete its step with one of its events;
    - claims show as "You have it" or "Taken by a colleague".
  - **e2e:** `worklists.spec.ts` covers a visit moving from the triage worklist to the consultation worklist, with axe.
  - **Decisions:**
    - definitions come from a development-only file at central until #89;
    - the screen does not show a colleague's identity, since `Subject.account` is an id.

  This completes #99.
- 2026-11-04: #99 part 4 merged (#314), completing #99. #101, workflow simulation and publication gates:
  - **`health_workflow::simulation`:** `tests/` scenarios (`"type": "workflow-scenario"`) run on the interpreter with stubbed actions and guards. Steps are events or waits that fire due timers, and each checks configuration, actions run, pending timers, open Tasks and `running`.
  - **`health-publication::WorkflowSandbox`** runs every scenario at submission. Any failure, or a scenario naming a definition the package lacks, returns the package to Draft. Central now uses it instead of `NoSandbox`.
  - **Acceptance criterion:** the static checks from #96 refuse a definition with an unreachable state at publication, and a publication test shows it.
  - **Fix:** the interpreter now evaluates a transition's guard only when its event matches.
  - **Decisions:**
    - guards are stubbed by each scenario, and an unstubbed guard fails it;
    - type-checking guards against data requirements waits for FHIRPath type inference over the data requirements, so it is not in this change.

  This completes #101.
- 2026-11-04: #101 merged (#315), completing #101. #120, Price Books and staged pricing, is split: part 1 is the domain (this), part 2 stores Price Books and rules at central with their approval, as profiled ChargeItemDefinitions.
  - **New domain crate `health-pricing`:**
    - `PriceBook`s: scoped, effective-dated and approved, with quantity tiers;
    - `PriceRule`s: staged, prioritised, with FHIRPath applicability through a port;
    - `validate` refuses conflicting rules;
    - `price` runs the six stages and records every book and rule version, and `check` is the authority's version check.
  - **Acceptance criterion:** a fixture charge passes through all six stages.
  - **Decisions:**
    - rules within a stage run in ascending priority;
    - two equally specific books pricing one item refuse the charge;
    - deductibles and limits run down across a charge's lines;
    - a bundle's member lines are priced at zero, referencing a bundle line priced by the rule;
    - currency conversion is not in this part.
- 2026-11-04: #120 part 1 merged (#316). Part 2, Price Books and rules kept at central:
  - **Lifecycle in `health-pricing`:** one finance approver who is not the author, no overlapping approved versions of a book or rule, and no conflicting rules. An approved version is immutable except for retirement from a time not yet passed.
  - **`PricingStore`:** migration `0030_pricing` (`price_book`, `price_rule`, one row per version, owned by pricing).
  - **New crate `health-pricing-app`:** drafting (the next version, or replacing the latest draft), approving, retiring, pricing and the authority's check.
  - **Decisions:**
    - the approver may not be the author (separation of duties, as for packages);
    - a superseding version starts after the previous one is retired, rather than ending it implicitly.
  - **Next:** part 3 serves this over HTTP at central and projects approved books and rules as ChargeItemDefinitions.
- 2026-11-04: #120 part 2 merged (#317), after CI ran out of disk (#318: the test job now frees the runner's unused toolchains and builds without debug info, which took `target/` from 43 GB to 23 GB). Part 3, pricing over HTTP and as FHIR:
  - **ChargeItemDefinitions:** `health-pricing-app` writes them when a version is approved or retired. Each book entry and each rule gets one, carrying the R5 effective period, price components and tiers, with platform extensions for scope, stage, priority and items.
  - **HTTP:** `health-fhir-server`'s `pricing_router` drafts, approves, retires, lists and quotes, under `pricing.manage`, `pricing.approve` and `pricing.quote`. Central mounts it at `/api/pricing` in `HEALTH_CURRENCY`.
  - **Quotes:** applicability is evaluated with the FHIRPath engine against each line's context resource, before pricing.
  - **Decisions:**
    - the quote route takes its pricing context from the caller until charge capture (#121) builds it from clinical events;
    - the extensions have no published StructureDefinitions yet.

  This completes #120.
- 2026-11-05: #120 part 3 merged (#319), completing #120. #121, charge capture, is split: part 1 is the domain (this), part 2 the application (storage, pricing at capture, ledger postings and reversals) and its replay tests.
  - **New domain crate `health-billing`:** charge triggers (configuration), clinical events, and `capture` with deterministic ids (identifiers.md's `{source}|{rule canonical}` derivation, per event item). It also covers the charge lifecycle (planned → billable → billed; cancellation to not-billable or entered-in-error with a reversal) and free care's zero patient share.
  - **Decisions:**
    - a charge id includes the event item's key, so a multi-item dispense gives one charge per item;
    - a billed charge is not cancelled here but corrected on its Invoice.
- 2026-11-05: #121 part 1 merged (#320). Part 2, capture in the application:
  - **New crate `health-billing-app`:** `capture` prices an event's charges together, keeps each once by id, and posts receivables per funding source against revenue per service class. `cancel_event` reverses posted charges once.
  - **Migration `0031_charges`** (`charge`, owned by billing).
  - **Acceptance criterion:** a replayed event never double-charges. Tests replay it on one store and capture it on two nodes.
  - **Free care** is priced at the reference tariff and paid wholly by its funding source.
  - **Decisions:**
    - an event's charges are priced in groups by funding: free care and billed;
    - bundled charges are priced at zero with the bundle line as its own charge;
    - coverage terms come from the caller, keyed by funding source, until coverage (#125) provides them;
    - cancellation loads the facility's postings into the ledger to reverse safely, which needs an index once ledgers grow.
  - **Not wired yet:** delivering clinical Modules' events to `capture` waits for the in-process event dispatch (`health-messaging`), which does not exist yet. Capture is driven directly until then.

  This completes #121.
- 2026-11-05: #121 part 2 merged (#321), completing #121. #122, Financial Gates, is split: part 1 is the contract and the rules (this), part 2 billing's gate query over Accounts (settlements, deferrals, waivers), and part 3 gate states in the workflow engine, with a cashier Task on block.
  - **New contract crate `health-billing-contract`:** `GateQuery`, `GateOutcome` and the `FinancialGates` port.
  - **`health-billing::evaluate`:** the ordered gate rules. Emergency and urgent care always defers, which is the acceptance criterion and is tested for every gate shape. A preauthorization must be approved, and `Deferral` and `Waiver` records must say who and why.
  - **Decisions:**
    - urgent care defers even when a preauthorization is missing;
    - a gate that does not allow blocking defers unpaid routine care rather than passing it, so the deferral is recorded.
- 2026-11-05: #122 part 1 merged (#322). Part 2, billing's gate query:
  - **The query:** `health-billing-app`'s `Billing` implements `FinancialGates`. What is owed is the patient share of owed charges less settlements and waivers, and recorded deferrals are read too.
  - **Use cases:** `settle` (posted payments to Accounts, never beyond the payment), `defer`, and `waive` (posted to `waiver/patient`).
  - **Storage:** migration `0032_billing_accounts`.
  - **Decisions:**
    - an unconfigured gate is unavailable, and the engine must defer on that;
    - no preauthorization counts as approved until coverage (#125);
    - a payment counts towards an Account only once settled against it.
- 2026-11-05: #122 part 2 merged (#323), after a `cargo doc` failure (a link to a private module). Part 3, Financial Gates in workflows:
  - **Gate states:** a `State` may name a `gate`. On entry the interpreter emits `EvaluateGate`, and `health-workflow-app` asks Finance through `with_gates`, raising `gate.pass`, `gate.defer` or `gate.block` in the same step, recorded as `GateEvaluated`.
  - **The query** uses the instance's context, `patient` and `urgency` data. Urgent triage always defers, which is the acceptance criterion, tested end to end.
  - **A block** waits with the gate state's cashier Task, and `recheck_gates` asks again on payment events.
  - **Publication** requires gate states to be atomic, with a cashier task, moving on at pass and defer, and not re-entering on block. Scenarios stub gate outcomes.
  - **Decisions:**
    - an unanswered gate, or a node with no gate port, defers and never blocks;
    - urgency is read from the `urgency` data field triage sets, with routine when absent;
    - no runtime wires Finance's gates into a node's workflows yet: central runs development definitions and no billing.

  This completes #122.
- 2026-11-05: #122 part 3 merged (#324), completing #122. #92, the Questionnaire renderer, is split: part 1 is the shared FHIRPath conformance suite (this). Later parts are the renderer on TanStack Form with Form Layouts and fhirpath.js live behaviour, server-side drafts with stable repeat `item.id`s, and server re-validation of submissions.
  - **New package `packages/fhirpath-conformance`:** fixtures for live forms (QuestionnaireResponse answers for enableWhen, calculated values and constraints) and decimal arithmetic.
  - **Two runners:** fhirpath.js in lossless precise mode in the package's tests, and the server's octofhir-fhirpath in `health-fhir-validation/tests/fhirpath_conformance.rs`. 36 cases pass in both engines, which is the acceptance criterion. frontend-ci and rust-ci run them.
  - **Decisions:**
    - numbers compare by exact text, because precise fhirpath.js returns integers as decimals;
    - the scale of arithmetic results differs between the engines and stays out of the suite;
    - fhirpath.js, `@lhncbc/ucum-lhc` and `argparse` are per-package licence exceptions, approved by the user.
- 2026-11-05: #92 part 1 merged (#325). Part 2, the renderer:
  - **Client `features/forms`:** `QuestionnaireRenderer` on TanStack Form, with the pure `model.ts` and `layout.ts`. It renders the answer types forms need with exact decimals, groups and repeats with stable `item.id`s. Live behaviour (enableWhen and enableWhenExpression, calculatedExpression, `questionnaire-constraint`, required questions) runs through the shared fhirpath.js configuration.
  - **Form Layouts:** their JSON Schema is built into the release (`health-packages`, `FORM_LAYOUT`). Layouts arrange sections, widgets and hints, and can never hide a question.
  - **Dependencies:** `@tanstack/react-form` 1.33.5 (MIT). The client uses `@health/fhirpath-conformance`'s engine configuration, which gains environment variables (`%resource`).
  - **Decisions:**
    - a failing enableWhenExpression leaves the question shown;
    - a failing constraint expression counts as failing;
    - submit is blocked while required questions or error constraints are unmet;
    - the form holds the response as an opaque value, because TanStack Form's deep key types cannot traverse FHIR's.
  - **Next:** part 3 adds the form screen with server-side drafts and its e2e test, and part 4 the server's re-validation of submissions.
- 2026-11-05: #92 part 2 merged (#326). Part 3, the forms screen with server-side drafts:
  - **Client `/forms`:** pick a Questionnaire, start or continue a draft, and fill it in the renderer.
    - drafts are saved through a new `forms` command port on the platform: a `PUT` with a UUIDv7 id from the new `newResourceId`, and `If-Match` after the first save;
    - a submit stores the response `completed`.
  - **Central, both development-only:**
    - `HEALTH_DEV_FHIR_SEED` writes definitional resources, such as Questionnaires, once at startup, indexed;
    - the search registry adds `status` on Questionnaire and QuestionnaireResponse.
  - **e2e:** `forms.spec.ts` covers live behaviour in the browser, the required block, a draft saved, continued and submitted, with axe.
  - **Decisions:**
    - a seed holds definitional resource types only, and leaves stored ones alone;
    - drafts are filtered to their form on the client until canonical searches come with activation (#89);
    - client-created resource ids are UUIDv7, as ADR 0013 requires (`crypto.randomUUID` is v4).
  - **Next:** part 4, the server's re-validation of submissions against their Questionnaire.
- 2026-11-05: #92 part 3 merged (#327). Part 4, the server's re-validation:
  - **`health_fhir_validation::check_response`:** it refuses unknown linkIds, wrong answer types or options, too many answers, answers to disabled questions, unanswered required questions on completion, and failing error constraints. It then recomputes `calculatedExpression` values with the server's engine, and the server's value is stored (ADR 0052).
  - **The write path:** `health-fhir-server`'s `with_questionnaire_checks` runs it on every `completed` or `amended` QuestionnaireResponse, in plain writes and in transaction and batch entries. It finds the Questionnaire by `url` and `version`, and refuses the submission if the Questionnaire is unknown. Central turns it on.
  - **Tests:** unit and HTTP tests, a transaction that cannot bypass the check, and `forms.spec.ts`, which now submits through it.
  - **Decisions:**
    - drafts are stored without the check;
    - misplaced or mistyped answers are refused before any expression reads them.

  This completes #92.
- 2026-11-05: #92 part 4 merged (#328), completing #92. #102, Encounters and patient-flow queues, is split: part 1 is the two domains (this), part 2 the application and storage (check-in in one transaction with its guard, the FHIR Encounter, queue entries from encounter events), and part 3 the reception and queue screens with the e2e test.
  - **New domain crate `health-encounters`:** check-in starts an `in-progress` Encounter at a service point. A guard key keeps one open Encounter per patient per service point, and the R5 lifecycle and moves between service points emit `EncounterEvent`s.
  - **New domain crate `health-patient-flow`:** queue entries joined from encounter events with derived ids (a replay queues no one twice). The queue is ordered emergency, urgent, routine, then by arrival. Steps run call, serve, finish, recall and leave.
  - **Decisions:**
    - the queue never touches the Encounter's status, which only encounters changes;
    - moving to another service point is an Encounter event that patient-flow follows.
- 2026-11-05: #102 part 1 merged (#329). Part 2, the application and storage:
  - **`health-encounters-app`:** check-in, transitions and moves, each in one transaction with:
    - the open-encounter guard (claimed, released when the Encounter ends or moves);
    - the FHIR R5 Encounter, facility-tagged;
    - the outbox event.
  - **`health-patient-flow-app`:** `follow` keeps queue entries from encounter events, once each. It also provides `queue` and the steps.
  - **Migration `0033_encounters_and_queues`:** `encounter_state` (encounters) and `queue_entry` (patient-flow).
  - **Decisions:**
    - the module keeps its Encounter in `encounter_state`, and the FHIR Encounter is its projection;
    - an emergency-class Encounter is queued as an emergency;
    - an ended Encounter closes its entries as done if the patient was called, else left.
  - **Next:** part 3 adds the reception and queue screens and the e2e test.
- 2026-11-05: #102 part 2 merged (#330). Part 3, reception and queues over HTTP and on screen:
  - **`health-fhir-server`'s `reception_router()`:**
    - check-in, Encounter moves and status, the queue of a service point, and the queue steps;
    - behind `encounters.checkin`, `encounters.manage`, `queues.view` and `queues.work`, for the session's facility;
    - central mounts it at `/api/reception`.
  - **Patient-flow:** each encounters step is followed into patient-flow in the same request, until in-process event dispatch (`health-messaging`) delivers the outbox events.
  - **Central's search:** Patient `family` and `given`, which reception finds a patient by.
  - **The client:** `/reception` (find a registered patient, then check them in at a service point) and `/reception/queues` (call, see, finish, or mark that the patient left).
  - **Tests:** HTTP tests for permissions, UUIDv7 ids, unknown patients and facility isolation, and `reception.spec.ts`. In it, a clerk registers and checks a patient in, the patient appears in the triage queue, and a nurse calls and sees them.
  - **Decisions:**
    - service points are named per facility, until facility configuration lists them;
    - reception lists only patients with an MRN, never the client registry's golden records.

  This completes #102.
- 2026-11-05: #102 part 3 merged (#331), completing #102; RUSTSEC-2026-0441 (`pin-utils` unmaintained, via SurrealDB) is ignored in `deny.toml`. #93, extraction and CompleteQuestionnaireResponse, is split: part 1 is the pure extractor (this), part 2 the `CompleteQuestionnaireResponse` use case (Owners, managed and seed targets, retraction, Provenance, one transaction) and the dry-run `$extract`, and part 3 the mapping's publication gates.
  - **New contracts crate `health-fhir-extraction`** (context clinical-documentation):
    - `Mapping::from_library` reads an `ExtractionMappingLibrary`;
    - `extract` evaluates SDC `templateExtractContext` and `templateExtractValue` into a plan of resources with derived ids, mode tags and `derivedFrom`;
    - `bundle` gives the plan as `$extract`'s transaction Bundle.
  - **Decisions:**
    - expressions are composed from the response root (`(context)[i].select(value)`), so `%resource` is always the response;
    - a template without an item context is keyed by its own id;
    - extracted resources are tagged `urn:health:extraction` with their mode, one tag rather than `extracted` plus the mode;
    - dateTimes come back from the engine normalized (`Z` as `+00:00`), the same instant on every node.
- 2026-11-05: #93 part 1 merged (#332). Part 2, completing a QuestionnaireResponse:
  - **New application crate `health-clinical-documentation-app`:**
    - `choose` picks the newest mapping whose version range takes the Questionnaire's version, and `record_mapping` records it on the response;
    - `admit` checks each Owner's opt-in (laboratory takes point-of-care results only);
    - `reconcile` writes managed targets when new or changed, retracts those that disappear, creates seed targets once and gives a review step for a changed seed;
    - `provenance` builds one Provenance per response, versioned per run.
  - **`health-fhir-server`'s `with_extraction()`:** completion on the write path, in the response's transaction. Each extracted resource goes through the write path, and a refused extraction keeps nothing.
  - **Central:** turns extraction on, and searches Library by `type`, `url` and `version`.
  - **Decisions:**
    - the last run's targets come from the response's Provenance, whose id is derived from the response, so no new table is needed;
    - a seed target an amendment would change raises `clinical-documentation.source-form-amended.v1` on the outbox, for the review queue to follow;
    - the Owners' opt-ins live in the application crate until the Owners' crates exist;
    - extracted resources are validated at tiers 1–2 like any write.
  - **Next:** part 3, the mapping's publication gates, and the dry-run `$extract`.
- 2026-11-05: #93 part 2 merged (#333). Part 3, the mapping's publication gates and the dry run:
  - **`health-fhir-extraction::gates`:**
    - expressions that evaluate;
    - linkIds the target Questionnaire has;
    - `round(n)` around the whole result of a value expression that computes (ADR 0052);
    - fixtures that give exactly their expected Bundle, with valid outputs.
  - **`health-publication`:** runs the gates at validation, with the Owners' opt-ins. A mapping needs at least one `tests/` fixture of type `extraction-fixture`.
  - **`health-fhir-server`:** `POST /QuestionnaireResponse/$extract`, the dry run, which keeps nothing.
  - **Still open on #93:**
    - re-derivation at the authority, which needs sync;
    - type-checking expressions against answer types;
    - the `extraction` activity code and the `extraction-mapping` QuestionnaireResponse extension in the core package.
- 2026-11-05: #93 part 3 merged (#334). #93 stays open for re-derivation at the authority, which needs sync, and two smaller items. Their follow-up issues are not filed yet: creating issues needs the maintainer's permission. #104, the consultation workspace, is split: part 1 is the patient summary (this), and part 2 the consultation screen (notes, conditions, orders through command ports, disposition) with its e2e test.
  - **New domain crate `health-shared-record`:** the summary's rules. It keeps current allergies (high criticality first), conditions, medications, the latest reading of each vital sign, the latest visits and active programs.
  - **New application crate `health-shared-record-app`:** the patients a golden record links, the searches per section, and FHIR R5 mapped into the summary.
  - **`health-fhir-server`:** `GET /Patient/{id}/$summary`.
    - Each section is a search under the clinician's own search policy, so sensitive labels and permissions are the policy decision point's.
    - A section the clinician may not search is `null`.
  - **Central:**
    - `patient` on the summary's types;
    - `_security` on every facility-scope type, Patient and those types. Without it, the policy's `_security:not` clause excluded every record for a subject with excluded labels.
  - **Decisions:**
    - the summary is JSON from a FHIR operation in the guarded router, not an IPS document;
    - records linked through the golden record are included, under the same policy.
- 2026-11-05: #104 part 1 merged (#335). Part 2, the consultation screen:
  - **`health-clinical-documentation-app::consultation`:** notes (Composition), diagnoses (Condition), test orders (ServiceRequest) and prescriptions (MedicationRequest), built from commands.
  - **`health-fhir-server`'s `consultation_router()`:**
    - the commands, behind `consultation.document`, `orders.request` and `medications.prescribe`;
    - each in an Encounter in progress at the session's facility;
    - central mounts it at `/api/consultation`.
  - **The client:**
    - `/consultation` lists who waits at consultation, and seeing a patient calls and serves their queue entry;
    - the workspace is one page: the summary (`Patient/$summary` through the FHIR port's new `summary`), the note, diagnoses, test orders, prescriptions, and sending the patient on or completing the visit through reception's Encounter routes.
  - **Tests:** builder and HTTP tests, and `consultation.spec.ts`. In it, a clerk checks a patient in at consultation, and a clinician records a note, a diagnosis, a test and a prescription, sees them in the summary, and completes the visit, keyboard only with axe.
  - **Decisions:**
    - orders are commands to the node, not resources assembled in the UI;
    - the order types' own Owners (orders, laboratory, medications) take over these commands when their contexts exist;
    - the author is the account until Practitioner records exist.

  This completes #104.
- 2026-11-05: #104 part 2 merged (#336), completing #104. #116, program definitions and enrolment, is split: part 1 is the domain and the use cases (this), and part 2 the program API, starting the program's workflows, the `workflow-episodeOfCare` context on completion, and regimens from approved sources at publication.
  - **New domain crate `health-programs`:**
    - enrolment into an `active` episode;
    - one open episode per patient per program, with programs running concurrently;
    - the R5 EpisodeOfCare lifecycle;
    - the transfer hand-over (finished here, naming the new facility; continued there);
    - `unapproved_regimens`.
  - **New application crate `health-programs-app`:**
    - ProgramPlanDefinitions read, with eligibility in FHIRPath;
    - enrolment, status and transfer, each in one transaction with the guard, the FHIR EpisodeOfCare and the outbox event.
  - **Decisions:**
    - the EpisodeOfCare is the module's record, read back from its extensions; there is no separate table;
    - `%patient` in eligibility is the Patient as the evaluation root;
    - the platform extensions on EpisodeOfCare are to be added to the core package with #93's.
- 2026-11-05: #116 part 1 merged (#337). Part 2, the program API:
  - **`health-fhir-server`'s `programs_router()`:**
    - program definitions, enrolment, a patient's enrolments, status and transfer;
    - behind `programs.view`, `programs.enrol` and `programs.manage`, at the session's facility;
    - central mounts it at `/api/programs`, and searches PlanDefinition by `type`.
  - **Workflows:** enrolment starts the program's Workflow Definitions about the episode (`EpisodeOfCare/{id}`), each once.
  - **Tests:** ART and TB enrolled concurrently with ART's workflow task on its worklist; eligibility, permissions, facilities, status, and the transfer hand-over to another facility.
  - **Next:** part 3, the `workflow-episodeOfCare` context on form completion, and regimens from approved sources at publication.
- 2026-11-05: #116 part 2 merged (#338). Part 3, program context and approved regimens:
  - **Program context on completion:**
    - a submitted response to a form a program lists records the patient's open episode of that program here with `workflow-episodeOfCare`;
    - with several open episodes, the clinician names one (422 until then);
    - every extracted resource records the same episode.
  - **Publication:** a ProgramPlanDefinition must be a usable program, and every regimen the package holds must cite one of its approved protocol sources.
  - **Programs** now list their forms (`composed-of` Questionnaires).

  This completes #116. Enrolment screens wait for the program designer UX (program-designer.md, fog).
- 2026-11-05: #116 part 3 merged (#339), completing #116. #107, clinical documentation with signing and amendments, is split: part 1 is signed notes made immutable (this), and part 2 the notes API (draft, edit, sign, amend, each signature and amendment with its Provenance) and the notes screen.
  - **New domain crate `health-clinical-documentation`:**
    - note kinds with their LOINC types and sections;
    - the draft, signed and amended lifecycle, with amendments and addenda relating to the original;
    - `may_replace` and `may_delete`.
  - **`health-clinical-documentation-app::notes`:** the rule on FHIR Compositions and DocumentReferences.
  - **`health-fhir-server`:** every replace and delete of a note asks it. A signed note edited in place or deleted answers 409; marking it entered in error with its content unchanged is allowed.
  - **Consultation notes** are signed by their author as they are saved.
  - **Decisions:**
    - a note is signed when its status is `final`, `amended`, `appended` or `corrected`;
    - even an identical rewrite of a signed note is refused, since only entered-in-error passes;
    - the Authority Node applies the same check when sync arrives.
- 2026-11-05: #107 part 1 merged (#340). Part 2, the notes API and screen:
  - **`health-clinical-documentation-app::notes`:**
    - notes mapped to their Compositions and back (sections in the narrative, the signature as `attester`, amendments with `relatesTo`);
    - the Provenance of a signature (`LA`) or an amendment (`UPDATE`, the original's version as `revision`).
  - **`health-fhir-server`'s `notes_router()`:**
    - draft, edit, cancel, sign, amend, and a patient's notes;
    - behind `consultation.document`, at the session's facility;
    - central mounts it at `/api/notes`, and searches Composition by `patient`.
  - **The client:** the consultation workspace's notes panel. A note is written in its kind's sections (SOAP for progress notes), saved as a draft or signed. A signed note is amended, never edited, and the amendment shows that it amends an earlier note.
  - **Tests:** mapping and Provenance tests; HTTP tests (draft, edit, sign, refused edits, amendment with its Provenance, facility and permission checks); and `consultation.spec.ts`, which signs a SOAP note and amends it.
  - **Decisions:**
    - an amendment is signed as it is made;
    - the signer is the account until Practitioner records exist;
    - DocumentReference letters with attachments come with document uploads.

  This completes #107.
- 2026-11-05: #107 part 2 merged (#341), completing #107; it also fixed a flaky registry test, whose same-instant tie-break had parsed UUIDv7 version ids as integers. #108, prescribing:
  - **New domain crate `health-medications`:**
    - prescriptions, with no stock effect;
    - the R5 lifecycle;
    - the clinical safety checks hook, with `BasicChecks` (duplicate therapy, allergy by name), whose warnings the prescriber acknowledges;
    - the pharmacy worklist.
  - **New application crate `health-medications-app`:** MedicationRequests both ways; prescribing after the checks, each with its outbox event; and status changes.
  - **`health-fhir-server`:**
    - the consultation's prescriptions go through medications (409 with the warnings until acknowledged);
    - `pharmacy_router()` gives the worklist and status changes, mounted by central at `/api/pharmacy`.
  - **The client:**
    - `/pharmacy` shows the worklist;
    - the consultation lists safety warnings and prescribes once the prescriber acknowledges them;
    - a pharmacist e2e user.
  - **Tests:** domain and app tests; HTTP tests (on the worklist and stock on hand unchanged; the allergy and duplicate warnings acknowledged; status, facilities and permissions); `consultation.spec.ts` checks the pharmacist's worklist.
  - **Decisions:**
    - `clinical-documentation-app` no longer builds MedicationRequests, since they are medications';
    - safety warnings are advisory but must be acknowledged;
    - dispensing (reservations, FEFO, MedicationDispense) is the inventory dispensing task.

  This completes #108.
- 2026-11-05: #108 merged (#342). #109, the laboratory workflow, is split: part 1 is the domain, storage and the use cases (this), and part 2 the lab Workflow Definition (reference package), the HTTP routes and worklists, the lab screens and the e2e test.
  - **New domain crate `health-laboratory`:**
    - a test from its order to its review, with specimen rejection, cancellation and corrections before verification;
    - results flagged against reference and critical ranges in exact decimals;
    - verification by a second person;
    - results released only once verified, with a critical result marked on `laboratory.verified.v1`.
  - **New application crate `health-laboratory-app`:** each step in one transaction with the Specimen, the laboratory Observations and the DiagnosticReport, and the event.
  - **Migration `0034_lab_tests`:** `lab_test`, owned by laboratory.
  - **Decisions:**
    - the module keeps the test in `lab_test`, and the FHIR resources are its projections;
    - Observations stay `preliminary` until verified;
    - a dropped result is entered in error, never deleted.
- 2026-11-05: #109 part 1 merged (#343). Part 2, the lab workflow and screens:
  - **The `ug.moh.lab` reference package** (`packages/reference-packages/lab/`): the lab Workflow Definition and its scenarios.
    - Collect, result and verify run on the lab worklists. Verified results go to `clinician-results`, at `stat` priority when critical.
    - `health-publication`'s `tests/reference_packages.rs` checks that the package publishes and holds its readable source.
  - **`health-fhir-server`:**
    - a consultation laboratory order starts the test and its workflow;
    - `laboratory_router()` drives each step and delivers it to the workflow (`deliver_for`);
    - a clinician sees no results until they are verified;
    - central mounts it at `/api/laboratory`, and its dev workflow definitions include the lab's.
  - **The client:**
    - `/laboratory`: the lab worklists, to collect, enter results, reject and verify;
    - `/results`: the clinician's verified results, critical ones urgent, to mark reviewed.
  - **Tests:** HTTP tests (the full flow through the worklists, release only after a second person verifies, stat priority for a critical result, permissions, facilities, cancellation, non-laboratory orders) and `laboratory.spec.ts`.
  - **Decisions:**
    - consultation test orders are laboratory orders by default;
    - the clinician's results worklist is the workflow's `clinician-results` until the clinical inbox (#110) gathers it;
    - point-of-care laboratory results from forms were opted in with #93 (`laboratory.record-point-of-care`).

  This completes #109.
- 2026-11-05: #109 part 2 merged (#344), completing #109. #110, care plans and the clinical inbox, is split: part 1 is the inbox (this), and part 2 the CarePlan basics.
  - **`health-fhir-server`'s `inbox_router()`** (central: `/api/inbox`):
    - verified results (critical first), the clinician's unsigned notes, and patients' requests;
    - each item names its authoritative resource;
    - each is filtered by the policy decision point as a read of that resource (`access::may_read`).
  - **The client:**
    - `/inbox`;
    - `/records/$type/$id`, the authoritative resource each item opens, read through the FHIR API.
  - **Tests:** HTTP tests (the three kinds, references that resolve, the PDP leaving out an unreadable result, other facilities, sign-in) and `laboratory.spec.ts`, now through the inbox to the DiagnosticReport.
  - **Decisions:**
    - the inbox is composed in `health-fhir-server` from the worklists, laboratory, notes and requests, as a read model across modules;
    - referrals and approvals join when their modules exist.
- 2026-11-05: #110 part 1 merged (#345). Part 2, the CarePlan basics:
  - **New domain crate `health-care-plans`:**
    - a plan's R5 lifecycle;
    - the Conditions it addresses;
    - its activities as the patient's own requests (R5 `plannedActivityReference`).
  - **New application crate `health-care-plans-app`:** the CarePlan as the module's record, each change with its outbox event, and only the patient's own records at the facility linked.
  - **`health-fhir-server`'s `care_plans_router()`** (central: `/api/care-plans`).
  - **The client:** the consultation's care plan panel. A plan is started, then the patient's prescriptions and diagnoses are added to it.
  - **Tests:** domain, app and HTTP tests, and `consultation.spec.ts`, which starts a plan and adds the prescription. The consultation and laboratory specs now allow 90 s, since several people sign in.
  - **Decisions:**
    - activities reference requests, as R5 does;
    - goals and care teams wait for their screens.

  This completes #110.

- **SurrealDB session cap (fix, before #114).** Signed-in users intermittently got a 401: the token check's grants read failed with "Method not allowed".
  - **Cause:** every clone of the SDK handle attaches a server session. Every `Connection` or `SurrealStore` clone held one for good, and each transaction opened another. Together they passed SurrealDB's 256 sessions per WebSocket.
  - **Fix:** `Connection` shares one session across its clones. Transactions borrow pooled sessions, at most 128 at once, and wait beyond that.
  - **Test:** `tests/remote_concurrency.rs`, 320 concurrent transactions, now runs in the e2e job against the real server.

- **#114 part 1: patient requests.** A patient's request is a proposal, never an order (§60).
  - **Proposals:** a refill (MedicationRequest `intent=proposal`), a test or other care (ServiceRequest `intent=proposal`), or an appointment (Appointment `proposed`). Each is tagged `patient-reported`, and only for a Patient the account is linked to.
  - **Staff:** accepting writes the prescription (through medications' safety checks), the order or the booking `basedOn` the proposal, and closes it. Declining needs a reason. The patient may withdraw while it is pending.
  - **Where:** `health-patient-engagement` `requests` (domain), `health-patient-engagement-app` `requests`, and fhir-server `requests.rs` (portal `/requests`, `/prescriptions`; staff `/api/requests`).
  - **Also:** the e2e specs now use random patient names, so repeats don't register near-duplicates the registry offers as matches. The reception spec looks only at its own queue entry.
  - **Next (part 2):** the portal screens, staff accept and decline in the inbox, and the e2e test.

- **#114 part 2: the requests screens.** This completes #114.
  - **Portal:** "Your requests" lists the patient's requests and their state. Patients ask for care or an appointment at a facility, or with `level3` access a refill, and may withdraw a pending request.
  - **Staff:** the record page an inbox item opens has accept (an appointment needs its time; a refill's safety warnings are acknowledged) and decline with a reason.
  - **Inbox:** appointment proposals now go to whoever has `appointments.book`.
  - **e2e:** `requests.spec.ts` checks that no patient request becomes an order without staff action.
  - **Decision:** the patient types the facility code, since there is no facility directory yet.

- **#115 part 1: the patient timeline.**
  - **Domain:** `health-shared-record::timeline`, a fold over resource versions: newer versions replace older ones, removals are remembered, and the order is deterministic. Kept from events in any order, it equals a rebuild (200 shuffles).
  - **Application:** `health-shared-record-app::timeline` maps nine types. Errors, drafts and unverified results stay off.
  - **HTTP:** `GET /Patient/{id}/$timeline` covers the golden record's linked records, with the PDP deciding each type at read time. Entries name their owner's current version, and the timeline writes nothing.
  - **Central:** indexes `patient` on DiagnosticReport and Task.
  - **Next (part 2):** the care journey (`health-care-journeys`).

- **#115 part 2: the Care Journey.** This completes #115.
  - **Domain:** `health-care-journeys`, a pure projection. Care is grouped by program (EpisodeOfCare), with care outside any program last. Steps are visits, appointments, orders, referrals, results and prescriptions. A step waits only while a Task on it is open (ADR 0042), and the journey is the same whatever the input order.
  - **Application:** `health-care-journeys-app` maps seven types. A referral is a SNOMED CT `3457005`-categorised ServiceRequest.
  - **HTTP:** `GET /Patient/{id}/$journey` shares the timeline's linked records and PDP-at-read-time searches. A withheld program's steps fall outside any program.
  - **Decision:** `health-care-journeys` is its own crate, as #115 names, not a module inside `health-patient-engagement`. The crate catalogue is updated.
  - **Not yet:** the timeline and journey UX (fog), and registration, follow-up and communication steps.

- **#128 part 1: reservations, FEFO and dispensing (inventory).** Stock moves only on a completed dispense.
  - **Domain:** `health-inventory::dispensing`:
    - reservations with lifetimes: 4 h for a dispense or kit, 72 h for a transfer pick; only live ones count;
    - FEFO with funding-source eligibility and an expiry buffer, and overrides that need a reason but never reach ineligible stock;
    - substitution approvals, partial settlement, and returns to quarantine.
  - **Application:** `prepare`, `complete_dispense` (dispense movements plus `inventory.dispensed.v1`, the charge trigger, in one transaction), `cancel_dispense` and `return_to_quarantine`. Every movement now counts only live reservations.
  - **Storage:** migration `0035_stock_reservations`.
  - **Tests:** six concurrent preparations of the last packs, and exactly one wins.
  - **Next (part 2):** the pharmacy's dispense routes and screens, the MedicationDispense, the charge from the event, and the e2e.

- **#128 part 2: dispensing at the pharmacy.** This completes #128.
  - **HTTP:** `/api/pharmacy` gains `prepare`, `complete` and `cancel`, with `pharmacy.dispense`. The MedicationDispense (`preparation`, `completed`, `cancelled`) is written in the same transaction as the reservations or the movements. Donor eligibility comes from the patient's active programs under `with_dispensing`.
  - **Client:** each worklist prescription has Prepare (store, stock product, quantity), then Hand over or Cancel.
  - **e2e:** `dispensing.spec.ts` checks that stock moves only when the dispense is handed over.
  - **Decisions:** the pharmacist names the stock product, since there is no product registry yet. Billing's capture of the charge from `inventory.dispensed.v1` waits for in-process event dispatch.

- **#125 part 1: coverage, Benefit Plans, eligibility and accumulators.**
  - **Domain:** `health-coverage` holds Benefit Plans, Coverages in force in order, and yearly accumulators.
    - Terms are final only from central's own accumulators; from a snapshot, or with none, they are estimates.
    - Eligibility goes through the `Payer` port. Offline it is pending verification, with the cached answer's age.
    - Split corrections are computed per line and coverage.
  - **Application:** `health-coverage-app`'s `pricing_terms` builds pricing's stage-4 terms. `settle_claim` re-prices estimated charges in service order from central's accumulators and returns the corrections and the accumulators after.
  - **Pricing:** lines record each coverage's deductible.
  - **Next:** storage of coverages and accumulators at central, snapshot sync, posting the corrections, claims, payer adapters and the HTTP API.

- **#125 part 2: coverage storage at central.** This completes #125's scope; claims and payer adapters are #126.
  - **Storage:** migration `0036_coverage` holds central's accumulators per coverage and year, which guard each settlement, and each coverage's latest eligibility, behind `CoverageStore`.
  - **Use cases:** `Coverages::settle` is one transaction, rerun on conflict, and never takes a charge in twice. `snapshot` is what a facility caches. `check_eligibility` keeps the payer's answer, which stands pending verification offline.
  - **Tests:** four concurrent settlements of one coverage: the deductible is taken once, and every payment counts.
  - **Waiting:** FHIR Coverage and InsurancePlan mapping needs the pinned local profile (ADR 0030). Snapshot sync waits for sync.

- **#126: claims and payer adapters.**
  - **Domain:** `health-coverage::claims` assembles claims per coverage in service order. It adjudicates them (approved, partial or denied, with each shortfall and the payer's reason), makes appeals that name the original, and defines the `ClaimPayer` port.
  - **Adapters:** `health-integration::payers` has `FhirPayer` (Claim, ClaimResponse and CoverageEligibilityRequest over a `PayerTransport`) and `ExportPayer` (CSV reports for government and donor funding). There is no NHIS assumption.
  - **Postings:** `health-coverage-app::remittance_posting` clears the coverage's receivable.
  - **Tests:** the contract tests against mocks found decimals sent as JSON strings; they are now numbers.
  - **Decisions needed:** who bears a shortfall or denial (a write-off, or a transfer to the patient). Binding a real payer's HTTP transport waits for a payer to be onboarded.
  - **Not yet:** assembling claims from billing's stored charges, keeping claims, and ExplanationOfBenefit.

- **#126 follow-up: shortfalls go to the patient** (decided 2026-11-06). `shortfall_posting` moves each denied or under-approved amount from the coverage's receivable to the patient's. In the test, the payer pays 60 000 of 72 000, and the patient's 28 000 becomes 40 000.

- **SurrealDB 4.0.0, with embedded SurrealKV (ADR 0060; decided 2026-11-06).**
  - **Why:** RUSTSEC-2026-0454, an unsound `reblessive` inside SurrealDB 3.3.5's executor, failed cargo-deny on every PR. The product owner chose to upgrade rather than ignore it. The 4.0 SDK has no embedded RocksDB, so edges and desktops now embed SurrealKV with `sync=every`.
  - **Stack:** SurrealDB 4 recurses, so the edge, backup and central runtimes use 10 MiB stacks (`THREAD_STACK`), and tests use `RUST_MIN_STACK` in `.cargo/config.toml`. The first test run overflowed the default stack.
  - **CI:** CI pins the 4.0.0 server by its published SHA-256. All 37 `.surql` files validate on it.
  - **Next:** spike #34 (power loss) now covers SurrealKV, and the 3.3.5 measurements are to be taken again.

- **Product master (#128 follow-up; decided 2026-11-06).**
  - **Domain:** `health-product-registry`: `Product` (code, name, GTIN with check digit, base unit, pack sizes, medicine codings, synonyms) and `resolve`, which matches by code, then by name or synonym, and reports when it is ambiguous or unknown.
  - **Application:** `health-product-registry-app` stores products as FHIR Medication. `Products::save` keeps a code unique (by guard) and unchanging.
  - **HTTP:** `/api/stock/products`. The pharmacy worklist resolves each prescription's product, and prepare uses it when none is named.
  - **Client:** Products page under Stock. The pharmacist's product field is now filled in from the product master.
  - **e2e:** `dispensing.spec.ts` adds the product on the Products page, and checks that it is filled in.

- **The core package defines the modules' own extensions (#93 follow-up).** Nine extensions were in use with no definition: `extraction-mapping` on QuestionnaireResponse (#93), the four `episode-*` extensions on EpisodeOfCare (#116), `care-plan-author` on CarePlan, and the three `product-*` extensions on Medication. `CodeSystem/platform` gains `extraction`, the Provenance activity of an extraction run. The package stays 0.1.0, since it is not yet published. Still open on #93: re-derivation at the authority, which needs sync, and type-checking expressions against answer types.

- **#93: expressions type-check against the Questionnaire's items.** `health-fhir-extraction::gates::mistyped` runs the mapping on `sample_response`, which answers every item with a value of its type, and passes every output through tier 1. Publication runs it for each Questionnaire version in the mapping's range. #93's only open item is now re-derivation at the authority, which needs sync (#81).

- **#79: the change log, commit order and outbox (decided 2026-11-07, ADR 0061).** Started before spike #36, at the product owner's choice.
  - **Every commit that wrote versions** appends them to `change_log` (keyed by `seq`, sharing a `tx_id`) and adds one `sync_outbox` row per upstream peer, in the write transaction. Migration `0037_change_log`.
  - **`seq` is taken under an in-process commit lock** held through the commit, in place of ADR 0019's counter record. Measured with 8 concurrent writers: the counter record caused 791 conflict retries for 200 writes; the lock caused none. A stress test tails the log during concurrent writes and finds no skipped or repeated `seq`.
  - **Epoch:** `sync_state:node`, replaced by `new_epoch` on backup restore.
  - **Not yet:** ledger operations on the log (#80); delivery (#81).
- **#80: the sync/v1 protocol contract (2026-11-07).** New contracts crate `health-sync-protocol`, with no I/O.
  - **Envelope** `Item`: `kind` (version or op), `op_id`, `tx_id`, the subject, provenance, `Hlc`, `content_hash` (sha-256 of canonical JSON: sorted keys, no whitespace), packages, snapshot, flags, blobs and body. Bodies are NDJSON. zstd is left to the transport (#81).
  - **Handshake:** a different protocol major is refused; software outside N-1, or an unapproved package, is quarantined. `expected_cursor` drops a checkpoint from another epoch.
  - **Push:** items grouped by `tx_id`; batches of at most 500 items or 4 MiB, a group never split.
  - **Receiver** `receive` over an `Authority` port: a retried group is answered from its receipts and applied once; an `op_id` reused with another hash is rejected `op_id_reused`; a malformed or partly known group is rejected without applying.
  - **Pull:** `decide` never applies an item twice and keeps an unaccepted local candidate under a pulled head; `apply_pull` over a `Replica` port applies one group per transaction with its checkpoint.
  - **Item states** as in synchronization.md.
  - **Not yet:** the adapters (the `Authority` and `Replica` ports over `health-surrealdb`), the HTTP endpoints, and ledger operations on the change log, all with #81.
- **#81, part 1: sync over the store (2026-11-08).** `health-surrealdb` implements the sync ports (migration `0038_sync_apply`; synchronization.md § In code).
  - **Tested** over two in-memory nodes:
    - after a partition, edge and central hold the same version with the same body for every record, and a second round moves nothing;
    - a retried push is applied once;
    - concurrent edits open a Conflict Record, and the edge keeps its candidate under central's head;
    - the facility Sync Scope refuses another facility's record on push and withholds it on pull;
    - central's own audit never goes down, and a registry record never goes up (the scope follows the data authority matrix's directions).
  - **Protocol change:** `Replica::advance` saves the cursor when a response read past items the node was not sent.
  - **Decided (2026-11-08, product owner):** an edge authenticates its sync requests with a JWS from its facility-intermediate key, verified against the node registry, rather than mTLS. ADR 0062 comes with the transport.
- **#81, part 2: the `/sync/v1` endpoints at central (2026-11-08, ADR 0062).**
  - **What:** `health-sync-server` provides the endpoints, with handshake, push, pull and cursor hints. Each request is signed by the calling node and checked by `NodeRegistry::authenticate_sync` against the node's key on record, its status, a 5-minute clock window, and the request's method, path and body.
  - **Tested over HTTP in process:**
    - an unsigned request, one signed for another request, or one signed off-clock is refused;
    - a node is held to its handshake;
    - a compatible edge pushes (a resend is answered from receipts) and pulls in its facility scope;
    - an out-of-range edge's pushes are quarantined and it is sent nothing.
- **#82, part 1: Conflict Records and signed notes (2026-11-08).** Stacked on #366.
  - **Conflict Records** keep both branches (the candidate group as pushed), with the class and policy of synchronization.md's table.
  - **Signed notes:** a signed Composition or DocumentReference edited in place by sync is rejected (`signed_version_edited`), while marking it entered-in-error as it was is accepted.
  - **Resolution:** the store resolves a record with a new version on the head (or keeps the head) and records who resolved it. The resolution syncs down.
  - **Ageing:** central alerts hourly on records open for more than 7 days.
  - **Next:** the review queue's API and screen, with the conflict-resolution permission.
- **#85: the multi-node sync simulation (2026-11-08).**
  - **Harness:** `crates/testing/health-sync-sim` gives central, edge and desktop each their own clock, minutes apart. Links can be cut, and rounds use the real protocol, including re-seeding. A restore is an epoch change.
  - **Clinical scenario (§117)**, on every PR: a visit recorded at a desktop through internet and LAN outages, an outage past the horizon (the edge re-seeds), and central's restore (everyone below re-seeds). All three nodes end with the same records, nothing is lost or refused, and nothing moves on another pass.
  - **Week of clinics** (nightly in `e2e.yml`): daily partitions, and two concurrent edits that end as Conflict Records.
- **#84, part 1: attachments follow their records (2026-11-08).**
  - **Reference format:** a blob is referenced as `Attachment.url` `urn:health:blob:sha256:<hex>`, with `size` and `contentType`, and items list their blobs.
  - **Upload:** accepted records queue their blobs for the upstream. The edge uploads each in 1 MiB chunks from the offset central reports, so an interrupted upload resumes; an out-of-order chunk gets 409 with the resume point.
  - **Download:** on demand, verified.
  - **Settings:** central's store is `HEALTH_FILES_DIR`.
  - **Tested:** a 2.5 MiB upload interrupted after one chunk resumes at 1 MiB over HTTP; a scanned consent's 3 MB blob follows its DocumentReference to central exactly once; a blob only at central comes down on demand.
  - **Still open:**
    - the pre-fetch flag;
    - the pending marker;
    - scope checks on download.
- **#83, part 1: the horizon and re-seeding (2026-11-08).**
  - **Pruning:** central prunes the change log and receipts past 180 days, hourly. Entries an outbox still needs are kept, and `seq` never goes below the pruned horizon. That was a bug the test found: pruning the whole log made `seq` restart at 1.
  - **Re-seeding:** a pull from below the horizon is answered `reseed_required`. The edge then fetches `POST /sync/v1/snapshot` (every head in its scope, with the cursor, from one transaction), applies the heads it lacks, takes a new epoch, and carries on. Its outbox goes up first, so nothing written offline is lost.
  - **Tested** in the store and over HTTPS: an edge offline beyond the horizon ends up with central's records, central with the edge's, and the next round moves nothing.
  - **Migration note:** SurrealDB 4.0 rejected a `DEFINE FIELD` on `sync_state` alongside other statements in one migration, so the horizon has its own table.
  - **Still open on #83:**
    - a streamed snapshot;
    - the signed USB seed;
    - pruning non-head history.
- **#82, part 2: the review queue over HTTP (2026-11-08).**
  - **What:** `/api/sync/conflicts` at central, for staff with `sync.resolve-conflicts`. It lists open Conflict Records with `overdue` after 7 days, shows both branches (the candidate as pushed and the current heads), and takes `keep-head` or `write`. The resolution and its AuditEvent commit in one transaction.
  - **Tested:** without the permission there is no queue; a decision about a record outside the Conflict Record is refused; a written resolution and its audit share one transaction.
  - **Still open on #82:**
    - the queue's screen;
    - commuting workflow transitions;
    - clearing the originating node's item state when the resolution arrives.
- **#81, part 3: the edge's sync task (2026-11-08).**
  - **What:** `health-edge-runtime::sync` runs handshake, push and pull rounds against central over HTTPS, signed per ADR 0062. It wakes on central's cursor hint or every `HEALTH_SYNC_INTERVAL_SECS` (backoff to 30 min while offline). `add_upstream` now queues the existing log for a new upstream.
  - **Tested** in process, over TLS against the real `/sync/v1` endpoints: what the edge wrote before first reaching central, and while central was unreachable, goes up; central's patient comes down; and the next round moves nothing.
  - **Still open for #81:**
    - priority lanes;
    - golden-record narrowing;
    - ledger operations on the log;
    - zstd;
    - a multi-node simulation with partitions in nightly CI (testing.md).
