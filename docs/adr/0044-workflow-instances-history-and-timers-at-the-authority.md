# Workflow instances keep an append-only history, transitions commit with their actions, and timers fire only at the instance's authority

**Instance record.** Each instance stores:
- the definition `url|version`, pinned for its whole life;
- the context;
- the active state configuration;
- a small typed data model, holding ids and flags only;
- a version.

**History.** Every transition appends a history entry: the event, the from- and to-configuration, the actions run, the actor, the time and the `op_id`.

**Transactions:**
- **User commands** run the transition and its typed actions in one transaction (ADR 0004).
- **Domain events** from other modules are consumed after commit, through an inbox deduplicated by event id. Each is processed in its own transaction.

**Timers** are durable records at the instance's authority node, or the lease holder for leased scope. A scheduler checks for due timers every 30 s and fires them idempotently, using the node clock protected by the clock floor.

**Escalation** (reassign, notify, priority) and lost-to-follow-up are timer transitions.

**Offline.** A desktop in offline fallback runs the engine for instances in its Sync Scope, but never fires timers for edge-owned instances. Its transitions are candidate operations that carry their base configuration. The authority re-evaluates each guard and applies the transition if it is allowed from the current configuration and commutes; otherwise it opens a Conflict Record (ADR 0020).

**Versioning.** Instances never change version implicitly. A new definition version may ship a migration map from old states to new states. An admin then runs an audited migration job, and unmapped instances finish on their old version.

We chose this so that every workflow decision is traceable (§46 history), restart-safe on power loss, and decided in one place, consistent with the Authority Node model (ADR 0012).

## Considered Options

- **Storing only the current state.** Rejected: §46 requires history, and conflict checks need the base configuration.
- **Timers firing on every node holding a replica.** Rejected: escalations and notifications would duplicate.
- **Automatic migration of in-flight instances to a new version.** Rejected: an instance could jump into a state it never validly reached.

## Consequences

- Tasks and worklists are rebuilt from instance state, and claiming a task is a workflow command with a 15-minute idle timeout.
- Fixture scenarios plus static checks for unreachable and stuck states are a publication gate.
