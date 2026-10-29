-- Reverse references (persistence.md § Reference integrity). Every literal `Type/id` reference
-- in a live resource writes `[target_type, target_id, source_type, source_id]`, listed in the
-- source head's `index_keys` like a search entry. A delete can then find the live records that
-- still reference it, whichever search parameters the node supports. Tombstones write none.
DEFINE TABLE idx_ref_in SCHEMAFULL;
