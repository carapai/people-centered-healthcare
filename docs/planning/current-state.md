# Current state

_Last updated: 2026-10-28_

## Phase

Phase 0 (research and Wayfinder) is complete: every decision ticket is resolved (ADRs 0001–0053), the §8 docs are written, and the known-scope backlog exists as 101 implementation issues in 17 milestones ([roadmap](roadmap.md), [task index](task-index.md)). No implementation has started. Five AFK validation spikes remain open and gate specific tasks ([dependencies](dependencies.md)).

## Wayfinder map

- Map: [Wayfinder: FHIR R5 healthcare platform architecture](https://github.com/carapai/people-centered-healthcare/issues/1) (#1). Tickets are its sub-issues; blocking uses GitHub native issue dependencies.
- Destination: all §7 blocking decisions resolved as ADRs, the §8 `docs/` tree written and consistent, and the known-scope implementation backlog on GitHub. Implementation is past the destination.
- Each resolved ticket also writes its ADR(s) and its `docs/architecture/` / `docs/research/` sections (override recorded in the map's Notes).

## Research tickets

Findings live on unmerged `research/<name>` branches, linked from each ticket's resolution comment; they are distilled into `docs/research/` when the dependent grilling ticket resolves.

| Ticket | Branch | Status |
|---|---|---|
| What do SurrealDB server and embedded engines guarantee today? (#2) | `research/surrealdb` | closed |
| How do comparable systems run an offline facility server on a LAN? (#3) | `research/facility-edge` | closed |
| What sync designs suit central / edge / desktop hierarchy for clinical data? (#4) | `research/synchronization` | closed |
| What are the options for FHIR R5 resource models, validation and search in Rust? (#5) | `research/fhir-r5` | closed |
| How should offline authentication and authorization work for health workers? (#6) | `research/offline-auth` | closed |
| How do FHIR R5 workflow, SDC extraction and WHO SMART Guidelines fit a generic engine? (#7) | `research/clinical-workflows` | closed |
| What finance, insurance and payment facts constrain the billing design? (#8) | `research/health-financing` | closed |
| What inventory and supply-chain patterns fit facility stock ledgers? (#9) | `research/inventory` | closed |
| What patient matching and MPI approaches suit a national-scale, offline-first registry? (#10) | `research/patient-identity` | closed |
| What does Tauri v2 offer for embedded local services and secure packaging? (#11) | `research/tauri` | closed |
| Which TanStack, XState, Ark UI and Panda CSS pieces are stable enough to depend on? (#12) | `research/frontend` | closed |

## Completed decisions

- Which bounded context owns each resource, and which deployables ship first? (#13) — see `docs/architecture/resource-ownership.md`, ADRs 0001–0004, `GLOSSARY.md`.
- What are the crate layers and the dependency rules that enforce them? (#14) — see `docs/architecture/dependency-rules.md`, `docs/architecture/crate-catalogue.md`, ADRs 0005–0006.
- What is the Facility Edge Node's process, storage and recovery model? (#18) — see `docs/architecture/facility-edge.md`, `docs/architecture/backup-and-recovery.md`, `docs/research/facility-edge.md`, ADR 0007.
- How do LAN clients find and trust the Facility Edge Node? (#19) — see `docs/architecture/lan-security.md`, ADRs 0008–0009.
- How do staff authenticate and get authorized offline? (#20) — see `docs/architecture/offline-auth.md`, `docs/research/offline-auth.md`, ADRs 0010–0011.
- Who holds write authority for each resource at central, edge and desktop? (#21) — see `docs/architecture/write-authority.md`, ADR 0012, `GLOSSARY.md`.
- How are global and facility-local identifiers minted? (#17) — see `docs/architecture/identifiers.md`, ADRs 0013–0014.
- How are FHIR R5 resources modelled, validated and searched in Rust? (#15) — see `docs/architecture/fhir-r5-model.md`, `docs/research/fhir-r5.md`, ADRs 0015–0016.
- How are FHIR resources, versions and history persisted in SurrealDB? (#16) — see `docs/architecture/persistence.md`, `docs/research/surrealdb.md`, ADRs 0017–0018.
- What is the sync protocol and per-resource conflict policy? (#22) — see `docs/architecture/synchronization.md`, `docs/research/synchronization.md`, ADRs 0019–0021, `GLOSSARY.md`.
- How are QuestionnaireResponses extracted into clinical resources? (#25) — see `docs/architecture/questionnaire-extraction.md`, `docs/research/clinical-workflows.md`, ADRs 0022–0023, `GLOSSARY.md`.
- How are terminology, metadata and configuration packaged and published? (#23) — see `docs/architecture/terminology.md`, `docs/architecture/metadata.md`, `docs/architecture/configuration.md`, `docs/research/metadata-terminology.md`, ADRs 0024–0026, `GLOSSARY.md`.
- How do patient matching, self-registration and caregiver delegation work? (#26) — see `docs/architecture/patient-identity.md`, `docs/research/patient-identity.md`, ADRs 0027–0029, `GLOSSARY.md`.
- How do pricing, coverage, billing, payments and the ledger fit together? (#27) — see `docs/architecture/pricing-and-insurance.md`, `docs/architecture/billing-and-payments.md`, `docs/research/health-financing.md`, ADRs 0030–0033, `GLOSSARY.md`.
- How do stock transactions stay consistent across stores, edges and dispensing? (#28) — see `docs/architecture/inventory-and-procurement.md`, `docs/research/inventory.md`, ADRs 0034–0035, `GLOSSARY.md`.
- How does one React app adapt across central, LAN and Tauri runtimes? (#29) — see `docs/architecture/client-application.md`, `docs/research/frontend.md`, ADRs 0036–0038.
- What can a standalone Tauri client do offline, and how is it packaged? (#30) — see `docs/architecture/desktop-client.md`, `docs/research/tauri.md`, ADRs 0039–0041.
- What is the generic workflow engine's definition and execution model? (#24) — see `docs/architecture/workflow-engine.md`, `docs/research/clinical-workflows.md`, ADRs 0042–0044, `GLOSSARY.md`.
- What are the CI/CD, release and production-operations rules? (#31) — see `docs/architecture/deployment.md`, `docs/architecture/testing.md`, ADRs 0045–0047.
- Is the documentation set complete and consistent? (#32) — all §8 architecture and research docs written; `docs/development/` guides (§107); `docs/crates`, `docs/services`, `docs/fhir` index pages; consistency fixes. Raised two new grilling tickets (#40, #41).
- Must Facility Edge Nodes require full-disk encryption? (#40) — see ADR 0048, `docs/architecture/security.md`, `docs/architecture/facility-edge.md`.
- Which FHIR resources carry engine-native Published Definitions? (#41) — see ADR 0049, `docs/fhir/README.md`, `docs/architecture/metadata.md`.
- Is the known-scope implementation backlog created? (#33) — 100 issues (#42–#141), 17 milestones; see `docs/planning/roadmap.md`, `milestones.md`, `task-index.md`, `dependencies.md`, `progress.md`.

## Outstanding decisions

- Which R5 model crate passes the round-trip spike: helios-fhir or fhir-r5? (#35) — `fhir-r5 =4.2.3`; see `docs/research/fhir-r5.md`, ADR 0050. Raised three grilling tickets (#142–#144).
- What happens to inbound R5 JSON that the strict model crate cannot parse? (#142) — see ADR 0051, `docs/architecture/fhir-r5-model.md`, `GLOSSARY.md`.
- How does FHIRPath evaluation stay decimal-exact? (#143) — see ADR 0052, `docs/architecture/fhir-r5-model.md`, `docs/architecture/questionnaire-extraction.md`.
- Which profile and slicing validator does tier 3 use? (#144) — see ADR 0053, `docs/research/fhir-r5.md`; added the task [Build the snapshot-driven profile validator (tier 3)](https://github.com/carapai/people-centered-healthcare/issues/145) (#145).

Wayfinder tickets open: the five AFK validation spikes. Frontier (open, unblocked, unclaimed), in map order:

1. Does embedded SurrealDB RocksDB survive hard power loss on facility hardware? (#34): AFK task; needs facility hardware
2. Does the per-node commit counter sustain facility write rates on edge hardware? (#36): AFK task; needs edge hardware
3. Can SurrealDB blocking indexes serve patient matching at 45M-person scale? (#37): AFK task
4. Does the frontend scaffold build on Bun, Vite 8, Panda 2, React Compiler and TS 7? (#38): AFK task; can run locally
5. Does embedded SurrealDB run reliably inside the Tauri process on facility PCs? (#39): AFK task; needs Windows facility PCs

Recommended next: the decision tickets surfaced by the model-crate spike are all resolved, and only the five spikes remain on the map. Start implementation with [Create the Cargo workspace with the six-layer layout](https://github.com/carapai/people-centered-healthcare/issues/42) (#42), and run the frontend-scaffold spike (#38) and the matching-scale spike (#37), which can run locally. The power-loss (#34), commit-counter (#36) and Tauri-on-facility-PC (#39) spikes need facility hardware.

Research also surfaced legal questions (SurrealDB BSL hosting, Bank of Uganda payment licensing, DPPA consent basis) and validation spikes; these sit in the map's Not yet specified section.

## Details left to the scaffold

Recorded by the documentation consistency check; each is an implementation detail to fix when the scaffold lands, not an architectural decision: `rust-toolchain.toml` channel; the time crate; the directory of `health-node-identity`; layer tag spelling (`domain` vs `domains`); the location of reference package sources; event naming and versioning convention; the `.surql` migrations directory and naming; command DTO and OpenAPI output paths; feature-module layout in `apps/client/src/`. The persistence sketch was validated with `surreal` 3.2.4, not the 3.3 pin.

## Known blockers

None.

## Failing tests

None (no code yet).

## Next recommended step

Implement **Create the Cargo workspace with the six-layer layout** (#42), then follow [dependencies.md](dependencies.md).
