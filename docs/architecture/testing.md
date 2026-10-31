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
| Metadata and terminology | Package validation; semver diff rules; extraction fixtures; FHIRPath conformance across fhirpath.js and octofhir, including the decimal conformance suite (ADR 0052); tier 3 differential test against the HL7 Java validator and the 2 ms p95 validation budget (ADR 0053) | Every PR |
| Pricing | Price Book and Price Rule fixtures through all six stages, coverage splits, rounding and largest-remainder allocation | Every PR |
| Payer adapters | Contract tests per payer adapter against recorded or mock payer responses (eligibility, claim, remittance) | Every PR (mock); nightly (sandbox where available) |
| Frontend | Vitest unit tests; component tests and axe accessibility checks in Chromium (Vitest browser mode), axe over the design system's example page in every theme and density | Every PR |
| End-to-end | Playwright on the web; Tauri via `tauri-driver` on Windows and Linux; the §117–119 scenarios | Nightly |
| Offline and sync | Multi-node simulation (central, edge, desktop) with partitions, clock skew, long outages, `reseed_required`, restore and epoch change | Nightly |
| Recovery | kill -9 loops; power-loss harness on hardware runners; export → import round trip; restore drills | Nightly / weekly |
| Stock concurrency | Concurrent issues and dispenses on the same Stock Unit; balances against ledger rebuild | Nightly (soak) |
| Payment reconciliation | Mock MoMo and Airtel with dropped callbacks, duplicates and statement mismatches | Nightly |
| Security | Authorization-matrix suite (facility isolation, sensitive labels, adolescent confidentiality, break-glass); ZAP baseline against staging; telemetry redaction test | Every PR (matrix, redaction); nightly (ZAP) |

### The client's FHIR contract

- **One suite, two base URLs** (`apps/client/test/contract/`, `bun run test:contract`). The suite runs once against central, with a bearer token, and once against an edge, with DPoP proofs.
- **What it covers:** read, vread, search over GET and POST (including a repeated parameter), instance and type history, a 404 as a `FhirError` carrying its OperationOutcome, and the credentials sent on every request.
- **Decimals:** `3.50` and a 28-digit decimal must come back with their exact source text, both as `LosslessNumber` and when serialized again. Parsing with `JSON.parse` fails this test.
- **Server:** the real `FhirServer`, from `health-fhir-server`'s `contract_server` example. It runs two nodes, each on its own SurrealDB memory engine and port. The example has no authentication: it ignores credentials, so the suite checks what the client sends, not the server's verification. `CONTRACT_SERVERS` points the suite at nodes that are already running.
- **Unit tests** of the platform layer are in `apps/client/test/` (`bun run test`).

## Acceptance scenarios (§117–119)

- The clinical, ART/TB and finance scenarios are automated end-to-end suites on staging.
- They run on a central plus edge plus desktop topology, including at least one offline period per scenario.

## Data

- **Synthetic only.** `tools/synthetic-data` generates patients with Ugandan name, clan-name and phone-sharing distributions, plus facilities, stock, prices and payer contracts.
  - **Reproducible:** a seed gives byte-identical NDJSON, with SHA-256s in its manifest. The tool uses its own PRNG.
  - **Tagged:** every resource carries the `urn:health:synthetic-data` tag.
  - **Loading:** batch Bundles through the FHIR API, so tiers 1 and 2 validate everything loaded.
  - **Details:** [tools/synthetic-data/README.md](../../tools/synthetic-data/README.md).
- **No real data** ever enters CI or staging.

## Coverage

- **Floor:** line coverage of at least 80% on domain and application crates (`cargo-llvm-cov`).
- **No floor** on infrastructure or runtime crates. Integration suites cover those instead.
