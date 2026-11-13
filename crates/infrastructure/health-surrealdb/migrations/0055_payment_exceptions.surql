-- The payment exceptions queue (#124; billing-and-payments.md § Other flows, provider
-- statements): what a provider statement or a provider's answer shows that a person must look
-- at, kept until resolved. Keyed `kind|reference`, so the same exception is opened once.
-- `kind` is `mismatched` (a statement amount differs), `unknown` (a statement line no payment
-- has) or `amount` (the provider confirmed another amount).
DEFINE TABLE payment_exception SCHEMAFULL;
DEFINE FIELD kind ON payment_exception TYPE string
  ASSERT $value IN ['mismatched', 'unknown', 'amount'];
DEFINE FIELD reference ON payment_exception TYPE string;
DEFINE FIELD payment ON payment_exception TYPE option<string>;
DEFINE FIELD recorded ON payment_exception TYPE option<string>;
DEFINE FIELD provider ON payment_exception TYPE option<string>;
DEFINE FIELD status ON payment_exception TYPE string
  ASSERT $value IN ['open', 'resolved'];
DEFINE FIELD opened_at ON payment_exception TYPE datetime;
DEFINE FIELD resolved_by ON payment_exception TYPE option<string>;
DEFINE FIELD note ON payment_exception TYPE option<string>;
DEFINE INDEX payment_exception_status ON payment_exception FIELDS status, opened_at;
