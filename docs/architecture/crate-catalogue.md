# Crate catalogue

Status: **accepted** (ADRs 0005, 0006). Produced while resolving the Wayfinder ticket [What are the crate layers and the dependency rules that enforce them?](https://github.com/carapai/people-centered-healthcare/issues/14). Layer definitions and dependency rules: [dependency-rules.md](dependency-rules.md).

"Starts as" says whether the crate exists from its first use, or begins as a Rust module inside a host crate and is split out when a second consumer or a different dependency set appears. Every crate is created lazily, when the first implementation task needs it.

## 1 · foundation

| Crate | Starts as |
|---|---|
| `health-identifiers` | crate |
| `health-errors` | crate |
| `health-types` | crate |
| `health-config` | crate |
| `health-time` | crate |

## 2 · contracts

| Crate | Starts as | Notes |
|---|---|---|
| `health-fhir-models` | crate | Facade over a generated R5 model; internals decided in [How are FHIR R5 resources modelled, validated and searched in Rust?](https://github.com/carapai/people-centered-healthcare/issues/15) |
| `health-fhir-validation` | crate | Pure; wraps the FHIRPath engine |
| `health-fhir-search` | crate | Parsed search AST + SearchParameter registry only; SurrealQL translation lives in `health-surrealdb` |
| `health-fhir-mapping` | crate | |
| `health-fhir-extraction` | crate | Pure SDC template-extraction executor and `$populate` evaluator; see [questionnaire-extraction.md](questionnaire-extraction.md) |
| `health-sync-protocol` | crate | Wire types and the shared client and apply logic; see [synchronization.md](synchronization.md) |
| `health-persistence` | crate | Moved here from §17 (ADR 0005): pure unit-of-work, outbox and versioning traits |
| `health-<context>-contract` | crate per Bounded Context | Events, typed Workflow actions, context ids, cross-context query DTOs; created when the context first publishes one |

## 3 · domains (one crate per Module)

| Bounded Context | Crates | Starts as modules inside |
|---|---|---|
| Registries | `health-client-registry`, `health-facility-registry`, `health-workforce-registry`, `health-location-registry`, `health-product-registry`, `health-service-catalogue` | — |
| Terminology & Metadata | `health-terminology`, `health-metadata`, `health-configuration` | `health-metadata-validation`, `health-questionnaire-engine` → `health-metadata`; `health-configuration-publisher` → `health-configuration`; guideline packages → `health-metadata` |
| Clinical Records | `health-encounters`, `health-clinical-data`, `health-orders`, `health-medications`, `health-clinical-documentation`, `health-shared-record` (Read Model) | — |
| Clinical Operations | `health-laboratory`, `health-imaging`, `health-immunization`, `health-scheduling`, `health-patient-flow` | — |
| Care Coordination | `health-referrals`, `health-care-plans` | — |
| Programs & Public Health | `health-programs`, `health-surveillance`, `health-population` | — |
| Finance | `health-pricing`, `health-coverage`, `health-billing`, `health-payments`, `health-financial-ledger` | — |
| Supply Chain | `health-inventory`, `health-procurement` | — |
| Identity & Access | `health-identity`, `health-consent`, `health-audit-domain` | — |
| Workflow | `health-workflow`, `health-decision-support` | `health-worklists` (Read Model) → `health-workflow` |
| Patient Engagement | `health-patient-requests`, `health-patient-engagement` | `health-patient-access` → `health-patient-engagement`; `health-care-journeys` (Read Model) → `health-patient-engagement` |
| Analytics | `health-analytics`, `health-reporting` | — |
| Platform | `health-communications`, `health-administration` | — |

`health-orders` and `health-clinical-documentation` follow from the Modules in the ownership matrix. The Audit Module's domain rules are named `health-audit-domain`, which keeps `health-audit` free for the infrastructure adapter that §17 names.

## 4 · application

| Crate | Notes |
|---|---|
| `health-<context>-app` | One per Bounded Context, e.g. `health-clinical-records-app`, `health-finance-app`; created with the context's first use case |

## 5 · infrastructure

| Crate | Starts as |
|---|---|
| `health-surrealdb` | crate (includes SurrealQL search translation) |
| `health-migrations` | module of `health-surrealdb` |
| `health-messaging` | crate |
| `health-http` | crate (Axum/Tower routers, middleware) |
| `health-fhir-server` | crate (FHIR REST handlers, per §16) |
| `health-auth` | crate |
| `health-audit` | crate |
| `health-observability` | crate |
| `health-files` | crate |
| `health-crypto` | crate |
| `health-sync-server` | crate |
| `health-integration` | crate (DHIS2, OpenMRS/UgandaEMR, lab, insurer, payment-gateway adapters) |

## 6 · runtime

| Crate / app | Starts as |
|---|---|
| `apps/server/central-server` | binary (gateway built in, ADR 0001) |
| `apps/server/facility-edge` | binary over `health-edge-runtime` |
| `health-edge-runtime` | crate; `health-edge-api`, `-storage`, `-sync`, `-discovery`, `-auth` and `-admin` start as its modules |
| `health-node-identity` | crate from the start (shared by edge and desktop) |
| `apps/client/src-tauri` | binary over `health-desktop-runtime` |
| `health-desktop-runtime` | crate; `health-desktop-storage`, `-sync`, `-auth`, `-workflow`, `-devices` and `-files` start as its modules |

The edge and desktop runtimes reuse the same domain and application crates as central. They add only runtime adapters (§32: "do not duplicate business logic inside edge-specific crates").

## testing

| Crate | Starts as |
|---|---|
| `health-test-support` | crate (fixtures, in-memory port fakes) |
| `health-contract-tests` | `tests/` directories in the crates exercised, until tests cross crates |
| `health-integration-tests` | `tests/` directories, until tests cross crates |
