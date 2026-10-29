-- Locks for conditional writes (persistence.md § REST semantics: conditions are resolved through
-- the search index inside the write transaction). A conditional create writes the record keyed
-- by its condition's hash and deletes it again in the same transaction. The record never
-- persists, but two transactions creating under the same condition both write the key, so
-- snapshot isolation fails one of them, and its rerun sees the winner's resource.
DEFINE TABLE condition_lock SCHEMAFULL;
DEFINE FIELD at ON condition_lock TYPE datetime;
