-- Break-glass grants travel up (#224; offline-auth.md § Break-glass): a grant made or
-- acknowledged at a node is queued for each upstream, keyed `[peer, grant id]`, until the
-- upstream has it. At central, `arrived_at` is when it arrived, and the privacy officer's review
-- is due 72 hours after that.
DEFINE TABLE sync_grant_outbox SCHEMAFULL;
DEFINE FIELD queued_at ON sync_grant_outbox TYPE datetime;
DEFINE FIELD arrived_at ON break_glass TYPE option<datetime>;
