# Current state

_Last updated: 2026-10-27_

## Phase

Phase 0 — Research and Wayfinder (charting complete; research in progress). No implementation has started.

## Wayfinder map

- Map: [Wayfinder: FHIR R5 healthcare platform architecture](https://github.com/carapai/people-centered-healthcare/issues/1) (#1). Tickets are its sub-issues; blocking uses GitHub native issue dependencies.
- Destination: all §7 blocking decisions resolved as ADRs, the §8 `docs/` tree written and consistent, and the known-scope implementation backlog on GitHub. Implementation is past the destination.
- Each resolved ticket also writes its ADR(s) and its `docs/architecture/` / `docs/research/` sections (override recorded in the map's Notes).

## Research tickets

Findings live on unmerged `research/<name>` branches, linked from each ticket's resolution comment; they are distilled into `docs/research/` when the dependent grilling ticket resolves.

| Ticket | Branch | Status |
|---|---|---|
| What do SurrealDB server and embedded engines guarantee today? (#2) | `research/surrealdb` | running |
| How do comparable systems run an offline facility server on a LAN? (#3) | `research/facility-edge` | running |
| What sync designs suit central / edge / desktop hierarchy for clinical data? (#4) | `research/synchronization` | running |
| What are the options for FHIR R5 resource models, validation and search in Rust? (#5) | `research/fhir-r5` | running |
| How should offline authentication and authorization work for health workers? (#6) | `research/offline-auth` | running |
| How do FHIR R5 workflow, SDC extraction and WHO SMART Guidelines fit a generic engine? (#7) | `research/clinical-workflows` | running |
| What finance, insurance and payment facts constrain the billing design? (#8) | `research/health-financing` | running |
| What inventory and supply-chain patterns fit facility stock ledgers? (#9) | `research/inventory` | running |
| What patient matching and MPI approaches suit a national-scale, offline-first registry? (#10) | `research/patient-identity` | queued (batch 2) |
| What does Tauri v2 offer for embedded local services and secure packaging? (#11) | `research/tauri` | queued (batch 2) |
| Which TanStack, XState, Ark UI and Panda CSS pieces are stable enough to depend on? (#12) | `research/frontend` | queued (batch 2) |

## Outstanding decisions

All 18 grilling tickets (#13–#31) are open and require the human in the loop. Unblocked now: **Which bounded context owns each resource, and which deployables ship first?** (#13). The rest unblock as research closes.

## Known blockers

None.

## Failing tests

None (no code yet).

## Next recommended step

Work the map: resolve **Which bounded context owns each resource, and which deployables ship first?** (#13) in a grilling session with the user.
