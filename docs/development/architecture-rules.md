# Architecture rules

> Status: planned layout. No code exists yet; the checks named here are confirmed when the scaffold and `tools/architecture-checks` land.

A one-page checklist of rules that every change must keep. Each rule links to where it was decided.

## Crates and dependencies

1. **Six layers, downward only:** foundation → contracts → domains → application → infrastructure → runtime, plus a dev-only testing layer. Exceptions: per-context contract crates may depend on FHIR and foundation contracts; infrastructure adapters may depend on `health-surrealdb`, `health-http` and other infrastructure crates ([dependency-rules.md](../architecture/dependency-rules.md), [ADR 0005](../adr/0005-six-layer-crate-stack.md)).
2. **No domain→domain and no application→application edges.** A domain needing another Module declares a port; the application layer implements it ([ADR 0006](../adr/0006-pure-isolated-domain-crates.md)).
3. **Pure domains.** Allowed externals: `serde`, `thiserror`, `rust_decimal`, `uuid`, the chosen time crate, `tracing` macros. No `tokio`, `axum`, `surrealdb`, `tauri`, `anyhow` and the rest of the forbidden list.
4. **No runtime switches** (`[features]` or `cfg` on central/edge/desktop) in domain or application crates. Runtime differences come through ports and configuration.
5. **`anyhow` only in runtime crates and tooling.** Everything else uses `thiserror`.
6. **Workspace hygiene:** every external version in root `[workspace.dependencies]`; `{ workspace = true }` everywhere; `[workspace.lints]` inherited (`unsafe_code = "forbid"`); items default to `pub(crate)`.
7. **Layer metadata** in every `Cargo.toml`: `[package.metadata.health] layer = …, context = …`.
8. **Runtime crates are leaves** and contain no business logic ([crate-catalogue.md](../architecture/crate-catalogue.md)).

## Ownership and transactions

9. **One Owner per resource**, and only the Owner's repository writes its tables ([resource-ownership.md](../architecture/resource-ownership.md), [ADR 0004](../adr/0004-transactions-within-one-deployable.md)).
10. **Workflow owns every Task and RequestOrchestration.** Domains never call Workflow; they emit events. Workflow calls domains through typed actions ([ADR 0002](../adr/0002-workflow-owns-tasks.md)).
11. **Terminology & Metadata owns every Published Definition**; other contexts use pinned versions ([ADR 0003](../adr/0003-metadata-owns-published-definitions.md)).
12. **One unit of work per accepted write:** head, history, index entries, `change_log`, outbox. Multi-record invariants use guard records or UNIQUE indexes, never read-then-check ([persistence.md](../architecture/persistence.md)).
13. **Domains return events; the application layer appends them to the outbox** in the same unit of work. Domains never publish.

## Data and ids

14. **Ids are UUIDv7, client-generated, typed newtypes** from `health-identifiers`; derived records use UUIDv5 ([identifiers.md](../architecture/identifiers.md)).
15. **Canonical R5 JSON at rest**, typed models in code through the `health-fhir-models` facade only ([fhir-r5-model.md](../architecture/fhir-r5-model.md)).
16. **Validate on the writing node and again at authority** ([ADR 0016](../adr/0016-validate-on-the-writing-node-and-again-at-authority.md)).
17. **No `set<record>`** in schemas (SurrealDB #7621).
18. **Money is `rust_decimal`**, never floats; corrections are reversals ([billing-and-payments.md](../architecture/billing-and-payments.md)).

## Client

19. **The UI talks only through ports** (`FhirClient`, per-context command ports, `EventSource`) over a transport. It never assembles the FHIR writes of a business operation ([ADR 0036](../adr/0036-ports-over-swappable-transport-with-tauri-ipc-into-the-shared-router.md)).
20. **`@tauri-apps/*` only in `src/platform/**`**; feature code reads `Platform.capabilities`, never the runtime.
21. **No clinical data in browser or webview storage** ([ADR 0037](../adr/0037-no-clinical-data-in-browser-or-webview-storage.md)).

## Operations

22. **No personal health data in telemetry** ([ADR 0047](../adr/0047-no-health-data-in-telemetry-and-in-country-observability.md)).
23. **Forward-only migrations**; an applied migration is never edited ([adding-a-migration.md](adding-a-migration.md)).

## How the rules are checked

`tools/architecture-checks` (cargo metadata), `cargo-deny`, `clippy -D warnings`, oxlint import rules plus `apps/client/scripts/check-boundaries.ts`, the production bundle scan, the table-ownership test and the ownership doc-sync test. The full list is in [deployment.md § Architecture enforcement](../architecture/deployment.md#architecture-enforcement-109).
