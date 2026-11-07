-- The sync change log (#79; synchronization.md § Change log, outbox and cursors; ADR 0019).
-- Written only by the unit of work, in the write transaction:
-- - `sync_state:node` holds the node's `epoch`, which changes when it is restored from backup
--   or re-seeded;
-- - `change_log` holds one entry per accepted version, keyed by its `seq`, taken under the
--   node's commit lock (ADR 0019 amendment); the version itself is read from history by
--   `[resource_id, version_seq]`;
-- - `sync_upstream` lists the node's upstream peers, keyed by peer node id;
-- - `sync_outbox` holds one row per upstream peer and entry, keyed `[peer, seq]`, until the
--   peer acknowledges it.
DEFINE TABLE sync_state SCHEMAFULL;
DEFINE FIELD epoch ON sync_state TYPE string;
CREATE sync_state:node SET epoch = <string> rand::uuid::v7();

DEFINE TABLE change_log SCHEMAFULL;
DEFINE FIELD tx_id ON change_log TYPE string;
DEFINE FIELD op_id ON change_log TYPE string;
DEFINE FIELD resource_type ON change_log TYPE string;
DEFINE FIELD resource_id ON change_log TYPE string;
DEFINE FIELD version_seq ON change_log TYPE int;
DEFINE FIELD deleted ON change_log TYPE bool;
DEFINE FIELD committed_at ON change_log TYPE datetime;

DEFINE TABLE sync_upstream SCHEMAFULL;
DEFINE FIELD added_at ON sync_upstream TYPE datetime;

DEFINE TABLE sync_outbox SCHEMAFULL;
DEFINE FIELD tx_id ON sync_outbox TYPE string;
