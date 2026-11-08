-- Inter-facility transfers through central (#247; inventory-and-procurement.md § Transfers;
-- write-authority.md: up, then to the counterpart via central):
-- - `sync_transfer_outbox` queues a transfer between facilities for each upstream, keyed
--   `[peer, transfer id]`, whenever it is saved, until the upstream has it;
-- - `sync_transfer_state` is central's view of each: whether the source's dispatch has arrived
--   (its lines are authoritative), and when central first saw it;
-- - `transfer_match` is central's matching of the source's dispatch with the destination's
--   receipt: `matched`, `mismatch` (each discrepancy a Reconciliation Record), or `undispatched`
--   (a receipt with no dispatch).
DEFINE TABLE sync_transfer_outbox SCHEMAFULL;
DEFINE FIELD queued_at ON sync_transfer_outbox TYPE datetime;

DEFINE TABLE sync_transfer_state SCHEMAFULL;
DEFINE FIELD source_seen ON sync_transfer_state TYPE bool;
DEFINE FIELD arrived_at ON sync_transfer_state TYPE datetime;

DEFINE TABLE transfer_match SCHEMAFULL;
DEFINE FIELD status ON transfer_match TYPE string
  ASSERT $value IN ['matched', 'mismatch', 'undispatched'];
DEFINE FIELD found ON transfer_match TYPE string;
DEFINE FIELD checked_at ON transfer_match TYPE datetime;
