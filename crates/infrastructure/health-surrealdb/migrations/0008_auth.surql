-- The authorization server's state (offline-auth.md; health-auth): Login Accounts with their
-- lockout, one-time authorization codes and sessions (keyed by SHA-256 hashes, never the
-- tokens), retired refresh-token hashes for replay detection, the DPoP jti cache, permission
-- snapshots and the clock floor. Owned by identity; expired rows are purged in the background.
DEFINE TABLE auth_account SCHEMAFULL;
DEFINE FIELD username ON auth_account TYPE string;
DEFINE FIELD credential ON auth_account TYPE string;
DEFINE FIELD status ON auth_account TYPE string
  ASSERT $value IN ['active', 'disabled'];
DEFINE FIELD failures ON auth_account TYPE int;
DEFINE FIELD locked_until ON auth_account TYPE option<datetime>;
DEFINE INDEX auth_account_username ON auth_account FIELDS username UNIQUE;
DEFINE TABLE auth_code SCHEMAFULL;
DEFINE FIELD data ON auth_code TYPE string;
DEFINE FIELD expires_at ON auth_code TYPE datetime;
DEFINE TABLE auth_session SCHEMAFULL;
DEFINE FIELD data ON auth_session TYPE string;
DEFINE FIELD refresh_hash ON auth_session TYPE string;
DEFINE FIELD live ON auth_session TYPE bool;
DEFINE FIELD ends_at ON auth_session TYPE datetime;
DEFINE INDEX auth_session_refresh ON auth_session FIELDS refresh_hash UNIQUE;
DEFINE TABLE auth_retired_refresh SCHEMAFULL;
DEFINE FIELD session ON auth_retired_refresh TYPE string;
DEFINE FIELD ends_at ON auth_retired_refresh TYPE datetime;
DEFINE TABLE auth_jti SCHEMAFULL;
DEFINE FIELD until ON auth_jti TYPE datetime;
DEFINE TABLE auth_snapshot SCHEMAFULL;
DEFINE FIELD jws ON auth_snapshot TYPE string;
DEFINE TABLE auth_clock_floor SCHEMAFULL;
DEFINE FIELD at ON auth_clock_floor TYPE datetime;
