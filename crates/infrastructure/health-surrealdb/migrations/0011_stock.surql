-- Stock (inventory-and-procurement.md; ADR 0034): the append-only movement ledger, and one
-- balance record per Stock Unit, written in every movement's transaction so concurrent issues
-- of the same unit conflict. Owned by inventory.
DEFINE TABLE stock_balance SCHEMAFULL;
-- The Stock Unit, as JSON written by inventory.
DEFINE FIELD unit ON stock_balance TYPE string;
DEFINE FIELD on_hand ON stock_balance TYPE int;
DEFINE FIELD reserved ON stock_balance TYPE int;
DEFINE FIELD on_hold ON stock_balance TYPE int;
DEFINE FIELD flagged ON stock_balance TYPE bool;
DEFINE TABLE stock_movement SCHEMAFULL;
DEFINE FIELD unit_key ON stock_movement TYPE string;
-- The movement, as JSON written by inventory.
DEFINE FIELD data ON stock_movement TYPE string;
DEFINE FIELD recorded_at ON stock_movement TYPE datetime;
DEFINE INDEX stock_movement_unit ON stock_movement FIELDS unit_key, recorded_at;
