-- Requests to a payment provider (#124): which facility's payment each provider reference is
-- for, written before the request goes out, so a callback or a poll finds the facility whose
-- ledger and merchant account the payment belongs to. Keyed by the provider reference.
DEFINE TABLE provider_request SCHEMAFULL;
DEFINE FIELD facility ON provider_request TYPE string;
DEFINE FIELD payment ON provider_request TYPE string;
DEFINE FIELD provider ON provider_request TYPE string;
DEFINE FIELD requested_at ON provider_request TYPE datetime;
