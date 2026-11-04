# health-workflow-actions

Layer: **contracts** (shared, no Bounded Context). The workflow action contract ([ADR 0043](../../../docs/adr/0043-typed-actions-fhirpath-guards-and-a-bounded-workflow-principal.md); [workflow-engine.md](../../../docs/architecture/workflow-engine.md) § Typed actions and guards). Every context's contract crate may depend on it, because it has no context of its own.

- **`WorkflowAction`** is what a context's contract crate implements for each action it accepts from workflows:
  - `ID` (`<context>/<action>`) and `MAJOR`: a definition names the action as `<id>@<major>`, and a breaking change takes the next major;
  - `PERMISSION`: the user's on a user-triggered transition, the workflow principal's on a timer or system transition;
  - `COMPENSATION`: `None` (cannot be undone; the cancellation records why), `Action(id)` or `HumanTask`;
  - `params()`: each parameter's name, kind (`Bool`, `Int`, `Text` or `Code` from a list) and whether it is required.
- **`ActionDescriptor::of::<A>()`** is what the workflow domain checks definitions against.
- **`ActionDescriptor::parameter_schema()`** generates the JSON Schema of the parameters for the Studio: an object with the required ones required and nothing else allowed.

## Tests

A unit test declares an action and checks its reference, permission and parameter schema.
