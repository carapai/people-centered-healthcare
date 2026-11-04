-- The worklists' read model (workflow-engine.md § Worklists and queues; #99). Owned by workflow.
-- `workflow_task`: each instance's projected Tasks, as JSON written by the workflow
-- application, replaced in the transaction that changes the instance, so they can always be
-- rebuilt from instance state. `rank` orders by priority (stat 3 … routine 0).
DEFINE TABLE workflow_task SCHEMAFULL;
DEFINE FIELD data ON workflow_task TYPE string;
DEFINE FIELD instance ON workflow_task TYPE string;
DEFINE FIELD worklist ON workflow_task TYPE string;
DEFINE FIELD open ON workflow_task TYPE bool;
DEFINE FIELD rank ON workflow_task TYPE int;
DEFINE FIELD authored_on ON workflow_task TYPE datetime;
DEFINE INDEX workflow_task_instance ON workflow_task FIELDS instance;
DEFINE INDEX workflow_task_worklist ON workflow_task FIELDS worklist, open;
