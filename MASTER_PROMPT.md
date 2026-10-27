# MASTER_PROMPT.md
# FHIR R5-NATIVE HEALTHCARE OPERATING PLATFORM
## Enterprise, National, Facility, Edge, Desktop and Patient-Centred Architecture

**Specification Version:** 2.0  
**Canonical Clinical Standard:** HL7 FHIR R5 (5.0.0)  
**Backend:** Rust 2024, Axum, Tokio, SurrealDB  
**Frontend:** React, TypeScript, Vite, Bun, TanStack  
**Design System:** Ark UI + Panda CSS  
**Desktop:** Tauri v2  
**Offline:** Embedded storage, Facility Edge Server, hierarchical synchronization  
**Development Process:** Wayfinder-first, research-driven, documentation-first, task-by-task implementation

---

# PART 1 — ROLE, VISION AND NON-NEGOTIABLE PRINCIPLES

## 1. Your role

Act as a team of senior specialists:

- Principal software architect.
- Healthcare informatics architect.
- HL7 FHIR R5 architect.
- Senior Rust engineer.
- SurrealDB database architect.
- Distributed systems engineer.
- Edge computing architect.
- Offline synchronization specialist.
- Frontend and design-system architect.
- Clinical workflow architect.
- Clinical terminology specialist.
- Healthcare finance and insurance architect.
- Healthcare inventory and procurement architect.
- Patient engagement specialist.
- Healthcare security and privacy architect.
- DevOps engineer.
- Quality assurance and test architect.
- Technical writer and developer-experience engineer.

Your mission is to research, design, document, plan, and incrementally build a production-oriented, modular, extensible healthcare operating platform.

The system must support day-to-day healthcare delivery rather than functioning merely as a patient record repository or reporting tool.

## 2. Vision

Build a people-centred healthcare operating platform suitable for:

- Government healthcare facilities.
- Private clinics and hospitals.
- Faith-based healthcare facilities.
- NGO-supported healthcare facilities.
- Employer and corporate healthcare services.
- Community healthcare programs.
- Referral networks.
- District and regional health systems.
- National health registries and exchanges.
- Multi-facility health organizations.

The platform must enable:

1. Health workers to perform their daily activities.
2. Patients to initiate and participate in their own healthcare.
3. Facilities to configure their services and operating policies.
4. Administrators to design clinical programs and workflows.
5. Clinical data to follow consistent FHIR R5 semantics.
6. Facilities to continue working without internet.
7. Multiple local computers to share one offline facility database.
8. Approved clinical and operational data to synchronize centrally.
9. Organizations to support free, paid, insured, subsidized and mixed healthcare.
10. Developers to extend the system without rewriting its foundations.

## 3. Non-negotiable principles

### 3.1 FHIR R5-native throughout

FHIR R5 is the canonical healthcare information model throughout the clinical platform, not merely an interoperability format.

Use FHIR R5 to define:

- Clinical resource representation.
- Clinical references.
- Terminology and coded meaning.
- Profiles and constraints.
- Clinical forms and responses.
- Care plans.
- Relevant workflow definitions.
- Clinical interoperability contracts.
- Clinical resource history and provenance.

Do not build a proprietary clinical data model and convert it to FHIR only at the API boundary.

However, use suitable domain-owned models for concepts that FHIR does not adequately represent, including operational workflow execution, synchronization, financial ledgers, inventory transactions and infrastructure configuration.

### 3.2 One shared frontend

Create one React + TypeScript + Vite application.

Use the same application code for:

- Central web access.
- Facility LAN browser access.
- Tauri desktop clients.
- Facility administration.
- Clinical workspaces.
- Patient-facing experiences.

Different deployments may expose different routes, authentication policies, themes, and feature bundles.

Do not create separate clinical React applications for web and desktop.

### 3.3 Configurable by design

Clinical programs, workflows, forms, terminology bindings, facility services, pricing, insurance, resource availability, and inventory policies must be configurable through authorized administration interfaces.

Do not hardcode ART, TB, OPD, government pricing, private pricing, or payer-specific behavior into generic engines.

### 3.4 Strong ownership and low coupling

Use domain-driven design, bounded contexts, hexagonal architecture, and explicit contracts.

Each authoritative resource or transaction has one defined owning domain.

Avoid cross-service database writes and circular dependencies.

### 3.5 Safety, privacy and auditability

Clinical, financial, identity and stock operations must be secure, traceable and recoverable.

Do not sacrifice correctness for superficial configurability.

### 3.6 Offline and distributed operation

Offline support is not a cosmetic feature.

Design it into persistence, authorization, workflows, synchronization, device management, financial transactions and inventory from the beginning.

### 3.7 Sustainable development

Prioritize maintainability, clarity, documented extension paths, incremental testing and automated architecture enforcement.

Do not generate hundreds of empty crates or services.

---

# PART 2 — MANDATORY WAYFINDER-FIRST DEVELOPMENT PROCESS

## 4. Workflow overview

Follow these stages in order:

1. Repository inspection and research.
2. Wayfinder architectural decision mapping.
3. Resolution of blocking decisions.
4. Complete architecture and developer documentation.
5. Complete known-scope implementation backlog.
6. Dependency-ordered, task-by-task implementation.
7. Verification, documentation updates and progress recording.

Do not begin broad feature implementation during the research or planning stages.

## 5. Read the installed Wayfinder skill

Before anything else:

- Inspect the repository.
- Read `AGENTS.md` if present.
- Discover and read the installed Wayfinder skill.
- Follow its actual current operating instructions.
- Identify its decision, dependency, research and completion mechanisms.

Wayfinder is used primarily to resolve architectural decisions.

Implementation tasks are tracked separately in GitHub Issues or the configured project tracker.

Do not fabricate Wayfinder capabilities or claim that decisions have been made when they remain unresolved.

## 6. Research requirements

Research current official documentation and implementation practices for:

### Rust and backend

- Latest compatible stable Rust.
- Rust 2024 Edition.
- Cargo workspaces and resolver 3.
- Axum.
- Tokio.
- Tower.
- Serde.
- HTTP and FHIR server implementation.
- SurrealDB server.
- SurrealDB embedded persistent engines.
- Tauri v2.
- Rust cryptography.
- Event-driven systems.
- Database migrations.
- Observability.
- Testing.

### Frontend

- React.
- TypeScript.
- Bun.
- Vite.
- TanStack Router.
- TanStack Query.
- TanStack Form.
- TanStack Table.
- TanStack Virtual.
- TanStack DB.
- TanStack Store.
- TanStack Pacer.
- TanStack Devtools.
- XState v5.
- Ark UI.
- Panda CSS.
- Zod.

### Healthcare standards

- HL7 FHIR R5.
- SMART on FHIR and applicable authorization patterns.
- FHIR clinical resources.
- FHIR terminology and metadata.
- FHIR clinical workflow resources.
- FHIR financial resources.
- QuestionnaireResponse extraction.
- FHIR resource versioning.
- FHIR search semantics.
- OpenHIE.
- WHO SMART Guidelines.
- DHIS2.
- UgandaEMR and OpenMRS.
- Applicable security and healthcare standards.

### Distributed architecture

- Facility edge computing.
- Offline LAN applications.
- Hierarchical synchronization.
- Distributed identity.
- Facility-local write authority.
- Resource versioning.
- Conflict detection.
- Device enrollment.
- Network discovery.
- Local TLS.
- Backup and restore.
- Failover.
- Database replication alternatives.
- Multi-node operational recovery.

### Clinical and operational domains

- OPD.
- Emergency.
- Inpatient.
- ART.
- TB treatment.
- ANC.
- Laboratory.
- Pharmacy.
- Referrals.
- Patient registration.
- Patient self-service.
- Facility pricing.
- Insurance.
- Payments.
- Inventory.
- Procurement.

Research security, local deployment, costs, performance and operational implications.

Use current primary-source documentation.

Do not invent stable versions or assume feature support.

## 7. Wayfinder architectural decision map

Create a comprehensive decision map covering:

1. FHIR R5-native data model.
2. FHIR Rust model implementation.
3. FHIR profiles and validation.
4. Domain boundaries.
5. Rust crate dependency rules.
6. Service deployment boundaries.
7. SurrealDB resource persistence.
8. Resource versions and history.
9. Clinical reference integrity.
10. Global and facility-specific identifiers.
11. Central resource ownership.
12. Facility-local authority.
13. Desktop-local authority.
14. Central/facility/desktop synchronization.
15. Conflict resolution.
16. Embedded SurrealDB compatibility.
17. Facility Edge Node architecture.
18. LAN client connectivity.
19. Offline authentication.
20. Device trust and certificates.
21. Facility data provisioning.
22. Metadata and terminology.
23. Configuration packaging.
24. Clinical program definitions.
25. Generic workflow engine.
26. Workflow execution persistence.
27. QuestionnaireResponse extraction.
28. Patient identity matching.
29. Patient-initiated workflows.
30. Patient access and caregiver delegation.
31. Clinical worklists.
32. Facility financing.
33. Insurance integration.
34. Pricing rules.
35. Inventory transaction consistency.
36. Payment reconciliation.
37. Shared frontend adapters.
38. Tauri packaging and native services.
39. GitHub Actions.
40. Production operations and recovery.

Record the rationale, alternatives, risks and consequences of each important decision.

Resolve all blocking decisions before implementing the affected components.

## 8. Required architecture documentation

Create:

```text
docs/
├── research/
│   ├── technology-landscape.md
│   ├── rust-and-axum.md
│   ├── fhir-r5.md
│   ├── surrealdb.md
│   ├── embedded-storage.md
│   ├── tauri.md
│   ├── frontend.md
│   ├── metadata-terminology.md
│   ├── clinical-workflows.md
│   ├── facility-edge.md
│   ├── synchronization.md
│   ├── health-financing.md
│   └── inventory.md
├── architecture/
│   ├── overview.md
│   ├── deployment-topology.md
│   ├── service-catalogue.md
│   ├── crate-catalogue.md
│   ├── dependency-rules.md
│   ├── resource-ownership.md
│   ├── fhir-r5-model.md
│   ├── persistence.md
│   ├── terminology.md
│   ├── metadata.md
│   ├── configuration.md
│   ├── program-designer.md
│   ├── workflow-engine.md
│   ├── patient-identity.md
│   ├── patient-portal.md
│   ├── clinical-workspaces.md
│   ├── care-journeys.md
│   ├── facility-configuration.md
│   ├── pricing-and-insurance.md
│   ├── billing-and-payments.md
│   ├── inventory-and-procurement.md
│   ├── facility-edge.md
│   ├── lan-security.md
│   ├── write-authority.md
│   ├── synchronization.md
│   ├── offline-auth.md
│   ├── backup-and-recovery.md
│   ├── security.md
│   ├── deployment.md
│   └── testing.md
├── adr/
├── crates/
├── services/
├── fhir/
├── development/
└── planning/
```

Include:

- System context diagrams.
- Component diagrams.
- Data-flow diagrams.
- Service dependency maps.
- Rust crate dependency maps.
- Resource ownership tables.
- Sequence diagrams.
- Network topologies.
- Sync state diagrams.
- Security boundaries.
- Failure scenarios.
- Deployment diagrams.

Documentation must be internally consistent and actionable.

## 9. Complete implementation backlog

After the documentation is complete:

- Create all foreseeable epics.
- Break epics into implementable tasks.
- Define dependencies.
- Define acceptance criteria.
- Define security and clinical safety requirements.
- Define test requirements.
- Identify expected crates and files.
- Prioritize tasks.
- Create milestones.
- Record architectural decision dependencies.

Use GitHub Issues or the configured tracker.

Create the known-scope backlog before broad feature implementation.

Newly discovered tasks may be added during development.

## 10. Task-by-task implementation

For every task:

1. Read the relevant architecture documents.
2. Read the issue and dependencies.
3. Confirm the owning domain.
4. Implement a focused vertical slice.
5. Add tests.
6. Run checks.
7. Verify contracts.
8. Review security implications.
9. Update documentation.
10. Update the task.
11. Update progress files.

Maintain:

```text
docs/planning/
├── roadmap.md
├── milestones.md
├── task-index.md
├── dependencies.md
├── progress.md
└── current-state.md
```

Ensure future coding-agent sessions can resume without relying on chat history.

---

# PART 3 — TECHNOLOGY STACK AND REPOSITORY

## 11. Rust architecture

Use:

- Rust 2024 Edition.
- Latest compatible stable Rust.
- Cargo resolver 3.
- Axum.
- Tokio.
- Tower.
- Serde.
- SurrealDB Rust SDK.
- Reqwest.
- Rustls.
- tracing.
- OpenTelemetry.
- thiserror.
- anyhow at application boundaries.
- Clap for developer tooling.

Use strongly typed identifiers, explicit error types, stable APIs and appropriate async patterns.

Avoid unnecessary unsafe code, global mutable state, excessive macros and artificial abstractions.

## 12. Frontend stack

Use:

- React.
- TypeScript.
- Vite.
- Bun.
- TanStack Router.
- TanStack Query.
- TanStack Form.
- TanStack Table.
- TanStack Virtual.
- TanStack DB where justified.
- TanStack Store where justified.
- TanStack Pacer where justified.
- TanStack Devtools in development.
- XState v5.
- Ark UI.
- Panda CSS.
- Zod.
- Lucide icons.

Do not use React Hook Form, Formik, Shadcn UI, Tailwind CSS, Ant Design, Chakra UI or Material UI.

Avoid duplicating state between TanStack, XState and application services.

## 13. Repository layout

Use one Cargo workspace and one Bun workspace.

```text
health-platform/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── package.json
├── bun.lock
├── README.md
├── ARCHITECTURE.md
├── CONTRIBUTING.md
├── AGENTS.md
├── .github/
│   ├── workflows/
│   ├── CODEOWNERS
│   ├── ISSUE_TEMPLATE/
│   └── PULL_REQUEST_TEMPLATE.md
│
├── apps/
│   ├── client/
│   │   ├── src/
│   │   ├── src-tauri/
│   │   └── vite.config.ts
│   │
│   └── server/
│       ├── gateway/
│       ├── registry-service/
│       ├── terminology-service/
│       ├── clinical-service/
│       ├── clinical-operations-service/
│       ├── care-coordination-service/
│       ├── public-health-service/
│       ├── business-service/
│       ├── interoperability-service/
│       ├── security-service/
│       ├── workflow-service/
│       ├── analytics-service/
│       ├── platform-service/
│       └── facility-edge/
│
├── crates/
│   ├── foundation/
│   ├── contracts/
│   ├── infrastructure/
│   ├── domains/
│   ├── desktop/
│   ├── edge/
│   └── testing/
│
├── packages/
│   ├── ui/
│   ├── clinical-forms/
│   ├── data-grid/
│   ├── api-client/
│   ├── domain-types/
│   ├── fhir-types/
│   ├── collections/
│   ├── validation/
│   └── platform/
│
├── tools/
│   ├── health-cli/
│   ├── codegen/
│   └── architecture-checks/
│
├── deploy/
│   ├── central/
│   ├── facility-edge/
│   └── desktop/
│
├── docs/
└── scripts/
```

Create crates progressively as implementation requires them.

Document the full planned catalogue before coding.

---

# PART 4 — COMPLETE SERVICE AND CRATE ARCHITECTURE

## 14. Service deployment philosophy

Use modular microservice-ready architecture.

Begin with cohesive deployable applications and explicit domain modules.

Do not turn every domain crate into a separate microservice.

Support later extraction without rewriting core business logic.

Use typed HTTP/FHIR APIs and domain events between services.

Each resource or transaction has one authoritative owning domain.

## 15. Foundation crates

Plan:

- `health-identifiers`
- `health-errors`
- `health-types`
- `health-config`
- `health-time`

## 16. FHIR and contract crates

Plan:

- `health-fhir-models`
- `health-fhir-validation`
- `health-fhir-search`
- `health-fhir-server`
- `health-fhir-mapping`
- `health-sync-protocol`

Put HTTP server runtime functionality in the infrastructure layer even when packaged as a reusable FHIR server crate.

## 17. Infrastructure crates

Plan:

- `health-persistence`
- `health-surrealdb`
- `health-migrations`
- `health-messaging`
- `health-http`
- `health-auth`
- `health-audit`
- `health-observability`
- `health-files`
- `health-crypto`
- `health-sync-server`

## 18. Group A — Master Registries

### `health-client-registry`

Owns:

- Patient identity.
- Patient demographics.
- Master Patient Index.
- Identifiers.
- Duplicate detection.
- Patient matching.
- Identity linkage.
- Merge history.
- Patient registration.

### `health-facility-registry`

Owns organizations, facilities, departments and facility structures.

### `health-workforce-registry`

Owns practitioner and practitioner-role information.

### `health-location-registry`

Owns geographical regions, administrative areas and catchments.

### `health-product-registry`

Owns medicines, devices, vaccines, consumables and product definitions.

### `health-service-catalogue`

Owns healthcare service definitions, facility offerings and service availability.

## 19. Group B — Terminology and Metadata

### `health-terminology`

Owns:

- CodeSystem.
- ValueSet.
- ConceptMap.
- NamingSystem.
- Terminology lookup.
- Validation.
- Expansion.
- Translation.

### `health-metadata`

Owns:

- StructureDefinition.
- FHIR profiles.
- Questionnaire.
- ImplementationGuide metadata.
- Element constraints.
- Metadata package definitions.

Add internal modules or focused crates as justified:

- `health-configuration`
- `health-metadata-validation`
- `health-configuration-publisher`
- `health-questionnaire-engine`
- `health-fhir-extraction`

## 20. Group C — Clinical Records

- `health-shared-record`
- `health-encounters`
- `health-clinical-data`
- `health-medications`

Responsibilities include longitudinal history, encounters, clinical findings, diagnoses, observations, prescribing, dispensing and medication histories.

## 21. Group D — Clinical Operations

- `health-laboratory`
- `health-imaging`
- `health-immunization`
- `health-scheduling`
- `health-patient-flow`

## 22. Group E — Care Coordination

- `health-referrals`
- `health-care-plans`

## 23. Group F — Public Health

- `health-programs`
- `health-surveillance`
- `health-population`

## 24. Group G — Finance and Supply Chain

Plan cohesive modules:

- `health-pricing`
- `health-coverage`
- `health-billing`
- `health-payments`
- `health-financial-ledger`
- `health-inventory`
- `health-procurement`

They may initially be deployed together.

## 25. Group H — Interoperability

Use:

- Gateway.
- `health-integration`.
- `health-messaging`.
- `health-sync-protocol`.
- `health-sync-server`.

Support FHIR R5, legacy FHIR adapters, DHIS2, OpenMRS, UgandaEMR, laboratory systems, insurer APIs, payment gateways and other approved integrations.

## 26. Group I — Security

- `health-identity`
- `health-consent`

## 27. Group J — Workflow and Clinical Intelligence

- `health-workflow`
- `health-decision-support`

## 28. Group K — Analytics

- `health-analytics`
- `health-reporting`

## 29. Group L — Platform

- `health-communications`
- `health-administration`

## 30. Additional patient and operational capabilities

Plan:

- `health-patient-access`
- `health-patient-requests`
- `health-care-journeys`
- `health-worklists`
- `health-patient-engagement`
- `health-clinical-documentation`
- `health-guideline-packages`

Start these as modules where appropriate.

## 31. Desktop crates

- `health-desktop-storage`
- `health-desktop-sync`
- `health-desktop-auth`
- `health-desktop-workflow`
- `health-desktop-devices`
- `health-desktop-files`
- `health-desktop-runtime`

## 32. New Facility Edge crates

Add:

- `health-edge-runtime`
- `health-edge-api`
- `health-edge-storage`
- `health-edge-sync`
- `health-edge-discovery`
- `health-edge-auth`
- `health-edge-admin`
- `health-node-identity`

The Facility Edge Node must reuse the same clinical domain services.

Do not duplicate business logic inside edge-specific crates.

## 33. Testing crates

- `health-test-support`
- `health-contract-tests`
- `health-integration-tests`

---

# PART 5 — FHIR R5 DATA PERSISTENCE

## 34. Canonical resource model

Use FHIR R5 resource-aware storage for clinical data.

Core resources include:

- Patient.
- Person.
- RelatedPerson.
- Practitioner.
- PractitionerRole.
- Organization.
- Location.
- HealthcareService.
- Encounter.
- EpisodeOfCare.
- CarePlan.
- Goal.
- Observation.
- Condition.
- Procedure.
- AllergyIntolerance.
- ServiceRequest.
- MedicationRequest.
- MedicationDispense.
- MedicationAdministration.
- Specimen.
- DiagnosticReport.
- ImagingStudy.
- Appointment.
- Schedule.
- Slot.
- Task.
- Questionnaire.
- QuestionnaireResponse.
- Consent.
- Provenance.
- AuditEvent.
- Composition.
- DocumentReference.

Use FHIR R5 as canonical, with legacy version conversion in explicit adapters.

## 35. Encounter and EpisodeOfCare

Encounter represents an actual healthcare interaction.

EpisodeOfCare represents a broader period of care responsibility.

Examples:

- OPD → Encounter.
- Emergency → Encounter.
- Inpatient → Encounter.
- ART → EpisodeOfCare and CarePlan.
- TB → EpisodeOfCare and CarePlan.
- ANC → longitudinal care context.

A patient may participate in multiple care programs simultaneously.

An Encounter can be associated with multiple EpisodesOfCare.

Do not require EpisodeOfCare for every OPD visit.

## 36. Questionnaire and QuestionnaireResponse

Questionnaire defines the clinical form.

QuestionnaireResponse stores captured form answers.

Preserve:

- Definition canonical URL and version.
- linkIds.
- Answers.
- Author.
- Source.
- Subject.
- Encounter.
- Status.
- Provenance.

## 37. Questionnaire extraction

Use metadata to determine whether an answer generates a clinical resource.

Examples:

- Weight → Observation.
- Blood pressure → Observation with components.
- Laboratory measurement → laboratory-owned Observation.
- Diagnosis → Condition.
- Medication order → MedicationRequest.
- Appointment → Appointment.
- Ordinary narrative answer → QuestionnaireResponse only unless additional representation is justified.

Do not create an Observation for every answer.

Use deterministic extraction identifiers, versioned mappings and provenance.

## 38. Program associations

Observation uses an Encounter reference but does not have a standard direct EpisodeOfCare reference.

Resolve program context through:

- Encounter.episodeOfCare.
- CarePlan.
- Orders.
- Appropriate clinical references.
- Explicit internal program-event associations where needed.

Do not duplicate an Observation just because ART and TB programs both use it.

## 39. Resource persistence

Use complete validated FHIR resource representations and indexed projections.

For example:

```text
observation
  id
  resource
  patient_id
  encounter_id
  code
  effective_at
  status
  version
  created_at
  updated_at
```

Keep resource ownership explicit.

Preserve resource histories, optimistic concurrency and appropriate audit trails.

---

# PART 6 — TERMINOLOGY, METADATA AND PROGRAM CONFIGURATION

## 40. Terminology layer

Terminology defines clinical meaning.

Use:

- CodeSystem.
- ValueSet.
- ConceptMap.
- NamingSystem.
- UCUM integration.
- Authorized national and international terminologies.

## 41. Metadata layer

Metadata defines:

- Structures.
- Profiles.
- Constraints.
- Field types.
- Questionnaires.
- Bindings.
- Canonical references.
- Validation.

Metadata references terminology rather than copying concept definitions.

## 42. Configuration layer

Configuration determines:

- Form layouts.
- Field behavior.
- Workflow placement.
- Rules.
- Service visibility.
- Facility overrides.
- Program configuration.
- Published deployment settings.

Separate clinical standards metadata from application-specific UI presentation metadata.

## 43. Clinical Metadata Studio

Build visual interfaces for:

- Terminology browsing.
- Data element configuration.
- ValueSet authoring.
- Questionnaire creation.
- Form design.
- Extraction mapping.
- Program design.
- Workflow design.
- Rule building.
- Scheduling.
- Version review.
- Approval.
- Publication.
- Simulation.

Use TanStack Form, Ark UI, Panda CSS and XState.

## 44. Clinical Program Designer

Configure programs including:

- ART.
- TB.
- ANC.
- Diabetes.
- Hypertension.
- Immunization.
- Other longitudinal services.

Support:

- Program eligibility.
- Enrollment.
- Stages.
- Clinical activities.
- Forms.
- Regimen templates.
- Investigations.
- Follow-ups.
- Workflow rules.
- Program outcomes.
- Reporting indicators.
- Facility applicability.

Program packages must reference approved clinical protocols and versioned metadata.

Do not hardcode treatment regimens without approved sources.

## 45. Configuration publishing

Implement:

Draft → Validation → Simulation → Review → Approval → Publication → Activation → Supersession.

Publish immutable versioned configuration packages.

Support dependency validation, rollback and historical interpretation.

---

# PART 7 — UNIVERSAL WORKFLOW ENGINE

## 46. Generic workflow engine

Build one reusable Rust workflow engine.

It must support:

- Sequential states.
- Conditional transitions.
- Nested states.
- Parallel states.
- Reusable subprocesses.
- Role assignments.
- Tasks.
- Timeouts.
- Escalation.
- Entry actions.
- Exit actions.
- Compensation.
- Workflow history.
- Workflow simulation.

Do not build a separate engine for OPD, ART, TB or patient self-service.

## 47. Workflow contexts

Support:

- Encounter.
- EpisodeOfCare.
- CarePlan.
- ServiceRequest.
- MedicationRequest.
- Appointment.
- ProgramEnrollment.
- PatientRequest.
- AdministrativeRequest.
- Other explicitly defined contexts.

Workflow state is different from clinical resource status.

## 48. FHIR workflow representation

Use relevant FHIR R5 resources:

- PlanDefinition.
- ActivityDefinition.
- RequestOrchestration.
- Task.
- CarePlan.
- ServiceRequest.
- MedicationRequest.

Use an internal Rust execution model where operational state cannot be adequately represented by FHIR.

## 49. OPD workflow

Example:

Reception → Triage → Consultation → Optional laboratory/pharmacy/admission/referral → Disposition.

Allow:

- Emergency bypass.
- Concurrent orders.
- Repeated consultation.
- Queue movement.
- Follow-up.
- Encounter completion.

## 50. ART and TB workflows

Support:

- Enrollment.
- Assessment.
- Treatment initiation.
- Investigations.
- Follow-up.
- Review.
- Referral.
- Outcomes.

Multiple programs may run concurrently.

## 51. Other workflows

Support:

- Emergency.
- Inpatient.
- Laboratory.
- Imaging.
- Pharmacy.
- Referral.
- Appointment.
- Procurement.
- Insurance approval.
- Payment clearance.
- Patient registration.

## 52. Workflow Studio

Build a visual editor with nodes, connectors, guards, forms, timers, role assignments, actions, branches, reusable subprocesses, simulation and publishing.

Workflow configuration must not execute arbitrary untrusted code.

Use typed registered actions and explicit authorization.

---

# PART 8 — DAILY HEALTH WORKER OPERATIONS

## 53. Health Worker Workspace

Build role-specific daily workspaces.

Roles include:

- Receptionist.
- Nurse.
- Clinician.
- Pharmacist.
- Laboratory professional.
- Community health worker.
- Facility manager.
- Cashier.
- Insurance officer.
- Storekeeper.
- Procurement officer.

Provide:

- Assigned patients.
- Worklists.
- Queues.
- Pending investigations.
- Results requiring review.
- Prescriptions.
- Appointments.
- Referrals.
- Care tasks.
- Handover.
- Clinical inbox.
- Notifications.
- Task reassignment.
- Escalation.
- Activity history.

## 54. Clinical Workspace

Allow a clinician to work through a consultation without unnecessary navigation.

Include:

- Patient summary.
- Allergies.
- Medications.
- Active conditions.
- Encounters.
- Longitudinal programs.
- Vital signs.
- Clinical notes.
- Investigations.
- Results.
- Orders.
- Prescriptions.
- Care plans.
- Follow-up.
- Clinical decision support.
- Document signing.

## 55. Clinical documentation

Support:

- SOAP.
- History and examination.
- Progress notes.
- Admission notes.
- Procedure notes.
- Discharge summaries.
- Clinical letters.
- Signatures.
- Amendments.
- Provenance.

## 56. Clinical inbox

Include:

- Critical results.
- Routine results.
- Referral requests.
- Prescription approvals.
- Clinical messages.
- Patient requests.
- Follow-up activities.
- Outstanding documentation.

---

# PART 9 — PATIENT PARTICIPATION AND SELF-SERVICE

## 57. Patient Workspace

Support:

- Account creation.
- Self-registration.
- Identity verification.
- Appointment booking.
- Pre-visit questionnaires.
- Patient-reported outcomes.
- Symptom reporting.
- Medication refill requests.
- Care requests.
- Demographic updates.
- Health record access.
- Released results.
- Consent.
- Authorized caregiver access.
- Patient education.
- Secure messages.
- Notifications.
- Payments.
- Feedback.

## 58. Identity separation

Keep distinct:

1. Authentication account.
2. Registration request.
3. Identity verification process.
4. FHIR Patient record.
5. Caregiver/dependent relationship.

Do not automatically associate a newly created phone account with an existing Patient record.

## 59. Patient self-registration

Support:

Draft → Submitted → Verification → Matching → Staff review if necessary → Link/Create Patient → Completed.

Include duplicate detection and provisional registrations.

## 60. Patient-initiated workflows

Allow:

- Patients.
- Authorized caregivers.
- Clinicians.
- Staff.
- External systems.
- Scheduled events.
- Clinical results.

To initiate appropriate workflows.

Patient requests do not automatically become authorized clinical orders.

## 61. Care Journey

Build a longitudinal patient-centred projection linking:

- Registration.
- Appointments.
- Encounters.
- Programs.
- Orders.
- Tasks.
- Results.
- Medication activities.
- Referrals.
- Follow-up.
- Communication.

Preserve clinical resource ownership.

## 62. Patient-reported information

Patient-submitted data must retain provenance and verification status.

Do not overwrite clinician-verified information automatically.

---

# PART 10 — FACILITY CONFIGURATION, FINANCE AND INSURANCE

## 63. Facility operating models

Support:

- Government/free.
- Private/self-pay.
- Insurance.
- Insurance with copayment.
- Donor funding.
- Government programs.
- Employer funding.
- Subsidies.
- Waivers.
- Credit.
- Mixed funding.

The same facility may use multiple models.

## 64. Configuration scope

Support configuration at:

- Tenant.
- Organization.
- Facility.
- Department.
- Service.
- Program.
- Payer contract.
- Patient eligibility.
- Transaction.

Use explicit policy precedence and audit.

## 65. Service Catalogue

Define services independently of prices.

Support:

- OPD.
- Laboratory.
- Imaging.
- Pharmacy.
- Procedures.
- Inpatient.
- Other services.

## 66. Pricing Engine

Support:

- Base prices.
- Facility-specific prices.
- Department tariffs.
- Insurance contract prices.
- Service bundles.
- Discounts.
- Subsidies.
- Waivers.
- Surcharges.
- Effective dates.
- Quantity-based pricing.
- Multiple currencies where required.

Use exact decimal monetary calculations.

Preserve historical pricing decisions.

## 67. Insurance and Coverage

Support:

- Multiple payers.
- Eligibility verification.
- Primary/secondary insurance.
- Copayments.
- Deductibles.
- Coverage limits.
- Exclusions.
- Preauthorization.
- Claims.
- Responses.
- Denials.
- Appeals.
- Reconciliation.
- Government funding.
- Donor funding.

## 68. Payments

Support:

- Cash.
- Mobile money.
- Bank transfer.
- Card provider integration.
- Deposits.
- Partial payments.
- Credit.
- Receipts.
- Refunds.
- Reversals.
- Settlements.
- Reconciliation.

Do not treat provisional offline payments as externally confirmed.

## 69. Relevant FHIR resources

Use applicable FHIR R5 financial resources:

- Coverage.
- CoverageEligibilityRequest.
- CoverageEligibilityResponse.
- Claim.
- ClaimResponse.
- ExplanationOfBenefit.
- Account.
- ChargeItem.
- ChargeItemDefinition.
- Invoice.
- PaymentNotice.
- PaymentReconciliation.

Use internal accounting ledgers where appropriate.

## 70. Clinical and financial separation

Clinical workflows must not contain hardcoded payment assumptions.

Financial steps are configurable.

Emergency and clinically urgent care must support appropriate bypass or deferral where required.

Free care must still track actual resource consumption and funding.

---

# PART 11 — INVENTORY AND PROCUREMENT

## 71. Inventory

Support:

- Multiple warehouses.
- Multiple stores.
- Pharmacy stores.
- Laboratory stores.
- Ward stock.
- Product masters.
- Batches.
- Expiry dates.
- Receipts.
- Issues.
- Transfers.
- Returns.
- Adjustments.
- Reservations.
- Stock counts.
- Reorder policies.
- FEFO selection.
- Donor-funded stock.
- Restricted stock.
- Consignment.
- Stock valuation.

Use a durable stock movement ledger.

Do not rely solely on editable stock balance fields.

## 72. Procurement

Support:

- Supplier registry.
- Requisitions.
- Quotations.
- Purchase orders.
- Approval workflows.
- Goods receiving.
- Supplier invoices.
- Returns.
- Procurement reporting.

## 73. Clinical stock integration

A MedicationRequest should not directly subtract stock.

Use appropriate dispensing and inventory transactions.

Track actual stock movement based on fulfillment.

Support stock availability checks, reservations, substitutions under policy, and reconciliations.

---

# PART 12 — FOUR RUNTIME AND DEPLOYMENT MODES

## 74. Deployment mode A — Central Online Web

Architecture:

React → Central API Gateway → Rust Services → Central SurrealDB.

Requirements:

- Requires central connectivity.
- No persistent browser clinical database.
- No clinical IndexedDB or OPFS.
- No offline mutation queue.
- Transient in-memory caching only.
- Server-confirmed writes.

## 75. Deployment mode B — Standalone Offline Tauri

Architecture:

React → Tauri Commands → Local Rust Services → Embedded Database.

Requirements:

- Works offline after authorized provisioning.
- Persists clinical records locally.
- Supports approved workflows.
- Maintains a durable outbox.
- Synchronizes with central services.
- Supports offline authentication policies.
- Uses native security capabilities.

Use an embedded SurrealDB engine only after validation.

## 76. Deployment mode C — Facility Edge Server

A facility can designate one computer as a local server.

It must:

- Run independently of a UI window.
- Serve multiple clients over Ethernet or Wi-Fi.
- Operate without internet.
- Use Rust/Axum APIs.
- Use persistent local storage.
- Enforce local authentication and authorization.
- Host facility-scoped data.
- Run clinical and operational workflows.
- Support local pricing and inventory.
- Synchronize centrally when connectivity returns.

Create:

```text
apps/server/facility-edge/
```

The Facility Edge Node must be able to run as an operating-system service.

Support Windows and Linux according to verified platform capabilities.

## 77. Deployment mode D — Facility LAN Client

LAN clients may be:

- Browser clients.
- Tauri desktop clients.
- Authorized kiosks.
- Other approved devices.

LAN browser clients call the Facility Edge API and have no independent offline clinical database.

They work without internet as long as the LAN and facility server are available.

Tauri clients may have optional independent offline fallback.

## 78. Shared React application requirement

All runtime modes use the same application.

Use platform adapters:

```text
ClinicalRepository
  CentralHttpRepository
  FacilityLanHttpRepository
  TauriLocalRepository
```

Avoid scattered `isTauri` checks in clinical UI components.

Runtime selection should occur at the application composition boundary.

---

# PART 13 — FACILITY EDGE SERVER ARCHITECTURE

## 79. Edge Node process

Create a standalone Rust executable that can run as:

- Windows service.
- Linux systemd service.
- Development process.
- Managed edge appliance process.

It must not depend on a logged-in user's Tauri application remaining open.

## 80. Edge responsibilities

The Edge Node must provide:

- Local API gateway.
- Local authentication.
- Facility-scoped authorization.
- Local FHIR R5 resource handling.
- Workflow execution.
- Local metadata.
- Local terminology.
- Facility patient registry operations.
- Encounter management.
- Clinical documentation.
- Laboratory workflow.
- Pharmacy workflow.
- Billing and pricing.
- Inventory.
- Local appointments.
- Durable events.
- Local audit.
- Central synchronization.
- Backup and recovery.

Use shared Rust domain crates.

## 81. Edge database ownership

One facility node process owns the embedded database engine.

Clients connect through APIs.

Do not allow browser clients, desktop clients or other processes to open and modify the facility database directly.

Investigate concurrency, file locking, schema migrations, backups and recovery.

## 82. LAN transport

Support:

- Ethernet.
- Wi-Fi.
- Authenticated HTTPS.
- Device enrollment.
- Server discovery.
- Certificate validation.
- Configurable addresses.
- Stable facility node identity.

Do not expose a raw database administration API to ordinary clients.

## 83. Local authentication

Research and implement:

- Provisioned staff identities.
- Offline-capable login.
- Cached permission snapshots.
- Configurable authorization expiry.
- Session locking.
- Shared-workstation controls.
- Device revocation.
- Account lockout.
- Audit.
- Server reconciliation on reconnect.

Do not allow indefinitely valid cached permissions without explicit governance.

## 84. Edge administration

Build an Edge Administration workspace supporting:

- Node registration.
- Facility assignment.
- Device pairing.
- User provisioning.
- Node health.
- Database health.
- Storage capacity.
- Connected clients.
- Sync progress.
- Sync errors.
- Conflict queues.
- Last successful synchronization.
- Software versions.
- Metadata versions.
- Backup status.
- Restore workflow.
- Recovery diagnostics.

## 85. Edge software updates

Support:

- Versioned release packages.
- Signed updates.
- Compatibility verification.
- Safe database migrations.
- Rollback where feasible.
- Update scheduling.
- Health validation after updates.

Do not automatically apply incompatible migrations.

---

# PART 14 — HIERARCHICAL SYNCHRONIZATION AND DATA AUTHORITY

## 86. Preferred topology

Use:

Central Platform ↔ Facility Edge Node ↔ Optional independently offline Tauri clients.

Standalone Tauri may synchronize directly to central services when no Facility Edge Node is assigned.

LAN browser clients do not synchronize independently.

Avoid unconstrained peer-to-peer synchronization among all clients.

## 87. Data authority matrix

Create an explicit matrix specifying:

- Resource type.
- Owning service.
- Global or facility scope.
- Allowed creators.
- Allowed offline changes.
- Write authority.
- Synchronization direction.
- Conflict strategy.
- Required central validation.

Examples:

| Data | Local behavior | Central coordination |
|---|---|---|
| Patient registration | Provisional creation allowed | Identity matching and reconciliation |
| Encounter | Facility-local creation | Replicate authorized records |
| Observation | Local capture allowed | Versioned replication and clinical rules |
| QuestionnaireResponse | Local capture allowed | Idempotent synchronization |
| Facility stock | Facility stock ledger authoritative for its scope | Central aggregates and reconciles |
| Insurance eligibility | Cached estimate or pending verification | Confirm through payer |
| Payment confirmation | Provisional unless locally verifiable | External reconciliation |
| Metadata | Use approved snapshot | Publish and distribute new versions |

The architecture must distinguish global identity authority from delegated local operational authority.

## 88. Sync protocol

Implement:

- Node IDs.
- Device IDs.
- Resource IDs.
- Operation IDs.
- Version identifiers.
- Change tracking.
- Durable outbox.
- Incremental pull.
- Incremental push.
- Cursors.
- Retry.
- Deduplication.
- Idempotency.
- Tombstones.
- Provisional states.
- Conflict records.
- Audit trails.
- Attachment synchronization.

Use version vectors or alternative concurrency metadata only where research shows they are justified.

## 89. Conflict policies

Do not use universal last-write-wins.

Different resources require different policies:

- Demographic identity conflicts → review and reconciliation.
- Clinical observations → version-safe corrections.
- Signed notes → amendment rather than silent overwrite.
- Workflow states → authorized transition reconciliation.
- Stock → transaction ledger reconciliation.
- Payments → idempotent financial reconciliation.
- Metadata → approved immutable version activation.

## 90. Tauri desktop ↔ Facility Edge reconciliation

When a desktop client works independently offline and reconnects:

1. Authenticate and re-establish node trust.
2. Check authorization.
3. Exchange changes through the defined sync protocol.
4. Detect duplicate operations.
5. Reconcile resource versions.
6. Validate clinical operations.
7. Reconcile financial and stock operations.
8. Record conflicts.
9. Update the local data scope.
10. Resume normal LAN-connected operation.

Do not silently switch write authorities while pending operations are unresolved.

## 91. Synchronization safety

Support:

- Power-loss recovery.
- Crash recovery.
- Interrupted uploads.
- Network retries.
- Partial batch processing.
- Recovery after long offline periods.
- Metadata compatibility checks.
- Remote revocation.
- Audit and traceability.

---

# PART 15 — BACKUPS, HARDWARE FAILURE AND BUSINESS CONTINUITY

## 92. Facility server failure

Research strategies for:

- Local backups.
- Encrypted backup storage.
- Backup verification.
- Restore testing.
- Replacement hardware.
- Recovery of unsynchronized data.
- Controlled failover.
- Standby nodes.

Do not operate multiple independent writable facility databases without a proven leader/replication strategy.

## 93. Power interruptions

Ensure:

- Durable local transactions.
- Safe database shutdown.
- Automatic service restart.
- Outbox recovery.
- Storage monitoring.
- Backup scheduling.
- UPS-compatible deployment.

## 94. Disaster recovery

Define:

- Recovery procedures.
- Backup retention.
- Encryption and key recovery.
- Recovery point objectives.
- Recovery time objectives.
- Central and edge restoration.
- Post-recovery reconciliation.
- Data integrity validation.

---

# PART 16 — SECURITY, PRIVACY AND GOVERNANCE

## 95. Security

Implement:

- OIDC/OAuth2 where suitable.
- RBAC.
- ABAC.
- Organization scoping.
- Facility scoping.
- Patient-level access.
- Purpose-of-use checks.
- Consent.
- AuditEvent.
- Provenance.
- Break-glass access.
- Session management.
- Device trust.
- Encryption in transit.
- Encryption at rest.
- Secure key management.
- Secure backups.
- Logging and monitoring.

Enforce permissions server-side and in local trusted Rust services.

## 96. Confidentiality

Support sensitive-program access restrictions, including appropriately protected HIV, TB and other confidential records.

Apply policies consistently to:

- Clinical workspaces.
- Patient portal.
- Facility Edge Node.
- Sync.
- Analytics.
- External integrations.
- Caregiver access.

## 97. Facility isolation

Define strict facility and organization scopes.

Do not expose unrelated patient records merely because multiple facilities share the central platform.

---

# PART 17 — REACT APPLICATION AND UX

## 98. Application areas

Use one React application with feature areas:

- Clinical.
- Reception.
- Triage.
- Laboratory.
- Pharmacy.
- Finance.
- Inventory.
- Procurement.
- Administration.
- Program Studio.
- Workflow Studio.
- Metadata Studio.
- Patient Portal.
- Edge Administration.

## 99. UI design system

Create a reusable Ark UI + Panda CSS design system.

Support:

- Accessibility.
- Keyboard navigation.
- Responsive layouts.
- Clinical density modes.
- Light/dark modes.
- Component recipes.
- Semantic tokens.
- Consistent forms.
- Data tables.
- Dialogs.
- Menus.
- Notifications.

## 100. Form engine

Use TanStack Form.

Build reusable controls for:

- Text.
- Numbers.
- Dates.
- Quantities.
- Codes.
- ValueSets.
- Diagnoses.
- Medications.
- Practitioners.
- Facilities.
- Repeatable groups.
- Nested fields.
- Conditional questions.

Use FHIR Questionnaire for relevant clinical forms.

## 101. Patient and health worker usability

Support:

- Role-aware navigation.
- Context-aware quick actions.
- Patient timelines.
- Worklists.
- Clinical summaries.
- Autosave drafts where appropriate.
- Explicit finalization and signing.
- Minimal duplicate entry.
- Clear offline/online status.
- Clear sync and conflict status.
- Accessible patient interfaces.
- Low-bandwidth behavior.

---

# PART 18 — ANALYTICS, PUBLIC HEALTH AND REPORTING

## 102. Analytics

Support:

- Indicators.
- Cohorts.
- Numerators.
- Denominators.
- Targets.
- Actuals.
- Disaggregation.
- Time periods.
- Aggregation.
- Data quality rules.
- Reporting datasets.

## 103. Program indicators

Support configurable indicators for ART, TB and other programs using flattened or indexed projections of FHIR clinical resources.

Do not require users to write SQL or Rust for routine indicators.

## 104. Reporting

Support:

- Clinical dashboards.
- Facility operational dashboards.
- Finance dashboards.
- Stock dashboards.
- Patient follow-up reports.
- Public health surveillance.
- Donor reporting.
- DHIS2 aggregate and tracker integrations.

## 105. Patient timeline

Create an authorized patient activity timeline spanning Encounters, EpisodesOfCare, Observations, orders, medication, referrals and communications.

It is a projection, not a replacement for authoritative resources.

---

# PART 19 — MAINTAINABILITY AND DEVELOPER EXTENSIBILITY

## 106. Dependency rules

Domain crates must not depend directly on Axum, Tauri, React or SurrealDB.

Use ports/adapters.

Application services orchestrate use cases.

Infrastructure implements persistence, messaging and external integrations.

Presentation layers handle HTTP and UI.

## 107. Development guides

Create:

```text
docs/development/
├── getting-started.md
├── architecture-rules.md
├── adding-a-domain.md
├── adding-a-crate.md
├── adding-a-feature.md
├── adding-a-fhir-resource.md
├── adding-an-api.md
├── adding-a-migration.md
├── adding-an-event.md
├── adding-a-form.md
├── adding-a-workflow.md
├── adding-a-program.md
├── adding-a-price-rule.md
├── adding-a-payer.md
├── adding-a-stock-operation.md
├── adding-a-tauri-command.md
├── adding-an-edge-api.md
├── adding-a-sync-resource.md
├── adding-a-ui-component.md
├── adding-an-integration.md
└── testing.md
```

Every guide must identify expected files, contracts and tests.

## 108. Development CLI

Plan tools such as:

```text
health-cli new domain
health-cli new feature
health-cli new workflow
health-cli new migration
health-cli new event
health-cli new integration
health-cli check architecture
```

Implement only justified commands.

## 109. Architecture enforcement

Automatically detect:

- Circular dependencies.
- Forbidden imports.
- Cross-domain database access.
- Duplicate resource ownership.
- Desktop imports in web builds.
- Contract inconsistencies.
- Missing migrations.
- Required tests.

---

# PART 20 — GITHUB ACTIONS, CI/CD AND RELEASES

## 110. Git standards

Use:

- Protected main branch.
- Short-lived branches.
- Pull requests.
- Conventional Commits.
- CODEOWNERS.
- ADR review.
- Lockfiles.
- Secure dependency updates.

## 111. GitHub Actions

Create:

```text
.github/workflows/
├── rust-ci.yml
├── frontend-ci.yml
├── integration-tests.yml
├── security.yml
├── dependency-review.yml
├── desktop-build.yml
├── edge-build.yml
├── release.yml
└── documentation.yml
```

## 112. Rust checks

Run:

- Formatting.
- Clippy.
- Compilation.
- Unit tests.
- Integration tests.
- Documentation checks.
- Architecture checks.

## 113. Frontend checks

Run:

- Bun frozen-lockfile install.
- TypeScript checks.
- Lint.
- Panda generation.
- Unit tests.
- UI tests.
- Production web build.

## 114. Edge and desktop builds

Create verified build matrices for supported Windows, Linux and macOS targets.

Build the Facility Edge Node independently from the Tauri UI.

Use signing and protected release processes where applicable.

## 115. Security

Use maintained actions pinned to reviewed versions or commit SHAs.

Run dependency audits, secret scanning and suitable security tests.

Do not invent action SHAs or pretend releases are signed when signing has not been configured.

---

# PART 21 — TESTING AND ACCEPTANCE

## 116. Test strategy

Include:

- Unit tests.
- Domain tests.
- Repository tests.
- FHIR conformance tests.
- API contract tests.
- Workflow tests.
- Metadata tests.
- Terminology tests.
- Frontend tests.
- End-to-end tests.
- Offline tests.
- Sync tests.
- Security tests.
- Payment reconciliation tests.
- Stock concurrency tests.
- Recovery tests.

Use synthetic patient data.

## 117. Clinical acceptance scenario

Demonstrate:

1. Patient self-registers.
2. Identity is matched or reviewed.
3. Appointment is requested.
4. Patient checks in.
5. Reception opens the Encounter.
6. Triage records observations.
7. Clinician documents assessment.
8. Laboratory receives an order.
9. Result is recorded and verified.
10. Clinician reviews the result.
11. Medication is prescribed.
12. Pharmacy dispenses.
13. Encounter is completed.
14. Patient receives follow-up information.

## 118. ART and TB scenario

Demonstrate:

- ART enrollment.
- TB enrollment.
- Concurrent care.
- Configurable forms.
- Treatment workflows.
- Shared observations.
- Follow-up.
- Program outcome reporting.

## 119. Finance scenario

Demonstrate:

- Free government OPD.
- Cash private consultation.
- Fully insured encounter.
- Insurance with copayment.
- Donor-funded service.
- Fee waiver.
- Mixed funding.
- Emergency payment deferral.
- Refund.
- Insurance claim denial.
- Historical invoice preservation.

## 120. Inventory scenario

Demonstrate:

- Stock receipt.
- Store transfer.
- Reservation.
- Dispensing.
- Expiry blocking.
- Stock adjustment.
- Reorder.
- Concurrent stock updates.
- Offline reconciliation.

## 121. Facility LAN offline scenario

Mandatory demonstration:

1. Deploy a Facility Edge Node to one computer.
2. Connect several computers over Ethernet or Wi-Fi.
3. Disconnect the facility internet.
4. Log in with authorized local accounts.
5. Register a patient.
6. Start an Encounter.
7. Record triage observations.
8. Review the updated patient chart from another LAN computer.
9. Submit laboratory orders.
10. Enter and verify results.
11. Dispense medication.
12. Update shared stock.
13. Apply facility pricing and funding rules.
14. Complete clinical workflow tasks.
15. Restart the Facility Edge Node.
16. Verify that committed data survives.
17. Restore internet connectivity.
18. Synchronize authorized changes centrally.
19. Verify that retries do not duplicate records.
20. Verify that conflicts are explicitly reported.
21. Confirm unauthorized LAN devices are denied access.
22. Restore an encrypted backup successfully.

## 122. Independent desktop scenario

Demonstrate:

- Offline work away from the LAN.
- Local persistence.
- Pending changes.
- Reconnection to the Facility Edge Node.
- Correct resource reconciliation.
- Clinical conflict handling.
- Stock and financial safeguards.
- Optional direct central synchronization when operating in standalone mode.

## 123. Reliability

Include:

- Health checks.
- Readiness checks.
- Tracing.
- Metrics.
- Monitoring.
- Backup validation.
- Disaster recovery tests.
- Disk-full recovery.
- Power-loss recovery.
- Sync interruption recovery.
- Safe schema updates.
- Controlled software updates.

---

# PART 22 — IMPLEMENTATION ROADMAP

## 124. Phase 0 — Research and Wayfinder

Complete repository analysis, technology research, Wayfinder decisions, documentation, ADRs and the known-scope implementation backlog.

## 125. Phase 1 — Monorepo foundations

Implement Cargo and Bun workspaces, shared React application, Rust gateway, UI design system, CI and architecture checks.

## 126. Phase 2 — Persistence and runtime adapters

Implement central and local persistence, migrations, API adapters and shared frontend repository interfaces.

## 127. Phase 3 — Identity, security and Client Registry

Implement minimum viable authentication, authorization, patient records, MPI and registration.

## 128. Phase 4 — Facility Edge Node foundation

Implement:

- Edge executable.
- Facility-local persistence.
- LAN API.
- Local user access.
- Node identity.
- Facility assignment.
- Service management.
- Health monitoring.

Build this early so later clinical slices can prove true multi-user offline operation.

## 129. Phase 5 — Synchronization foundation

Implement edge-to-central synchronization, device provisioning, outbox, cursors, idempotency, resource scopes and initial conflict handling.

## 130. Phase 6 — Terminology and metadata

Implement FHIR terminology, profiles, questionnaires, metadata publishing and local snapshot activation.

## 131. Phase 7 — Universal workflow engine

Implement workflow definitions, runtime, instances, states, tasks, guards and actions.

## 132. Phase 8 — OPD clinical vertical slice

Implement reception, triage, consultation, observations, questionnaires, orders and Encounter completion.

Verify multi-user LAN operation without internet.

## 133. Phase 9 — Clinical services

Implement documentation, medication management, laboratory workflows, clinical summaries and care plans.

## 134. Phase 10 — Patient engagement

Implement self-registration, identity linkage, appointments, patient requests, patient portal and care journeys.

## 135. Phase 11 — Clinical Program Studio

Implement program definition, ART, TB, ANC and longitudinal care workflow configuration.

## 136. Phase 12 — Finance and insurance

Implement facility financing policies, price books, coverage, billing, payments, claims and reconciliation.

## 137. Phase 13 — Inventory and procurement

Implement stores, stock movements, batches, reservations, procurement and dispensing integration.

## 138. Phase 14 — Additional clinical operations

Implement emergency, inpatient, imaging, immunization, referrals and expanded facility workflows.

## 139. Phase 15 — Public health and analytics

Implement cohorts, indicators, surveillance, dashboards, reporting and DHIS2 integration.

## 140. Phase 16 — Advanced desktop offline mode

Extend standalone Tauri and desktop-to-edge reconciliation, including safe independent offline operation.

Basic Tauri runtime support must be established earlier.

## 141. Phase 17 — Production hardening

Complete performance, security, offline stress, disaster recovery, user acceptance, deployment and operational readiness testing.

Use vertical slices across phases where necessary.

Do not postpone essential security, persistence or authorization controls until late phases.

---

# PART 23 — CODING AGENT EXECUTION CONTRACT

## 142. Before each task

- Read AGENTS.md.
- Read applicable Wayfinder decisions.
- Read the task.
- Verify dependencies.
- Review relevant architecture docs.
- Identify domain ownership.
- Identify tests and risks.

## 143. During implementation

- Use stable verified APIs.
- Follow architectural dependency rules.
- Keep feature changes focused.
- Reuse existing abstractions.
- Implement tests.
- Preserve FHIR R5 semantics.
- Respect clinical safety.
- Respect financial consistency.
- Preserve offline guarantees.
- Avoid creating unnecessary services.

## 144. After implementation

- Format.
- Compile.
- Lint.
- Test.
- Validate contracts.
- Check migrations.
- Review security.
- Review diff.
- Update documentation.
- Update the issue.
- Record progress.

Do not claim checks passed unless they were actually executed successfully.

## 145. Session continuity

Update:

```text
docs/planning/current-state.md
```

Include:

- Completed tasks.
- Current task.
- Outstanding decisions.
- Known blockers.
- Relevant implementation links.
- Failing tests.
- Next recommended unblocked task.

## 146. Architecture consistency

Before every milestone, ensure alignment among:

- Wayfinder decisions.
- ADRs.
- Master prompt.
- Crate catalogue.
- Service ownership.
- FHIR R5 model.
- Workflow design.
- Metadata design.
- Financial model.
- Offline sync topology.
- Tests.

Resolve contradictions instead of allowing parallel incompatible designs to accumulate.

---

# PART 24 — FINAL SUCCESS CRITERIA

## 147. The platform is successful when

1. Health workers use it to perform real daily clinical work.
2. Patients can initiate and participate in healthcare activities.
3. Clinical data follows FHIR R5 semantics throughout.
4. Clinical programs are metadata-configurable.
5. OPD, ART, TB and other workflows reuse one workflow engine.
6. Questionnaires preserve responses and generate appropriate clinical resources through validated extraction.
7. Facilities configure free, paid, insured and mixed services.
8. Prices, payments and insurance decisions are traceable.
9. Inventory and procurement are auditable.
10. Multiple staff members work concurrently over a facility LAN with no internet.
11. Facility data synchronizes centrally when connected.
12. Standalone Tauri clients work offline within authorized scopes.
13. Conflicts are handled explicitly rather than silently overwritten.
14. Security and confidentiality are enforced.
15. The shared React app works across all supported modes.
16. Developers can add features using clear documented extension paths.
17. CI/CD and automated tests prevent architectural regressions.
18. The platform can support healthcare facilities of different sizes and business models.

---

# FINAL INSTRUCTION TO THE CODING AGENT

Read this entire master specification.

Read and follow the installed Wayfinder skill.

Inspect the repository.

Research the current technical ecosystem and the healthcare standards.

Create a complete architectural decision map, paying particular attention to FHIR R5-native persistence, SurrealDB, shared React/Tauri runtimes, Facility Edge Servers, hierarchical synchronization, resource ownership, metadata, workflows, security, finance and inventory.

Resolve all blocking architectural decisions.

Write the architecture documents, diagrams, ADRs, crate catalogue, service ownership matrices, security plans, developer guides, deployment topology and synchronization design.

Create the complete known-scope GitHub Issues backlog with dependencies, milestones, acceptance criteria and tests.

Validate that all documentation and planned work are consistent.

Only then begin implementing the highest-priority unblocked task.

Work incrementally, implement focused vertical slices, test rigorously, document changes and maintain session continuity.

**Do not build a collection of disconnected applications. Build one coherent, modular, secure, offline-capable, FHIR R5-native healthcare operating platform that supports facilities, health workers, patients, administrators and national health systems.**
