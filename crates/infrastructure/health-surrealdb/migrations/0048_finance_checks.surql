-- What central checks of a facility's payments and postings (#81; synchronization.md: payments
-- idempotent by external transaction id, postings balanced and reversed once). A payment's
-- provider reference names one payment only, and a posting is reversed at most once.
DEFINE FIELD reference ON payment TYPE option<string>;
DEFINE INDEX payment_reference ON payment FIELDS reference;
DEFINE FIELD reverses ON ledger_posting TYPE option<string>;
DEFINE INDEX ledger_posting_reverses ON ledger_posting FIELDS reverses;
