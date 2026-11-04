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
- **Timers** are recorded, and cancelled when their state is left. `fire_due` fires them (#98):
  - only at the instance's authority: a timer whose instance another node decides is skipped and stays pending, so a desktop in offline fallback never fires an edge-owned timer;
  - by the node clock checked against the clock floor (`with_floor`, a `TimeFloor` port): if the clock reads before the floor, nothing fires (`ClockBroken`);
  - each in its own transaction, which re-reads the timer, marks it fired and runs its event as the workflow principal. A failure keeps it pending, so restarts neither lose nor repeat a firing;
  - escalation (reassign, notify, raise priority) is an ordinary timer transition with its typed actions;
  - the runtime calls it every `SCHEDULER_INTERVAL` (30 s), at most 200 timers per run.
- **Compensation.** `cancel` runs, newest first, the compensation of each completed action:
  - a compensating action, through the port, with the original's parameters;
  - a human task or none: noted on the cancellation's history entry as `human-task.N` or `not-undone.N`, with the reason. #99 projects the human tasks.
- **Definitions** are `Compiled` (statechart and machine), so the application knows each state's human task.
- **Tasks and worklists** (#99):
  - every step rebuilds the instance's Tasks from its state and history (`health_workflow::worklists::project`) and stores them in the worklists' read model, in the same transaction;
  - task states left behind lose their claim fields;
  - `claim`, `release`, `reassign` and `escalate` are workflow commands. Each sets the reserved `task.<state>.*` fields and appends a history entry (`task.claimed`, `task.released`, `task.reassigned`, `task.escalated`). Completing a task is an ordinary `command`;
  - **claim rules:** claiming refreshes your own claim, and is refused while someone else's claim is live. Only the holder releases. Reassignment overrides, and escalation only raises priority;
  - `tasks(instance)` rebuilds an instance's Tasks, and `worklist(key)` lists the open ones most urgent first, then by arrival, with lapsed claims dropped as read;
  - permission checks are the caller's, after the policy decision point.
- **Fails closed:** sub-workflow invocations are refused until the engine supports them, and so are definitions this node does not have.
