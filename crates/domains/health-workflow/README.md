# health-workflow

Layer: **domain** (workflow). The workflow engine's pure statechart interpreter (workflow-engine.md § Statechart model; ADR 0042).

- **The model** (`Statechart`, `State`): atomic, compound, parallel and final states.
  - **Transitions** have event descriptors, a FHIRPath guard, typed actions, and targets, which may be empty for a targetless transition. They can be internal or external, and a transition with no events is eventless.
  - **Each state** has entry and exit handlers (each a separate block), timers (`after` → an event), and invocations of sub-workflows (`url|version`).
  - **The data model** holds typed fields (`Value`) only.
- **`Machine::start` and `Machine::step`** run SCXML's algorithm (W3C SCXML 1.0 Appendix D), without history states:
  - **Selection:** each atomic state, then its ancestors, picks its first enabled transition in document order. Conflicts are removed by exit-set overlap, and a descendant's transition preempts its ancestor's.
  - **Microstep:** exits (reverse document order), then transition content, then entries (document order). Internal transitions are honoured.
  - **Run to completion:** eventless transitions first, then the internal queue.
  - **Completion:** `done.state.<id>` for compound and parallel states. A top-level final state completes the instance.
  - **Invocations** start after the macrostep, only for states still active.
- **Pure and deterministic:**
  - **Effects:** the run returns `Effect`s for the workflow application to carry out in its transaction (ADR 0044): typed actions with an idempotency key (microstep, index), timers to start and cancel, invocations to start and cancel, and completion.
  - **Guards** go through the `Guards` port, since FHIRPath lives outside the domain.
  - **No clock and no I/O.**
- **`Instance`** is the persisted state: the configuration, data model, microstep sequence and the invocations started.

## `definition`

A Workflow Definition (#96; ADRs 0042, 0043, 0049): what a `WorkflowDefinitionLibrary`'s `application/vnd.health.workflow+json` content holds.

- **`WorkflowDefinition`** has:
  - `url` and `version`;
  - the `statechart`, whose `execute` actions name an action as `<id>@<major>`;
  - the `dataModel`'s typed fields;
  - `dataRequirements` (each a resource type and the paths guards read);
  - `decisionTables` (rows of FHIRPath conditions, each with an outcome, and a default);
  - the workflow `principal`'s permissions.
- **`problems(catalog)`** is the publication gate's check against the release's actions (`health-workflow-actions`' `ActionDescriptor`s). It finds:
  - an invalid statechart;
  - a reference that is not `<id>@<major>`, or an unknown action or major;
  - unknown, missing or invalid parameters, including a code the action does not allow;
  - an assignment to an undeclared field, or of another kind;
  - unreachable states, following entry into ancestors, default children and parallel regions;
  - non-final states with no transition on themselves or any ancestor;
  - decision tables with a repeated id, no rows, or an empty condition;
  - principal permissions that no action of the definition needs.
- **No scripting:** guards and conditions are FHIRPath, evaluated through the `Guards` port.
- **Not yet:** type-checking guards against the data requirements comes with workflow simulation (#101).

## Tests

`tests/scxml.rs` is the curated W3C SCXML conformance subset: one test per assertion of §3 that the model has, named by its id in the W3C implementation report manifest (30 assertions: 355, 364, 372, 375, 376, 377, 378, 396, 399, 403, 404, 405, 406, 407, 409, 411, 412, 413, 416, 417, 419, 421, 422, 503, 504, 505, 506, 533, 570, 576). Further tests cover guards in document order, timers, idempotency keys, determinism, instance persistence and invalid definitions.

`definition`'s unit tests cover a valid definition; unknown actions and bad references; each parameter problem; typed assignments; unreachable states and dead ends; an ancestor's transition as a way out inside parallel regions; malformed decision tables; an unneeded principal permission; an invalid statechart; and reading a definition from its JSON.

**Left out:** history states, which the model lacks; error events, since our content cannot fail; and the ECMAScript data model.

## `WorkflowStore`

What the engine persists (#97; ADR 0044), through the unit of work's transaction:

- **`InstanceRecord`:** the definition `url|version` (pinned), the context, the authority node, the interpreter's `Instance`, and a version. `update_instance` is a compare-and-set on the version.
- **`HistoryEntry`:** every transition, appended once at `[instance, seq]`: the event, the from- and to-configuration, the effects, the actor, the time and the `op_id`. Its `seq` is the version the transition produced.
- **`TimerRecord`:** a durable timer and its status (pending, fired, cancelled).
- **The inbox:** `claim_event` records a domain event as processed, and is false if it already was.
