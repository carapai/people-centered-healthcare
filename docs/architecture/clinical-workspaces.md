# Clinical and health worker workspaces

This page consolidates the decided foundations for the health worker and clinical workspaces (MASTER_PROMPT §53–56, §98–101). The underlying data, workflow and client decisions are settled. **Workspace UX** (layouts, navigation, density defaults, worklist design) is not, and stays in the map's fog as expected prototype work.

## What the workspaces are built on

| Need (§53–56) | Built on | Source |
|---|---|---|
| Worklists, queues, care tasks, task reassignment, escalation | `worklists` read model over Task projections; claiming, reassigning and escalating are workflow commands; escalation is a timer transition | workflow-engine.md, ADR 0002 |
| Pending investigations, results requiring review, prescriptions, referrals | Tasks from lab, pharmacy and referral Workflow Definitions | workflow-engine.md |
| Clinical inbox: critical and routine results, referral requests, prescription approvals, patient requests, outstanding documentation | `worklists` read model; patient requests arrive as proposals | patient-identity.md |
| Patient summary, allergies, medications, conditions, encounters, programs, vitals | `shared-record` read model through the golden record | patient-identity.md, care-journeys.md |
| Clinical notes (SOAP, admission, procedure, discharge, letters) | Composition / DocumentReference owned by clinical-documentation | resource-ownership.md |
| Signatures and amendments | Signed notes are immutable; changes are amendments that `relatesTo` the original | synchronization.md |
| Forms | Questionnaires with Form Layouts; live behaviour in fhirpath.js; extraction in Rust | questionnaire-extraction.md, ADR 0038 |
| Orders | Command ports into orders, laboratory and medications; never assembled in the UI | client-application.md |
| Decision support | FHIRPath guards and decision tables in workflows; CQL-based CDS is in the fog | ADR 0043 |
| Activity history, provenance | Workflow history, Provenance, AuditEvent | ADR 0044 |
| Notifications | Server-sent or Tauri events carrying invalidation hints; message channels are in the communications fog | client-application.md |
| Handover | Task reassignment plus the worklist read model | workflow-engine.md |

## Roles (§53)

Receptionist, nurse, clinician, pharmacist, laboratory professional, community health worker, facility manager, cashier, insurance officer, storekeeper and procurement officer.
- **Gating.** Each role sees the feature modules its permission snapshot allows, plus what configuration enables (active services and programs).
- **One app.** All roles use the same app and route tree (client-application.md).

## Cross-cutting UX rules already decided

- **Status.** A persistent sync and connectivity status area, per mode (client-application.md).
- **Confirmation.** No optimistic clinical writes. Tauri shows record badges: saved locally, synced, under review, rejected (ADR 0037).
- **Density.** Density modes, high contrast, WCAG 2.2 AA, and 1366×768 PCs and tablets (client-application.md).
- **Shared PCs.** Idle lock after 15 min and PIN user switch, both of which clear the view (offline-auth.md).
- **Emergency.** Emergency bypass in OPD; urgent care is never blocked by a Financial Gate (ADR 0033).

## Open (fog)

- Workspace and inbox UX, worklist layouts, and density defaults per role.
- Notification channels (SMS and push).
- CQL-based clinical decision support.
