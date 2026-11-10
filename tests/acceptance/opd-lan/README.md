# §132 acceptance: multi-user OPD on the LAN without internet (#106)

> **§132. Phase 8 — OPD clinical vertical slice.** Implement reception, triage, consultation, observations, questionnaires, orders and Encounter completion. Verify multi-user LAN operation without internet.

## What is verified, and where

| Claim | Evidence | Where |
| --- | --- | --- |
| Several browsers on one edge, with no internet, run an OPD visit end to end | Edge e2e `opd-lan.spec.ts`: three enrolled devices (Reception PC, Triage PC, Consultation PC). The edge is the only server they reach | `apps/client/e2e/edge/opd-lan.spec.ts` |
| Registration on the edge | The patient is registered at the edge, with an MRN minted under the edge's own code (`KLA001-E1-nnnnnn`) | same |
| Reception | The patient is found in the edge's store and checked in at triage | same |
| Triage with questionnaires and observations | The OPD triage form (`ug.moh.opd`) is filled on another device. The edge extracts the vital signs as Observations (blood pressure as one panel), with Provenance | same |
| Consultation | A third device sees the extracted vital signs in the patient summary | same |
| Encounter completion | The consultation completes the visit | same |
| Access on the edge is central's | Only enrolled devices and signed-in staff. The bundle's role mappings and label rules apply (#399, #401) | edge e2e `clinical.spec.ts`, `enrollment.spec.ts`; health-fhir-server `authorization.rs` |
| Reconnect and sync, no data loss | What an edge wrote before and while central was unreachable reaches central over the real `/sync/v1` endpoints, and nothing moves on the next round | `crates/edge/health-edge-runtime/tests/sync.rs`: `the_edge_syncs_what_it_wrote_before_and_while_central_was_unreachable` |
| Clinical records survive an offline period | The §117 clinical scenario across a cut link, at the store level | `crates/testing/health-sync-sim/tests/clinical.rs` |
| Central checks what the edge extracted | Central re-derives a completed response's extraction before applying its group (#389) | health-fhir-server `completion.rs`: `the_authority_re_derives_what_a_completed_response_produced`; health-sync-server `http.rs` |

## Results

**2026-11-10** (local, macOS, debug build):

- `bunx playwright test -c playwright.edge.config.ts e2e/edge/opd-lan.spec.ts`: **passed** (3.2 min).
  - Registration with MRN `KLA001-E1-000001`, check-in, triage with temperature 38.6 °C, pulse 104 and BP 112/70 extracted, consultation seeing them, and the visit completed.
  - Axe found no violations on the consultation.
- The whole edge e2e suite: **passed** (9/9).
- `cargo test -p health-edge-runtime --test sync`, `-p health-sync-sim` (clinical): **passed**.

### Found by this run and fixed

- **Records the edge receives by sync weren't indexed for search** (the sync store had no registry), so a triage form sent down by central could not be found. The sync store now indexes with the shared registry (#106, part 4).
- **Under any label rule, searches of unlabelled types found nothing** (`_security` wasn't indexed on Questionnaire, Library, PlanDefinition). This was latent at central too (#403).

## Not yet covered by one run

- **One run across both nodes.** The offline edge run and the reconnect run are separate harnesses. The edge run has no central (its definitions are written as central's sync would bring them), and the sync run uses records rather than browsers. A staging run with a real central and edge, offline then reconnected, is the automated suite testing.md plans for staging.
- **Orders** (lab and pharmacy) from the edge, and workflows on the edge: the edge serves reception, consultation, notes and FHIR, but not yet laboratory, pharmacy or workflow routes.
