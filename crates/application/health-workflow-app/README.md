# health-workflow-app

Layer: **application** (Workflow). Runs workflow instances through `health-workflow`'s pure interpreter and persists every step through `WorkflowStore` (#97; workflow-engine.md § Persistence and transactions; ADRs 0043, 0044).

- **User commands** run the transition, its typed actions, its timers and its history entry in one transaction. If an action fails, nothing is kept and the command can be retried.
  - `start(definition, context, actor, op_id)`: the instance id is the UUIDv5 of `op_id`.
  - `command(instance, event, actor, op_id)`: refused if the instance has ended, or if the event enables nothing from where it is.
  - `cancel(instance, reason, actor, op_id)`: see compensation below.
- **Domain events** (`deliver(event_id, instance, event)`) are consumed after their own commit, one transaction each, as the workflow principal. An event for an ended instance, or one that enables nothing, is consumed and ignored.
- **Replay is a no-op:** the inbox claims each event id, and each command's `op_id`, once.
- **History** is appended at `[instance, version]`, and each change is a compare-and-set on the version, so of two concurrent steps one fails with `Conflict`.
- **Actions** are carried out through the `Actions` port in the same transaction. Each call is keyed by the UUIDv5 of instance, microstep and index (`action_key`), and must be in the node's action catalogue.
- **Timers** are recorded, and cancelled when their state is left. #98 fires them.
- **Compensation.** `cancel` runs, newest first, the compensation of each completed action:
  - a compensating action, through the port, with the original's parameters;
  - a human task or none: noted on the cancellation's history entry as `human-task.N` or `not-undone.N`, with the reason. #99 projects the human tasks.
- **Fails closed:** sub-workflow invocations are refused until the engine supports them, and so are definitions this node does not have.
