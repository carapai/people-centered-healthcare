# Workflow actions are typed Rust registrations, guards are FHIRPath only, and system transitions run as a bounded workflow principal

**Actions.** Each Bounded Context's contract crate declares the workflow actions it accepts as Rust types:
- a versioned id;
- a parameter type, from which a JSON Schema is generated for the Studio;
- a required permission;
- a compensation: `none`, a compensating action, or a human task.

**Using actions.** Definitions reference actions by id plus major version, and publication validates the parameters. Each action invocation is idempotent, keyed by UUIDv5 of instance + transition sequence + action index.

**Guards** are FHIRPath expressions with no side effects, evaluated over a declared context bundle: the context resource, patient, encounter, instance data model and event payload. A built-in **decision-table** construct (rows of FHIRPath conditions → outcomes) carries WHO SMART DAK decision logic. There is no scripting, and no CQL in workflow guards.

**Authorization:**
- User-triggered transitions execute with the user's authorization.
- Timer and system transitions run as a **workflow principal**, limited to the permissions the definition declares, which are approved at publication.

**Compensation.** On cancellation or failure of a scope, the engine runs the compensations of that scope's completed actions in reverse. Steps that cannot be undone (a collected specimen, an administered dose) declare `none`, and the cancellation path records why.

We chose this because §52 forbids arbitrary untrusted code and requires typed actions and explicit authorization. FHIRPath is already a hard dependency for forms and validation, and no production Rust CQL engine is known.

## Considered Options

- **An embedded scripting language (Lua, Rhai, JavaScript) for actions or guards.** Rejected by §52.
- **CQL guards through a sidecar engine.** Rejected for workflow. CQL for clinical decision support remains in the map's fog.
- **System transitions running with full service rights.** Rejected: a misconfigured definition could act beyond what was approved.

## Consequences

- Adding a new kind of action is a code change in the target context's contract and application crates, released with the platform.
- Definitions must declare their data requirements, so that the engine loads only what the guards need.
