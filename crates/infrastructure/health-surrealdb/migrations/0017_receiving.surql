-- Goods received notes and requisitions (#130; inventory-and-procurement.md § Receiving,
-- requisitions and reorder). Owned by inventory.
DEFINE TABLE stock_goods_received SCHEMAFULL;
-- The note, as JSON written by inventory.
DEFINE FIELD data ON stock_goods_received TYPE string;
DEFINE TABLE stock_requisition SCHEMAFULL;
-- The requisition, as JSON written by inventory.
DEFINE FIELD data ON stock_requisition TYPE string;
