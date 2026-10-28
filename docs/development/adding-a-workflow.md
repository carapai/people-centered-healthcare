# Adding a workflow

> Status: planned layout. No code exists yet; where package sources live in the repository is to be fixed by the scaffold.

## When to use

A new process (OPD flow, lab, pharmacy, referral, program follow-up, procurement approval…) on the generic engine. A workflow is a **Workflow Definition** (a Published Definition), not code. Code is needed only when the definition needs a **typed action** that does not exist yet.

## Steps

1. **Pick the context resource** the instance binds to (Encounter, EpisodeOfCare, CarePlan, ServiceRequest, Appointment, …). It is immutable for the instance.
2. **Write the statechart**: states (`atomic`, `compound`, `parallel`, `final`), transitions (`event` + FHIRPath `guard` + `actions[]`), timers (`after` / `at`), human tasks (role, form `url|version`, worklist key, priority, claim timeout), gates (Financial Gate configuration), invokes (sub-workflows), decision tables, a typed data model and the data requirements the guards read.
3. **Or import** a PlanDefinition and its ActivityDefinitions into a draft, then edit it.
4. **Declare the workflow-principal permissions** used by timer and system transitions; they are approved at publication.
5. **New typed action (code)**, only if needed: a struct in the target context's `health-<context>-contract` implementing `WorkflowAction` with `ID`, `MAJOR`, `PERMISSION` and `COMPENSATION` (or `none`, with a reason). Implement it in the target context's application crate and wire the port in each runtime. Its parameter JSON Schema is generated for the Studio.
6. **Scenarios** in the package `tests/`: event sequences with expected configurations, Tasks, actions and timers.
7. **Migration map** (when replacing a version with running instances): old states → new states, run as an audited admin job. Unmapped instances finish on their old version.
8. **Publish** through the package lifecycle ([configuration.md](../architecture/configuration.md)).

## Expected files

- `<package>/package/<workflow definition>` (representation to be fixed by the scaffold)
- `<package>/package/tests/<scenarios>`
- For a new action: `crates/contracts/health-<context>-contract/src/…`, `crates/application/health-<context>-app/src/…`, runtime wiring

## Contracts

`WorkflowAction` (id, major, permission, compensation); action idempotency key UUIDv5(instance + transition seq + action index); `evaluate_gate` (Finance contract); Task projection (`businessStatus` = state label); `workflow_instance`, `workflow_history`, `workflow_timer`, `workflow_inbox`.

## Tests (publication gates)

- **Workflow:** every scenario passes with stubbed actions; SCXML conformance subset for the interpreter.
- Static checks: no unreachable states, every non-final state has an exit, every action reference and parameter set is valid, every guard type-checks against the data requirements.
- **Unit and domain** for a new action; **Security** for its permission.
- **Offline and sync:** candidate transitions re-evaluated at authority.

## Governing decisions

[ADR 0002](../adr/0002-workflow-owns-tasks.md), [ADR 0042](../adr/0042-engine-native-statecharts-with-plandefinition-as-import.md), [ADR 0043](../adr/0043-typed-actions-fhirpath-guards-and-a-bounded-workflow-principal.md), [ADR 0044](../adr/0044-workflow-instances-history-and-timers-at-the-authority.md), [workflow-engine.md](../architecture/workflow-engine.md).
