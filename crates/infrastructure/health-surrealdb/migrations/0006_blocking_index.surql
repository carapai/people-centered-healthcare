-- The patient-matching blocking index (ADR 0055; patient-identity.md § Matching). Each live
-- Patient version writes `[key_kind, key_value, patient_id]` rows in its own transaction,
-- listed in the head's `index_keys` like search entries, so the next version replaces them and
-- a tombstone removes them. A block is read with a record-id range.
DEFINE TABLE idx_block SCHEMAFULL;
-- Block sizes, recomputed in the background (hourly) and never in a write transaction, so
-- common surnames cause no write conflicts. A query skips blocks larger than the rules' limit.
DEFINE TABLE block_count SCHEMAFULL;
DEFINE FIELD count ON block_count TYPE int;
DEFINE FIELD counted_at ON block_count TYPE datetime;
