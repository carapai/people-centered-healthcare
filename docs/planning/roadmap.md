# Roadmap

_Generated from the backlog spec when the backlog was created (2026-10-28). GitHub issues are authoritative; update this file when the plan changes._

The phases follow MASTER_PROMPT Part 22 (§124–141). Phase 0 (research and Wayfinder) is complete: see the map [Wayfinder: FHIR R5 healthcare platform architecture](https://github.com/carapai/people-centered-healthcare/issues/1) and [docs/architecture/overview.md](../architecture/overview.md).

| Phase | Milestone | Issues | Summary |
|---|---|---|---|
| 1 | [Phase 1 — Monorepo foundations](https://github.com/carapai/people-centered-healthcare/milestone/1) | 10 | Cargo and Bun workspaces, shared React app shell, central gateway skeleton, design system, CI and architecture checks (§125). |
| 2 | [Phase 2 — Persistence and runtime adapters](https://github.com/carapai/people-centered-healthcare/milestone/2) | 11 | FHIR models, SurrealDB persistence, migrations, FHIR REST, client transports (§126). |
| 3 | [Phase 3 — Identity, security and Client Registry](https://github.com/carapai/people-centered-healthcare/milestone/3) | 9 | Authorization server, permission snapshots, PDP, audit, client registry and matching (§127). |
| 4 | [Phase 4 — Facility Edge Node foundation](https://github.com/carapai/people-centered-healthcare/milestone/4) | 7 | Edge executable, provisioning, LAN security, local auth, monitoring, backups (§128). |
| 5 | [Phase 5 — Synchronization foundation](https://github.com/carapai/people-centered-healthcare/milestone/5) | 7 | Change log, outbox, sync protocol, scopes, conflicts, seeding (§129). |
| 6 | [Phase 6 — Terminology and metadata](https://github.com/carapai/people-centered-healthcare/milestone/6) | 9 | Packages, lifecycle, activation, terminology service, validation, forms and extraction (§130). |
| 7 | [Phase 7 — Universal workflow engine](https://github.com/carapai/people-centered-healthcare/milestone/7) | 7 | Statechart interpreter, definitions, instances, timers, Tasks and worklists (§131). |
| 8 | [Phase 8 — OPD clinical vertical slice](https://github.com/carapai/people-centered-healthcare/milestone/8) | 5 | Reception, triage, consultation, orders and Encounter completion, multi-user LAN offline (§132). |
| 9 | [Phase 9 — Clinical services](https://github.com/carapai/people-centered-healthcare/milestone/9) | 4 | Documentation, medications, laboratory, care plans, clinical inbox (§133). |
| 10 | [Phase 10 — Patient engagement](https://github.com/carapai/people-centered-healthcare/milestone/10) | 5 | Accounts, self-registration, verification, caregivers, requests, care journeys (§134). |
| 11 | [Phase 11 — Clinical Program Studio](https://github.com/carapai/people-centered-healthcare/milestone/11) | 3 | Program definitions, enrolment, ART and TB reference packages (§135). Studio UX is in the map's fog. |
| 12 | [Phase 12 — Finance and insurance](https://github.com/carapai/people-centered-healthcare/milestone/12) | 8 | Ledger, pricing, charges, gates, payments, coverage and claims (§136). |
| 13 | [Phase 13 — Inventory and procurement](https://github.com/carapai/people-centered-healthcare/milestone/13) | 5 | Stock ledger, reservations, FEFO, dispensing, transfers, counts, receiving (§137). |
| 14 | [Phase 14 — Additional clinical operations](https://github.com/carapai/people-centered-healthcare/milestone/14) | 0 | Emergency, inpatient, imaging, immunization, referrals (§138). Not yet ticketed: these depend on the map's 'additional clinical operations' fog and become issues once that is charted. |
| 15 | [Phase 15 — Public health and analytics](https://github.com/carapai/people-centered-healthcare/milestone/15) | 0 | Cohorts, indicators, surveillance, dashboards, DHIS2 (§139). Not yet ticketed: depends on the map's analytics fog. |
| 16 | [Phase 16 — Advanced desktop offline mode](https://github.com/carapai/people-centered-healthcare/milestone/16) | 5 | Tauri runtime, failover under an edge, standalone mode B, leases, packaging (§140). |
| 17 | [Phase 17 — Production hardening](https://github.com/carapai/people-centered-healthcare/milestone/17) | 5 | Release pipeline, DR, observability, security and stress testing (§141). |

## Principles

- Vertical slices across phases where needed (§141). The OPD slice (Phase 8) proves multi-user LAN operation offline early.
- Security, persistence and authorization controls land in Phases 1–5, never late (§141).
- Validation spikes gate the tasks they inform through native dependencies; see [dependencies.md](dependencies.md).
- Phases 14 and 15 have milestones but no issues yet: their scope sits in the map's Not yet specified and becomes issues once charted.
