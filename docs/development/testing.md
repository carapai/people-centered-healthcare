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

## Planned commands

These are the jobs CI runs (from [deployment.md](../architecture/deployment.md)); local wrappers are to be fixed by the scaffold.

```text
cargo fmt --check
cargo clippy (with -D warnings)                 # exact flags to be fixed by the scaffold
cargo nextest run
cargo doc  (with -D warnings)
tools/architecture-checks
cargo-deny                                      # licences, advisories, duplicates, bans
cargo llvm-cov                                  # ≥ 80% lines on domain and application crates
surreal validate <file>.surql ; surql-fmt --check
bun install --frozen-lockfile ; tsc ; panda codegen ; vitest
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
- `.github/workflows/rust-ci.yml`, `frontend-ci.yml`, `integration-tests.yml`, `security.yml`, `documentation.yml`

## Governing decisions

[ADR 0045](../adr/0045-pinned-least-privilege-ci-and-signed-or-failed-releases.md), [testing.md](../architecture/testing.md), [deployment.md](../architecture/deployment.md).
