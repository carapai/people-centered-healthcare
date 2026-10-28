# Workflows run as engine-native statecharts in our own Rust interpreter; PlanDefinition is an import source, never the runtime

**Workflow Definitions** are Published Definitions (metadata-owned, `url|version`, same package lifecycle). Each one is a **statechart**:
- atomic, compound, parallel and final states;
- transitions made of event + FHIRPath guard + typed actions;
- entry and exit actions;
- durable timers;
- **human-task** states (role, form, worklist);
- **gate** states (Financial Gates, ADR 0033);
- **invoke** of sub-workflows by `url|version`.

**Interpreter.** Our own pure interpreter in the `health-workflow` domain crate runs these with SCXML-style semantics: run-to-completion, one event queue per instance, transitions chosen by document order, deterministic. A curated subset of the W3C SCXML conformance tests pins its behaviour.

**Simulation.** The Workflow Studio's simulation uses this same engine with stubbed actions. XState remains UI-local.

**PlanDefinition and ActivityDefinition:**
- PlanDefinition is an import and authoring source, converted into a draft Workflow Definition.
- ActivityDefinitions feed a typed "create request from ActivityDefinition" action.
- A definition may cite its source in `relatedArtifact`.

**Contexts.** One engine covers every context (§46–47). An instance binds to exactly one context resource, and that reference never changes:
- Encounter, EpisodeOfCare, CarePlan;
- ServiceRequest, MedicationRequest, Appointment;
- a PatientRequest Task, an AdministrativeRequest;
- procurement or coverage documents.

A patient may have many concurrent instances.

We chose this for three reasons:
- FHIR R5 deliberately defines no execution model, and `$apply` is transient.
- §46 needs nested and parallel states, timers, escalation and compensation, which RequestOrchestration only partly describes.
- Statechart semantics are well specified and testable.

## Considered Options

- **Executing PlanDefinition / RequestOrchestration directly.** Rejected: there is no persistence, timers, queues or compensation, and SMART content is R4 and CQL-based.
- **BPMN with an external engine (Camunda, Flowable).** Rejected: it is Java, runs online-only next to an offline Rust edge, and is a second runtime to operate.
- **XState in TypeScript as the engine.** Rejected: workflows must run on the edge, desktop and central inside Rust transactions.

## Consequences

- No mature Rust SCXML crate is assumed. The interpreter and its conformance subset are our own code.
- Tasks, RequestOrchestration and CarePlan are projections of instance state (ADR 0002). `Task.status` stays within the 12 R5 codes, and the engine state goes in `businessStatus`.
