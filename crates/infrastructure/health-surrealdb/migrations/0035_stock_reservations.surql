-- Stock reservations (#128; ADR 0035). `stock_reservation` is owned by inventory: stock held
-- for a dispense being prepared (4 h), a transfer pick (72 h) or a procedure kit, as JSON. A
-- reservation reduces what is available, never what is on hand; it is released on cancellation,
-- consumed on completion, and lapses at its expiry.
DEFINE TABLE stock_reservation SCHEMAFULL;
DEFINE FIELD data ON stock_reservation TYPE string;
DEFINE FIELD unit_key ON stock_reservation TYPE string;
DEFINE FIELD reference ON stock_reservation TYPE string;
DEFINE FIELD state ON stock_reservation TYPE string;
DEFINE INDEX stock_reservation_unit ON stock_reservation FIELDS unit_key, state;
DEFINE INDEX stock_reservation_reference ON stock_reservation FIELDS reference;
