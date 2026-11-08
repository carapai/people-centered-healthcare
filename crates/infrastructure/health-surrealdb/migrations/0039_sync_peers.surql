-- Sync peers (#81; synchronization.md § Handshake and compatibility; ADR 0062).
-- - `sync_state:node.node` is the node's own id on the sync protocol where nothing else names
--   it (central); it survives backup and restore with the rest of the store;
-- - `sync_peer` holds, per downstream node, its last handshake and the verdict on it, which
--   every push and pull of the node is held to;
-- - `sync_quarantine` holds, per `op_id`, the items an out-of-range peer pushed, for review.
DEFINE FIELD node ON sync_state TYPE option<string>;
UPDATE sync_state:node SET node = <string> rand::uuid::v7();

DEFINE TABLE sync_peer SCHEMAFULL;
DEFINE FIELD hello ON sync_peer TYPE string;
DEFINE FIELD verdict ON sync_peer TYPE string;
DEFINE FIELD seen_at ON sync_peer TYPE datetime;

DEFINE TABLE sync_quarantine SCHEMAFULL;
DEFINE FIELD peer ON sync_quarantine TYPE string;
DEFINE FIELD item ON sync_quarantine TYPE string;
DEFINE FIELD received_at ON sync_quarantine TYPE datetime;
