-- Which facility a ledger scope belongs to (#81; write-authority.md: each ledger's authority is the
-- facility that keeps it): keyed `[ledger, scope]` (a stock store, a billing account, a posting,
-- payment or shift), claimed by the first facility to send an operation on it; central refuses
-- operations on it from any other facility.
DEFINE TABLE ledger_claim SCHEMAFULL;
DEFINE FIELD facility ON ledger_claim TYPE string;
DEFINE FIELD claimed_at ON ledger_claim TYPE datetime;
