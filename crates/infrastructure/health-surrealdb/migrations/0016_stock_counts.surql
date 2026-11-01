-- Stock counts (#129; inventory-and-procurement.md § Stock counts): full, cycle and blind
-- counts from draft to approval. Owned by inventory.
DEFINE TABLE stock_count SCHEMAFULL;
-- The count, as JSON written by inventory.
DEFINE FIELD data ON stock_count TYPE string;
