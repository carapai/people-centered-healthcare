# Workflow owns every Task; dependencies run one way

The Workflow context owns all FHIR Task and RequestOrchestration resources as projections of its own execution state, because FHIR R5 describes workflow but does not execute it (Task has a fixed status set; `$apply` results are not persisted). Workflow calls domain modules only through typed registered actions; domain modules never call Workflow and instead emit domain events (`SpecimenCollected`, `ResultVerified`, …) that advance workflow instances. Worklists and the clinical inbox are Read Models over Tasks and events.

## Considered Options

- Each domain owns its own Tasks: rejected, it splits workflow state across owners and creates a Workflow ↔ domain cycle.

## Consequences

- Laboratory, pharmacy and other modules remain usable and testable without the engine.
- Reassigning or escalating a worklist item is a workflow command, never a direct Task edit.
