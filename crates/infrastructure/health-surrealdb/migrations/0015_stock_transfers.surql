-- Stock transfers and Reconciliation Records (#129; inventory-and-procurement.md § Transfers):
-- a transfer's two steps, and each discrepancy until its root cause is recorded (GSDP §13.2).
-- Owned by inventory.
DEFINE TABLE stock_transfer SCHEMAFULL;
-- The transfer, as JSON written by inventory.
DEFINE FIELD data ON stock_transfer TYPE string;
DEFINE TABLE stock_reconciliation SCHEMAFULL;
DEFINE FIELD unit_key ON stock_reconciliation TYPE string;
DEFINE FIELD resolved ON stock_reconciliation TYPE bool;
DEFINE FIELD opened_at ON stock_reconciliation TYPE datetime;
-- The record, as JSON written by inventory.
DEFINE FIELD data ON stock_reconciliation TYPE string;
DEFINE INDEX stock_reconciliation_open ON stock_reconciliation FIELDS resolved, opened_at;
