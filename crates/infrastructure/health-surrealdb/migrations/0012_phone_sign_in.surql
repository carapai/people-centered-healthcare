-- Patients' phone accounts and their one-time codes (ADR 0029; #111): one Login Account per
-- verified phone number, and each phone's current code challenge. Owned by identity.
DEFINE TABLE auth_phone_account SCHEMAFULL;
-- E.164, unique among phone accounts.
DEFINE FIELD phone ON auth_phone_account TYPE string;
DEFINE FIELD status ON auth_phone_account TYPE string;
DEFINE FIELD verified_at ON auth_phone_account TYPE datetime;
DEFINE INDEX auth_phone_account_phone ON auth_phone_account FIELDS phone UNIQUE;
-- Keyed by the phone number. `data` is the challenge and the times codes were sent, as JSON
-- written by health-auth; `attempts` and `issued_at` are compared when a code is tried, so two
-- concurrent tries cannot both count as the same one.
DEFINE TABLE auth_otp SCHEMAFULL;
DEFINE FIELD data ON auth_otp TYPE string;
DEFINE FIELD attempts ON auth_otp TYPE int;
DEFINE FIELD issued_at ON auth_otp TYPE datetime;
