-- The sync horizon (#83; synchronization.md § Tombstones, retention and long outages): the
-- change log and receipts are kept at least 180 days, then pruned; `sync_horizon:node` holds the
-- highest `seq` pruned, and a peer whose cursor is below it must re-seed.
DEFINE TABLE sync_horizon SCHEMAFULL;
DEFINE FIELD pruned_through ON sync_horizon TYPE int;
DEFINE INDEX change_log_committed ON change_log FIELDS committed_at;
DEFINE INDEX sync_receipt_received ON sync_receipt FIELDS received_at;
