# Current state

Last updated: 2026-10-29.

## Phase

Phase 0 (research and Wayfinder) is complete: every decision ticket is resolved (ADRs 0001–0055), the §8 docs are written, and the known-scope backlog exists as 101 implementation issues in 17 milestones ([roadmap](roadmap.md), [task index](task-index.md)). Implementation is under way: 58 of 129 tasks are done ([progress](progress.md)). On `main`: the Cargo workspace, architecture checks, the client shell, identifiers, persistence traits, the SurrealDB adapter, the write transaction, search, and central-server with telemetry. [Implement the FHIR REST API](https://github.com/carapai/people-centered-healthcare/issues/58) (#58) is merged. Its follow-ups are #157, #158 and #159. [Implement conditional create, update and delete](https://github.com/carapai/people-centered-healthcare/issues/157) (#157) is merged. [Implement transaction and batch Bundles](https://github.com/carapai/people-centered-healthcare/issues/158) (#158) is merged. [Enforce reference integrity on write and delete](https://github.com/carapai/people-centered-healthcare/issues/159) (#159) is merged; flagging out-of-scope references `unresolved` waits for node scope. [Adopt the R5 model crate behind health-fhir-models](https://github.com/carapai/people-centered-healthcare/issues/53) (#53) is merged. [Implement validation tiers 1–2 on the write path](https://github.com/carapai/people-centered-healthcare/issues/59) (#59) is merged as tier 1; the Java validator cross-check and the Inbound Dead Letter are #166 and #167. [Enforce the core invariants (tier 2) with the decimal-exact FHIRPath engine](https://github.com/carapai/people-centered-healthcare/issues/165) (#165) is merged. [Build the snapshot-driven profile validator (tier 3)](https://github.com/carapai/people-centered-healthcare/issues/145) (#145) is merged; the tier 3 differential is in #166, and #91 wires it in. [Bring validation tiers 1–3 within the 2 ms p95 budget](https://github.com/carapai/people-centered-healthcare/issues/170) (#170) is merged: tiers 1–3 take p95 1.12 ms on clinical examples on a laptop; the reference edge hardware run is pending. [Add the CI workflows with pinned, least-privilege actions](https://github.com/carapai/people-centered-healthcare/issues/47) (#47) is merged; [Run Vitest and component tests in frontend-ci](https://github.com/carapai/people-centered-healthcare/issues/173) (#173) is done with #62. [Configure branch protection, merge queue and CODEOWNERS](https://github.com/carapai/people-centered-healthcare/issues/48) (#48) is merged and applied: the repository is public, `main` has a ruleset with every CI job required, and the merge queue needs an organization. [Cross-check tiers 1 and 3 against the HL7 Java validator](https://github.com/carapai/people-centered-healthcare/issues/166) (#166) is merged. [Enforce the urn:ietf:rfc:3986 Identifier value rule](https://github.com/carapai/people-centered-healthcare/issues/176) (#176) is merged. [Build @health/design-system on Panda CSS 2 and Ark UI](https://github.com/carapai/people-centered-healthcare/issues/45) (#45) is merged; the client loads it and its strings are ICU messages. [Build the accessible DataTable component](https://github.com/carapai/people-centered-healthcare/issues/46) (#46) is merged; its NVDA check is #180, which needs Windows. [Generate FHIR R5 TypeScript types and the OpenAPI client types](https://github.com/carapai/people-centered-healthcare/issues/50) (#50) is merged. [Implement FhirClient, command ports and HttpTransport in the client](https://github.com/carapai/people-centered-healthcare/issues/62) (#62) is merged: the FhirClient, the operation client, bearer and DPoP auth, server-sent events and the Query wiring, with one contract suite passing against a central and an edge FhirServer. The per-context command ports wait for their server use cases. [Build tools/synthetic-data](https://github.com/carapai/people-centered-healthcare/issues/51) (#51) is merged: 1M patients generate reproducibly and load through the FHIR API; its loader is HTTP only until a licence decision on TLS root certificates. [Implement the content-addressed blob store](https://github.com/carapai/people-centered-healthcare/issues/60) (#60) is merged. [Build the OIDC/SMART v2 authorization server (central)](https://github.com/carapai/people-centered-healthcare/issues/63) (#63) is merged: `health-identity` and `health-auth`, end to end over a memory store; follow-ups #187, #188 and #189. [Implement signed permission snapshots and RBAC/ABAC](https://github.com/carapai/people-centered-healthcare/issues/64) (#64) is merged. [Implement the policy decision point with query filtering and break-glass](https://github.com/carapai/people-centered-healthcare/issues/65) (#65) is merged; per-request wiring is #192. [Implement audit and provenance](https://github.com/carapai/people-centered-healthcare/issues/66) (#66) is merged. [Implement the Inbound Dead Letter and the quarantine queue](https://github.com/carapai/people-centered-healthcare/issues/167) (#167) is merged. [Implement source Patients, golden records and links](https://github.com/carapai/people-centered-healthcare/issues/68) (#68) is merged; follow-ups #196 (Provenance storage, needs a decision) and #197 (HTTP operations). [Implement the matching engine and Patient/$match](https://github.com/carapai/people-centered-healthcare/issues/69) (#69) is merged. [Store blocking keys and block counts, and serve Patient/$match](https://github.com/carapai/people-centered-healthcare/issues/199) (#199) is merged; the 45M load test is #200 (needs hardware). [Build the registry review queue](https://github.com/carapai/people-centered-healthcare/issues/70) (#70) is merged; its screen is #203. [Implement encrypted logical backup and the round-trip fixture](https://github.com/carapai/people-centered-healthcare/issues/61) (#61) is merged, completing Phase 2. [Persist the authorization server's state in SurrealDB](https://github.com/carapai/people-centered-healthcare/issues/187) (#187) is merged. [Mount the authorization server in central-server, with its signing key and the client sign-in flow](https://github.com/carapai/people-centered-healthcare/issues/188) (#188) is merged (#207, #208). [Apply the policy decision point to every FHIR interaction](https://github.com/carapai/people-centered-healthcare/issues/192) (#192) is merged; follow-ups are #209 (resolving the subject at central and mounting `/fhir`), #210 (break-glass over the API) and #211 (a record's facility). Each task gets its own PR against `main`; no more stacking. Three AFK validation spikes remain open and gate specific tasks ([dependencies](dependencies.md)).

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
- Does the frontend scaffold build on Bun, Vite 8, Panda 2, React Compiler and TS 7? (#38) — every item passes (Bun only, TS 7.0.2, Panda presets explicit); fhirpath.js in lossless precise mode over a gated subset; see ADR 0054, `docs/research/frontend.md`.
- Can SurrealDB blocking indexes serve patient matching at 45M-person scale? (#37) — storage holds; base keys too broad; see `docs/research/patient-identity.md`; raised #146.
- Which default blocking keys and stop-list do the Matching Rules use? (#146) — see ADR 0055, `docs/architecture/patient-identity.md`, `GLOSSARY.md`.

Wayfinder tickets open: three AFK validation spikes, all needing facility hardware. Frontier (open, unblocked, unclaimed), in map order:

1. Does embedded SurrealDB RocksDB survive hard power loss on facility hardware? (#34): AFK task; needs facility hardware
2. Does the per-node commit counter sustain facility write rates on edge hardware? (#36): AFK task; needs edge hardware
3. Does embedded SurrealDB run reliably inside the Tauri process on facility PCs? (#39): AFK task; needs Windows facility PCs

[Resolve each request's subject at central and mount the FHIR API behind it](https://github.com/carapai/people-centered-healthcare/issues/209) (#209) is done: ADR 0056 (#213), the facility at sign-in (#214), and the subject layer with `/fhir` mounted at central (#215). [Expose the client registry's link, unlink and \$merge operations over FHIR](https://github.com/carapai/people-centered-healthcare/issues/197) (#197) is merged (#217); `Patient/$match` over HTTP is #216. [Break-glass over the API](https://github.com/carapai/people-centered-healthcare/issues/210) (#210) is merged (#219); pushing alerts and the privacy officer's review are #218. [Serve Patient/\$match over HTTP](https://github.com/carapai/people-centered-healthcare/issues/216) (#216) is merged (#220). [Name a record's facility and isolate facility-scope records in searches](https://github.com/carapai/people-centered-healthcare/issues/211) (#211) is merged (ADR 0057). [Check new passwords against a breached-password list](https://github.com/carapai/people-centered-healthcare/issues/189) (#189) is merged (#222). [Deliver break-glass alerts and run the privacy officer's review](https://github.com/carapai/people-centered-healthcare/issues/218) (#218) is merged (#223, #225); the edge review clock is #224, which needs sync. [Register a patient](https://github.com/carapai/people-centered-healthcare/issues/226) (#226) is merged (#227). [Build the reception registration screen](https://github.com/carapai/people-centered-healthcare/issues/71) (#71) is merged (#229); its e2e scenario is #228. [Build the registry review screen](https://github.com/carapai/people-centered-healthcare/issues/203) (#203) is merged (#230). [The Playwright end-to-end harness](https://github.com/carapai/people-centered-healthcare/issues/228) (#228) is merged (#231). The rest of Phase 3 needs a person (#67, #180, #200) or a decision (#196). Phase 4 waits on the power-loss spike #34, which needs facility hardware. [Build the statechart interpreter](https://github.com/carapai/people-centered-healthcare/issues/95) (#95) is merged (#232). [Implement the financial ledger](https://github.com/carapai/people-centered-healthcare/issues/119) (#119) is merged (#233). [Implement the stock ledger and balance guards](https://github.com/carapai/people-centered-healthcare/issues/127) (#127) is merged (#234); it unblocks #129 and #130, and #131 with #119. [Implement Login Accounts, Person links and self-registration](https://github.com/carapai/people-centered-healthcare/issues/111) (#111) is split in three. Part 1, the domain, is merged (#236). Part 2a, phone sign-in, is merged (#237). Part 2b, the self-registration use case and its `/registration` API, is merged (#238). Part 2c, staff review of registration requests in the registry review queue, is merged (#239). Part 3, the portal screens, central wiring and e2e, is merged (#240), completing #111. [Implement caregiver delegation](https://github.com/carapai/people-centered-healthcare/issues/113) (#113) part 1, the `health-consent` rules, is merged (#241). Part 2, storing grants with their RelatedPerson and Consent and the staff and portal APIs, is in a PR against `main`. Part 3 is the staff Caregivers panel, the portal's dependents list, and e2e. Next unblocked in Phase 13: transfers and counts (#129), receiving and requisitions (#130), and stock valuation (#131). Patients sign in with a one-time code every time. The SMS provider is #235, which needs a person. Also unblocked in Phase 3: #189 and #71. #196 needs a decision, and #67, #180 and #200 need a person. #180 whenever a Windows machine with NVDA is available. REST writes pass tiers 1 and 2; tier 3 runs once profiles are active (#89, #91). Central mounts `/fhir` behind authorization when its snapshot keys and access policy are configured. Open questions:
- **Ownership:** recorded in resource-ownership.md § In code (the Location, QuestionnaireResponse and Provenance discriminators; the DiagnosticReport default).
- **Search:** the CapabilityStatement's supported-parameter set, UCUM canonicalization (not applied yet), the refused modifiers, chaining and `_has`.
- **History:** `_at` and paging beyond `_count`.

The power-loss (#34), commit-counter (#36) and Tauri-on-facility-PC (#39) spikes need facility hardware.

Research also surfaced legal questions (SurrealDB BSL hosting, Bank of Uganda payment licensing, DPPA consent basis) and validation spikes; these sit in the map's Not yet specified section.

## Details left to the scaffold

Fixed by the Cargo workspace scaffold:
- the toolchain is `1.98.0`, and `rust-version = "1.98"`;
- the layer tag is spelled `domain` (singular), while the directory stays `crates/domains/`;
- the time crate is `jiff` 0.2, used through `health-time`, which provides `Clock`, `SystemClock` and `FixedClock`.

Still open, recorded by the documentation consistency check. Each is an implementation detail to fix when the relevant scaffold lands, not an architectural decision: the directory of `health-node-identity`; the location of reference package sources; event naming and versioning convention; the `.surql` migrations directory and naming; command DTO and OpenAPI output paths; feature-module layout in `apps/client/src/` beyond one folder per Bounded Context under `src/features/`. The persistence sketch was validated with `surreal` 3.2.4; the adapter is pinned to and tested on 3.3.5.

## Known blockers

None.

## Failing tests

None (no code yet).

## Next recommended step

Implement **Create the Cargo workspace with the six-layer layout** (#42), then follow [dependencies.md](dependencies.md).
