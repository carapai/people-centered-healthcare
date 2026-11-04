-- The workflow engine's records (workflow-engine.md § Persistence and transactions; ADR 0044;
-- #97). Owned by workflow.
-- `workflow_instance`: one row per instance, as JSON written by the workflow application.
-- `version` counts its transitions: each change is a compare-and-set on it.
DEFINE TABLE workflow_instance SCHEMAFULL;
DEFINE FIELD data ON workflow_instance TYPE string;
DEFINE FIELD version ON workflow_instance TYPE int;
DEFINE FIELD definition ON workflow_instance TYPE string;
DEFINE FIELD context ON workflow_instance TYPE string;
DEFINE FIELD running ON workflow_instance TYPE bool;
DEFINE INDEX workflow_instance_context ON workflow_instance FIELDS context;
-- `workflow_history`: every transition, keyed `instance|seq` (seq zero-padded, so ids sort in
-- order). Created once and never changed: appending a taken `seq` fails, and every field is
-- READONLY, so an UPDATE cannot rewrite an entry.
DEFINE TABLE workflow_history SCHEMAFULL;
DEFINE FIELD data ON workflow_history TYPE string READONLY;
DEFINE FIELD instance ON workflow_history TYPE string READONLY;
DEFINE FIELD seq ON workflow_history TYPE int READONLY;
DEFINE INDEX workflow_history_instance ON workflow_history FIELDS instance, seq
  UNIQUE;
-- `workflow_timer`: durable timers, keyed `instance|timer`. #98 fires the pending ones due.
DEFINE TABLE workflow_timer SCHEMAFULL;
DEFINE FIELD data ON workflow_timer TYPE string;
DEFINE FIELD instance ON workflow_timer TYPE string;
DEFINE FIELD status ON workflow_timer TYPE string;
DEFINE FIELD fire_at ON workflow_timer TYPE datetime;
DEFINE INDEX workflow_timer_due ON workflow_timer FIELDS status, fire_at;
DEFINE INDEX workflow_timer_instance ON workflow_timer FIELDS instance;
-- `workflow_inbox`: the domain events consumed, by event id, so a replayed event is a no-op.
DEFINE TABLE workflow_inbox SCHEMAFULL;
DEFINE FIELD processed_at ON workflow_inbox TYPE datetime;
