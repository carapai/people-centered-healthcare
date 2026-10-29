# Testing (developer guide)

> Status: planned layout. No code or CI exists yet, so no suite has run. Local commands are confirmed when the scaffold lands.

The strategy is decided in [docs/architecture/testing.md](../architecture/testing.md); the CI wiring is in [deployment.md](../architecture/deployment.md). This page tells you which suites a change must touch.

## Which suites your change needs

| You changed | Suites |
|---|---|
| A domain rule, pricing stage, statechart interpreter, matching scorer | Unit and domain |
| A repository, migration or query | Repository (embedded and server SurrealDB, `EXPLAIN` plans, guard-record conflicts) |
| An endpoint, DTO or transport | API contract (OpenAPI ↔ TS types; HTTP and IPC envelopes identical) |
| A FHIR type, profile or fixture | FHIR conformance (R5 example round-trip; HL7 Java validator nightly oracle) |
| A workflow definition or the engine | Workflow (scenarios from package `tests/`; SCXML subset) |
| A package, form, mapping or FHIRPath | Metadata and terminology (validation, semver diff, extraction fixtures, fhirpath.js ↔ octofhir conformance) |
| React code | Frontend (Vitest, component tests, axe) |
| Anything written offline or synced | Offline and sync (nightly multi-node simulation) |
| Backup, restore, migrations | Recovery |
| Stock movements | Stock concurrency (nightly soak) |
| Payments | Payment reconciliation (nightly) |
| Permissions, tracing, data exposure | Security (authorization matrix and redaction every PR; ZAP nightly) |

## Commands

These are the commands CI runs (`.github/workflows/`; [deployment.md](../architecture/deployment.md) § Workflows in code). Run them locally before a PR.

```text
# rust-ci.yml
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo run --locked --quiet -p architecture-checks
cargo deny --locked check
cargo nextest run --workspace --locked
cargo test --workspace --doc --locked
cargo llvm-cov nextest --package=<each domain and application crate> --fail-under-lines 80

# rust-ci.yml, fhir-corpus job
tools/fhir/fetch-r5-core.sh && tools/fhir/fetch-r5-examples.sh
tools/fhir/check-element-table.sh target/fhir/r5-core/package
HEALTH_FHIR_R5_EXAMPLES=target/fhir/r5-examples/json \
  cargo test --release -p health-fhir-models -p health-fhir-validation

# frontend-ci.yml (in apps/client)
bun install --frozen-lockfile && bun run codegen && bun run typecheck && bun run lint && bun run build && bun run scan

# documentation.yml
bunx markdownlint-cli2@0.18.1
tools/docs/check-adr-index.sh
tools/docs/render-mermaid.sh                    # needs mmdc
git ls-files '*.surql' | xargs surreal validate
```

## Rules

- **Synthetic data only**, from `tools/synthetic-data`. No real data in CI or staging.
- **Coverage floor:** 80% line coverage on domain and application crates; integration suites cover infrastructure and runtime.
- **Port fakes** live in `health-test-support` and are used as dev-dependencies only.
- **Cross-crate tests** start as `tests/` directories in the crates exercised, and move to `health-contract-tests` / `health-integration-tests` only when they cross crates ([crate-catalogue.md](../architecture/crate-catalogue.md)).
- **Publication gates** (fixtures, scenarios, FHIRPath conformance) are tests too: a definition without them cannot be published.
- **Acceptance scenarios** (§117 clinical, §118 ART/TB, §119 finance) run end-to-end on staging over central + edge + desktop with at least one offline period.
- **Never claim a check passed unless you ran it** (AGENTS.md).

## Expected files

- `crates/<layer>/<crate>/src/**` unit tests and `crates/<layer>/<crate>/tests/`
- `crates/testing/health-test-support/`
- Package `tests/` folders (fixtures, scenarios)
- `.github/workflows/rust-ci.yml`, `frontend-ci.yml`, `security.yml`, `dependency-review.yml`, `documentation.yml`; `integration-tests.yml` comes with the integration suites

## Governing decisions

[ADR 0045](../adr/0045-pinned-least-privilege-ci-and-signed-or-failed-releases.md), [testing.md](../architecture/testing.md), [deployment.md](../architecture/deployment.md).
