# Task index

_Generated from the backlog spec when the backlog was created (2026-10-28). GitHub issues are authoritative; update this file when the plan changes._

| # | Task | Phase | Blocked by | Human |
|---|---|---|---|---|
| [#42](https://github.com/carapai/people-centered-healthcare/issues/42) | Create the Cargo workspace with the six-layer layout | 1 | — |  |
| [#43](https://github.com/carapai/people-centered-healthcare/issues/43) | Build tools/architecture-checks | 1 | #42 |  |
| [#44](https://github.com/carapai/people-centered-healthcare/issues/44) | Create the Bun workspace and the React app shell with the platform boundary | 1 | #38 |  |
| [#45](https://github.com/carapai/people-centered-healthcare/issues/45) | Build @health/design-system on Panda CSS 2 and Ark UI | 1 | #44 |  |
| [#46](https://github.com/carapai/people-centered-healthcare/issues/46) | Build the accessible DataTable component | 1 | #45 |  |
| [#47](https://github.com/carapai/people-centered-healthcare/issues/47) | Add the CI workflows with pinned, least-privilege actions | 1 | #42, #44 |  |
| [#48](https://github.com/carapai/people-centered-healthcare/issues/48) | Configure branch protection, merge queue and CODEOWNERS | 1 | #47 | yes |
| [#49](https://github.com/carapai/people-centered-healthcare/issues/49) | Create central-server with the shared Axum router and telemetry | 1 | #42 |  |
| [#50](https://github.com/carapai/people-centered-healthcare/issues/50) | Generate FHIR R5 TypeScript types and the OpenAPI client types | 1 | #44, #49 |  |
| [#51](https://github.com/carapai/people-centered-healthcare/issues/51) | Build tools/synthetic-data | 1 | #42 |  |
| [#173](https://github.com/carapai/people-centered-healthcare/issues/173) | Run Vitest and component tests in frontend-ci | 1 | #47 |  |
| [#180](https://github.com/carapai/people-centered-healthcare/issues/180) | Run the NVDA manual check of the DataTable | 1 | #46 |  |
| [#52](https://github.com/carapai/people-centered-healthcare/issues/52) | Implement typed identifiers and node-prefixed sequences | 2 | #42 |  |
| [#53](https://github.com/carapai/people-centered-healthcare/issues/53) | Adopt the R5 model crate behind health-fhir-models | 2 | #42, #35 |  |
| [#54](https://github.com/carapai/people-centered-healthcare/issues/54) | Define health-persistence unit-of-work, outbox and versioning traits | 2 | #42 |  |
| [#55](https://github.com/carapai/people-centered-healthcare/issues/55) | Build the health-surrealdb adapter and migrations runner | 2 | #54 |  |
| [#56](https://github.com/carapai/people-centered-healthcare/issues/56) | Implement head/history storage and the write transaction | 2 | #55, #53, #52 |  |
| [#57](https://github.com/carapai/people-centered-healthcare/issues/57) | Implement search index tables and the search compiler | 2 | #56 |  |
| [#58](https://github.com/carapai/people-centered-healthcare/issues/58) | Implement the FHIR REST API | 2 | #56, #57, #49 |  |
| [#157](https://github.com/carapai/people-centered-healthcare/issues/157) | Implement conditional create, update and delete | 2 | #58 |  |
| [#158](https://github.com/carapai/people-centered-healthcare/issues/158) | Implement transaction and batch Bundles | 2 | #157 |  |
| [#159](https://github.com/carapai/people-centered-healthcare/issues/159) | Enforce reference integrity on write and delete | 2 | #158 |  |
| [#59](https://github.com/carapai/people-centered-healthcare/issues/59) | Implement validation tiers 1–2 on the write path | 2 | #53 |  |
| [#165](https://github.com/carapai/people-centered-healthcare/issues/165) | Enforce the core invariants (tier 2) with the decimal-exact FHIRPath engine | 2 | #59 |  |
| [#166](https://github.com/carapai/people-centered-healthcare/issues/166) | Cross-check tiers 1 and 3 against the HL7 Java validator | 2 | #59, #145 |  |
| [#167](https://github.com/carapai/people-centered-healthcare/issues/167) | Implement the Inbound Dead Letter and the quarantine queue | 2 | #59, #66 |  |
| [#60](https://github.com/carapai/people-centered-healthcare/issues/60) | Implement the content-addressed blob store | 2 | #55 |  |
| [#61](https://github.com/carapai/people-centered-healthcare/issues/61) | Implement encrypted logical backup and the round-trip fixture | 2 | #55, #60 |  |
| [#62](https://github.com/carapai/people-centered-healthcare/issues/62) | Implement FhirClient, command ports and HttpTransport in the client | 2 | #44, #50, #58 |  |
| [#63](https://github.com/carapai/people-centered-healthcare/issues/63) | Build the OIDC/SMART v2 authorization server (central) | 3 | #49, #55 |  |
| [#64](https://github.com/carapai/people-centered-healthcare/issues/64) | Implement signed permission snapshots and RBAC/ABAC | 3 | #63 |  |
| [#65](https://github.com/carapai/people-centered-healthcare/issues/65) | Implement the policy decision point with query filtering and break-glass | 3 | #64, #57 |  |
| [#66](https://github.com/carapai/people-centered-healthcare/issues/66) | Implement audit and provenance | 3 | #56 |  |
| [#67](https://github.com/carapai/people-centered-healthcare/issues/67) | Build private PKI tooling for the national root and facility intermediates | 3 | #63 | yes |
| [#68](https://github.com/carapai/people-centered-healthcare/issues/68) | Implement source Patients, golden records and links | 3 | #58, #65 |  |
| [#69](https://github.com/carapai/people-centered-healthcare/issues/69) | Implement the matching engine and Patient/$match | 3 | #68, #51, #37 |  |
| [#70](https://github.com/carapai/people-centered-healthcare/issues/70) | Build the registry review queue | 3 | #69 |  |
| [#71](https://github.com/carapai/people-centered-healthcare/issues/71) | Build the reception registration screen | 3 | #62, #69, #45 |  |
| [#187](https://github.com/carapai/people-centered-healthcare/issues/187) | Persist the authorization server's state in SurrealDB | 3 | #63 |  |
| [#188](https://github.com/carapai/people-centered-healthcare/issues/188) | Mount the authorization server in central-server, with its signing key and the client sign-in flow | 3 | #63 |  |
| [#189](https://github.com/carapai/people-centered-healthcare/issues/189) | Check new passwords against a breached-password list | 3 | #63 |  |
| [#192](https://github.com/carapai/people-centered-healthcare/issues/192) | Apply the policy decision point to every FHIR interaction | 3 | #65, #188 |  |
| [#196](https://github.com/carapai/people-centered-healthcare/issues/196) | Store Provenance with its target's Owner | 3 | #68 |  |
| [#197](https://github.com/carapai/people-centered-healthcare/issues/197) | Expose the client registry's link, unlink and $merge operations over FHIR | 3 | #68, #192 |  |
| [#72](https://github.com/carapai/people-centered-healthcare/issues/72) | Create the facility-edge executable as an OS service | 4 | #55, #49, #34 |  |
| [#73](https://github.com/carapai/people-centered-healthcare/issues/73) | Implement edge provisioning, node identity and the FDE check | 4 | #72, #67 |  |
| [#74](https://github.com/carapai/people-centered-healthcare/issues/74) | Implement LAN TLS, pairing and device enrollment with DPoP | 4 | #73 |  |
| [#75](https://github.com/carapai/people-centered-healthcare/issues/75) | Run the local authorization server on the edge | 4 | #74, #64 |  |
| [#76](https://github.com/carapai/people-centered-healthcare/issues/76) | Serve the web app from the edge | 4 | #72, #62 |  |
| [#77](https://github.com/carapai/people-centered-healthcare/issues/77) | Add edge health monitoring and the admin page | 4 | #72 |  |
| [#78](https://github.com/carapai/people-centered-healthcare/issues/78) | Schedule edge backups and self-test restores | 4 | #72, #61 |  |
| [#79](https://github.com/carapai/people-centered-healthcare/issues/79) | Implement the change log, commit counter and outbox in the write transaction | 5 | #56, #36 |  |
| [#80](https://github.com/carapai/people-centered-healthcare/issues/80) | Implement health-sync-protocol | 5 | #79 |  |
| [#81](https://github.com/carapai/people-centered-healthcare/issues/81) | Run sync between central and edges with Sync Scopes | 5 | #80, #75 |  |
| [#82](https://github.com/carapai/people-centered-healthcare/issues/82) | Implement conflict detection, Conflict Records and review queues | 5 | #81 |  |
| [#83](https://github.com/carapai/people-centered-healthcare/issues/83) | Implement seeding, re-seeding and the tombstone horizon | 5 | #81 |  |
| [#84](https://github.com/carapai/people-centered-healthcare/issues/84) | Sync attachments as resumable chunks | 5 | #81, #60 |  |
| [#85](https://github.com/carapai/people-centered-healthcare/issues/85) | Build the multi-node sync simulation harness | 5 | #81, #51 |  |
| [#86](https://github.com/carapai/people-centered-healthcare/issues/86) | Implement the package format, signing and verification | 6 | #42, #67 |  |
| [#87](https://github.com/carapai/people-centered-healthcare/issues/87) | Publish the platform core package (ADR 0049 profiles and CodeSystem) | 6 | #86 |  |
| [#88](https://github.com/carapai/people-centered-healthcare/issues/88) | Implement the publication lifecycle at central | 6 | #86, #65 |  |
| [#89](https://github.com/carapai/people-centered-healthcare/issues/89) | Implement scoped activation, staging and rollback on nodes | 6 | #88, #81 |  |
| [#90](https://github.com/carapai/people-centered-healthcare/issues/90) | Implement the terminology service | 6 | #86, #58 |  |
| [#145](https://github.com/carapai/people-centered-healthcare/issues/145) | Build the snapshot-driven profile validator (tier 3) | 6 | #59, #165 |  |
| [#170](https://github.com/carapai/people-centered-healthcare/issues/170) | Bring validation tiers 1–3 within the 2 ms p95 budget | 6 | #145 |  |
| [#176](https://github.com/carapai/people-centered-healthcare/issues/176) | Enforce the urn:ietf:rfc:3986 Identifier value rule | 6 | #166 |  |
| [#91](https://github.com/carapai/people-centered-healthcare/issues/91) | Implement validation tiers 3–4 (profiles and bindings) | 6 | #59, #90, #145 |  |
| [#92](https://github.com/carapai/people-centered-healthcare/issues/92) | Build the Questionnaire renderer and Form Layouts | 6 | #62, #86, #45 |  |
| [#93](https://github.com/carapai/people-centered-healthcare/issues/93) | Implement extraction and CompleteQuestionnaireResponse | 6 | #91, #92, #87 |  |
| [#94](https://github.com/carapai/people-centered-healthcare/issues/94) | Implement $populate | 6 | #93 |  |
| [#95](https://github.com/carapai/people-centered-healthcare/issues/95) | Build the statechart interpreter | 7 | #42 |  |
| [#96](https://github.com/carapai/people-centered-healthcare/issues/96) | Implement Workflow Definitions, typed actions and FHIRPath guards | 7 | #95, #87 |  |
| [#97](https://github.com/carapai/people-centered-healthcare/issues/97) | Persist workflow instances, history and the event inbox | 7 | #96, #56 |  |
| [#98](https://github.com/carapai/people-centered-healthcare/issues/98) | Implement durable timers and escalation | 7 | #97 |  |
| [#99](https://github.com/carapai/people-centered-healthcare/issues/99) | Project Tasks and build worklists | 7 | #97 |  |
| [#100](https://github.com/carapai/people-centered-healthcare/issues/100) | Sync workflow transitions as candidates | 7 | #97, #82 |  |
| [#101](https://github.com/carapai/people-centered-healthcare/issues/101) | Implement workflow simulation and publication gates | 7 | #96, #88 |  |
| [#102](https://github.com/carapai/people-centered-healthcare/issues/102) | Implement Encounters, check-in and patient-flow queues | 8 | #99, #68 |  |
| [#103](https://github.com/carapai/people-centered-healthcare/issues/103) | Build triage with vitals capture | 8 | #93, #102 |  |
| [#104](https://github.com/carapai/people-centered-healthcare/issues/104) | Build the consultation workspace | 8 | #102 |  |
| [#105](https://github.com/carapai/people-centered-healthcare/issues/105) | Author the OPD reference Workflow Definition package | 8 | #101, #103, #104 |  |
| [#106](https://github.com/carapai/people-centered-healthcare/issues/106) | Prove multi-user OPD on the LAN without internet | 8 | #105, #76, #81 |  |
| [#107](https://github.com/carapai/people-centered-healthcare/issues/107) | Implement clinical documentation with signing and amendments | 9 | #104 |  |
| [#108](https://github.com/carapai/people-centered-healthcare/issues/108) | Implement prescribing | 9 | #104, #99 |  |
| [#109](https://github.com/carapai/people-centered-healthcare/issues/109) | Implement the laboratory workflow | 9 | #99, #104 |  |
| [#110](https://github.com/carapai/people-centered-healthcare/issues/110) | Build care plans and the clinical inbox | 9 | #99 |  |
| [#111](https://github.com/carapai/people-centered-healthcare/issues/111) | Implement Login Accounts, Person links and self-registration | 10 | #68, #63 |  |
| [#112](https://github.com/carapai/people-centered-healthcare/issues/112) | Integrate NIRA verification | 10 | #111 | yes |
| [#113](https://github.com/carapai/people-centered-healthcare/issues/113) | Implement caregiver delegation | 10 | #111, #65 |  |
| [#114](https://github.com/carapai/people-centered-healthcare/issues/114) | Implement patient requests and appointment proposals | 10 | #111, #99 |  |
| [#115](https://github.com/carapai/people-centered-healthcare/issues/115) | Build the care journey and patient timeline read models | 10 | #68, #104 |  |
| [#116](https://github.com/carapai/people-centered-healthcare/issues/116) | Implement program definitions and enrolment | 11 | #96, #102 |  |
| [#117](https://github.com/carapai/people-centered-healthcare/issues/117) | Author the ART reference program package | 11 | #116 | yes |
| [#118](https://github.com/carapai/people-centered-healthcare/issues/118) | Author the TB reference program package | 11 | #116 | yes |
| [#119](https://github.com/carapai/people-centered-healthcare/issues/119) | Implement the financial ledger | 12 | #56 |  |
| [#120](https://github.com/carapai/people-centered-healthcare/issues/120) | Implement Price Books, Price Rules and staged pricing | 12 | #119, #86 |  |
| [#121](https://github.com/carapai/people-centered-healthcare/issues/121) | Implement charge capture triggers | 12 | #120, #97 |  |
| [#122](https://github.com/carapai/people-centered-healthcare/issues/122) | Implement Financial Gates | 12 | #121, #96 |  |
| [#123](https://github.com/carapai/people-centered-healthcare/issues/123) | Implement tills and two-phase payments | 12 | #119 |  |
| [#124](https://github.com/carapai/people-centered-healthcare/issues/124) | Integrate mobile money at central | 12 | #123 | yes |
| [#125](https://github.com/carapai/people-centered-healthcare/issues/125) | Implement coverage, benefit plans, eligibility and accumulators | 12 | #120 |  |
| [#126](https://github.com/carapai/people-centered-healthcare/issues/126) | Implement claims and payer adapters | 12 | #125, #119 |  |
| [#127](https://github.com/carapai/people-centered-healthcare/issues/127) | Implement the stock ledger and balance guards | 13 | #56 |  |
| [#128](https://github.com/carapai/people-centered-healthcare/issues/128) | Implement reservations, FEFO and dispensing | 13 | #127, #108 |  |
| [#129](https://github.com/carapai/people-centered-healthcare/issues/129) | Implement transfers, counts and stock reconciliation | 13 | #127 |  |
| [#130](https://github.com/carapai/people-centered-healthcare/issues/130) | Implement receiving, requisitions and reorder suggestions | 13 | #127 |  |
| [#131](https://github.com/carapai/people-centered-healthcare/issues/131) | Implement stock valuation and optional financial postings | 13 | #127, #119 |  |
| [#132](https://github.com/carapai/people-centered-healthcare/issues/132) | Build the Tauri desktop runtime | 16 | #62, #58, #39 |  |
| [#133](https://github.com/carapai/people-centered-healthcare/issues/133) | Implement desktop pass-through and offline failover under an edge | 16 | #132, #81, #75 |  |
| [#134](https://github.com/carapai/people-centered-healthcare/issues/134) | Implement standalone mode B provisioning and backups | 16 | #132, #83, #78 |  |
| [#135](https://github.com/carapai/people-centered-healthcare/issues/135) | Implement Authority Leases | 16 | #133, #127 |  |
| [#136](https://github.com/carapai/people-centered-healthcare/issues/136) | Package, sign and update the desktop | 16 | #132, #137 | yes |
| [#137](https://github.com/carapai/people-centered-healthcare/issues/137) | Build the signed release pipeline | 17 | #47, #48 | yes |
| [#138](https://github.com/carapai/people-centered-healthcare/issues/138) | Stand up central disaster recovery | 17 | #61, #81 | yes |
| [#139](https://github.com/carapai/people-centered-healthcare/issues/139) | Deploy the in-country observability stack | 17 | #77 | yes |
| [#140](https://github.com/carapai/people-centered-healthcare/issues/140) | Complete security and confidentiality testing | 17 | #65, #113, #74 |  |
| [#141](https://github.com/carapai/people-centered-healthcare/issues/141) | Run performance and offline stress testing | 17 | #85, #69, #127 |  |
