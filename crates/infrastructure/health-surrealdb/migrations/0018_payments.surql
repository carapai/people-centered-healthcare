-- The financial ledger's postings, payments and cashier shifts (#254; billing-and-payments.md;
-- ADRs 0030, 0032). Postings are appended once and never changed. Owned by financial-ledger
-- (postings) and payments (payments, shifts).
DEFINE TABLE ledger_posting SCHEMAFULL;
DEFINE FIELD at ON ledger_posting TYPE datetime;
-- The posting, as JSON written by the ledger.
DEFINE FIELD data ON ledger_posting TYPE string;
DEFINE INDEX ledger_posting_at ON ledger_posting FIELDS at;
DEFINE TABLE payment SCHEMAFULL;
DEFINE FIELD status ON payment TYPE string;
DEFINE FIELD shift ON payment TYPE option<string>;
DEFINE FIELD recorded_at ON payment TYPE datetime;
-- The payment, as JSON written by payments.
DEFINE FIELD data ON payment TYPE string;
DEFINE INDEX payment_status ON payment FIELDS status, recorded_at;
DEFINE INDEX payment_shift ON payment FIELDS shift, recorded_at;
DEFINE TABLE cash_shift SCHEMAFULL;
DEFINE FIELD till ON cash_shift TYPE string;
DEFINE FIELD status ON cash_shift TYPE string;
-- The shift, as JSON written by payments.
DEFINE FIELD data ON cash_shift TYPE string;
DEFINE INDEX cash_shift_till ON cash_shift FIELDS till, status;
