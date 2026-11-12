-- Workflow candidates received from other nodes (#100; workflow-engine.md § Offline and sync):
-- transitions a desktop made on its replica of an instance this node decides, kept until the
-- workflow engine judges them. `data` is the candidate, `item` the operation as it was pushed
-- (the Conflict Record's candidate branch), and `status` is `pending`, `applied` or `conflict`.
DEFINE TABLE workflow_candidate SCHEMAFULL;
DEFINE FIELD peer ON workflow_candidate TYPE string;
DEFINE FIELD instance ON workflow_candidate TYPE string;
DEFINE FIELD data ON workflow_candidate TYPE string;
DEFINE FIELD item ON workflow_candidate TYPE string;
DEFINE FIELD status ON workflow_candidate TYPE string
  ASSERT $value IN ['pending', 'applied', 'conflict'];
DEFINE FIELD reason ON workflow_candidate TYPE option<string>;
DEFINE FIELD received_at ON workflow_candidate TYPE datetime;
DEFINE INDEX workflow_candidate_status ON workflow_candidate FIELDS status, received_at;
