# Testing strategy

Status: **accepted** (ADR 0045). Produced while resolving the Wayfinder ticket [What are the CI/CD, release and production-operations rules?](https://github.com/carapai/people-centered-healthcare/issues/31). Spec: MASTER_PROMPT §116–119. Workflow wiring is in [deployment.md](deployment.md).

## Suites (§116)

| Suite | Scope | Runs |
|---|---|---|
| Unit and domain | Pure domain functions, the statechart interpreter, the pricing stages, the matching scorer | Every PR |
| Repository | Repositories against the pinned SurrealDB (embedded and server); guard-record conflicts; `EXPLAIN` plans | Every PR |
| API contract | OpenAPI and generated TypeScript types match; HTTP and IPC transports return identical envelopes | Every PR |
| FHIR conformance | Round-trip of the R5 examples; profile validation of fixtures, with the HL7 Java validator as an oracle | Every PR (fixtures); nightly (oracle) |
| Workflow | Definition scenarios from `tests/`; SCXML conformance subset | Every PR |
| Metadata and terminology | Package validation; semver diff rules; extraction fixtures; FHIRPath conformance across fhirpath.js and octofhir | Every PR |
| Frontend | Vitest unit tests; component tests; axe accessibility checks | Every PR |
| End-to-end | Playwright on the web; Tauri via `tauri-driver` on Windows and Linux; the §117–119 scenarios | Nightly |
| Offline and sync | Multi-node simulation (central, edge, desktop) with partitions, clock skew, long outages, `reseed_required`, restore and epoch change | Nightly |
| Recovery | kill -9 loops; power-loss harness on hardware runners; export → import round trip; restore drills | Nightly / weekly |
| Stock concurrency | Concurrent issues and dispenses on the same Stock Unit; balances against ledger rebuild | Nightly (soak) |
| Payment reconciliation | Mock MoMo and Airtel with dropped callbacks, duplicates and statement mismatches | Nightly |
| Security | Authorization-matrix suite (facility isolation, sensitive labels, adolescent confidentiality, break-glass); ZAP baseline against staging; telemetry redaction test | Every PR (matrix, redaction); nightly (ZAP) |

## Acceptance scenarios (§117–119)

- The clinical, ART/TB and finance scenarios are automated end-to-end suites on staging.
- They run on a central plus edge plus desktop topology, including at least one offline period per scenario.

## Data

- **Synthetic only.** `tools/synthetic-data` generates patients with Ugandan name, clan-name and phone-sharing distributions, plus facilities, stock, prices and payer contracts.
- **No real data** ever enters CI or staging.

## Coverage

- **Floor:** line coverage of at least 80% on domain and application crates (`cargo-llvm-cov`).
- **No floor** on infrastructure or runtime crates. Integration suites cover those instead.
