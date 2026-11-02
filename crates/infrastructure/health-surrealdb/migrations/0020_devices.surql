-- The edge's device registry and pending enrollments (lan-security.md § Device enrollment; ADR
-- 0009; #74). `data` is the device or request as JSON written by health-auth; the fields beside
-- it are what lookups and the unique indexes need. Owned by identity. A device row is never
-- deleted: revocation is a status, and the registry syncs to central by `version`.
DEFINE TABLE auth_device SCHEMAFULL;
DEFINE FIELD data ON auth_device TYPE string;
DEFINE FIELD thumbprint ON auth_device TYPE string;
DEFINE FIELD status ON auth_device TYPE string
  ASSERT $value IN ['active', 'revoked'];
DEFINE FIELD version ON auth_device TYPE int;
DEFINE INDEX auth_device_thumbprint ON auth_device FIELDS thumbprint UNIQUE;
-- One pending request per key and per pairing code; expired ones are purged in the background
-- and before each new request.
DEFINE TABLE auth_enrollment SCHEMAFULL;
DEFINE FIELD data ON auth_enrollment TYPE string;
DEFINE FIELD thumbprint ON auth_enrollment TYPE string;
DEFINE FIELD code ON auth_enrollment TYPE string;
DEFINE FIELD expires ON auth_enrollment TYPE datetime;
DEFINE INDEX auth_enrollment_thumbprint ON auth_enrollment FIELDS thumbprint
  UNIQUE;
DEFINE INDEX auth_enrollment_code ON auth_enrollment FIELDS code UNIQUE;
