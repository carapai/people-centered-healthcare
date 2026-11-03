-- Offline user and role administration on an edge (offline-auth.md § Offline user and role
-- administration; ADR 0011; #75): roles a facility admin granted locally, each with its JWS
-- signed by the node's facility intermediate key for central to ratify; local revocations, keyed
-- by account; and the country's role mappings and locally grantable roles from central's
-- provisioning bundle, one record. `data` is JSON written by health-auth. Owned by identity.
DEFINE TABLE auth_local_grant SCHEMAFULL;
DEFINE FIELD data ON auth_local_grant TYPE string;
DEFINE FIELD jws ON auth_local_grant TYPE string;
DEFINE FIELD account ON auth_local_grant TYPE string;
DEFINE FIELD facility ON auth_local_grant TYPE string;
DEFINE FIELD not_after ON auth_local_grant TYPE datetime;
DEFINE INDEX auth_local_grant_by ON auth_local_grant FIELDS account, facility;
DEFINE TABLE auth_local_revocation SCHEMAFULL;
DEFINE FIELD data ON auth_local_revocation TYPE string;
DEFINE TABLE auth_local_config SCHEMAFULL;
DEFINE FIELD data ON auth_local_config TYPE string;
