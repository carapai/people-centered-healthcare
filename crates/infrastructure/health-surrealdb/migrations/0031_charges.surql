-- Charges (billing-and-payments.md § Charge capture; #121). Owned by billing. One row per
-- ChargeItem, keyed by its deterministic id, as JSON written by billing: capturing the same
-- event again finds its charges already here, so it never double-charges.
DEFINE TABLE charge SCHEMAFULL;
DEFINE FIELD data ON charge TYPE string;
DEFINE FIELD event ON charge TYPE string;
DEFINE FIELD account ON charge TYPE string;
DEFINE FIELD status ON charge TYPE string;
DEFINE FIELD at ON charge TYPE datetime;
DEFINE INDEX charge_event ON charge FIELDS event;
DEFINE INDEX charge_account ON charge FIELDS account, at;
