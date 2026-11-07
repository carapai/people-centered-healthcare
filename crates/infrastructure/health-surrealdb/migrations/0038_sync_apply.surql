-- Applying synced versions (#81; synchronization.md § Push, § Pull and Sync Scope, § Item states).
-- - `change_log.origin` names the node that first committed a version received by sync; NONE
--   for a local write;
-- - `sync_receipt` holds, per received `op_id`, its content hash and outcome, written with the
--   write it caused so a retried push is answered from it;
-- - `sync_conflict` is the Conflict Record opened when a pushed group's version does not build on
--   the head, keyed by the group's `tx_id`;
-- - `sync_checkpoint` holds, per upstream peer, the cursor into its log this node has applied;
-- - `sync_item_state` holds the local candidates the upstream answered with a conflict or a
--   rejection, keyed by `op_id`, so pulled data never overwrites them;
-- - `sync_outbox.resource` names the queued version's resource (`Type/id`), indexed, so a node
--   finds its unsent candidate for a resource without reading the log.
DEFINE FIELD origin ON change_log TYPE option<string>;
DEFINE FIELD resource ON sync_outbox TYPE option<string>;
DEFINE INDEX sync_outbox_resource ON sync_outbox FIELDS resource;

DEFINE TABLE sync_receipt SCHEMAFULL;
DEFINE FIELD content_hash ON sync_receipt TYPE string;
DEFINE FIELD outcome ON sync_receipt TYPE string;
DEFINE FIELD received_at ON sync_receipt TYPE datetime;

DEFINE TABLE sync_conflict SCHEMAFULL;
DEFINE FIELD peer ON sync_conflict TYPE string;
DEFINE FIELD items ON sync_conflict TYPE string;
DEFINE FIELD status ON sync_conflict TYPE string
  ASSERT $value IN ['open', 'resolved', 'superseded'];
DEFINE FIELD opened_at ON sync_conflict TYPE datetime;

DEFINE TABLE sync_checkpoint SCHEMAFULL;
DEFINE FIELD epoch ON sync_checkpoint TYPE string;
DEFINE FIELD seq ON sync_checkpoint TYPE int;

DEFINE TABLE sync_item_state SCHEMAFULL;
DEFINE FIELD resource_type ON sync_item_state TYPE string;
DEFINE FIELD resource_id ON sync_item_state TYPE string;
DEFINE FIELD state ON sync_item_state TYPE string
  ASSERT $value IN ['conflict', 'rejected'];
DEFINE FIELD outcome ON sync_item_state TYPE string;
DEFINE INDEX sync_item_state_resource ON sync_item_state FIELDS resource_type, resource_id;
