-- Ledger operations on the change log (#81; synchronization.md § Envelope; write-authority.md:
-- stock goes up as operations): an entry with `op` carries an operation's body, appended to a
-- ledger rather than a resource version; `resource_type` names the ledger and `resource_id` its
-- scope.
DEFINE FIELD op ON change_log TYPE option<string>;
