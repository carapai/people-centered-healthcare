# Milestones

_Generated from the backlog spec when the backlog was created (2026-10-28). GitHub issues are authoritative; update this file when the plan changes._

## [Phase 1 — Monorepo foundations](https://github.com/carapai/people-centered-healthcare/milestone/1)

Cargo and Bun workspaces, shared React app shell, central gateway skeleton, design system, CI and architecture checks (§125).

- [Create the Cargo workspace with the six-layer layout](https://github.com/carapai/people-centered-healthcare/issues/42)
- [Build tools/architecture-checks](https://github.com/carapai/people-centered-healthcare/issues/43)
- [Create the Bun workspace and the React app shell with the platform boundary](https://github.com/carapai/people-centered-healthcare/issues/44)
- [Build @health/design-system on Panda CSS 2 and Ark UI](https://github.com/carapai/people-centered-healthcare/issues/45)
- [Build the accessible DataTable component](https://github.com/carapai/people-centered-healthcare/issues/46)
- [Add the CI workflows with pinned, least-privilege actions](https://github.com/carapai/people-centered-healthcare/issues/47)
- [Configure branch protection, merge queue and CODEOWNERS](https://github.com/carapai/people-centered-healthcare/issues/48) — needs a human
- [Create central-server with the shared Axum router and telemetry](https://github.com/carapai/people-centered-healthcare/issues/49)
- [Generate FHIR R5 TypeScript types and the OpenAPI client types](https://github.com/carapai/people-centered-healthcare/issues/50)
- [Build tools/synthetic-data](https://github.com/carapai/people-centered-healthcare/issues/51)
- [Run Vitest and component tests in frontend-ci](https://github.com/carapai/people-centered-healthcare/issues/173)
- [Run the NVDA manual check of the DataTable](https://github.com/carapai/people-centered-healthcare/issues/180) — needs a human

## [Phase 2 — Persistence and runtime adapters](https://github.com/carapai/people-centered-healthcare/milestone/2)

FHIR models, SurrealDB persistence, migrations, FHIR REST, client transports (§126).

- [Implement typed identifiers and node-prefixed sequences](https://github.com/carapai/people-centered-healthcare/issues/52)
- [Adopt the R5 model crate behind health-fhir-models](https://github.com/carapai/people-centered-healthcare/issues/53)
- [Define health-persistence unit-of-work, outbox and versioning traits](https://github.com/carapai/people-centered-healthcare/issues/54)
- [Build the health-surrealdb adapter and migrations runner](https://github.com/carapai/people-centered-healthcare/issues/55)
- [Implement head/history storage and the write transaction](https://github.com/carapai/people-centered-healthcare/issues/56)
- [Implement search index tables and the search compiler](https://github.com/carapai/people-centered-healthcare/issues/57)
- [Implement the FHIR REST API](https://github.com/carapai/people-centered-healthcare/issues/58)
- [Implement conditional create, update and delete](https://github.com/carapai/people-centered-healthcare/issues/157)
- [Implement transaction and batch Bundles](https://github.com/carapai/people-centered-healthcare/issues/158)
- [Enforce reference integrity on write and delete](https://github.com/carapai/people-centered-healthcare/issues/159)
- [Implement validation tiers 1–2 on the write path](https://github.com/carapai/people-centered-healthcare/issues/59)
- [Enforce the core invariants (tier 2) with the decimal-exact FHIRPath engine](https://github.com/carapai/people-centered-healthcare/issues/165)
- [Cross-check tiers 1 and 3 against the HL7 Java validator](https://github.com/carapai/people-centered-healthcare/issues/166)
- [Implement the Inbound Dead Letter and the quarantine queue](https://github.com/carapai/people-centered-healthcare/issues/167)
- [Implement the content-addressed blob store](https://github.com/carapai/people-centered-healthcare/issues/60)
- [Implement encrypted logical backup and the round-trip fixture](https://github.com/carapai/people-centered-healthcare/issues/61)
- [Implement FhirClient, command ports and HttpTransport in the client](https://github.com/carapai/people-centered-healthcare/issues/62)

## [Phase 3 — Identity, security and Client Registry](https://github.com/carapai/people-centered-healthcare/milestone/3)

Authorization server, permission snapshots, PDP, audit, client registry and matching (§127).

- [Build the OIDC/SMART v2 authorization server (central)](https://github.com/carapai/people-centered-healthcare/issues/63)
- [Implement signed permission snapshots and RBAC/ABAC](https://github.com/carapai/people-centered-healthcare/issues/64)
- [Implement the policy decision point with query filtering and break-glass](https://github.com/carapai/people-centered-healthcare/issues/65)
- [Implement audit and provenance](https://github.com/carapai/people-centered-healthcare/issues/66)
- [Build private PKI tooling for the national root and facility intermediates](https://github.com/carapai/people-centered-healthcare/issues/67) — needs a human
- [Implement source Patients, golden records and links](https://github.com/carapai/people-centered-healthcare/issues/68)
- [Implement the matching engine and Patient/$match](https://github.com/carapai/people-centered-healthcare/issues/69)
- [Build the registry review queue](https://github.com/carapai/people-centered-healthcare/issues/70)
- [Build the reception registration screen](https://github.com/carapai/people-centered-healthcare/issues/71)
- [Persist the authorization server's state in SurrealDB](https://github.com/carapai/people-centered-healthcare/issues/187)
- [Mount the authorization server in central-server, with its signing key and the client sign-in flow](https://github.com/carapai/people-centered-healthcare/issues/188)
- [Check new passwords against a breached-password list](https://github.com/carapai/people-centered-healthcare/issues/189)
- [Apply the policy decision point to every FHIR interaction](https://github.com/carapai/people-centered-healthcare/issues/192)
- [Resolve each request's subject at central and mount the FHIR API behind it](https://github.com/carapai/people-centered-healthcare/issues/209)
- [Break-glass over the API: the request endpoint, stored grants, audit obligations and the in-charge alert](https://github.com/carapai/people-centered-healthcare/issues/210)
- [Deliver break-glass alerts and run the privacy officer's review](https://github.com/carapai/people-centered-healthcare/issues/218)
- [Name a record's facility and isolate facility-scope records in searches](https://github.com/carapai/people-centered-healthcare/issues/211)
- [Store Provenance with its target's Owner](https://github.com/carapai/people-centered-healthcare/issues/196)
- [Expose the client registry's link, unlink and $merge operations over FHIR](https://github.com/carapai/people-centered-healthcare/issues/197)
- [Store blocking keys and block counts, and serve Patient/$match](https://github.com/carapai/people-centered-healthcare/issues/199)
- [Serve Patient/$match over HTTP](https://github.com/carapai/people-centered-healthcare/issues/216)
- [Load-test patient matching at 45M on central-class hardware](https://github.com/carapai/people-centered-healthcare/issues/200) — needs a human
- [Build the registry review screen](https://github.com/carapai/people-centered-healthcare/issues/203)

## [Phase 4 — Facility Edge Node foundation](https://github.com/carapai/people-centered-healthcare/milestone/4)

Edge executable, provisioning, LAN security, local auth, monitoring, backups (§128).

- [Create the facility-edge executable as an OS service](https://github.com/carapai/people-centered-healthcare/issues/72)
- [Implement edge provisioning, node identity and the FDE check](https://github.com/carapai/people-centered-healthcare/issues/73)
- [Implement LAN TLS, pairing and device enrollment with DPoP](https://github.com/carapai/people-centered-healthcare/issues/74)
- [Run the local authorization server on the edge](https://github.com/carapai/people-centered-healthcare/issues/75)
- [Serve the web app from the edge](https://github.com/carapai/people-centered-healthcare/issues/76)
- [Add edge health monitoring and the admin page](https://github.com/carapai/people-centered-healthcare/issues/77)
- [Schedule edge backups and self-test restores](https://github.com/carapai/people-centered-healthcare/issues/78)

## [Phase 5 — Synchronization foundation](https://github.com/carapai/people-centered-healthcare/milestone/5)

Change log, outbox, sync protocol, scopes, conflicts, seeding (§129).

- [Implement the change log, commit counter and outbox in the write transaction](https://github.com/carapai/people-centered-healthcare/issues/79)
- [Implement health-sync-protocol](https://github.com/carapai/people-centered-healthcare/issues/80)
- [Run sync between central and edges with Sync Scopes](https://github.com/carapai/people-centered-healthcare/issues/81)
- [Implement conflict detection, Conflict Records and review queues](https://github.com/carapai/people-centered-healthcare/issues/82)
- [Implement seeding, re-seeding and the tombstone horizon](https://github.com/carapai/people-centered-healthcare/issues/83)
- [Sync attachments as resumable chunks](https://github.com/carapai/people-centered-healthcare/issues/84)
- [Build the multi-node sync simulation harness](https://github.com/carapai/people-centered-healthcare/issues/85)

## [Phase 6 — Terminology and metadata](https://github.com/carapai/people-centered-healthcare/milestone/6)

Packages, lifecycle, activation, terminology service, validation, forms and extraction (§130).

- [Implement the package format, signing and verification](https://github.com/carapai/people-centered-healthcare/issues/86)
- [Publish the platform core package (ADR 0049 profiles and CodeSystem)](https://github.com/carapai/people-centered-healthcare/issues/87)
- [Implement the publication lifecycle at central](https://github.com/carapai/people-centered-healthcare/issues/88)
- [Implement scoped activation, staging and rollback on nodes](https://github.com/carapai/people-centered-healthcare/issues/89)
- [Implement the terminology service](https://github.com/carapai/people-centered-healthcare/issues/90)
- [Build the snapshot-driven profile validator (tier 3)](https://github.com/carapai/people-centered-healthcare/issues/145)
- [Bring validation tiers 1–3 within the 2 ms p95 budget](https://github.com/carapai/people-centered-healthcare/issues/170)
- [Enforce the urn:ietf:rfc:3986 Identifier value rule](https://github.com/carapai/people-centered-healthcare/issues/176)
- [Implement validation tiers 3–4 (profiles and bindings)](https://github.com/carapai/people-centered-healthcare/issues/91)
- [Build the Questionnaire renderer and Form Layouts](https://github.com/carapai/people-centered-healthcare/issues/92)
- [Implement extraction and CompleteQuestionnaireResponse](https://github.com/carapai/people-centered-healthcare/issues/93)
- [Implement $populate](https://github.com/carapai/people-centered-healthcare/issues/94)

## [Phase 7 — Universal workflow engine](https://github.com/carapai/people-centered-healthcare/milestone/7)

Statechart interpreter, definitions, instances, timers, Tasks and worklists (§131).

- [Build the statechart interpreter](https://github.com/carapai/people-centered-healthcare/issues/95)
- [Implement Workflow Definitions, typed actions and FHIRPath guards](https://github.com/carapai/people-centered-healthcare/issues/96)
- [Persist workflow instances, history and the event inbox](https://github.com/carapai/people-centered-healthcare/issues/97)
- [Implement durable timers and escalation](https://github.com/carapai/people-centered-healthcare/issues/98)
- [Project Tasks and build worklists](https://github.com/carapai/people-centered-healthcare/issues/99)
- [Sync workflow transitions as candidates](https://github.com/carapai/people-centered-healthcare/issues/100)
- [Implement workflow simulation and publication gates](https://github.com/carapai/people-centered-healthcare/issues/101)

## [Phase 8 — OPD clinical vertical slice](https://github.com/carapai/people-centered-healthcare/milestone/8)

Reception, triage, consultation, orders and Encounter completion, multi-user LAN offline (§132).

- [Implement Encounters, check-in and patient-flow queues](https://github.com/carapai/people-centered-healthcare/issues/102)
- [Build triage with vitals capture](https://github.com/carapai/people-centered-healthcare/issues/103)
- [Build the consultation workspace](https://github.com/carapai/people-centered-healthcare/issues/104)
- [Author the OPD reference Workflow Definition package](https://github.com/carapai/people-centered-healthcare/issues/105)
- [Prove multi-user OPD on the LAN without internet](https://github.com/carapai/people-centered-healthcare/issues/106)

## [Phase 9 — Clinical services](https://github.com/carapai/people-centered-healthcare/milestone/9)

Documentation, medications, laboratory, care plans, clinical inbox (§133).

- [Implement clinical documentation with signing and amendments](https://github.com/carapai/people-centered-healthcare/issues/107)
- [Implement prescribing](https://github.com/carapai/people-centered-healthcare/issues/108)
- [Implement the laboratory workflow](https://github.com/carapai/people-centered-healthcare/issues/109)
- [Build care plans and the clinical inbox](https://github.com/carapai/people-centered-healthcare/issues/110)

## [Phase 10 — Patient engagement](https://github.com/carapai/people-centered-healthcare/milestone/10)

Accounts, self-registration, verification, caregivers, requests, care journeys (§134).

- [Implement Login Accounts, Person links and self-registration](https://github.com/carapai/people-centered-healthcare/issues/111)
- [Integrate NIRA verification](https://github.com/carapai/people-centered-healthcare/issues/112) — needs a human
- [Implement caregiver delegation](https://github.com/carapai/people-centered-healthcare/issues/113)
- [Implement patient requests and appointment proposals](https://github.com/carapai/people-centered-healthcare/issues/114)
- [Build the care journey and patient timeline read models](https://github.com/carapai/people-centered-healthcare/issues/115)

## [Phase 11 — Clinical Program Studio](https://github.com/carapai/people-centered-healthcare/milestone/11)

Program definitions, enrolment, ART and TB reference packages (§135). Studio UX is in the map's fog.

- [Implement program definitions and enrolment](https://github.com/carapai/people-centered-healthcare/issues/116)
- [Author the ART reference program package](https://github.com/carapai/people-centered-healthcare/issues/117) — needs a human
- [Author the TB reference program package](https://github.com/carapai/people-centered-healthcare/issues/118) — needs a human

## [Phase 12 — Finance and insurance](https://github.com/carapai/people-centered-healthcare/milestone/12)

Ledger, pricing, charges, gates, payments, coverage and claims (§136).

- [Implement the financial ledger](https://github.com/carapai/people-centered-healthcare/issues/119)
- [Implement Price Books, Price Rules and staged pricing](https://github.com/carapai/people-centered-healthcare/issues/120)
- [Implement charge capture triggers](https://github.com/carapai/people-centered-healthcare/issues/121)
- [Implement Financial Gates](https://github.com/carapai/people-centered-healthcare/issues/122)
- [Implement tills and two-phase payments](https://github.com/carapai/people-centered-healthcare/issues/123)
- [Integrate mobile money at central](https://github.com/carapai/people-centered-healthcare/issues/124) — needs a human
- [Implement coverage, benefit plans, eligibility and accumulators](https://github.com/carapai/people-centered-healthcare/issues/125)
- [Implement claims and payer adapters](https://github.com/carapai/people-centered-healthcare/issues/126)

## [Phase 13 — Inventory and procurement](https://github.com/carapai/people-centered-healthcare/milestone/13)

Stock ledger, reservations, FEFO, dispensing, transfers, counts, receiving (§137).

- [Implement the stock ledger and balance guards](https://github.com/carapai/people-centered-healthcare/issues/127)
- [Implement reservations, FEFO and dispensing](https://github.com/carapai/people-centered-healthcare/issues/128)
- [Implement transfers, counts and stock reconciliation](https://github.com/carapai/people-centered-healthcare/issues/129)
- [Implement receiving, requisitions and reorder suggestions](https://github.com/carapai/people-centered-healthcare/issues/130)
- [Implement stock valuation and optional financial postings](https://github.com/carapai/people-centered-healthcare/issues/131)

## [Phase 14 — Additional clinical operations](https://github.com/carapai/people-centered-healthcare/milestone/14)

Emergency, inpatient, imaging, immunization, referrals (§138). Not yet ticketed: these depend on the map's 'additional clinical operations' fog and become issues once that is charted.

No issues yet (scope in the map's fog).

## [Phase 15 — Public health and analytics](https://github.com/carapai/people-centered-healthcare/milestone/15)

Cohorts, indicators, surveillance, dashboards, DHIS2 (§139). Not yet ticketed: depends on the map's analytics fog.

No issues yet (scope in the map's fog).

## [Phase 16 — Advanced desktop offline mode](https://github.com/carapai/people-centered-healthcare/milestone/16)

Tauri runtime, failover under an edge, standalone mode B, leases, packaging (§140).

- [Build the Tauri desktop runtime](https://github.com/carapai/people-centered-healthcare/issues/132)
- [Implement desktop pass-through and offline failover under an edge](https://github.com/carapai/people-centered-healthcare/issues/133)
- [Implement standalone mode B provisioning and backups](https://github.com/carapai/people-centered-healthcare/issues/134)
- [Implement Authority Leases](https://github.com/carapai/people-centered-healthcare/issues/135)
- [Package, sign and update the desktop](https://github.com/carapai/people-centered-healthcare/issues/136) — needs a human

## [Phase 17 — Production hardening](https://github.com/carapai/people-centered-healthcare/milestone/17)

Release pipeline, DR, observability, security and stress testing (§141).

- [Build the signed release pipeline](https://github.com/carapai/people-centered-healthcare/issues/137) — needs a human
- [Stand up central disaster recovery](https://github.com/carapai/people-centered-healthcare/issues/138) — needs a human
- [Deploy the in-country observability stack](https://github.com/carapai/people-centered-healthcare/issues/139) — needs a human
- [Complete security and confidentiality testing](https://github.com/carapai/people-centered-healthcare/issues/140)
- [Run performance and offline stress testing](https://github.com/carapai/people-centered-healthcare/issues/141)
